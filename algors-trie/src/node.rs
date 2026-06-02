use crate::prefix::Prefix;
use crate::util::{self, Iter, IterMut};
use alloc::alloc::{Layout, alloc, dealloc};
use core::marker::PhantomData;
use core::mem;
use core::ptr;

// TODO: currently nodes use Option<T> for the value. this is fine but many
// types like regular numbers and UUIDs don't have niches and will waste a
// lot of space in padding.
// A different approach is using MaybeUninit<T> and storing a flag for the
// value in the pointer
// Further, applications that use this as a set with a zero sized type will
// truly allocate no space for the value. right now a ZST will cost us
// a byte and some padding

pub struct LeafNode<T> {
    prefix: Prefix,
    val: Option<T>,
}

pub struct InternalNode<T> {
    prefix: Prefix,
    bitmap: [u64; 4],
    children: util::BoundedRawVec<Node<T>>,
    val: Option<T>,
}

impl<T> InternalNode<T> {
    #[inline(always)]
    pub fn len_children(&self) -> usize {
        let bitmap = self.bitmap;
        (bitmap[0].count_ones()
            + bitmap[1].count_ones()
            + bitmap[2].count_ones()
            + bitmap[3].count_ones()) as usize
    }
}

impl<T> Drop for InternalNode<T> {
    fn drop(&mut self) {
        let child_len = self.len_children();
        self.children.deallocate(child_len);
    }
}

/// Node represents a single Radix Tree node, implemented as a tagged pointer.
pub struct Node<T> {
    ptr: usize,
    _marker: PhantomData<T>,
}

const LEAF_TAG: usize = 0x1;
const TAG_MASK: usize = 0x1;

// The location of the bit representing a child.
#[derive(Clone, Copy)]
pub struct ChildPos {
    pub idx: usize,
    pub bit: u8,
}

impl<T> Node<T> {
    #[inline(always)]
    pub fn new(prefix: &[u8]) -> Node<T> {
        Self::new_leaf(prefix, None)
    }

    pub fn new_leaf(prefix: &[u8], val: Option<T>) -> Self {
        let node = LeafNode {
            prefix: Prefix::new(prefix),
            val,
        };
        let layout = Layout::new::<LeafNode<T>>().align_to(2).unwrap();
        unsafe {
            let ptr = alloc(layout) as *mut LeafNode<T>;
            if ptr.is_null() {
                alloc::alloc::handle_alloc_error(layout);
            }
            ptr::write(ptr, node);
            Self {
                ptr: (ptr as usize) | LEAF_TAG,
                _marker: PhantomData,
            }
        }
    }

    pub fn new_internal(prefix: &[u8], val: Option<T>) -> Self {
        let node = InternalNode {
            prefix: Prefix::new(prefix),
            bitmap: [0; 4],
            children: util::BoundedRawVec::new(),
            val,
        };
        let layout = Layout::new::<InternalNode<T>>().align_to(2).unwrap();
        unsafe {
            let ptr = alloc(layout) as *mut InternalNode<T>;
            if ptr.is_null() {
                alloc::alloc::handle_alloc_error(layout);
            }
            ptr::write(ptr, node);
            Self {
                ptr: ptr as usize,
                _marker: PhantomData,
            }
        }
    }

    #[inline(always)]
    pub fn is_leaf(&self) -> bool {
        (self.ptr & TAG_MASK) == LEAF_TAG
    }

    #[inline(always)]
    fn as_leaf_ptr(&self) -> *mut LeafNode<T> {
        (self.ptr & !TAG_MASK) as *mut LeafNode<T>
    }

    #[inline(always)]
    fn as_internal_ptr(&self) -> *mut InternalNode<T> {
        (self.ptr & !TAG_MASK) as *mut InternalNode<T>
    }

    #[inline(always)]
    pub fn prefix(&self) -> &Prefix {
        if self.is_leaf() {
            unsafe { &(*self.as_leaf_ptr()).prefix }
        } else {
            unsafe { &(*self.as_internal_ptr()).prefix }
        }
    }

    #[inline(always)]
    pub fn prefix_mut(&mut self) -> &mut Prefix {
        if self.is_leaf() {
            unsafe { &mut (*self.as_leaf_ptr()).prefix }
        } else {
            unsafe { &mut (*self.as_internal_ptr()).prefix }
        }
    }

    #[inline(always)]
    pub fn val(&self) -> &Option<T> {
        if self.is_leaf() {
            unsafe { &(*self.as_leaf_ptr()).val }
        } else {
            unsafe { &(*self.as_internal_ptr()).val }
        }
    }

    #[inline(always)]
    pub fn val_mut(&mut self) -> &mut Option<T> {
        if self.is_leaf() {
            unsafe { &mut (*self.as_leaf_ptr()).val }
        } else {
            unsafe { &mut (*self.as_internal_ptr()).val }
        }
    }

    /// Returns the number of children the node has
    #[inline(always)]
    pub fn len_children(&self) -> usize {
        if self.is_leaf() {
            return 0;
        }
        unsafe { (*self.as_internal_ptr()).len_children() }
    }

    #[inline(always)]
    pub fn child_exists(&self, pos: ChildPos) -> bool {
        if self.is_leaf() {
            return false;
        }
        let node = unsafe { &*self.as_internal_ptr() };
        node.bitmap[pos.idx] & (1 << pos.bit) != 0
    }

    /// Given a byte, it returns its position among the children
    #[inline(always)]
    pub fn child_pos_from_byte(byte: u8) -> ChildPos {
        let idx = byte >> 6;
        let bit = byte & 63;

        ChildPos {
            idx: idx as usize,
            bit,
        }
    }

    /// Given a child index, it returns its position in the bitmap.
    #[inline]
    pub fn child_pos_from_idx(&self, idx: usize) -> ChildPos {
        // NOTE: This is a hacky/easy wat to do this. It does a memory
        // read to get the first which could cause a chache miss. Most of the
        // time the prefix should be inlined and the value in cache.
        let child = self.get_child_by_idx(idx).expect("Index out of bounds");
        Self::child_pos_from_byte(child.prefix().as_ref()[0])
    }

    /// Returns the index of the child, if exists, given the child's expected
    /// first byte
    #[inline]
    pub fn child_idx_from_pos(&self, pos: ChildPos) -> usize {
        if self.is_leaf() {
            return 0;
        }
        let node = unsafe { &*self.as_internal_ptr() };
        let bitmap = node.bitmap;
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
    #[inline]
    pub fn get_child_by_idx(&self, idx: usize) -> Option<&Node<T>> {
        if self.is_leaf() {
            return None;
        }
        let node = unsafe { &*self.as_internal_ptr() };
        node.children.get(idx, node.len_children())
    }

    /// Returns a mutable reference to the child.
    /// # Panic
    /// Panics if the index is out of bounds.
    #[inline]
    pub fn get_mut_child_by_idx(&mut self, idx: usize) -> Option<&mut Node<T>> {
        if self.is_leaf() {
            return None;
        }
        let node = unsafe { &mut *self.as_internal_ptr() };
        let len = node.len_children();
        node.children.get_mut(idx, len)
    }

    /// Stores the given node as a child to this node.
    #[inline]
    pub fn insert_child(&mut self, pos: ChildPos, child: Node<T>) {
        if self.is_leaf() {
            self.upgrade_to_internal();
        }
        let node = unsafe { &mut *self.as_internal_ptr() };
        let idx = self.child_idx_from_pos(pos);
        let len = node.len_children();
        node.children.insert(idx, child, len);
        node.bitmap[pos.idx] |= 1 << pos.bit;
    }

    /// Removes the children at the given position, and returns it.
    #[inline]
    pub fn remove_child(&mut self, pos: ChildPos) -> Option<Node<T>> {
        if self.is_leaf() {
            return None;
        }
        let node = unsafe { &mut *self.as_internal_ptr() };
        let idx = self.child_idx_from_pos(pos);
        let len = node.len_children();
        let res = node.children.remove(idx, len);
        node.bitmap[pos.idx] &= !(1 << pos.bit);
        res
    }

    /// Upgrades a leaf node to an internal node
    pub fn upgrade_to_internal(&mut self) {
        // some callers might not know if the node is leaf or not
        // so we have to guard this method
        if !self.is_leaf() {
            return;
        }
        let leaf_ptr = self.as_leaf_ptr();
        let leaf = unsafe { ptr::read(leaf_ptr) };

        let internal = InternalNode {
            prefix: leaf.prefix,
            bitmap: [0; 4],
            children: util::BoundedRawVec::new(),
            val: leaf.val,
        };

        // deallocate old node
        let layout = Layout::new::<LeafNode<T>>().align_to(2).unwrap();
        unsafe {
            dealloc(leaf_ptr as *mut u8, layout);

            let layout_int = Layout::new::<InternalNode<T>>().align_to(2).unwrap();
            let ptr = alloc(layout_int) as *mut InternalNode<T>;
            if ptr.is_null() {
                alloc::alloc::handle_alloc_error(layout_int);
            }
            ptr::write(ptr, internal);
            self.ptr = ptr as usize;
        }
    }

    /// Gives its children state to another node.
    /// This method overwrites the children state in the
    /// other node.
    #[inline]
    pub fn transfer_children(&mut self, other: &mut Node<T>) {
        if self.is_leaf() {
            return;
        }
        // here we know that self is not a leaf and other
        // will no longer be a leaf after this transfer
        other.upgrade_to_internal();
        let this = unsafe { &mut *self.as_internal_ptr() };
        let that = unsafe { &mut *other.as_internal_ptr() };
        that.children = mem::take(&mut this.children);
        that.bitmap = this.bitmap;
        this.bitmap = [0u64; 4];
    }

    pub fn iter_children(&self) -> Iter<'_, Node<T>> {
        if self.is_leaf() {
            Iter::new_empty()
        } else {
            let node = unsafe { &*self.as_internal_ptr() };
            node.children.iter(node.len_children())
        }
    }

    pub fn iter_mut_children(&mut self) -> IterMut<'_, Node<T>> {
        if self.is_leaf() {
            IterMut::new_empty()
        } else {
            let node = unsafe { &mut *self.as_internal_ptr() };
            let len = node.len_children();
            node.children.iter_mut(len)
        }
    }
}

impl<T> Drop for Node<T> {
    fn drop(&mut self) {
        if self.is_leaf() {
            let ptr = self.as_leaf_ptr();
            unsafe {
                ptr::drop_in_place(ptr);
                let layout = Layout::new::<LeafNode<T>>().align_to(2).unwrap();
                dealloc(ptr as *mut u8, layout);
            }
        } else {
            let ptr = self.as_internal_ptr();
            unsafe {
                ptr::drop_in_place(ptr);
                let layout = Layout::new::<InternalNode<T>>().align_to(2).unwrap();
                dealloc(ptr as *mut u8, layout);
            }
        }
    }
}
