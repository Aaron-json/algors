use core::{
    cell::UnsafeCell,
    mem,
    mem::MaybeUninit,
    sync::atomic::{AtomicUsize, Ordering},
};
extern crate alloc;
use alloc::boxed::Box;
use alloc::sync::Arc;

use algors_utils::{CachePadded, alloc::alloc_uninit_slice, backoff::Backoff};

use crate::slot::SequencedSlot;

// Used to implement a Multi-producer Multi-consumer queue. The design
// implements Dmitry vyukov's MPMC Queue ideas.
struct Inner<T> {
    pub buf: Box<[SequencedSlot<T>]>,

    pub head: CachePadded<AtomicUsize>,
    pub tail: CachePadded<AtomicUsize>,
}

impl<T> Inner<T> {
    /// Creates a new instance. Panics if pow is not less than
    /// usize::BITS
    pub fn new(pow: u8) -> Self {
        assert!(u32::from(pow) < usize::BITS);

        let size: usize = 1 << pow;
        let buf_raw = alloc_uninit_slice::<SequencedSlot<T>>(size);

        let mut buf: Box<[SequencedSlot<T>]>;
        unsafe {
            // SAFETY: We can cast Box<[MaybeUninit<SequencedSlot<T>>]> to
            // Box<[SequencedSlot<T>]> since SequencedSlot<T> contains
            // MaybeUninit<T> internally for the data and the sequence
            // is initialized manually.
            buf = Box::from_raw(Box::into_raw(buf_raw) as *mut [SequencedSlot<T>]);
        }

        // initialize the sequence numbers
        for i in 0..size {
            buf[i] = SequencedSlot {
                seq: AtomicUsize::new(i),
                data: UnsafeCell::new(MaybeUninit::uninit()),
            }
        }

        Inner {
            buf,
            head: CachePadded(AtomicUsize::new(0)),
            tail: CachePadded(AtomicUsize::new(0)),
        }
    }

    #[inline(always)]
    fn mask(&self, num: usize) -> usize {
        num & (self.buf.len() - 1)
    }

    #[inline(always)]
    fn len(&self) -> usize {
        self.buf.len()
    }
}

impl<T> Drop for Inner<T> {
    fn drop(&mut self) {
        if !mem::needs_drop::<T>() {
            return;
        }

        let h = self.head.0.load(Ordering::Relaxed);
        let t = self.tail.0.load(Ordering::Relaxed);

        for i in 0..t.wrapping_sub(h) {
            let idx = h.wrapping_add(i);
            let slot = &self.buf[self.mask(idx)];

            // SAFETY: We drop an element when its sequence number indicates
            // it was fully written (ready for reading). This risks
            // a memory leak if the slot was written to but the sequence
            // never updated. The alternative is dropping anyways which might
            // drop an uninitialized/no longer owned value leading to UB.
            if slot.seq.load(Ordering::Relaxed) == idx + 1 {
                unsafe {
                    (*self.buf[self.mask(idx)].data.get()).assume_init_drop();
                }
            }
        }
    }
}

pub struct Producer<T> {
    inner: Arc<Inner<T>>,
}

impl<T> Producer<T> {
    pub fn try_push(&self, val: T) -> Result<(), T> {
        let inner = &self.inner;
        let mut backoff = Backoff::new();

        let mut idx = inner.tail.0.load(Ordering::Relaxed);

        loop {
            let slot = &inner.buf[inner.mask(idx)];
            let seq = slot.seq.load(Ordering::Acquire);

            let diff = seq.wrapping_sub(idx) as isize;
            if diff == 0 {
                match inner.tail.0.compare_exchange_weak(
                    idx,
                    idx + 1,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    // we sucessfully claimed the slot
                    Ok(_) => unsafe {
                        (*slot.data.get()).write(val);
                        // update to t + 1 to signal consumers this slot is
                        // ready for reading
                        slot.seq.store(idx + 1, Ordering::Release);
                        return Ok(());
                    },
                    Err(new_idx) => {
                        // Another producer won the CAS and updated it before
                        // us. Advance to the new tail and try again
                        idx = new_idx;
                        backoff.spin();
                    }
                }
            } else if diff < 0 {
                // this slot contains data from the previous lap's write. has
                // not been read yet
                return Err(val);
            } else if diff > 0 {
                // another producer claimed the slot and updated tail and
                // our copy is stale so we reload it.

                // No guarantees on when/if the CAS will succeed, so we may
                // pause
                backoff.pause();
                idx = inner.tail.0.load(Ordering::Relaxed);
            }
        }
    }
}

pub struct Consumer<T> {
    inner: Arc<Inner<T>>,
}

impl<T> Consumer<T> {
    pub fn try_pop(&self) -> Option<T> {
        let inner = &self.inner;
        let mut backoff = Backoff::new();

        let mut idx = inner.head.0.load(Ordering::Relaxed);

        loop {
            let slot = &inner.buf[inner.mask(idx)];

            let seq = slot.seq.load(Ordering::Acquire);

            // consumers look for sequence number `position + 1` (set by the
            // producer) to read this slot
            // this may also underflow
            let diff = seq.wrapping_sub(idx + 1) as isize;

            if diff == 0 {
                match inner.head.0.compare_exchange_weak(
                    idx,
                    idx + 1,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        let data = unsafe { (*slot.data.get()).assume_init_read() };

                        slot.seq.store(idx + inner.len(), Ordering::Release);

                        return Some(data);
                    }
                    Err(new_idx) => {
                        // another reader won the CAS
                        idx = new_idx;
                        backoff.spin();
                    }
                }
            } else if diff < 0 {
                // producer has not written here yet
                return None;
            } else {
                // another consumer has read this position and advanced
                // the sequence number but the head is stale

                // No guarantees on when/if the CAS will succeed, so we may
                // pause
                backoff.pause();
                idx = inner.head.0.load(Ordering::Relaxed);
            }
        }
    }
}

// Implement clone so users do not have to wrap in another Arc, increasing
// indirections.
impl<T> Clone for Producer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
impl<T> Clone for Consumer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

// Both the Consumer and Producer can be sent and shared from one thread to
// another
unsafe impl<T: Send> Send for Producer<T> {}
unsafe impl<T: Send> Send for Consumer<T> {}
unsafe impl<T: Send> Sync for Producer<T> {}
unsafe impl<T: Send> Sync for Consumer<T> {}

pub fn new_mpmc<T>(pow: u8) -> (Consumer<T>, Producer<T>) {
    let inner = Arc::new(Inner::new(pow));

    let c = Consumer {
        inner: inner.clone(),
    };

    let p = Producer {
        inner: inner,
    };

    (c, p)
}
