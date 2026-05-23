use alloc::alloc;
use core::ptr::NonNull;
use core::{mem, ptr};

const TAG_MASK: u8 = 0b111;
const TAG_MAX: u8 = 0b111;

/// BoundedRawVec is a growable array that only tracks its capacity and grows
/// up to 256.
/// This type assumes length is tracked externally.
/// All this is to make it as compact as possible.
/// Since pointers on most systems are 8 byte aligned, and most allocators
/// return memory that is at least 8 byte aligned, we can use the last 3 bits
/// to store the size class of the allocation. Together will "Nullness" of the
/// pointer, this gives us 9 possible states.
///
/// # Zero Sized Types
/// This type does not have optimizations for zero sized types for simplicity.
/// Our main use case does not benefit from this optimization.
///
/// # Drop
/// Since this type does not keep track of the length, it can not exhaustively
/// implement Drop. Callers must call `deallocate` in their Drop
/// implementations.
struct BoundedRawVec<T> {
    ptr: *mut T,
}

impl<T> BoundedRawVec<T> {
    #[inline(always)]
    fn new() -> Self {
        Self {
            ptr: ptr::null_mut(),
        }
    }

    /// This function returns the current value of the tag.
    /// Note that a null pointer and a pointer with tag 0 both return the
    /// same value although they might mean different things.
    #[inline(always)]
    fn cap_tag(&self) -> u8 {
        ((self.ptr as usize) & (TAG_MASK as usize)) as u8
    }

    #[inline(always)]
    fn capacity(&self) -> usize {
        if self.ptr.is_null() {
            0
        } else {
            1 << (self.cap_tag() + 1)
        }
    }

    /// Returns the alignment of the pointer to the data
    #[inline(always)]
    fn alignment() -> usize {
        // alignment must be at least 8 to use the last 3 bits
        mem::align_of::<T>().max(8)
    }

    #[inline(always)]
    fn as_mut_ptr(&self) -> *mut T {
        ((self.ptr as usize) & (!TAG_MASK as usize)) as *mut T
    }

    #[inline(always)]
    pub fn get(&self, i: usize, len: usize) -> Option<&T> {
        if self.ptr.is_null() || i >= len {
            None
        } else {
            unsafe { Some(&*self.as_mut_ptr().add(i)) }
        }
    }

    #[inline(always)]
    pub fn get_mut(&self, i: usize, len: usize) -> Option<&mut T> {
        if self.ptr.is_null() || i >= len {
            None
        } else {
            unsafe { Some(&mut *self.as_mut_ptr().add(i)) }
        }
    }

    fn insert(&mut self, i: usize, element: T, len: usize) {
        let cur_cap = self.capacity();
        let cur_tag = self.cap_tag();
        let mut ptr = self.as_mut_ptr();
        if len == cur_cap {
            let new_tag = if cur_cap == 0 {
                0
            } else {
                assert!(cur_tag < TAG_MAX);
                cur_tag + 1
            };
            // capacity will not be 0 after any insertion
            let new_cap = 1usize << (new_tag + 1);

            let align = Self::alignment();
            let layout = alloc::Layout::from_size_align(mem::size_of::<T>() * new_cap, align)
                .expect("Invalid layout for BoundedRawVec");

            unsafe {
                let new_ptr = alloc::alloc(layout) as *mut T;
                if new_ptr.is_null() {
                    alloc::handle_alloc_error(layout);
                }

                // pointer copy functions need a non-null, well aligned
                // pointer to avoid UB.
                if !ptr.is_null() {
                    // copy the elements before the insertion position
                    ptr::copy_nonoverlapping(ptr, new_ptr, i);
                    // copy the elements after the insertion position.
                    ptr::copy_nonoverlapping(ptr.add(i), new_ptr.add(i + 1), len - i);

                    let old_size = cur_cap * mem::size_of::<T>();
                    // unlikely to fail since we used this same layout
                    // in the allocation.
                    let old_layout = alloc::Layout::from_size_align(old_size, align).unwrap();
                    alloc::dealloc(ptr as *mut u8, old_layout);
                }

                ptr = new_ptr;
                self.ptr = ((ptr as usize) | new_tag as usize) as *mut T;
            }
        } else {
            unsafe {
                ptr::copy(ptr.add(i), ptr.add(i + 1), len - i);
            }
        }
        unsafe {
            ptr::write(ptr.add(i), element);
        }
    }
    pub fn deallocate(&mut self, len: usize) {
        let ptr = self.as_mut_ptr();
        if ptr.is_null() {
            return;
        }

        unsafe {
            for i in 0..len {
                self.as_mut_ptr().add(i).drop_in_place();
            }
            let size = self.capacity() * mem::size_of::<T>();
            let align = Self::alignment();
            let layout = alloc::Layout::from_size_align(size, align).unwrap();
            alloc::dealloc(ptr as *mut u8, layout);
        }
        self.ptr = ptr::null_mut();
    }
}

/// Calculates the longest common prefix
pub fn lcp(buf1: &[u8], buf2: &[u8]) -> usize {
    let len = core::cmp::min(buf1.len(), buf2.len());
    if len == 0 {
        return 0;
    }

    let p1 = buf1.as_ptr();
    let p2 = buf2.as_ptr();

    macro_rules! diff {
        ($x:expr, $y:expr, $offset: expr) => {
            if $x != $y {
                // find the first byte that is different
                let diff = $x ^ $y;
                #[cfg(target_endian = "little")]
                return $offset + (diff.trailing_zeros() as usize / 8);
                #[cfg(target_endian = "big")]
                return $offset + (diff.leading_zeros() as usize / 8);
            }
        };
    }

    // We compare memory in chunks of 8, 4, 2 or 1 depending on the length
    if len >= 8 {
        unsafe {
            let mut i = 0usize;
            while i + 8 <= len {
                let x = p1.add(i).cast::<u64>().read_unaligned();
                let y = p2.add(i).cast::<u64>().read_unaligned();
                diff!(x, y, i);
                i += 8;
            }
            let tail = len - 8;
            let x = p1.add(tail).cast::<u64>().read_unaligned();
            let y = p2.add(tail).cast::<u64>().read_unaligned();
            diff!(x, y, tail);
        }
        return len;
    }
    if len >= 4 {
        unsafe {
            let x = p1.cast::<u32>().read_unaligned();
            let y = p2.cast::<u32>().read_unaligned();
            diff!(x, y, 0);

            let tail = len - 4;
            let x = p1.add(tail).cast::<u32>().read_unaligned();
            let y = p2.add(tail).cast::<u32>().read_unaligned();
            diff!(x, y, tail);
        }
        return len;
    }
    if len >= 2 {
        unsafe {
            let x = p1.cast::<u16>().read_unaligned();
            let y = p2.cast::<u16>().read_unaligned();
            diff!(x, y, 0);

            let tail = len - 2;
            let x = p1.add(tail).cast::<u16>().read_unaligned();
            let y = p2.add(tail).cast::<u16>().read_unaligned();
            diff!(x, y, tail);
        }
        return len;
    }
    // Alignment is not an issue here since u8 have alignment of 1 so we can
    // just read the first element.
    if buf1[0] != buf2[0] {
        return 0;
    } else {
        return 1;
    }
}
