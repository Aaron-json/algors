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
    unsafe {
        if p1.read() != p2.read() {
            return 0;
        } else {
            return 1;
        }
    }
}
