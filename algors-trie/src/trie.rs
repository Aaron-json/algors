struct Node {
    prefix: Box<[u8]>,
    bitmap: Option<Box<[u64; 4]>>,
    children: Option<Vec<Box<Node>>>,
}
struct Trie {
    root: Option<Box<Node>>,
}

impl Trie {
    pub fn new() -> Self {
        Self { root: None }
    }

    #[inline]
    fn new_node(prefix: &[u8]) -> Box<Node> {
        let pref_copy = prefix.to_vec().into_boxed_slice();
        Box::new(Node {
            prefix: pref_copy,
            bitmap: None,
            children: None,
        })
    }

    /// Returns (idx, bit) pair.
    #[inline(always)]
    fn bit_index(byte: u8) -> (usize, u8) {
        // divide by 64 to get the elememt index
        let idx = byte >> 6;
        // modulo to get the bit
        let bit = byte & 63;

        (idx as usize, bit)
    }

    /// Returns the index of the child, if exists, given the child's expected
    /// first byte
    /// This function ALWAYS assumes that the child exists to avoid duplicate
    /// checks.
    /// You must first check that the child exists.
    fn child_idx_from_byte(bitmap: &[u64; 4], byte: u8) -> usize {
        let (idx, bit) = Self::bit_index(byte);

        // find the total number of nodes in the array that are before
        // this byte's node
        let mut pos = 0;
        let mut i = 0;
        while i < idx {
            pos += bitmap[i].count_ones();
        }

        // mask off the bits after the current one
        let mask = (1 << bit) - 1;
        pos += (bitmap[idx] & mask).count_ones();

        pos as usize
    }

    #[inline(always)]
    fn child_exists(bitmap: &[u64; 4], byte: u8) -> bool {
        let (idx, bit) = Self::bit_index(byte);
        bitmap[idx] & (1 << bit) != 0
    }

    pub fn insert<R>(&mut self, data: R)
    where
        R: AsRef<[u8]>,
    {
        let data_ref = data.as_ref();
        if data_ref.is_empty() {
            return;
        }

        if let None = self.root {
            self.root = Some(Self::new_node(data_ref));
            return;
        }

        let mut cur_data = data_ref;
        let mut cur_node = self.root.as_mut().unwrap();
        loop {
            let lcp_len = Self::lcp(cur_node.prefix.as_ref(), cur_data);
            if lcp_len < cur_node.prefix.len() {
                let mut split_node = Self::new_node(&cur_node.prefix[lcp_len..]);
                split_node.children = cur_node.children.take();
                split_node.bitmap = cur_node.bitmap.take();
                let split_node_byte = split_node.prefix[0];

                cur_node.prefix = cur_node.prefix[..lcp_len].to_vec().into_boxed_slice();

                let mut new_bitmap = Box::new([0u64; 4]);
                let (idx, bit) = Self::bit_index(split_node_byte);
                new_bitmap[idx] |= 1 << bit;

                cur_node.bitmap = Some(new_bitmap);
                cur_node.children = Some(vec![split_node]);
            }

            if lcp_len == cur_data.len() {
                // already exists
                return;
            }

            cur_data = &cur_data[lcp_len..];
            let byte = cur_data[0];
            if cur_node.bitmap.is_none() {
                cur_node.bitmap = Some(Box::new([0u64; 4]));
                // if there are no children we will for sure add one
                cur_node.children = Some(Vec::with_capacity(1));
            }
            let children = cur_node.children.as_mut().unwrap();
            let bitmap = cur_node.bitmap.as_mut().unwrap();
            let (idx, bit) = Self::bit_index(byte);
            if Self::child_exists(bitmap, byte) {
                let child_idx = Self::child_idx_from_byte(bitmap, byte);
                cur_node = &mut children[child_idx];
            } else {
                // no child exists with this byte prefix
                let new_child = Self::new_node(cur_data);
                bitmap[idx] |= 1 << bit;
                let child_idx = Self::child_idx_from_byte(bitmap, byte);
                children.insert(child_idx, new_child);
                return;
            }
        }
    }

    fn lcp(buf1: &[u8], buf2: &[u8]) -> usize {
        let len = core::cmp::min(buf1.len(), buf2.len());

        let mut i = 0;
        let p1 = buf1.as_ptr();
        let p2 = buf2.as_ptr();

        while i + 8 <= len {
            unsafe {
                let x = u64::from_ne_bytes(p1.add(i).cast::<[u8; 8]>().read_unaligned());
                let y = u64::from_ne_bytes(p2.add(i).cast::<[u8; 8]>().read_unaligned());

                if x != y {
                    // find the first byte that is different
                    let diff = x ^ y;
                    #[cfg(target_endian = "little")]
                    return i + (diff.trailing_zeros() as usize / 8);
                    #[cfg(target_endian = "big")]
                    return i + (diff.leading_zeros() as usize / 8);
                }
            }
            i += 8;
        }

        while i < len {
            if buf1[i] != buf2[i] {
                return i;
            }
            i += 1;
        }
        len
    }
}
