use crate::node::{ChildPos, Node};
use crate::prefix::Prefix;
use crate::util;
use core::mem;

pub struct TrieMap<T> {
    root: Option<Node<T>>,
}

impl<T> Default for TrieMap<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> TrieMap<T> {
    pub fn new() -> Self {
        Self { root: None }
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    pub fn clear(&mut self) {
        self.root = None;
    }

    pub fn insert<R>(&mut self, key: R, val: T)
    where
        R: AsRef<[u8]>,
    {
        let data_ref = key.as_ref();
        if data_ref.is_empty() {
            return;
        }

        if self.root.is_none() {
            let mut new_node = Node::new(data_ref);
            new_node.set_val(val);
            self.root = Some(new_node);
            return;
        }

        let mut cur_data = data_ref;
        let mut cur_node = self.root.as_mut().unwrap();
        loop {
            let lcp_len = util::lcp(cur_node.prefix().as_ref(), cur_data);
            if lcp_len < cur_node.prefix().len() {
                // create new split node to become the child of the
                // current node that inherits its children.
                let mut split_node = Node::new(&cur_node.prefix().as_ref()[lcp_len..]);
                if let Some(v) = cur_node.take_val() {
                    split_node.set_val(v);
                }
                let new_prefix = Prefix::new(&cur_node.prefix().as_ref()[..lcp_len]);
                *cur_node.prefix_mut() = new_prefix;
                cur_node.transfer_children(&mut split_node);

                // update the parent
                let split_node_byte = split_node.prefix().as_ref()[0];

                // we just split the node so it has no children
                cur_node.insert_child(Node::<T>::child_pos_from_byte(split_node_byte), split_node);
            }

            if lcp_len == cur_data.len() {
                // already exists. overwrite the value
                cur_node.set_val(val);
                return;
            }

            cur_data = &cur_data[lcp_len..];
            let byte = cur_data[0];
            let child_pos = Node::<T>::child_pos_from_byte(byte);
            if cur_node.child_exists(child_pos) {
                cur_node = cur_node.get_mut_child_by_pos(child_pos).unwrap();
            } else {
                // no child exists with this byte prefix
                let mut new_child = Node::new(cur_data);
                new_child.set_val(val);
                cur_node.insert_child(child_pos, new_child);
                return;
            }
        }
    }

    pub fn compress(root: &mut Option<Node<T>>, visited: Vec<(*mut Node<T>, ChildPos)>) {
        // backtrack and compress previous nodes if necessary
        for cur in visited.into_iter().rev() {
            let parent: &mut Node<T>;
            let child_pos: ChildPos;
            // SAFETY: We need to circumvent the borrow checker here.
            // We know that this is the only mutable reference to this
            // trie.
            unsafe {
                parent = &mut *cur.0;
            }
            child_pos = cur.1;
            // SAFETY: child must exist since this position was traversed.
            let visited_child = parent.get_child_by_pos(child_pos).unwrap();
            if visited_child.len_children() == 0 && !visited_child.has_val() {
                parent.remove_child(child_pos);
            }
            // a node can only be compressed if it has a single child
            // and no value.
            if parent.len_children() == 1 && !parent.has_val() {
                // SAFETY: we checked that the child exists above.
                let mut child = parent.remove_child(parent.child_pos_from_idx(0)).unwrap();
                *parent.prefix_mut() =
                    Prefix::from_slices(&[parent.prefix().as_ref(), child.prefix().as_ref()]);
                child.transfer_children(parent);
                if let Some(v) = child.take_val() {
                    parent.set_val(v);
                }
            } else {
                // if the parent has multiple children or has a value then
                // no further compression is possible.
                break;
            }
        }
        // root node has to be compressed manually since it has no parent.
        if let Some(_root) = root.as_mut()
            && !_root.has_val()
        {
            let count = _root.len_children();
            if count == 0 {
                *root = None;
            } else if count == 1 {
                // make the child the new root
                let mut child = _root.remove_child(_root.child_pos_from_idx(0)).unwrap();
                *child.prefix_mut() =
                    Prefix::from_slices(&[_root.prefix().as_ref(), child.prefix().as_ref()]);
                *root = Some(child);
            }
        }
    }

    pub fn remove<R>(&mut self, key: R) -> Option<T>
    where
        R: AsRef<[u8]>,
    {
        let key_ref = key.as_ref();
        if key_ref.is_empty() || self.root.is_none() {
            return None;
        }

        let mut cur_node = self.root.as_mut().unwrap();
        let mut cur_data = key_ref;
        let mut visited: Vec<(*mut Node<T>, ChildPos)> = Vec::with_capacity(8);
        let res: Option<T> = loop {
            let lcp_len = util::lcp(cur_node.prefix().as_ref(), cur_data);
            if lcp_len < cur_node.prefix().len() {
                return None;
            }
            if lcp_len == cur_data.len() {
                break cur_node.take_val();
            }
            cur_data = &cur_data[lcp_len..];
            let next_pos = Node::<T>::child_pos_from_byte(cur_data[0]);
            if cur_node.child_exists(next_pos) {
                visited.push((cur_node, next_pos));
                cur_node = cur_node.get_mut_child_by_pos(next_pos).unwrap();
            } else {
                return None;
            }
        };

        // no value existed for this prefix which means it is only a branching
        // node and contains no value. There's no need to compress here
        if res.is_none() {
            return res;
        }
        Self::compress(&mut self.root, visited);
        res
    }

    // Returns the node a prefix ends at and the offset inside that node's
    // prefix where the searched prefix ends.
    fn find_prefix_node<R>(&self, key: R) -> Option<(&Node<T>, usize)>
    where
        R: AsRef<[u8]>,
    {
        let key_ref = key.as_ref();
        let mut cur_node = self.root.as_ref()?;
        if key_ref.is_empty() {
            return Some((cur_node, 0));
        }

        let mut cur_data = key_ref;
        loop {
            let lcp_len = util::lcp(cur_node.prefix().as_ref(), cur_data);
            if lcp_len == cur_data.len() {
                // prefix ends at this node. whole or in part
                return Some((cur_node, lcp_len));
            }
            if lcp_len < cur_node.prefix().len() {
                // mismatch before the whole prefix is consumed
                return None;
            }

            // go down the tree
            cur_data = &cur_data[lcp_len..];
            let byte = cur_data[0];
            let child_pos = Node::<T>::child_pos_from_byte(byte);
            if cur_node.child_exists(child_pos) {
                cur_node = cur_node.get_child_by_pos(child_pos).unwrap();
            } else {
                return None;
            }
        }
    }

    pub fn remove_prefix<R>(&mut self, prefix: R)
    where
        R: AsRef<[u8]>,
    {
        let key_ref = prefix.as_ref();
        if key_ref.is_empty() {
            self.root = None;
            return;
        }
        if self.root.is_none() {
            return;
        }

        let mut cur_node = self.root.as_mut().unwrap();
        let mut rem_pref = key_ref;
        let mut visited: Vec<(*mut Node<T>, ChildPos)> = Vec::with_capacity(8);
        let res: Option<(&mut Node<T>, usize)> = loop {
            let lcp_len = util::lcp(cur_node.prefix().as_ref(), rem_pref);
            if lcp_len == rem_pref.len() {
                break Some((cur_node, lcp_len));
            }
            if lcp_len < cur_node.prefix().len() {
                break None;
            }
            rem_pref = &rem_pref[lcp_len..];
            let next_pos = Node::<T>::child_pos_from_byte(rem_pref[0]);
            if cur_node.child_exists(next_pos) {
                visited.push((cur_node, next_pos));
                cur_node = cur_node.get_mut_child_by_pos(next_pos).unwrap();
            } else {
                break None;
            }
        };

        let Some((term_node, match_len)) = res else {
            // no value existed for this prefix which means it is only a branching
            // node and contains no value. There's no need to compress here
            return;
        };

        // replace with a dummy node that will be removed during
        // compression
        let new = Node::new(&term_node.prefix().as_ref()[0..match_len]);
        let old = mem::replace(term_node, new);
        drop(old);

        Self::compress(&mut self.root, visited);
    }

    /// Runs the given operation once for every value with the given prefix.
    pub fn for_each_prefix<'trie, R, F>(&'trie self, prefix: R, mut f: F)
    where
        R: AsRef<[u8]>,
        F: FnMut(SuffixRef<'_, 'trie>, &'trie T),
    {
        let Some((node, offset)) = self.find_prefix_node(prefix) else {
            return;
        };

        let mut chunks = Vec::new();
        let mut current_len = 0;
        let first = &node.prefix().as_ref()[offset..];
        if !first.is_empty() {
            chunks.push(first);
            current_len += first.len();
        }

        Self::for_each_prefix_from_node(node, &mut chunks, current_len, &mut f);
    }

    /// Recursive helper for a DFS traversal of the trie.
    fn for_each_prefix_from_node<'trie, F>(
        node: &'trie Node<T>,
        chunks: &mut Vec<&'trie [u8]>,
        // keeping track of current_len during iteration helps the Suffix
        // returned to the user to be able to cache the length and not
        // compute it.
        current_len: usize,
        f: &mut F,
    ) where
        F: FnMut(SuffixRef<'_, 'trie>, &'trie T),
    {
        // TODO: provide iterative fallback for exceedingly deep traversals
        // to avoid stack overflow bugs.
        if let Some(value) = node.val() {
            f(
                SuffixRef {
                    chunks: chunks.as_slice(),
                    len: current_len,
                },
                value,
            );
        }

        for child in node.iter_children() {
            let prefix = child.prefix().as_ref();
            chunks.push(prefix);
            Self::for_each_prefix_from_node(child, chunks, current_len + prefix.len(), f);
            chunks.pop();
        }
    }

    pub fn get<R>(&self, key: R) -> Option<&T>
    where
        R: AsRef<[u8]>,
    {
        let key_ref = key.as_ref();
        if key_ref.is_empty() || self.root.is_none() {
            return None;
        }

        let mut cur_node = self.root.as_ref().unwrap();
        let mut cur_data = key_ref;
        loop {
            let lcp_len = util::lcp(cur_node.prefix().as_ref(), cur_data);
            if lcp_len < cur_node.prefix().len() {
                return None;
            }
            if lcp_len == cur_data.len() {
                return cur_node.val();
            }
            // keep traversing down.
            cur_data = &cur_data[lcp_len..];
            let byte = cur_data[0];
            let child_pos = Node::<T>::child_pos_from_byte(byte);
            if cur_node.child_exists(child_pos) {
                cur_node = cur_node.get_child_by_pos(child_pos).unwrap();
            } else {
                return None;
            }
        }
    }

    pub fn get_mut<R>(&mut self, key: R) -> Option<&mut T>
    where
        R: AsRef<[u8]>,
    {
        let key_ref = key.as_ref();
        if key_ref.is_empty() || self.root.is_none() {
            return None;
        }

        let mut cur_node = self.root.as_mut().unwrap();
        let mut cur_data = key_ref;
        loop {
            let lcp_len = util::lcp(cur_node.prefix().as_ref(), cur_data);
            if lcp_len < cur_node.prefix().len() {
                return None;
            }
            if lcp_len == cur_data.len() {
                return cur_node.val_mut();
            }
            // keep traversing down.
            cur_data = &cur_data[lcp_len..];
            let byte = cur_data[0];
            let child_pos = Node::<T>::child_pos_from_byte(byte);
            if cur_node.child_exists(child_pos) {
                cur_node = cur_node.get_mut_child_by_pos(child_pos).unwrap();
            } else {
                return None;
            }
        }
    }

    pub fn contains<R>(&self, key: R) -> bool
    where
        R: AsRef<[u8]>,
    {
        self.get(key).is_some()
    }
}

/// A non-owning list of prefixes that make up a suffix.
///
/// # Iterator
/// Implements an iterator over the disjoint byte arrays
pub struct SuffixRef<'buf, 'trie> {
    chunks: &'buf [&'trie [u8]],
    len: usize,
}

impl<'buf, 'trie> SuffixRef<'buf, 'trie> {
    /// Returns an iterator over the chunks that make up the suffix.
    pub fn chunks(&self) -> impl Iterator<Item = &'trie [u8]> {
        self.chunks.iter().copied()
    }

    /// Returns the total number across all chunks.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.len);
        for chunk in self.chunks {
            out.extend_from_slice(chunk);
        }
        out
    }
}
