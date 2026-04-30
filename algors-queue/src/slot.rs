use core::cell::UnsafeCell;
use core::{mem::MaybeUninit, sync::atomic::AtomicUsize};

pub type Slot<T> = UnsafeCell<MaybeUninit<T>>;

pub struct SequencedSlot<T> {
    pub seq: AtomicUsize,
    pub data: UnsafeCell<MaybeUninit<T>>,
}
