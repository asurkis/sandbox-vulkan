use std::collections::{HashMap, VecDeque};

use crate::math::{vec3, Vector};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Node {
    voxel: usize,
    children: [usize; 8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Octree {
    nodes: Vec<Node>,
    free_nodes: Vec<usize>,
    root: usize,
    log_extent: usize,
}

impl Default for Node {
    fn default() -> Self {
        Self::leaf(!0)
    }
}

impl Node {
    fn leaf(voxel: usize) -> Self {
        Self {
            voxel,
            children: [!0; 8],
        }
    }

    fn is_leaf(&self) -> bool {
        self.children[0] == !0
    }

    #[allow(unused)]
    fn is_branch(&self) -> bool {
        !self.is_leaf()
    }
}

impl Octree {
    pub fn new() -> Self {
        Self {
            nodes: vec![Node::default()],
            free_nodes: Vec::new(),
            root: 0,
            log_extent: 0,
        }
    }

    pub fn from_voxels(voxels: &[usize]) -> Self {
        let mut log_extent = 0;
        while 1 << (3 * log_extent) < voxels.len() {
            log_extent += 1;
        }
        if 1 << (3 * log_extent) > voxels.len() {
            panic!("Array size is not a precise 8^n");
        }
        let mut n_nodes = 0;
        for i in 0..=log_extent {
            n_nodes += 1 << (3 * i);
        }
        let mut self_ = Self {
            nodes: vec![Node::default(); n_nodes],
            free_nodes: Vec::new(),
            root: 0,
            log_extent,
        };

        let leaf_off = n_nodes - (1 << (3 * log_extent));
        for (i_voxel, &voxel) in voxels.iter().enumerate() {
            let x = ((1 << log_extent) - 1) & i_voxel;
            let y = ((1 << log_extent) - 1) & (i_voxel >> log_extent);
            let z = ((1 << log_extent) - 1) & (i_voxel >> (2 * log_extent));
            let mut i_leaf = 0;
            for i in 0..log_extent {
                i_leaf |= (((x >> i) & 1) | (((y >> i) & 1) << 1) | (((z >> i) & 1) << 2)) << (3 * i);
            }
            let i_node = i_leaf + leaf_off;
            self_.nodes[i_node].voxel = voxel;
        }
        for i_node in (0..leaf_off).rev() {
            for i_child in 0..8 {
                self_.nodes[i_node].children[i_child] = 8 * i_node + 1 + i_child;
            }
            self_.merge_branch(i_node);
        }
        self_.shrinked()
    }

    #[allow(unused)]
    pub fn get(&self, offset: [usize; 3]) -> usize {
        self.sample(offset, 0)
    }

    #[allow(unused)]
    pub fn sample(&self, [x, y, z]: [usize; 3], log_extent: usize) -> usize {
        if x.max(y).max(z) >= 1 << self.log_extent {
            return !0;
        }
        let mut i_node = self.root;
        let mut node_log_extent = self.log_extent;
        loop {
            let node = &self.nodes[i_node];
            if node.is_leaf() || node_log_extent <= log_extent {
                return node.voxel;
            }
            node_log_extent -= 1;
            let dix = ((x >> node_log_extent) & 1);
            let diy = ((y >> node_log_extent) & 1) << 1;
            let diz = ((z >> node_log_extent) & 1) << 2;
            i_node = node.children[dix | diy | diz];
        }
    }

    pub fn set(&mut self, [x, y, z]: [usize; 3], [ex, ey, ez]: [usize; 3], voxel: usize) {
        let needed_extent = (x + ex).max(y + ey).max(z + ez);
        while needed_extent > 1 << self.log_extent {
            let new_root = self.new_leaf(!0);
            self.nodes[new_root].children[0] = self.root;
            for i in 1..8 {
                let new_leaf = self.new_leaf(!0);
                self.nodes[new_root].children[i] = new_leaf;
            }
            self.root = new_root;
            self.log_extent += 1;
        }
        self.set_descend([x, y, z], [ex, ey, ez], voxel, self.root, self.log_extent);
    }

    fn set_descend(
        &mut self,
        offset: [usize; 3],
        extent: [usize; 3],
        voxel: usize,
        i_node: usize,
        node_log_extent: usize,
    ) {
        for ei in extent {
            if ei == 0 {
                return;
            }
        }
        if offset == [0; 3] && extent == [1 << node_log_extent; 3] {
            self.drop_children(i_node);
            self.nodes[i_node].voxel = voxel;
            return;
        }
        assert_ne!(node_log_extent, 0);
        if self.nodes[i_node].is_leaf() {
            self.split_leaf(i_node);
        }
        let half_extent = 1 << (node_log_extent - 1);
        for i_child in 0..8 {
            let mut next_offset = [0; 3];
            let mut next_extent = [0; 3];
            for i in 0..3 {
                if i_child & (1 << i) != 0 {
                    next_offset[i] = half_extent.max(offset[i]) - half_extent;
                } else {
                    next_offset[i] = offset[i];
                }
                next_extent[i] =
                    (offset[i] + extent[i]).min(half_extent).max(next_offset[i]) - next_offset[i];
            }
            self.set_descend(
                next_offset,
                next_extent,
                voxel,
                self.nodes[i_node].children[i_child],
                node_log_extent - 1,
            );
        }
        self.merge_branch(i_node);
    }

    pub fn shrinked(&self) -> Self {
        let mut reindex = vec![!0; self.nodes.len()];
        let mut next_index = 0;
        let mut queue = VecDeque::new();
        queue.push_back(self.root);
        while let Some(i_node) = queue.pop_front() {
            reindex[i_node] = next_index;
            next_index += 1;
            for j_node in self.nodes[i_node].children {
                if j_node != !0 {
                    queue.push_back(j_node);
                }
            }
        }

        let mut nodes = vec![Node::default(); next_index];
        for i_node in 0..self.nodes.len() {
            let mut node = self.nodes[i_node];
            let ri_node = reindex[i_node];
            if ri_node == !0 {
                continue;
            }
            for j_node in &mut node.children {
                if *j_node != !0 {
                    *j_node = reindex[*j_node];
                }
            }
            nodes[ri_node] = node;
        }
        Self {
            nodes,
            free_nodes: Vec::new(),
            root: 0,
            log_extent: self.log_extent,
        }
    }

    pub fn shrink(&mut self) {
        *self = self.shrinked();
    }

    pub fn gpu_data(&self) -> Vec<u32> {
        assert!(self.free_nodes.is_empty());
        assert!(self.root == 0);
        let mut out = vec![0; 4 + 12 * self.nodes.len()];
        out[0] = self.log_extent as _;
        for i_node in 0..self.nodes.len() {
            let node = self.nodes[i_node];
            out[4 + 12 * i_node] = node.voxel as _;
            for (i_child, &j_node) in node.children.iter().enumerate() {
                out[8 + 12 * i_node + i_child] = j_node as _;
            }
        }
        out
    }

    fn new_leaf(&mut self, voxel: usize) -> usize {
        let i = match self.free_nodes.pop() {
            Some(i) => i,
            None => {
                self.nodes.push(Node::default());
                self.nodes.len() - 1
            }
        };
        self.nodes[i] = Node::leaf(voxel);
        i
    }

    fn drop_node(&mut self, i: usize) {
        if i == !0 {
            return;
        }
        self.drop_children(i);
        self.nodes[i] = Node::default();
        self.free_nodes.push(i);
    }

    fn drop_children(&mut self, i_node: usize) {
        for j_node in self.nodes[i_node].children {
            self.drop_node(j_node);
        }
        self.nodes[i_node].children = [!0; 8];
    }

    fn split_leaf(&mut self, i_node: usize) {
        assert!(self.nodes[i_node].is_leaf());
        let voxel = self.nodes[i_node].voxel;
        for i in 0..8 {
            let i_new_leaf = self.new_leaf(voxel);
            self.nodes[i_node].children[i] = i_new_leaf;
        }
    }

    fn merge_branch(&mut self, i_node: usize) {
        let v = self.nodes[i_node];
        assert!(v.is_branch());
        let voxel = self.nodes[v.children[0]].voxel;
        self.nodes[i_node].voxel = voxel;
        for j_node in v.children {
            let u = &self.nodes[j_node];
            if u.is_branch() || u.voxel != voxel {
                return;
            }
        }
        self.drop_children(i_node);
    }

    pub fn debug_boxes(&self) -> Vec<([usize; 3], usize)> {
        let mut acc = Vec::new();
        self.debug_boxes_descend(&mut acc, self.root, [0; 3], self.log_extent);
        acc
    }

    fn debug_boxes_descend(
        &self,
        acc: &mut Vec<([usize; 3], usize)>,
        i_node: usize,
        offset: [usize; 3],
        node_log_extent: usize,
    ) {
        let node = &self.nodes[i_node];
        if node.is_leaf() {
            if node.voxel != !0 {
                acc.push((offset, 1 << node_log_extent));
            }
        } else {
            let half_extent = 1 << (node_log_extent - 1);
            for i_child in 0..8 {
                let mut next_offset = offset;
                for (i, no) in next_offset.iter_mut().enumerate() {
                    if i_child & (1 << i) != 0 {
                        *no += half_extent;
                    }
                }
                self.debug_boxes_descend(
                    acc,
                    node.children[i_child],
                    next_offset,
                    node_log_extent - 1,
                );
            }
        }
    }

    #[allow(unused)]
    pub fn debug_mesh(&self) -> (Vec<u32>, Vec<vec3>) {
        let mut indices = Vec::new();
        let mut vertices = Vec::new();
        let mut index_of = HashMap::new();

        for ([x, y, z], e) in self.debug_boxes() {
            let i0 = vertex_index(&mut index_of, &mut vertices, [x, y, z]);
            let i1 = vertex_index(&mut index_of, &mut vertices, [x + e, y, z]);
            let i2 = vertex_index(&mut index_of, &mut vertices, [x, y + e, z]);
            let i3 = vertex_index(&mut index_of, &mut vertices, [x + e, y + e, z]);
            let i4 = vertex_index(&mut index_of, &mut vertices, [x, y, z + e]);
            let i5 = vertex_index(&mut index_of, &mut vertices, [x + e, y, z + e]);
            let i6 = vertex_index(&mut index_of, &mut vertices, [x, y + e, z + e]);
            let i7 = vertex_index(&mut index_of, &mut vertices, [x + e, y + e, z + e]);

            //    6--------7
            //   /|       /|
            //  / |      / |
            // 4--------5  |
            // |  |     |  |
            // |  2-----|--3
            // | /      | /
            // |/       |/
            // 0--------1

            // -Z
            indices.push(i0 as _);
            indices.push(i2 as _);
            indices.push(i3 as _);
            indices.push(i3 as _);
            indices.push(i1 as _);
            indices.push(i0 as _);

            // +Z
            indices.push(i4 as _);
            indices.push(i5 as _);
            indices.push(i7 as _);
            indices.push(i7 as _);
            indices.push(i6 as _);
            indices.push(i4 as _);

            // -Y
            indices.push(i0 as _);
            indices.push(i1 as _);
            indices.push(i5 as _);
            indices.push(i5 as _);
            indices.push(i4 as _);
            indices.push(i0 as _);

            // +Y
            indices.push(i2 as _);
            indices.push(i6 as _);
            indices.push(i7 as _);
            indices.push(i7 as _);
            indices.push(i3 as _);
            indices.push(i2 as _);

            // -X
            indices.push(i0 as _);
            indices.push(i4 as _);
            indices.push(i6 as _);
            indices.push(i6 as _);
            indices.push(i2 as _);
            indices.push(i0 as _);

            // +X
            indices.push(i1 as _);
            indices.push(i3 as _);
            indices.push(i7 as _);
            indices.push(i7 as _);
            indices.push(i5 as _);
            indices.push(i1 as _);
        }

        (indices, vertices)
    }
}

fn vertex_index(
    index_of: &mut HashMap<[usize; 3], usize>,
    vertices: &mut Vec<vec3>,
    key: [usize; 3],
) -> usize {
    match index_of.entry(key) {
        std::collections::hash_map::Entry::Occupied(o) => *o.get(),
        std::collections::hash_map::Entry::Vacant(v) => {
            let i = vertices.len();
            v.insert(i);
            vertices.push(Vector([key[0] as f32, key[1] as f32, key[2] as f32]));
            i
        }
    }
}
