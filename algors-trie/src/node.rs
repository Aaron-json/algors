use crate::prefix::Prefix;
use crate::util::{self, Iter, IterMut};
use alloc::alloc::{Layout, alloc, dealloc};
use core::marker::PhantomData;
use core::mem::{self, MaybeUninit};
use core::ptr;

const NODE_ALLOC_ALIGN: usize = 4;

/// Common fields at the start of every node to allow branchless access,
/// which avoids a performance regression that comes up when every field
/// access (especially the prefix) requires an if check (branch).
///
/// We use the C memory layout to prevent the compiler from reordering
#[repr(C)]
struct Header<T> {
    prefix: Prefix,
    val: MaybeUninit<T>,
}

#[repr(C)]
pub struct LeafNode<T> {
    header: Header<T>,
}

#[repr(C)]
pub struct InternalNode<T> {
    header: Header<T>,
    bitmap: [u64; 4],
    children: util::BoundedRawVec<Node<T>>,
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

/// Node represents a single Radix Tree node.
///
/// The pointer is tagged with 2 bits:
/// Bit 0: Node Type (internal or leaf)
/// Bit 1: Has Value
pub struct Node<T> {
    ptr: usize,
    _marker: PhantomData<T>,
}

const LEAF_TAG: usize = 0x1;
const VAL_TAG: usize = 0x2;
const TAG_MASK: usize = 0x3;

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
        let has_val = val.is_some();
        let node = LeafNode {
            header: Header {
                prefix: Prefix::new(prefix),
                val: if let Some(v) = val {
                    MaybeUninit::new(v)
                } else {
                    MaybeUninit::uninit()
                },
            },
        };
        // We need 4-byte alignment to use 2 bits for tagging.
        let layout = Layout::new::<LeafNode<T>>()
            .align_to(NODE_ALLOC_ALIGN)
            .unwrap();
        unsafe {
            let ptr = alloc(layout) as *mut LeafNode<T>;
            if ptr.is_null() {
                alloc::alloc::handle_alloc_error(layout);
            }
            ptr::write(ptr, node);
            let mut ptr_val = (ptr as usize) | LEAF_TAG;
            if has_val {
                ptr_val |= VAL_TAG;
            }
            Self {
                ptr: ptr_val,
                _marker: PhantomData,
            }
        }
    }

    pub fn new_internal(prefix: &[u8], val: Option<T>) -> Self {
        let has_val = val.is_some();
        let node = InternalNode {
            header: Header {
                prefix: Prefix::new(prefix),
                val: if let Some(v) = val {
                    MaybeUninit::new(v)
                } else {
                    MaybeUninit::uninit()
                },
            },
            bitmap: [0; 4],
            children: util::BoundedRawVec::new(),
        };
        let layout = Layout::new::<InternalNode<T>>()
            .align_to(NODE_ALLOC_ALIGN)
            .unwrap();
        unsafe {
            let ptr = alloc(layout) as *mut InternalNode<T>;
            if ptr.is_null() {
                alloc::alloc::handle_alloc_error(layout);
            }
            ptr::write(ptr, node);
            let mut ptr_val = ptr as usize;
            if has_val {
                ptr_val |= VAL_TAG;
            }
            Self {
                ptr: ptr_val,
                _marker: PhantomData,
            }
        }
    }

    #[inline(always)]
    pub fn is_leaf(&self) -> bool {
        (self.ptr & LEAF_TAG) != 0
    }

    #[inline(always)]
    pub fn has_val(&self) -> bool {
        (self.ptr & VAL_TAG) != 0
    }

    #[inline(always)]
    fn as_header(&self) -> *mut Header<T> {
        (self.ptr & !TAG_MASK) as *mut Header<T>
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
        unsafe { &(*self.as_header()).prefix }
    }

    #[inline(always)]
    pub fn prefix_mut(&mut self) -> &mut Prefix {
        unsafe { &mut (*self.as_header()).prefix }
    }

    /// Access the value branchlessly if it exists.
    #[inline(always)]
    pub fn val(&self) -> Option<&T> {
        if !self.has_val() {
            return None;
        }
        unsafe { Some((*self.as_header()).val.assume_init_ref()) }
    }

    // Returns a mutable referene to the value
    #[inline(always)]
    pub fn val_mut(&mut self) -> Option<&mut T> {
        if !self.has_val() {
            return None;
        }
        unsafe { Some((*self.as_header()).val.assume_init_mut()) }
    }

    // Returns an owned value of T.
    #[inline]
    pub fn take_val(&mut self) -> Option<T> {
        if !self.has_val() {
            return None;
        }
        let val = unsafe { Some((*self.as_header()).val.as_ptr().read()) };
        self.ptr &= !VAL_TAG;

        val
    }

    // Sets the value for this node
    #[inline]
    pub fn set_val(&mut self, val: T) {
        if self.has_val() {
            // drop old value to avoid leaking it
            unsafe { (*self.as_header()).val.assume_init_drop() }
        }
        unsafe {
            (*self.as_header()).val.as_mut_ptr().write(val);
        }
        self.ptr |= VAL_TAG;
    }

    /// Returns the number of children the node has
    #[inline]
    pub fn len_children(&self) -> usize {
        if self.is_leaf() {
            0
        } else {
            unsafe { (*self.as_internal_ptr()).len_children() }
        }
    }

    #[inline]
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
        ChildPos {
            idx: (byte >> 6) as usize,
            bit: byte & 63,
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
    #[inline(always)]
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
    #[inline(always)]
    pub fn get_child_by_pos(&self, pos: ChildPos) -> Option<&Node<T>> {
        let idx = self.child_idx_from_pos(pos);
        self.get_child_by_idx(idx)
    }

    /// Returns a mutable reference to the child.
    /// The given posision must be for an existing child, otherwise the
    /// wrong child might be returned.
    #[inline(always)]
    pub fn get_mut_child_by_pos(&mut self, pos: ChildPos) -> Option<&mut Node<T>> {
        let idx = self.child_idx_from_pos(pos);
        self.get_mut_child_by_idx(idx)
    }

    /// Returns the child at the given index.
    #[inline(always)]
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
        let has_val = self.has_val();
        let leaf_ptr = self.as_leaf_ptr();
        let leaf = unsafe { ptr::read(leaf_ptr) };

        let internal = InternalNode {
            header: leaf.header,
            bitmap: [0; 4],
            children: util::BoundedRawVec::new(),
        };

        // deallocate old node
        let layout = Layout::new::<LeafNode<T>>()
            .align_to(NODE_ALLOC_ALIGN)
            .unwrap();
        unsafe {
            dealloc(leaf_ptr as *mut u8, layout);

            let layout_int = Layout::new::<InternalNode<T>>()
                .align_to(NODE_ALLOC_ALIGN)
                .unwrap();
            let ptr = alloc(layout_int) as *mut InternalNode<T>;
            if ptr.is_null() {
                alloc::alloc::handle_alloc_error(layout_int);
            }
            ptr::write(ptr, internal);
            let mut ptr_val = ptr as usize;
            if has_val {
                ptr_val |= VAL_TAG;
            }
            self.ptr = ptr_val;
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
        let has_val = self.has_val();
        let header_ptr = self.as_header();
        unsafe {
            if has_val {
                (*header_ptr).val.as_mut_ptr().drop_in_place();
            }
            // Drop prefix (works for both since it's in the header)
            ptr::drop_in_place(&mut (*header_ptr).prefix);

            if self.is_leaf() {
                let layout = Layout::new::<LeafNode<T>>()
                    .align_to(NODE_ALLOC_ALIGN)
                    .unwrap();
                dealloc(header_ptr as *mut u8, layout);
            } else {
                let internal = self.as_internal_ptr();
                // children requires users to deallocate manually since
                // it does not maintain length
                let len = (*internal).len_children();
                (*internal).children.deallocate(len);

                // drop the type since it might have other data
                // that needs to drop or an implementations of Drop
                ptr::drop_in_place(&mut (*internal).children);
                let layout = Layout::new::<InternalNode<T>>()
                    .align_to(NODE_ALLOC_ALIGN)
                    .unwrap();
                dealloc(header_ptr as *mut u8, layout);
            }
        }
    }
}
