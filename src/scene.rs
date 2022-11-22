
use math::{
    vec::{Vec2, Vec3},
    mat::Mat4,
};

#[derive(Debug)]
pub struct Mesh {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    pub indices: Vec<u32>,

    pub transform: Mat4,
    pub material: Material,
}

#[derive(Debug)]
pub struct Scene {
    pub meshes: Vec<Mesh>,
    pub images: Vec<Image>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            meshes: Vec::new(),
            images: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub enum Format {
    RGBA8,
}

#[derive(Debug)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub format: Format,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub struct Material {
    pub albedo_texture: u32,
}