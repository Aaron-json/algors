use alloc::alloc;
use core::iter::FusedIterator;
use core::marker::PhantomData;
use core::{mem, ptr};

const TAG_MASK: u8 = 0b111;
const TAG_MAX: u8 = 0b111;
const CAPACITY_MAX: usize = 256;

/// BoundedRawVec is a growable array that only tracks its capacity and grows
/// up to 256.
/// This type assumes length is tracked externally.
/// All this is to make it as compact as possible.
/// Since pointers on most systems are 8 byte aligned, and most allocators
/// return memory that is at least 8 byte aligned, we can use the last 3 bits
/// to store the size class of the allocation. Together will "Nullness" of the
/// pointer, this gives us 9 possible states.
///
/// # Drop
/// Since this type does not keep track of the length, it can not exhaustively
/// implement Drop. Callers must call `deallocate` in their Drop
/// implementations.
pub struct BoundedRawVec<T> {
    tagged: *mut T,
}

impl<T> BoundedRawVec<T> {
    #[inline]
    pub fn new() -> Self {
        let ptr_val = if Self::is_zst() {
            ptr::dangling_mut()
        } else {
            ptr::null_mut()
        };
        Self { tagged: ptr_val }
    }

    /// Checks whether the type stored is a Zero Sized Type.
    #[inline(always)]
    const fn is_zst() -> bool {
        mem::size_of::<T>() == 0
    }

    /// This function returns the current value of the tag.
    /// Note that a null pointer and a pointer with tag 0 both return the
    /// same value although they might mean different things.
    #[inline(always)]
    fn cap_tag(&self) -> u8 {
        ((self.tagged as usize) & (TAG_MASK as usize)) as u8
    }

    #[inline(always)]
    fn capacity(&self) -> usize {
        if Self::is_zst() {
            return CAPACITY_MAX;
        }
        if self.tagged.is_null() {
            0
        } else {
            1 << (self.cap_tag() + 1)
        }
    }

    /// Returns the alignment of the pointer to the data
    /// Must NOT be used on ZST since we do not control their pointer value.
    #[inline(always)]
    fn alignment() -> usize {
        // alignment must be at least 8 to use the last 3 bits
        mem::align_of::<T>().max(8)
    }

    #[inline(always)]
    fn as_mut_ptr(&self) -> *mut T {
        if mem::size_of::<T>() == 0 {
            // ZST dangling pointers do not go through the allocator and
            // so we don't have control over their alignment
            // We handle them as a special case.
            return self.tagged;
        }
        ((self.tagged as usize) & (!TAG_MASK as usize)) as *mut T
    }

    #[inline(always)]
    pub fn get(&self, i: usize, len: usize) -> Option<&T> {
        if self.tagged.is_null() || i >= len {
            None
        } else {
            unsafe { Some(&*self.as_mut_ptr().add(i)) }
        }
    }

    #[inline(always)]
    pub fn get_mut(&mut self, i: usize, len: usize) -> Option<&mut T> {
        if self.tagged.is_null() || i >= len {
            None
        } else {
            unsafe { Some(&mut *self.as_mut_ptr().add(i)) }
        }
    }

    pub fn insert(&mut self, i: usize, element: T, len: usize) {
        if Self::is_zst() {
            unsafe {
                // ZST pointers are no-ops for writes/reads and arithmetic.
                // We still need to write this to avoid dropping the element
                // prematurely
                let p = self.as_mut_ptr();
                ptr::copy(p.add(i), p.add(i + 1), len - i);
                ptr::write(p.add(i), element);
            }
            return;
        }
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
                self.tagged = ((ptr as usize) | new_tag as usize) as *mut T;
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

    /// Removes an element from the container and returns it, if it exists.
    pub fn remove(&mut self, i: usize, len: usize) -> Option<T> {
        if self.tagged.is_null() || i >= len {
            return None;
        }

        unsafe {
            let p = self.as_mut_ptr();
            let val = p.add(i).read();

            ptr::copy(p.add(i + 1), p.add(i), len - i - 1);
            Some(val)
        }
    }

    pub fn deallocate(&mut self, len: usize) {
        // ZST never go through an allocator.
        // We just need to drop
        if Self::is_zst() {
            for i in 0..len {
                unsafe {
                    self.as_mut_ptr().add(i).drop_in_place();
                }
            }
            return;
        }

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
        self.tagged = ptr::null_mut();
    }

    pub fn iter(&self, len: usize) -> Iter<T> {
        Iter {
            ptr: self.as_mut_ptr(),
            len,
            _marker: PhantomData,
        }
    }
    pub fn iter_mut(&mut self, len: usize) -> IterMut<T> {
        IterMut {
            ptr: self.as_mut_ptr(),
            len,
            _marker: PhantomData,
        }
    }
}

/// Immutable iterator over BoundedRawVec.
pub struct Iter<'a, T> {
    ptr: *const T,
    len: usize,
    _marker: PhantomData<&'a T>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            None
        } else {
            let res = unsafe { &*self.ptr };
            // ZSTs will return the same pointer for all elements
            // naturally
            self.ptr = unsafe { self.ptr.add(1) };
            self.len -= 1;
            Some(res)
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }

    #[inline]
    fn count(self) -> usize {
        self.len
    }
}

impl<'a, T> DoubleEndedIterator for Iter<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            unsafe {
                let p = if mem::size_of::<T>() == 0 {
                    self.ptr
                } else {
                    self.ptr.add(self.len)
                };
                Some(&*p)
            }
        }
    }
}

impl<'a, T> ExactSizeIterator for Iter<'a, T> {}
impl<'a, T> FusedIterator for Iter<'a, T> {}

impl<'a, T> Clone for Iter<'a, T> {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr,
            len: self.len,
            _marker: PhantomData,
        }
    }
}

/// Mutable iterator over BoundedRawVec.
pub struct IterMut<'a, T> {
    ptr: *mut T,
    len: usize,
    _marker: PhantomData<&'a mut T>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            None
        } else {
            let res = unsafe { &mut *self.ptr };
            if mem::size_of::<T>() != 0 {
                self.ptr = unsafe { self.ptr.add(1) };
            }
            self.len -= 1;
            Some(res)
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }

    #[inline]
    fn count(self) -> usize {
        self.len
    }
}

impl<'a, T> DoubleEndedIterator for IterMut<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            unsafe {
                let p = if mem::size_of::<T>() == 0 {
                    self.ptr
                } else {
                    self.ptr.add(self.len)
                };
                Some(&mut *p)
            }
        }
    }
}

impl<'a, T> ExactSizeIterator for IterMut<'a, T> {}
impl<'a, T> FusedIterator for IterMut<'a, T> {}

impl<T> Default for BoundedRawVec<T> {
    fn default() -> Self {
        Self::new()
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
