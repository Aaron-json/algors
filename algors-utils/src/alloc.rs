extern crate alloc;

use core::mem::MaybeUninit;

use alloc::boxed;

// Allocates memory for `size` uninitialized elements of type `T`.
pub fn alloc_uninit_slice<T>(size: usize) -> boxed::Box<[MaybeUninit<T>]> {
    let mut data = Vec::with_capacity(size);
    unsafe {
        data.set_len(size);
    }
    data.into_boxed_slice()
}
