use core::mem;

#[derive(Clone, Copy)]
struct Inline {
    // stores the length in the first 7 bits and the variant tag in the
    // least significant bit.
    len_tag: u8,
    data: [u8; 15],
}
// A compact prefix that avoids heap allocations for prefixes under 16 bytes.
pub union Prefix {
    alloc: mem::ManuallyDrop<Box<[u8]>>,
    inline: Inline,
}

impl Prefix {
    pub fn new(data: &[u8]) -> Self {
        let len = data.len();
        if len < 16 {
            let mut res = Self {
                inline: Inline {
                    len_tag: ((len as u8) << 1) | 0x1,
                    data: [0u8; 15],
                },
            };
            unsafe {
                res.inline.data[..len].copy_from_slice(data);
            }

            res
        } else {
            Self {
                alloc: mem::ManuallyDrop::new(data.to_vec().into_boxed_slice()),
            }
        }
    }

    #[inline(always)]
    pub fn is_inline(&self) -> bool {
        unsafe { self.inline.len_tag & 0x1 == 1 }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        unsafe {
            if self.is_inline() {
                (self.inline.len_tag >> 1) as usize
            } else {
                self.alloc.len()
            }
        }
    }

    #[inline(always)]
    pub fn as_ref(&self) -> &[u8] {
        unsafe {
            if self.is_inline() {
                &self.inline.data[..self.len()]
            } else {
                self.alloc.as_ref()
            }
        }
    }
}

impl Drop for Prefix {
    fn drop(&mut self) {
        unsafe {
            if !self.is_inline() {
                mem::ManuallyDrop::drop(&mut self.alloc);
            }
        }
    }
}
