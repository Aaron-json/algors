use crate::node::{ChildPos, Node};
use crate::prefix::Prefix;
use crate::util;

pub struct TrieMap<T> {
    root: Option<Node<T>>,
}

impl<T> TrieMap<T> {
    pub fn new() -> Self {
        Self { root: None }
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
            *new_node.val_mut() = Some(val);
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
                *split_node.val_mut() = cur_node.val_mut().take();
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
                *cur_node.val_mut() = Some(val);
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
                *new_child.val_mut() = Some(val);
                cur_node.insert_child(child_pos, new_child);
                return;
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
        let mut visited: Vec<(*mut Node<T>, ChildPos)> = vec![];
        let res: Option<T> = loop {
            let lcp_len = util::lcp(cur_node.prefix().as_ref(), cur_data);
            if lcp_len < cur_node.prefix().len() {
                return None;
            }
            if lcp_len == cur_data.len() {
                break cur_node.val_mut().take();
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

        if res.is_none() {
            return res;
        }

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
            if visited_child.len_children() == 0 && visited_child.val().is_none() {
                parent.remove_child(child_pos);
            }
            // a node can only be compressed if it has a single child
            // and no value.
            if parent.len_children() == 1 && parent.val().is_none() {
                // SAFETY: we checked that the child exists above.
                let mut child = parent.remove_child(parent.child_pos_from_idx(0)).unwrap();
                *parent.prefix_mut() =
                    Prefix::from_slices(&[parent.prefix().as_ref(), child.prefix().as_ref()]);
                child.transfer_children(parent);
                *parent.val_mut() = child.val_mut().take();
            }
        }
        // root node has to be compressed manually since it has no parent.
        if let Some(root) = self.root.as_mut()
            && root.val().is_none()
        {
            let count = root.len_children();
            if count == 0 {
                self.root = None;
            } else if count == 1 {
                // make the child the new root
                let mut child = root.remove_child(root.child_pos_from_idx(0)).unwrap();
                *child.prefix_mut() = Prefix::from_slices(&[root.prefix().as_ref(), child.prefix().as_ref()]);
                self.root = Some(child);
            }
        }

        res
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
                return cur_node.val().as_ref();
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
                return cur_node.val_mut().as_mut();
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
