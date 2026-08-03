use alloc::alloc;
use core::{mem, ptr, slice};

// To avoid heap allocating small prefixes on the heap, we only
// when we cannot store the prefix inline.

const PREFIX_SIZE: usize = 16;
const INLINE_CAP: usize = PREFIX_SIZE - 1;
const PTR_SIZE: usize = mem::size_of::<*const u8>();
const ALLOC_LEN_SIZE: usize = mem::size_of::<u32>();

// This is the only offset that can be used by both the inlined and allocating
// variants. Specifically, only the tag BIT can be read by both variants.
// This byte MUST overlap with the pointer's Least significant Byte.
const INLINE_TAG_LEN_BYTE: usize = if cfg!(target_endian = "little") {
    0
} else {
    PREFIX_SIZE - 1
};

const INLINE_DATA: usize = if cfg!(target_endian = "little") { 1 } else { 0 };

const ALLOC_DATA_PTR: usize = if cfg!(target_endian = "little") {
    0
} else {
    PREFIX_SIZE - PTR_SIZE
};

// Only used in the non inlined variant.
// When data is inlined, the length is packed into the tag byte
// to maximize space for inlining.
const ALLOC_LEN: usize = if cfg!(target_endian = "little") {
    PTR_SIZE
} else {
    PREFIX_SIZE - PTR_SIZE - ALLOC_LEN_SIZE
};

const ALLOC_FIRST_BYTE: usize = if cfg!(target_endian = "little") {
    PTR_SIZE + ALLOC_LEN_SIZE
} else {
    PREFIX_SIZE - PTR_SIZE - ALLOC_LEN_SIZE - 1
};

/// Compact owned prefix with a 16-byte representation.
///
/// ```text
/// 64-bit layout, low address on the left:
/// LE inline  [ tag + len: 1 | data: 15 ]
/// BE inline  [ data: 15 | tag + len: 1 ]
/// LE heap    [ pointer: 8 | len: 4 | first: 1 | unused: 3 ]
/// BE heap    [ unused: 3 | first: 1 | len: 4 | pointer: 8 ]
/// ```
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
            res.bytes[INLINE_TAG_LEN_BYTE] = ((len << 1) as u8) | 0x1;
            unsafe {
                ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    res.bytes.as_mut_ptr().add(INLINE_DATA),
                    len,
                );
            };
        } else {
            let stored_len = u32::try_from(len).expect("Prefix length exceeds u32::MAX");
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
                    res.bytes.as_mut_ptr().add(ALLOC_LEN) as *mut u32,
                    stored_len,
                );
                res.bytes[ALLOC_FIRST_BYTE] = data[0];
                ptr::write_unaligned(
                    res.bytes.as_mut_ptr().add(ALLOC_DATA_PTR) as *mut *mut u8,
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
            res.bytes[INLINE_TAG_LEN_BYTE] = ((total_len << 1) as u8) | 0x1;
            let mut i = 0;
            for slice in slices.iter() {
                // we are certain the memory is not overlapping since we
                // are creating a new immutable object
                let slice_len = slice.len();
                unsafe {
                    res.bytes
                        .as_mut_ptr()
                        .add(INLINE_DATA)
                        .add(i)
                        .copy_from_nonoverlapping(slice.as_ptr(), slice_len);
                }
                i += slice_len;
            }
        } else {
            let stored_len = u32::try_from(total_len).expect("Prefix length exceeds u32::MAX");
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
                    res.bytes.as_mut_ptr().add(ALLOC_LEN) as *mut u32,
                    stored_len,
                );
                res.bytes[ALLOC_FIRST_BYTE] = slices
                    .iter()
                    .find_map(|slice| slice.first())
                    .copied()
                    .expect("A non-empty prefix must contain a first byte");
                ptr::write_unaligned(
                    res.bytes.as_mut_ptr().add(ALLOC_DATA_PTR) as *mut *mut u8,
                    pt,
                );
            }
        }
        res
    }

    #[inline(always)]
    pub fn is_inline(&self) -> bool {
        (self.bytes[INLINE_TAG_LEN_BYTE] & 0x1) == 1
    }

    #[inline(always)]
    fn len_inline(&self) -> usize {
        (self.bytes[INLINE_TAG_LEN_BYTE] >> 1) as usize
    }

    #[inline(always)]
    fn len_alloc(&self) -> usize {
        unsafe { (self.bytes.as_ptr().add(ALLOC_LEN) as *const u32).read_unaligned() as usize }
    }

    /// Returns the pointer to the heap allocated data.
    /// This can ONLY be called if this is the heap allocated variant.
    #[inline(always)]
    fn ptr_alloc(&self) -> *mut u8 {
        unsafe { (self.bytes.as_ptr().add(ALLOC_DATA_PTR) as *const *mut u8).read_unaligned() }
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
    pub fn first_byte(&self) -> u8 {
        debug_assert!(self.len() > 0, "Prefix cannot be empty");
        if self.is_inline() {
            self.bytes[INLINE_DATA]
        } else {
            self.bytes[ALLOC_FIRST_BYTE]
        }
    }

    #[inline(always)]
    pub fn as_ref(&self) -> &[u8] {
        unsafe {
            if self.is_inline() {
                let len = self.len_inline();
                &self.bytes[INLINE_DATA..INLINE_DATA + len]
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
