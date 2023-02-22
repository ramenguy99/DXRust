import os
import subprocess
from pycparser import c_ast, parse_file


SHADERS_SOURCE_DIR="shaders"
SHADERS_OUT_DIR="res"

DXC="dxc"
DXC_FLAGS=""

CPP_PATH="clang-cpp"

version = "6_3"
shader_types = {
    "vs",
    "ps",
    "cs",
    "lib"
}

def cmd(command):
    print(command)
    p = subprocess.run(command, shell=True, text=True, stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT)

    if p.returncode != 0:
        print(f"ERROR command exited with code {p.returncode}:\n{command}")
        print(p.stdout)
        exit(1)

output = []
if SHADERS_OUT_DIR:
    os.makedirs(SHADERS_OUT_DIR, exist_ok=True)

for f in os.listdir(SHADERS_SOURCE_DIR):
    root, ext = os.path.splitext(f)

    if ext != ".hlsl":
        continue

    elems = root.split(".")
    if len(elems) < 2:
        continue

    name, typ = elems
    if typ not in shader_types:
        continue

    in_path = os.path.join(SHADERS_SOURCE_DIR, f)
    out_path = os.path.join(SHADERS_OUT_DIR, f"{root}.bin")

    print(f)
    cmd(f"{DXC} {DXC_FLAGS} -T {typ}_{version} -Fo {out_path} {in_path}")
    #cmd(f"{DXC} {DXC_FLAGS} -T {typ}_{version} -Frs {out_path}.rs -Fh {out_path}.h -Fo {out_path} {in_path}")
    #cmd(f"{DXC} {DXC_FLAGS} -T {typ}_{version} -Fh {out_path}.h -Fo {out_path} {in_path}")

    output.append((name, out_path, typ))

rs = open(os.path.join("src", "shaders.rs"), "w")
rs.write('use bytemuck::{Zeroable, Pod};\n')
rs.write("\n")
rs.write('#[allow(unused_imports)]\nuse math::{vec::{Vec2, Vec3, Vec4}, mat::Mat4};\n')
rs.write('use crate::d3d12::Shader;\n')
rs.write("\n")

ast = parse_file("shaders/types.hlsl", use_cpp=True, cpp_path=CPP_PATH,
        cpp_args=[
            "-Dfloat2=int",
            "-Dfloat3=int",
            "-Dfloat4=int",
            "-Dint2=int",
            "-Dint3=int",
            "-Dint4=int",
            "-Duint2=int",
            "-Duint3=int",
            "-Duint4=int",
            "-Dfloat3x3=int",
            "-Dfloat4x4=int",
            "-Duint=int",
        ]
    )

type_info = {
    "u32":   ("u32",   4),
    "float": ("f32",   4),
    "vec2":  ("Vec2",  8),
    "vec3":  ("Vec3", 12),
    "vec4":  ("Vec4", 16),
    "mat3":  ("Mat3", 16),
    "mat4":  ("Mat4", 16),
}

def error(coord, msg):
    print("ERROR generating rust:", coord, msg)
    exit(1)

for e in ast.ext:
    a = c_ast.TypeDecl
    if isinstance(e, c_ast.Decl) and isinstance(e.type, c_ast.Struct):
        struct = e.type
        rs.write(f"#[allow(dead_code)]\n#[derive(Default, Clone, Copy, Pod, Zeroable)]\n#[repr(C)]\n")
        rs.write(f"pub struct {struct.name} {{\n")

        current_align = 0
        padding_index = 0
        do_alignment = "constants" in struct.name.lower()
        for i, mem_decl in enumerate(struct.decls):
            name = mem_decl.name
            typ = mem_decl.type.children()[0][1].names[0]
            if typ not in type_info:
                error(mem_decl.coord, f"Type '{typ}' not supported in shared struct")
            info = type_info[typ]

            space = 16 - current_align % 16
            if do_alignment:
                if space < info[1]:
                    rs.write(f"    pub _padding{padding_index}: u32,\n")
                    padding_index += 1
                    current_align += space
                    rs.write("\n")
                current_align += info[1]
            rs.write(f"    pub {name}: {info[0]},\n")
            if do_alignment:
                if current_align % 16 == 0:
                    rs.write("\n")
        rs.write("}\n\n")

for n, p,typ in output:
    rs.write(f'#[allow(dead_code)]\npub const {n.upper()}_{typ.upper()}: Shader = Shader {{\n    data: include_bytes!("../{p.replace(os.sep, "/")}"),\n    name: "{n}",\n}};\n\n')

