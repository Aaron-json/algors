use alloc::alloc;
use core::{mem, ptr, slice};

// To avoid heap allocating small prefixes on the heap, we only
// when we cannot store the prefix inline.

const PREFIX_SIZE: usize = 16;
const INLINE_CAP: usize = PREFIX_SIZE - 1;
const PTR_SIZE: usize = mem::size_of::<*const u8>();
const USIZE_SIZE: usize = mem::size_of::<usize>();

// Index of the byte that stores the tag (length when inlining).
// Must overlap with the pointer's Least significant Byte.
const TAG_LEN_BYTE: usize = if cfg!(target_endian = "little") {
    0
} else {
    PREFIX_SIZE - 1
};

const POINTER_OFFSET: usize = if cfg!(target_endian = "little") {
    0
} else {
    PREFIX_SIZE - PTR_SIZE
};

// Only used in the non inlined variant.
// When data is inlined, the length is packed into the tag byte
// to maximize space for inlining.
const ALLOC_LEN_OFFSET: usize = if cfg!(target_endian = "little") {
    PTR_SIZE
} else {
    PREFIX_SIZE - PTR_SIZE - USIZE_SIZE
};

// Obviously only used when the prefix is inlined.
const INLINE_DATA_OFFSET: usize = if cfg!(target_endian = "little") { 1 } else { 0 };

/// A compact prefix that avoids heap allocations for small prefixes.
/// Small prefixes are stored inline and larger ones are heap allocated
pub struct Prefix {
    // Forces alignment to the native pointer size to avoid
    // unaligned accesses at runtime which might be slower
    // on some systems.
    _align: [*mut u8; 0],
    bytes: [u8; PREFIX_SIZE],
}

impl Prefix {
    pub fn new(data: &[u8]) -> Self {
        let mut res = Self {
            _align: [],
            bytes: [0u8; PREFIX_SIZE],
        };
        let len = data.len();
        if len <= INLINE_CAP {
            res.bytes[TAG_LEN_BYTE] = ((len << 1) as u8) | 0x1;
            unsafe {
                ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    res.bytes.as_mut_ptr().add(INLINE_DATA_OFFSET),
                    len,
                );
            };
        } else {
            unsafe {
                // Aligning by 2 bytes lets us use the last bit as the tag.
                let layout = alloc::Layout::from_size_align(len, 2)
                    .expect("Prefix layout could not be created");

                let pt = alloc::alloc(layout);
                if pt.is_null() {
                    alloc::handle_alloc_error(layout);
                }
                ptr::copy_nonoverlapping(data.as_ptr(), pt, len);
                ptr::write_unaligned(
                    res.bytes.as_mut_ptr().add(ALLOC_LEN_OFFSET) as *mut usize,
                    len,
                );
                ptr::write_unaligned(
                    res.bytes.as_mut_ptr().add(POINTER_OFFSET) as *mut *mut u8,
                    pt,
                );
            }
        }
        res
    }

    /// Allows creating a single Prefix from multiple efficiently.
    pub fn from_slices(slices: &[&[u8]]) -> Self {
        let total_len: usize = slices.iter().map(|s| s.len()).sum();
        let mut res = Self {
            _align: [],
            bytes: [0u8; PREFIX_SIZE],
        };

        if total_len <= INLINE_CAP {
            res.bytes[TAG_LEN_BYTE] = ((total_len << 1) as u8) | 0x1;
            let mut i = 0;
            for slice in slices.iter() {
                // we are certain the memory is not overlapping since we
                // are creating a new immutable object
                let slice_len = slice.len();
                unsafe {
                    res.bytes
                        .as_mut_ptr()
                        .add(INLINE_DATA_OFFSET)
                        .add(i)
                        .copy_from_nonoverlapping(slice.as_ptr(), slice_len);
                }
                i += slice_len;
            }
        } else {
            unsafe {
                // Aligning by 2 bytes lets us use the last bit as the tag.
                let layout = alloc::Layout::from_size_align(total_len, 2)
                    .expect("Prefix layout could not be created");

                let pt = alloc::alloc(layout);
                if pt.is_null() {
                    alloc::handle_alloc_error(layout);
                }

                let mut i = 0;
                for slice in slices.iter() {
                    // we are certain the memory is not overlapping since we
                    // are creating a new immutable object
                    let slice_len = slice.len();
                    pt.add(i)
                        .copy_from_nonoverlapping(slice.as_ptr(), slice_len);
                    i += slice_len;
                }
                ptr::write_unaligned(
                    res.bytes.as_mut_ptr().add(ALLOC_LEN_OFFSET) as *mut usize,
                    total_len,
                );
                ptr::write_unaligned(
                    res.bytes.as_mut_ptr().add(POINTER_OFFSET) as *mut *mut u8,
                    pt,
                );
            }
        }
        res
    }

    #[inline(always)]
    pub fn is_inline(&self) -> bool {
        (self.bytes[TAG_LEN_BYTE] & 0x1) == 1
    }

    #[inline(always)]
    fn len_inline(&self) -> usize {
        (self.bytes[TAG_LEN_BYTE] >> 1) as usize
    }

    #[inline(always)]
    fn len_alloc(&self) -> usize {
        unsafe { (self.bytes.as_ptr().add(ALLOC_LEN_OFFSET) as *const usize).read_unaligned() }
    }

    /// Returns the pointer to the heap allocated data.
    /// This can ONLY be called if this is the heap allocated variant.
    #[inline(always)]
    fn ptr_alloc(&self) -> *mut u8 {
        unsafe { (self.bytes.as_ptr().add(POINTER_OFFSET) as *const *mut u8).read_unaligned() }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        if self.is_inline() {
            self.len_inline()
        } else {
            self.len_alloc()
        }
    }

    #[inline(always)]
    pub fn as_ref(&self) -> &[u8] {
        unsafe {
            if self.is_inline() {
                let len = self.len_inline();
                &self.bytes[INLINE_DATA_OFFSET..INLINE_DATA_OFFSET + len]
            } else {
                slice::from_raw_parts(self.ptr_alloc(), self.len_alloc())
            }
        }
    }
}

impl Drop for Prefix {
    fn drop(&mut self) {
        unsafe {
            if !self.is_inline() {
                let layout = alloc::Layout::from_size_align(self.len_alloc(), 2).unwrap();
                alloc::dealloc(self.ptr_alloc(), layout);
            }
        }
    }
}
