
use math::{
    vec::Vec3,
    mat::Mat4,
};

#[derive(Debug)]
pub struct Mesh {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub indices: Vec<u32>,

    pub transform: Mat4,
}

#[derive(Debug)]
pub struct Scene {
    pub meshes: Vec<Mesh>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            meshes: Vec::new(),
        }
    }
}
