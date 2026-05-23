use crate::node::Node;
use crate::prefix::Prefix;
use crate::util;
use core::mem;

struct Trie {
    root: Option<Node>,
}

impl Trie {
    pub fn new() -> Self {
        Self { root: None }
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

                cur_node.prefix = Prefix::new(&cur_node_prefix[..lcp_len]);
                cur_node.transfer_children(&mut split_node);

                // update the parent
                let split_node_byte = split_node.prefix.as_ref()[0];

                // we just split the node so it has no children
                cur_node.insert_child(Node::child_pos(split_node_byte), split_node);
            }

            if lcp_len == cur_data.len() {
                // already exists
                return;
            }

            cur_data = &cur_data[lcp_len..];
            let byte = cur_data[0];
            let child_pos = Node::child_pos(byte);
            if cur_node.child_exists(child_pos) {
                cur_node = cur_node.get_mut_child(child_pos).unwrap();
            } else {
                // no child exists with this byte prefix
                let new_child = Node::new(cur_data);
                cur_node.insert_child(child_pos, new_child);
                return;
            }
        }
    }

    pub fn contains<R>(&self, data: R) -> bool
    where
        R: AsRef<[u8]>,
    {
        let data_ref = data.as_ref();
        if data_ref.is_empty() {
            return false;
        }

        if let None = self.root {
            return false;
        }

        let cur_node = self.root.as_ref().unwrap();
        let cur_data = data_ref;
        loop {
            let lcp_len = util::lcp(cur_node.prefix.as_ref(), cur_data);
            if lcp_len < cur_node.prefix.len() {
                return false;
            }
        }
        unreachable!()
    }
}
