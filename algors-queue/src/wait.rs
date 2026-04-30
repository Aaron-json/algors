use algors_utils::backoff::Backoff;
extern crate alloc;
use std::sync::{Condvar, Mutex};

pub trait WaitStrategy {
    fn wait_for<F, T>(&self, op: F) -> T
    where
        F: FnMut() -> Option<T>;
    fn notify(&self);
}

// Implements a spin-only wait strategy.
pub struct SpinWait;

impl WaitStrategy for SpinWait {
    fn wait_for<F, T>(&self, mut op: F) -> T
    where
        F: FnMut() -> Option<T>,
    {
        let mut backoff = Backoff::new();
        loop {
            if let Some(val) = op() {
                return val;
            };
            backoff.spin();
        }
    }
    fn notify(&self) {}
}

// Implements a wait strategy that starts with a spin wait and then
// yields to the OS when the spin limit is reached.
pub struct YieldWait;

impl WaitStrategy for YieldWait {
    fn wait_for<F, T>(&self, mut op: F) -> T
    where
        F: FnMut() -> Option<T>,
    {
        let mut backoff = Backoff::new();
        loop {
            if let Some(val) = op() {
                return val;
            };
            backoff.pause();
        }
    }
    fn notify(&self) {}
}

pub struct BlockingWait {
    state: Mutex<u64>,
    cvar: Condvar,
}

impl BlockingWait {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(0),
            cvar: Condvar::new(),
        }
    }
}

impl WaitStrategy for BlockingWait {
    fn wait_for<F, T>(&self, mut op: F) -> T
    where
        F: FnMut() -> Option<T>,
    {
        let mut backoff = Backoff::new();
        loop {
            if !backoff.done() {
                if let Some(val) = op() {
                    return val;
                }
                backoff.spin();
            } else {
                let prev = *self.state.lock().unwrap();

                if let Some(val) = op() {
                    return val;
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
