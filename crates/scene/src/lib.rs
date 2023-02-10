use math::{
    vec::{Vec2, Vec3},
    mat::Mat4,
};

pub mod camera;

pub use camera::*;
use bytemuck::{bytes_of, cast_slice, Pod, Zeroable, pod_read_unaligned};

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

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub enum Format {
    RGBA8,
}

impl Into<u32> for Format {
    fn into(self) -> u32 {
        match self {
            Format::RGBA8 => 0,
        }
    }
}

impl TryFrom<u32> for Format {
    type Error = &'static str;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Format::RGBA8),
            _ => Err("Unknown format"),
        }
    }
}

#[derive(Debug)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub format: Format,
    pub data: Vec<u8>,
}


#[derive(Debug, Pod, Zeroable, Clone, Copy)]
#[repr(C)]
pub struct Material {
    pub albedo_texture: u32,
}


pub trait Serialize {
    fn serialize_buf(&self, buf: &mut Vec<u8>);
    fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        self.serialize_buf(&mut buf);
        buf
    }
}

impl Serialize for Image {
    fn serialize_buf(&self, buf: &mut Vec<u8>) {
        (&self.width).serialize_buf(buf);
        (&self.height).serialize_buf(buf);
        (&self.format).serialize_buf(buf);
        self.data.serialize_buf(buf);
    }
}

impl Serialize for Format {
    fn serialize_buf(&self, buf: &mut Vec<u8>) {
        let v: u32 = (*self).into();
        (&v).serialize_buf(buf);
    }
}

impl Serialize for Mesh {
    fn serialize_buf(&self, buf: &mut Vec<u8>) {
        self.positions.serialize_buf(buf);
        self.normals.serialize_buf(buf);
        self.uvs.serialize_buf(buf);
        self.indices.serialize_buf(buf);
        (&self.transform).serialize_buf(buf);
        (&self.material).serialize_buf(buf);
    }
}

impl Serialize for Scene {
    fn serialize_buf(&self, buf: &mut Vec<u8>) {
        let meshes_count = self.meshes.len() as u64;
        (&meshes_count).serialize_buf(buf);
        for m in self.meshes.iter() {
            m.serialize_buf(buf);
        }

        let images_count = self.images.len() as u64;
        (&images_count).serialize_buf(buf);
        for img in self.images.iter() {
            img.serialize_buf(buf);
        }
    }
}

impl<T: Pod> Serialize for Vec<T> {
    fn serialize_buf(&self, buf: &mut Vec<u8>) {
        let size = self.len() * core::mem::size_of::<T>();
        buf.extend_from_slice(&size.to_le_bytes());
        buf.extend_from_slice(cast_slice(&self));
    }
}

impl<T: Pod> Serialize for &T {
    fn serialize_buf(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(bytes_of(*self));
    }
}


impl<T: Pod> Serialize for &[T] {
    fn serialize_buf(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(cast_slice(*self));
    }
}

pub trait Deserialize {
    type Item;

    fn deserialize(buf: &mut &[u8]) -> Self::Item;
}

impl<T: Pod> Deserialize for Vec<T> {
    type Item = Self;

    fn deserialize(buf: &mut &[u8]) -> Self {
        let size = u64::from_le_bytes(buf[..8].try_into().unwrap()) as usize;
        let mut v = Vec::new();
        v.extend_from_slice(cast_slice(&buf[8..8 + size]));
        *buf = &buf[8 + size as usize..];
        v
    }
}

impl<T: Pod> Deserialize for &T {
    type Item = T;

    fn deserialize(buf: &mut &[u8]) -> T {
        let v = pod_read_unaligned(&buf[..core::mem::size_of::<T>()]);
        *buf = &buf[core::mem::size_of::<T>() as usize..];
        v
    }
}

impl Deserialize for Mesh {
    type Item = Self;

    fn deserialize(buf: &mut &[u8]) -> Mesh {
        Mesh {
            positions: Vec::<Vec3>::deserialize(buf),
            normals: Vec::<Vec3>::deserialize(buf),
            uvs: Vec::<Vec2>::deserialize(buf),
            indices: Vec::<u32>::deserialize(buf),

            transform: <&Mat4>::deserialize(buf),
            material: <&Material>::deserialize(buf),
        }
    }
}


impl Deserialize for Image {
    type Item = Self;

    fn deserialize(buf: &mut &[u8]) -> Image {
        Image {
            width: <&u32>::deserialize(buf),
            height: <&u32>::deserialize(buf),
            format: <&u32>::deserialize(buf).try_into().unwrap(),
            data: Vec::<u8>::deserialize(buf),
        }
    }
}

impl Deserialize for Scene {
    type Item = Self;

    fn deserialize(buf: &mut &[u8]) -> Scene {
        let meshes_count = <&u64>::deserialize(buf);
        let mut meshes = Vec::with_capacity(meshes_count as usize);
        for _ in 0..meshes_count {
            meshes.push(Mesh::deserialize(buf));
        }

        let images_count = <&u64>::deserialize(buf);
        let mut images = Vec::with_capacity(images_count as usize);
        for _ in 0..images_count {
            images.push(Image::deserialize(buf));
        }

        Scene {
            meshes,
            images,
        }
    }
}
