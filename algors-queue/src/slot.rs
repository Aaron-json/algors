use crate::sync::atomic::AtomicUsize;
use core::cell::UnsafeCell;
use core::mem::MaybeUninit;

/// A slot representing a
pub type Slot<T> = UnsafeCell<MaybeUninit<T>>;

pub struct SequencedSlot<T> {
    pub seq: AtomicUsize,
    pub data: UnsafeCell<MaybeUninit<T>>,
}
