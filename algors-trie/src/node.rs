use crate::prefix::Prefix;
use crate::util;
use core::mem;

pub struct Node {
    pub prefix: Prefix,
    bitmap: [u64; 4],
    children: util::BoundedRawVec<Node>,
}

// The location of the bit representing a child.
#[derive(Clone, Copy)]
pub struct ChildPos {
    idx: usize,
    bit: u8,
}

impl Node {
    #[inline(always)]
    pub fn new(prefix: &[u8]) -> Node {
        Node {
            prefix: Prefix::new(prefix),
            bitmap: [0; 4],
            children: util::BoundedRawVec::new(),
        }
    }

    /// Returns the number of children the node has
    #[inline(always)]
    pub fn len_children(&self) -> usize {
        let bitmap = self.bitmap;
        (bitmap[0].count_ones()
            + bitmap[1].count_ones()
            + bitmap[2].count_ones()
            + bitmap[3].count_ones()) as usize
    }

    #[inline(always)]
    pub fn child_exists(&self, pos: ChildPos) -> bool {
        self.bitmap[pos.idx] & (1 << pos.bit) != 0
    }

    /// Given a byte, it returns its position among the children
    #[inline(always)]
    pub fn child_pos(byte: u8) -> ChildPos {
        // divide by 64 to get the elememt index
        let idx = byte >> 6;
        // modulo to get the bit
        let bit = byte & 63;

        ChildPos {
            idx: idx as usize,
            bit,
        }
    }

    /// Returns the index of the child, if exists, given the child's expected
    /// first byte
    /// This function ALWAYS assumes that the child exists to avoid duplicate
    /// checks.
    /// You must first check that the child exists or set it in the bitmap.
    #[inline]
    pub fn child_idx_from_pos(&self, pos: ChildPos) -> usize {
        let bitmap = self.bitmap;
        // eliminate bounds checks
        let safe_idx = pos.idx & 3;

        // avoids branching.
        // on ARM this relies on NEON (CNT) which is mandatory for aarch64.
        // on x86 this relies on SSE(4.2) (POPCOUNT` in the SSE 4.2 and is present
        // since x86-64-v2.
        let before = (safe_idx > 0) as u32 * bitmap[0].count_ones()
            + (safe_idx > 1) as u32 * bitmap[1].count_ones()
            + (safe_idx > 2) as u32 * bitmap[2].count_ones();

        let mask = (1u64 << pos.bit) - 1;
        let current = (bitmap[safe_idx] & mask).count_ones();

        (before + current) as usize
    }

    /// Returns a reference to the child.
    /// Can only be called if the caller is certain the child exists.
    pub fn get_child(&self, pos: ChildPos) -> Option<&Node> {
        let idx = self.child_idx_from_pos(pos);
        let len = self.len_children();
        self.children.get(idx, len)
    }

    /// Returns a mutable reference to the child.
    /// Can only be called if the caller knows the child exists.
    pub fn get_mut_child(&mut self, pos: ChildPos) -> Option<&mut Node> {
        let idx = self.child_idx_from_pos(pos);
        let len = self.len_children();
        self.children.get_mut(idx, len)
    }

    /// Stores the given node as a child to this node.
    #[inline]
    pub fn insert_child(&mut self, pos: ChildPos, child: Node) {
        self.bitmap[pos.idx] |= 1 << pos.bit;
        let idx = self.child_idx_from_pos(pos);
        let len = self.len_children();
        self.children.insert(idx, child, len);
    }

    // Gives its children state to another node.
    // useful when splitting nodes
    #[inline]
    pub fn transfer_children(&mut self, other: &mut Node) {
        other.children = mem::take(&mut self.children);
        other.bitmap = self.bitmap;
        self.bitmap = [0u64; 4];
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        let child_len = self.len_children();
        self.children.deallocate(child_len);
    }
}
