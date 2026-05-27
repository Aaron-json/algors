use crate::node::Node;
use crate::prefix::Prefix;
use crate::util;

struct TrieMap<T> {
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

        if let None = self.root {
            self.root = Some(Node::new(data_ref));
            return;
        }

        let mut cur_data = data_ref;
        let mut cur_node = self.root.as_mut().unwrap();
        loop {
            let lcp_len = util::lcp(cur_node.prefix.as_ref(), cur_data);
            if lcp_len < cur_node.prefix.len() {
                // create new split node to become the child of the
                // current node that inherits its children.
                let cur_node_prefix = cur_node.prefix.as_ref();
                let mut split_node = Node::new(&cur_node_prefix[lcp_len..]);
                split_node.val = cur_node.val.take();
                cur_node.prefix = Prefix::new(&cur_node_prefix[..lcp_len]);
                cur_node.transfer_children(&mut split_node);

                // update the parent
                let split_node_byte = split_node.prefix.as_ref()[0];

                // we just split the node so it has no children
                cur_node.insert_child(Node::<T>::child_pos(split_node_byte), split_node);
            }

            if lcp_len == cur_data.len() {
                // already exists. overwrite the value
                cur_node.val = Some(val);
                return;
            }

            cur_data = &cur_data[lcp_len..];
            let byte = cur_data[0];
            let child_pos = Node::<T>::child_pos(byte);
            if cur_node.child_exists(child_pos) {
                cur_node = cur_node.get_mut_child(child_pos).unwrap();
            } else {
                // no child exists with this byte prefix
                let mut new_child = Node::new(cur_data);
                new_child.val = Some(val);
                cur_node.insert_child(child_pos, new_child);
                return;
            }
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
            let lcp_len = util::lcp(cur_node.prefix.as_ref(), cur_data);
            if lcp_len < cur_node.prefix.len() {
                return None;
            }
            if lcp_len == cur_data.len() {
                return cur_node.val.as_ref();
            }
            // keep traversing down.
            cur_data = &cur_data[lcp_len..];
            let byte = cur_data[0];
            let child_pos = Node::<T>::child_pos(byte);
            if cur_node.child_exists(child_pos) {
                cur_node = cur_node.get_child(child_pos).unwrap();
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
            let lcp_len = util::lcp(cur_node.prefix.as_ref(), cur_data);
            if lcp_len < cur_node.prefix.len() {
                return None;
            }
            if lcp_len == cur_data.len() {
                return cur_node.val.as_mut();
            }
            // keep traversing down.
            cur_data = &cur_data[lcp_len..];
            let byte = cur_data[0];
            let child_pos = Node::<T>::child_pos(byte);
            if cur_node.child_exists(child_pos) {
                cur_node = cur_node.get_mut_child(child_pos).unwrap();
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
