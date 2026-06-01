use crate::prefix::Prefix;
use crate::util::{self, Iter, IterMut};
use core::mem;

/// Node represents a single Radix Tree node.
pub struct Node<T> {
    pub prefix: Prefix,
    bitmap: [u64; 4],
    children: util::BoundedRawVec<Node<T>>,
    pub val: Option<T>,
}

// The location of the bit representing a child.
#[derive(Clone, Copy)]
pub struct ChildPos {
    idx: usize,
    bit: u8,
}

impl<T> Node<T> {
    #[inline(always)]
    pub fn new(prefix: &[u8]) -> Node<T> {
        Node {
            prefix: Prefix::new(prefix),
            bitmap: [0; 4],
            children: util::BoundedRawVec::new(),
            val: None,
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
    pub fn child_pos_from_byte(byte: u8) -> ChildPos {
        // divide by 64 to get the elememt index
        let idx = byte >> 6;
        // modulo to get the bit
        let bit = byte & 63;

        ChildPos {
            idx: idx as usize,
            bit,
        }
    }

    /// Given a child index, it returns its position in the bitmap.
    /// # Panic
    /// Panics if the index is out of bounds.
    #[inline]
    pub fn child_pos_from_idx(&self, idx: usize) -> ChildPos {
        // NOTE: This is a hacky/easy wat to do this. It does a memory
        // read to get the first which could cause a chache miss. Most of the
        // time the prefix should be inlined and the value in cache.
        let len = self.len_children();
        let child = self.children.get(idx, len).expect("Index out of bounds");
        Self::child_pos_from_byte(child.prefix.as_ref()[0])
    }

    /// Returns the index of the child, if exists, given the child's expected
    /// first byte
    #[inline]
    pub fn child_idx_from_pos(&self, pos: ChildPos) -> usize {
        let bitmap = self.bitmap;
        // eliminate bounds checks
        let safe_idx = pos.idx & 3;

        // avoids branching.
        // on ARM this relies on NEON (CNT) which is mandatory for aarch64.
        // on x86 this relies on SSE 4.2 (POPCOUNT) and is present since
        // x86-64-v2.
        let before = (safe_idx > 0) as u32 * bitmap[0].count_ones()
            + (safe_idx > 1) as u32 * bitmap[1].count_ones()
            + (safe_idx > 2) as u32 * bitmap[2].count_ones();

        let mask = (1u64 << pos.bit) - 1;
        let current = (bitmap[safe_idx] & mask).count_ones();

        (before + current) as usize
    }

    /// Returns a reference to the child.
    /// The given posision must be for an existing child, otherwise the
    /// wrong child might be returned.
    #[inline]
    pub fn get_child_by_pos(&self, pos: ChildPos) -> Option<&Node<T>> {
        let idx = self.child_idx_from_pos(pos);
        self.get_child_by_idx(idx)
    }

    /// Returns a mutable reference to the child.
    /// The given posision must be for an existing child, otherwise the
    /// wrong child might be returned.
    #[inline]
    pub fn get_mut_child_by_pos(&mut self, pos: ChildPos) -> Option<&mut Node<T>> {
        let idx = self.child_idx_from_pos(pos);
        self.get_mut_child_by_idx(idx)
    }

    /// Returns the child at the given index.
    /// # Panic
    /// Panics if the index is out of bounds.
    #[inline]
    pub fn get_child_by_idx(&self, idx: usize) -> Option<&Node<T>> {
        let len = self.len_children();
        self.children.get(idx, len)
    }

    /// Returns a mutable reference to the child.
    /// # Panic
    /// Panics if the index is out of bounds.
    #[inline]
    pub fn get_mut_child_by_idx(&mut self, idx: usize) -> Option<&mut Node<T>> {
        let len = self.len_children();
        self.children.get_mut(idx, len)
    }

    /// Stores the given node as a child to this node.
    #[inline]
    pub fn insert_child(&mut self, pos: ChildPos, child: Node<T>) {
        let idx = self.child_idx_from_pos(pos);
        let len = self.len_children();
        self.children.insert(idx, child, len);
        self.bitmap[pos.idx] |= 1 << pos.bit;
    }

    /// Removes the children at the given position, and returns the it.
    #[inline]
    pub fn remove_child(&mut self, pos: ChildPos) -> Option<Node<T>> {
        let idx = self.child_idx_from_pos(pos);
        let len = self.len_children();
        let res = self.children.remove(idx, len);
        self.bitmap[pos.idx] &= !(1 << pos.bit);
        res
    }

    /// Gives its children state to another node.
    /// useful when splitting nodes
    #[inline]
    pub fn transfer_children(&mut self, other: &mut Node<T>) {
        other.children = mem::take(&mut self.children);
        other.bitmap = self.bitmap;
        self.bitmap = [0u64; 4];
    }

    pub fn iter_children(&self) -> Iter<Node<T>> {
        self.children.iter(self.len_children())
    }

    pub fn iter_mut_children(&mut self) -> IterMut<Node<T>> {
        self.children.iter_mut(self.len_children())
    }
}

impl<T> Drop for Node<T> {
    fn drop(&mut self) {
        let child_len = self.len_children();
        self.children.deallocate(child_len);
    }
}
