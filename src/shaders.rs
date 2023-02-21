use bytemuck::{Zeroable, Pod};

#[allow(unused_imports)]
use math::{vec::{Vec2, Vec3, Vec4}, mat::Mat4};
use crate::d3d12::Shader;

#[allow(dead_code)]
#[derive(Default, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct Constants {
    pub camera_position: Vec3,
    pub _padding0: u32,

    pub camera_direction: Vec3,
    pub _padding1: u32,

    pub light_direction: Vec3,
    pub light_radiance: f32,

    pub diffuse_color: Vec3,
    pub film_dist: f32,

    pub projection: Mat4,

    pub view: Mat4,

    pub frame_index: u32,
    pub samples: u32,
    pub emissive_multiplier: f32,
    pub debug: u32,

    pub lights_pdf_normalization: f32,
    pub num_lights: u32,
    pub bounces: u32,
    pub sampling_mode: u32,

    pub use_alias_table: u32,
}

#[allow(dead_code)]
#[derive(Default, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct RayMeshInstance {
    pub vertex_offset: u32,
    pub index_offset: u32,
    pub albedo_index: u32,
    pub normal_index: u32,
    pub specular_index: u32,
    pub emissive_index: u32,
    pub albedo_value: Vec4,
    pub specular_value: Vec4,
    pub emissive_value: Vec4,
}

#[allow(dead_code)]
#[derive(Default, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct RasterMeshInstance {
    pub transform: Mat4,
    pub albedo_index: u32,
    pub normal_index: u32,
    pub specular_index: u32,
    pub emissive_index: u32,
    pub albedo_value: Vec4,
    pub specular_value: Vec4,
    pub emissive_value: Vec4,
}

#[allow(dead_code)]
#[derive(Default, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct Light {
    pub p0: Vec3,
    pub p1: Vec3,
    pub p2: Vec3,
    pub emissive: Vec3,
}

#[allow(dead_code)]
#[derive(Default, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct Alias {
    pub p: f32,
    pub a: u32,
}

#[allow(dead_code)]
pub const CLEAR_CS: Shader = Shader {
    data: include_bytes!("../res/clear.cs.bin"),
    name: "clear",
};

#[allow(dead_code)]
pub const IMGUI_PS: Shader = Shader {
    data: include_bytes!("../res/imgui.ps.bin"),
    name: "imgui",
};

#[allow(dead_code)]
pub const IMGUI_VS: Shader = Shader {
    data: include_bytes!("../res/imgui.vs.bin"),
    name: "imgui",
};

#[allow(dead_code)]
pub const MESH_PS: Shader = Shader {
    data: include_bytes!("../res/mesh.ps.bin"),
    name: "mesh",
};

#[allow(dead_code)]
pub const MESH_VS: Shader = Shader {
    data: include_bytes!("../res/mesh.vs.bin"),
    name: "mesh",
};

#[allow(dead_code)]
pub const POSTPROCESS_CS: Shader = Shader {
    data: include_bytes!("../res/postprocess.cs.bin"),
    name: "postprocess",
};

#[allow(dead_code)]
pub const RAY_LIB: Shader = Shader {
    data: include_bytes!("../res/ray.lib.bin"),
    name: "ray",
};

