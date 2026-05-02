use algors_utils::{CachePadded, alloc::alloc_uninit_slice, waiter::Waiter};
use core::mem;
use crate::sync::atomic::{AtomicUsize, Ordering};
use alloc::boxed::Box;
use crate::sync::Arc;

use crate::slot::Slot;

struct Inner<T> {
    // padded to avoid false sharing.
    head: CachePadded<AtomicUsize>,
    tail: CachePadded<AtomicUsize>,

    // this is not padded since the 'fat pointer' is never changed, only
    // the data behind it.
    buf: Box<[Slot<T>]>,
}

impl<T> Inner<T> {
    pub fn new(pow: u8) -> Self {
        assert!(u32::from(pow) < usize::BITS);
        let size: usize = 1 << pow;
        let buf_raw = alloc_uninit_slice::<Slot<T>>(size);

        let buf: Box<[Slot<T>]>;
        unsafe {
            // SAFETY: We can cast Box<[MaybeUninit<Slot<T>>]> to
            // Box<[Slot<T>]> since Slot<T> contains MaybeUninit<T> internally
            // so we will not drop uninitialized memory..
            buf = Box::from_raw(Box::into_raw(buf_raw) as *mut [Slot<T>]);
        }

        Inner {
            buf,
            head: CachePadded(AtomicUsize::new(0)),
            tail: CachePadded(AtomicUsize::new(0)),
        }
    }

    #[inline(always)]
    fn len(&self) -> usize {
        self.buf.len()
    }

    #[inline(always)]
    fn mask(&self, num: usize) -> usize {
        num & (self.buf.len() - 1)
    }
}

impl<T> Drop for Inner<T> {
    fn drop(&mut self) {
        if !mem::needs_drop::<T>() {
            return;
        }

        // SAFETY: here we know that we are the only threads accessing this
        // since all references are dropped before this is called.
        // The standard library also uses Odrdering::Release to decerement
        // the Arc counter, then an Ordering::Acquire fence after the last
        // decrement so we can safely load here with Ordering::Relaxed.
        let h = self.head.0.load(Ordering::Relaxed);
        let t = self.tail.0.load(Ordering::Relaxed);

        for i in 0..t.wrapping_sub(h) {
            let idx = h.wrapping_add(i);

            unsafe {
                (*self.buf[self.mask(idx)].get()).assume_init_drop();
            }
        }
    }
}

pub struct Producer<T> {
    inner: Arc<Inner<T>>,
    // The consumer writes to the head. Doing an atomic load every time would
    // create cache coherence traffic and bouncing between the producer and
    // consumer threads. Since we know that the head only moves forward, we
    // can cache the last seen value and write up to it without loading the
    // actual head.
    head_cache: usize,
}

impl<T> Producer<T> {
    pub fn try_push(&mut self, val: T) -> Result<(), T> {
        let inner = &self.inner;
        let t = inner.tail.0.load(Ordering::Relaxed);

        // Read the cached head, until it tells us the buffer is full then
        // load the actual value into it.
        if t.wrapping_sub(self.head_cache) == inner.len() {
            self.head_cache = inner.head.0.load(Ordering::Acquire);
            if t.wrapping_sub(self.head_cache) == inner.len() {
                return Err(val);
            }
        }

        unsafe {
            (*inner.buf[inner.mask(t)].get()).write(val);
        }

        // publish the data
        inner.tail.0.store(t + 1, Ordering::Release);

        Ok(())
    }

    /// Attempts to push the object using the given waiter.
    /// Returns a result since some waiters could allow giving up and
    /// aborting even when not successful.
    ///
    /// If not successful, the waiter's error is returned together with
    /// the value.
    ///
    /// The waiter is used to retry until successful completion or abortion.
    /// The notifier is used to broadcast the change. The waiter and notifier
    /// may or may not be the same object.
    ///
    /// The separation allows for precise notifications to consumers without
    /// waking up other producers.
    pub fn push<W: Waiter, N: Waiter>(
        &mut self,
        val: T,
        waiter: &W,
        notifier: &N,
    ) -> Result<(), (T, W::Error)> {
        // We need the value after waiting, so we cannot directly
        // move into the closure since it is an FnMut.
        let mut store = Some(val);
        let res = waiter.wait_for(|| match self.try_push(store.take()?) {
            Ok(()) => Some(()),
            Err(v) => {
                store = Some(v);
                None
            }
        });

        match res {
            Ok(()) => {
                notifier.notify();
                Ok(())
            }
            Err(e) => Err((store.take().unwrap(), e)),
        }
    }
}

pub struct Consumer<T> {
    inner: Arc<Inner<T>>,
    // Cached tail to avoid cache coherence traffic and cache bouncing.
    // For more details, read the producer's type.
    tail_cache: usize,
}

impl<T> Consumer<T> {
    pub fn try_pop(&mut self) -> Option<T> {
        let inner = &self.inner;
        let h = inner.head.0.load(Ordering::Relaxed);

        if self.tail_cache.wrapping_sub(h) == 0 {
            self.tail_cache = inner.tail.0.load(Ordering::Acquire);
            if self.tail_cache.wrapping_sub(h) == 0 {
                return None;
            }
        }

        let val = unsafe { (*inner.buf[inner.mask(h)].get()).assume_init_read() };

        inner.head.0.store(h + 1, Ordering::Release);

        Some(val)
    }

    /// Attempts to pop a value.
    ///
    /// The waiter is used to retry until successful completion or abortion.
    /// The notifier is used to broadcast the change. The waiter and notifier
    /// may or may not be the same object.
    ///
    /// The separation allows for precise notifications to producers without
    /// waking up other consumers.
    pub fn pop<W: Waiter, N: Waiter>(&mut self, waiter: &W, notifier: &N) -> Result<T, W::Error> {
        let res = waiter.wait_for(|| self.try_pop());

        match res {
            Ok(val) => {
                notifier.notify();
                Ok(val)
            }
            _ => res,
        }
    }
}

// Both the Consumer and Producer can be sent from one thread to another
// but only one of each can exist. That is, you can not have shared references
// to any one of them.
unsafe impl<T: Send> Send for Consumer<T> {}
unsafe impl<T: Send> Send for Producer<T> {}

pub fn new_spsc<T>(pow: u8) -> (Consumer<T>, Producer<T>) {
    let inner = Arc::new(Inner::<T>::new(pow));
    let c = Consumer {
        inner: inner.clone(),
        tail_cache: 0,
    };

    let p = Producer {
        inner: inner,
        head_cache: 0,
    };

    (c, p)
}
