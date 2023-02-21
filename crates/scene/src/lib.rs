#![feature(allocator_api)]

use std::alloc::{Global, Allocator};

use math::{
    vec::{Vec2, Vec3, Vec4},
    mat::Mat4,
};

pub mod camera;

pub use camera::*;
use bytemuck::{bytes_of, cast_slice, Pod, pod_read_unaligned};

#[derive(Debug)]
pub struct Mesh<A: Allocator + Copy=Global> {
    pub positions: Vec<Vec3, A>,
    pub normals: Vec<Vec3, A>,
    pub tangents: Vec<Vec3, A>,
    pub uvs: Vec<Vec2, A>,
    pub indices: Vec<u32, A>,

    pub transform: Mat4,
    pub material: Material,
}

#[derive(Debug)]
pub struct Scene<A: Allocator + Copy=Global> {
    pub meshes: Vec<Mesh<A>, A>,
    pub images: Vec<Image<A>, A>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            meshes: Vec::new(),
            images: Vec::new(),
        }
    }
}

impl<A: Allocator + Copy> Scene<A> {
    pub fn new_in(a: A) -> Self {
        Self {
            meshes: Vec::<Mesh<A>,A>::new_in(a),
            images: Vec::<Image<A>,A>::new_in(a),
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub enum Format {
    RGBA8,
    SRGBA8,
}

impl Into<u32> for Format {
    fn into(self) -> u32 {
        match self {
            Format::RGBA8 => 0,
            Format::SRGBA8 => 1,
        }
    }
}

impl TryFrom<u32> for Format {
    type Error = &'static str;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Format::RGBA8),
            1 => Ok(Format::SRGBA8),
            _ => Err("Unknown format"),
        }
    }
}

#[derive(Debug)]
pub struct Image<A: Allocator + Copy=Global> {
    pub width: u32,
    pub height: u32,
    pub format: Format,
    pub data: Vec<u8, A>,
}


#[derive(Debug, Clone, Copy)]
pub enum MaterialParameter {
    None,
    Texture(u32),
    Vec2(Vec2),
    Vec3(Vec3),
    Vec4(Vec4),
}

impl Into<u32> for MaterialParameter {
    fn into(self) -> u32 {
        match self {
            MaterialParameter::None => 0,
            MaterialParameter::Texture(_) => 1,
            MaterialParameter::Vec2(_)    => 2,
            MaterialParameter::Vec3(_)    => 3,
            MaterialParameter::Vec4(_)    => 4,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Material {
    pub base_color: MaterialParameter,
    pub normal:     MaterialParameter,
    pub specular:   MaterialParameter,
    pub emissive:   MaterialParameter,
}


pub trait Serialize {
    fn serialize_buf(&self, buf: &mut Vec<u8>);
    fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        self.serialize_buf(&mut buf);
        buf
    }
}

impl<A: Allocator + Copy> Serialize for Image<A> {
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

impl Serialize for MaterialParameter {
    fn serialize_buf(&self, buf: &mut Vec<u8>) {
        let typ: u32 = (*self).into();
        (&typ).serialize_buf(buf);

        match *self {
            MaterialParameter::None => {},
            MaterialParameter::Texture(v) => (&v).serialize_buf(buf),
            MaterialParameter::Vec2(v)    => (&v).serialize_buf(buf),
            MaterialParameter::Vec3(v)    => (&v).serialize_buf(buf),
            MaterialParameter::Vec4(v)    => (&v).serialize_buf(buf),
        }
    }
}

impl Serialize for Material {
    fn serialize_buf(&self, buf: &mut Vec<u8>) {
        self.base_color.serialize_buf(buf);
        self.normal.serialize_buf(buf);
        self.specular.serialize_buf(buf);
        self.emissive.serialize_buf(buf);
    }
}

impl Serialize for Mesh {
    fn serialize_buf(&self, buf: &mut Vec<u8>) {
        self.positions.serialize_buf(buf);
        self.normals.serialize_buf(buf);
        self.tangents.serialize_buf(buf);
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

impl<T: Pod, A: Allocator> Serialize for Vec<T, A> {
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

pub trait Deserialize<A: Allocator + Copy=Global> {
    type Item;
    type AllocatorItem;

    fn deserialize(buf: &mut &[u8]) -> Self::Item;
    fn deserialize_in(buf: &mut &[u8], a: A) -> Self::AllocatorItem;

}

impl<T: Pod, A: Allocator + Copy> Deserialize<A> for Vec<T, A> {
    type Item = Vec<T>;
    type AllocatorItem = Vec<T, A>;

    fn deserialize(buf: &mut &[u8]) -> Self::Item {
        let size = u64::from_le_bytes(buf[..8].try_into().unwrap()) as usize;
        let mut v = Vec::new();
        v.extend_from_slice(cast_slice(&buf[8..8 + size]));
        *buf = &buf[8 + size as usize..];
        v
    }

    fn deserialize_in(buf: &mut &[u8], a: A) -> Self::AllocatorItem {
        let size = u64::from_le_bytes(buf[..8].try_into().unwrap()) as usize;
        let mut v = Vec::new_in(a);
        v.extend_from_slice(cast_slice(&buf[8..8 + size]));
        *buf = &buf[8 + size as usize..];
        v
    }
}


impl<T: Pod> Deserialize for &T {
    type Item = T;
    type AllocatorItem = T;

    fn deserialize(buf: &mut &[u8]) -> T {
        let v = pod_read_unaligned(&buf[..core::mem::size_of::<T>()]);
        *buf = &buf[core::mem::size_of::<T>() as usize..];
        v
    }

    fn deserialize_in(buf: &mut &[u8], _a: Global) -> T {
        <&T>::deserialize(buf)
    }
}

impl Deserialize for MaterialParameter {
    type Item = MaterialParameter;
    type AllocatorItem = MaterialParameter;

    fn deserialize(buf: &mut &[u8]) -> Self::Item {
        let typ = <&u32>::deserialize(buf);
        match typ {
            0 => MaterialParameter::None,
            1 => MaterialParameter::Texture(<&u32>::deserialize(buf)),
            2 => MaterialParameter::Vec2(<&Vec2>::deserialize(buf)),
            3 => MaterialParameter::Vec3(<&Vec3>::deserialize(buf)),
            4 => MaterialParameter::Vec4(<&Vec4>::deserialize(buf)),
            _ => panic!(),
        }
    }

    fn deserialize_in(buf: &mut &[u8], _a: Global) -> Self::AllocatorItem {
        Self::deserialize(buf)
    }
}

impl Deserialize for Material {
    type Item = Material;
    type AllocatorItem = Material;

    fn deserialize(buf: &mut &[u8]) -> Self::Item {
        Material {
            base_color: MaterialParameter::deserialize(buf),
            normal:     MaterialParameter::deserialize(buf),
            specular:   MaterialParameter::deserialize(buf),
            emissive:   MaterialParameter::deserialize(buf),
        }
    }

    fn deserialize_in(buf: &mut &[u8], _a: Global) -> Self::AllocatorItem {
        Self::deserialize(buf)
    }
}

impl<A: Allocator + Copy> Deserialize<A> for Mesh<A> {
    type Item = Mesh;
    type AllocatorItem = Mesh<A>;

    fn deserialize(buf: &mut &[u8]) -> Mesh {
        Mesh {
            positions: Vec::<Vec3>::deserialize(buf),
            normals: Vec::<Vec3>::deserialize(buf),
            tangents: Vec::<Vec3>::deserialize(buf),
            uvs: Vec::<Vec2>::deserialize(buf),
            indices: Vec::<u32>::deserialize(buf),

            transform: <&Mat4>::deserialize(buf),
            material: Material::deserialize(buf),
        }
    }

    fn deserialize_in(buf: &mut &[u8], a: A) -> Mesh<A> {
        Mesh {
            positions: Vec::<Vec3, A>::deserialize_in(buf, a),
            normals: Vec::<Vec3, A>::deserialize_in(buf, a),
            tangents: Vec::<Vec3, A>::deserialize_in(buf, a),
            uvs: Vec::<Vec2, A>::deserialize_in(buf, a),
            indices: Vec::<u32, A>::deserialize_in(buf, a),

            transform: <&Mat4>::deserialize(buf),
            material: Material::deserialize(buf),
        }
    }
}


impl<A: Allocator + Copy> Deserialize<A> for Image<A> {
    type Item = Image;
    type AllocatorItem = Image<A>;

    fn deserialize(buf: &mut &[u8]) -> Image {
        Image {
            width: <&u32>::deserialize(buf),
            height: <&u32>::deserialize(buf),
            format: <&u32>::deserialize(buf).try_into().unwrap(),
            data: Vec::<u8>::deserialize(buf),
        }
    }

    fn deserialize_in(buf: &mut &[u8], a: A) -> Image<A> {
        Image {
            width: <&u32>::deserialize(buf),
            height: <&u32>::deserialize(buf),
            format: <&u32>::deserialize(buf).try_into().unwrap(),
            data: Vec::<u8, A>::deserialize_in(buf, a),
        }
    }
}

impl<A: Allocator + Copy> Deserialize<A> for Scene<A> {
    type Item = Scene;
    type AllocatorItem = Scene<A>;

    fn deserialize(buf: &mut &[u8]) -> Scene {
        let meshes_count = <&u64>::deserialize(buf);
        let mut meshes = Vec::with_capacity(meshes_count as usize);
        for _ in 0..meshes_count {
            meshes.push(Mesh::<Global>::deserialize(buf));
        }

        let images_count = <&u64>::deserialize(buf);
        let mut images = Vec::with_capacity(images_count as usize);
        for _ in 0..images_count {
            images.push(Image::<Global>::deserialize(buf));
        }

        Scene {
            meshes,
            images,
        }
    }


    fn deserialize_in(buf: &mut &[u8], a: A) -> Scene<A> {
        let meshes_count = <&u64>::deserialize(buf);
        let mut meshes = Vec::with_capacity_in(meshes_count as usize, a);
        for _ in 0..meshes_count {
            meshes.push(Mesh::deserialize_in(buf, a));
        }

        let images_count = <&u64>::deserialize(buf);
        let mut images = Vec::with_capacity_in(images_count as usize, a);
        for _ in 0..images_count {
            images.push(Image::deserialize_in(buf, a));
        }

        Scene {
            meshes,
            images,
        }
    }
}
