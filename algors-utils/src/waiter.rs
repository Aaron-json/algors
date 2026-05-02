use crate::backoff::Backoff;

#[cfg(feature = "std")]
use std::sync::{Condvar, Mutex};

pub trait Waiter {
    /// Error type is left up to implementation.
    ///
    /// We avoid a generic error since one waiter could have multiple
    /// implementations with different error types, which adds complexity.
    ///
    /// If a single implementation could have different approaches, then they
    /// should be split into different implementations.
    type Error;

    /// Wait for accepts an op function that is waited for, for successful
    /// completion. Some waiters may support aborting/giving up
    /// (e.x timeout).
    fn wait_for<F, T>(&self, op: F) -> Result<T, Self::Error>
    where
        F: FnMut() -> Option<T>;
    fn notify(&self);
}

/// Implements a spin-only waiter.
#[derive(Default)]
pub struct SpinWait;

impl Waiter for SpinWait {
    type Error = core::convert::Infallible;

    fn wait_for<F, T>(&self, mut op: F) -> Result<T, Self::Error>
    where
        F: FnMut() -> Option<T>,
    {
        let mut backoff = Backoff::new();
        loop {
            if let Some(val) = op() {
                return Ok(val);
            };
            backoff.spin();
        }
    }

    fn notify(&self) {}
}

/// Implements a waiter that starts with a spin wait and then
/// attempts to yield to the OS when the spin limit is reached.
#[derive(Default)]
pub struct YieldWait;

impl Waiter for YieldWait {
    type Error = core::convert::Infallible;

    fn wait_for<F, T>(&self, mut op: F) -> Result<T, Self::Error>
    where
        F: FnMut() -> Option<T>,
    {
        let mut backoff = Backoff::new();
        loop {
            if let Some(val) = op() {
                return Ok(val);
            };
            backoff.pause();
        }
    }

    fn notify(&self) {}
}

/// Implements a blocking waiter. This is only available with
/// the `std` feature compiled since it relies on OS features.
///
/// This waiter holds shared memory and thus users must share the
/// same object.
#[cfg(feature = "std")]
#[derive(Default)]
pub struct BlockingWait {
    state: Mutex<u64>,
    cvar: Condvar,
}

#[cfg(feature = "std")]
impl Waiter for BlockingWait {
    type Error = core::convert::Infallible;

    fn wait_for<F, T>(&self, mut op: F) -> Result<T, Self::Error>
    where
        F: FnMut() -> Option<T>,
    {
        let mut backoff = Backoff::new();
        loop {
            if !backoff.done() {
                if let Some(val) = op() {
                    return Ok(val);
                }
                backoff.spin();
            } else {
                let prev = *self.state.lock().unwrap();

                if let Some(val) = op() {
                    return Ok(val);
                }

                let mut cur = self.state.lock().unwrap();
                while *cur == prev {
                    cur = self.cvar.wait(cur).unwrap();
                }

                // We must reset since the operation could fail again after
                // the wakeup, in which case we will start all over again.
                backoff.reset();
            }
        }
    }

    fn notify(&self) {
        let mut state = self.state.lock().unwrap();
        *state += 1;
        self.cvar.notify_one();
    }
}
