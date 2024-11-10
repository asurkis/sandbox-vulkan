pub mod octree;

#[derive(Debug, Default, Clone, Copy)]
pub struct VoxelInfo {
    diffuse_color: [f32; 3],
    opacity: f32,
}
