use crate::d3d12::Shader;


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
pub const RAY_LIB: Shader = Shader {
    data: include_bytes!("../res/ray.lib.bin"),
    name: "ray",
};

