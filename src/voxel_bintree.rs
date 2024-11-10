#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Node {
    Leaf(usize),
    Branch([usize; 2]),
}

use Node::*;

#[derive(Debug, Clone)]
pub struct VoxelBintree {
    nodes: Vec<Option<Node>>,
    free_nodes: Vec<usize>,
    log_size: u32,
}

impl Default for Node {
    fn default() -> Self {
        Self::Leaf(0)
    }
}

impl Node {
    fn is_leaf(&self) -> bool {
        match self {
            Leaf(_) => true,
            Branch(_) => false,
        }
    }

    fn is_branch(&self) -> bool {
        match self {
            Leaf(_) => true,
            Branch(_) => false,
        }
    }

    fn unwrap_value(&self) -> usize {
        match self {
            Leaf(x) => *x,
            Branch(_) => panic!(),
        }
    }

    fn unwrap_branch(&self) -> [usize; 2] {
        match self {
            Leaf(_) => panic!(),
            Branch(x) => *x,
        }
    }
}

impl VoxelBintree {
    pub fn new() -> Self {
        Self {
            nodes: vec![Some(Leaf(0))],
            free_nodes: Vec::new(),
            log_size: 0,
        }
    }

    pub fn get(&self, idx: usize) -> usize {
        if idx >= 1 << self.log_size {
            return 0;
        }
        let (node_idx, _, _) = self.descend(idx);
        self.nodes[node_idx].unwrap().unwrap_value()
    }

    pub fn set(&mut self, idx: usize, x: usize) {
        while idx >= 1 << self.log_size {
            let new_left_idx = self.alloc_node();
            let new_right_idx = self.alloc_node();
            self.nodes[new_left_idx] = Some(Leaf(0));
            self.nodes[new_right_idx] = Some(Leaf(0));
            self.nodes.swap(0, new_left_idx);
            self.nodes[0] = Some(Branch([new_left_idx, new_right_idx]));
            self.log_size += 1;
        }
        let (mut node_idx, mut node_log_size, mut stack) = self.descend(idx);
        let node = &self.nodes[node_idx];
        let old_value = node.unwrap().unwrap_value();
        if old_value == x {
            return;
        }
        while node_log_size > 0 {
            let new_left_idx = self.alloc_node();
            let new_right_idx = self.alloc_node();
            self.nodes[new_left_idx] = Some(Leaf(old_value));
            self.nodes[new_right_idx] = Some(Leaf(old_value));
            let children = [new_left_idx, new_right_idx];
            self.nodes[node_idx] = Some(Branch(children));
            stack.push(node_idx);
            node_log_size -= 1;
            node_idx = children[(idx >> node_log_size) & 1];
        }
        assert!(self.nodes[node_idx].unwrap().is_leaf());
        self.nodes[node_idx] = Some(Leaf(x));

        while let Some(node_idx) = stack.pop() {
            if let Some((y, children)) = self.can_merge(self.nodes[node_idx].unwrap()) {
                assert_eq!(x, y);
                for child_idx in children {
                    self.free_node(child_idx);
                }
                self.nodes[node_idx] = Some(Leaf(x));
            }
        }
    }

    fn descend(&self, idx: usize) -> (usize, u32, Vec<usize>) {
        let mut node_idx = 0;
        let mut node_log_size = self.log_size;
        let mut path = Vec::new();
        loop {
            match self.nodes[node_idx].unwrap() {
                Leaf(_) => return (node_idx, node_log_size, path),
                Branch(children) => {
                    assert!(node_log_size >= 1);
                    path.push(node_idx);
                    node_log_size -= 1;
                    node_idx = children[(idx >> node_log_size) & 1];
                }
            }
        }
    }

    fn alloc_node(&mut self) -> usize {
        let i = if let Some(i) = self.free_nodes.pop() {
            i
        } else {
            self.nodes.push(None);
            self.nodes.len() - 1
        };
        assert!(self.nodes[i].is_none());
        i
    }

    fn free_node(&mut self, i: usize) {
        assert!(self.nodes[i].is_some());
        self.nodes[i] = None;
        self.free_nodes.push(i);
        while self.free_nodes.last().cloned() == Some(self.nodes.len() - 1) {
            assert_eq!(self.nodes.last().cloned(), Some(None));
            self.free_nodes.pop();
            self.nodes.pop();
        }
    }

    fn can_merge(&self, node: Node) -> Option<(usize, [usize; 2])> {
        match node {
            Leaf(_) => None,
            Branch(children) => {
                let x = match self.nodes[children[0]].unwrap() {
                    Leaf(x) => x,
                    Branch(_) => return None,
                };
                let y = match self.nodes[children[1]].unwrap() {
                    Leaf(y) => y,
                    Branch(_) => return None,
                };
                if x == y {
                    Some((x, children))
                } else {
                    None
                }
            }
        }
    }

    pub fn shrinked(&self) -> Self {
        let mut new_index = vec![0; self.nodes.len()];
        let nodes = vec![todo!()];
        Self {
            nodes,
            free_nodes: Vec::new(),
            log_size: self.log_size,
        }
    }

    pub fn shrink(&mut self) {
        let new = self.shrinked();
        *self = new;
    }
}

pub fn expand_bits_2(mut x: u64) -> u64 {
    x = x & 0x0000_0000_FFFF_FFFF;
    x = (x | (x << 16)) & 0x0000_FFFF_0000_FFFF;
    x = (x | (x << 8)) & 0x00FF_00FF_00FF_00FF;
    x = (x | (x << 4)) & 0x0F0F_0F0F_0F0F_0F0F;
    x = (x | (x << 2)) & 0x3333_3333_3333_3333;
    x = (x | (x << 1)) & 0x5555_5555_5555_5555;
    x
}

pub fn shrink_bits_2(mut x: u64) -> u64 {
    x = x & 0x5555_5555_5555_5555;
    x = (x | (x >> 1)) & 0x3333_3333_3333_3333;
    x = (x | (x >> 2)) & 0x0F0F_0F0F_0F0F_0F0F;
    x = (x | (x >> 4)) & 0x00FF_00FF_00FF_00FF;
    x = (x | (x >> 8)) & 0x0000_FFFF_0000_FFFF;
    x = (x | (x >> 16)) & 0x0000_0000_FFFF_FFFF;
    x
}
