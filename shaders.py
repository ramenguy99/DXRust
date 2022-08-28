import os
import subprocess

SHADERS_SOURCE_DIR="shaders"
SHADERS_OUT_DIR="res"

DXC="C:\\Users\\dmylo\\Desktop\\Code\\D3D12\\dependencies\\dxc\\bin\\x64\\dxc.exe"
DXC_FLAGS=""

version = "6_3"

def cmd(command):
    print(command)
    p = subprocess.run(command, shell=True, text=True, stdout=subprocess.PIPE, 
            stderr=subprocess.STDOUT)

    if p.returncode != 0:
        print(f"ERROR command exited with code {p.returncode}:\n{command}")
        print(p.stdout)
        exit(1)

output = []

for f in os.listdir(SHADERS_SOURCE_DIR):
    root, ext = os.path.splitext(f)

    if ext != ".hlsl":
        continue

    elems = root.split(".")
    if len(elems) < 2:
        continue

    name, typ = elems

    in_path = os.path.join(SHADERS_SOURCE_DIR, f)
    out_path = os.path.join(SHADERS_OUT_DIR, f"{root}.bin")

    print(f)
    cmd(f"{DXC} {DXC_FLAGS} -T {typ}_{version} -Fo {out_path} {in_path}")
    #cmd(f"{DXC} {DXC_FLAGS} -T {typ}_{version} -Frs {out_path}.rs -Fh {out_path}.h -Fo {out_path} {in_path}")
    #cmd(f"{DXC} {DXC_FLAGS} -T {typ}_{version} -Fh {out_path}.h -Fo {out_path} {in_path}")

    output.append((name, out_path, typ))

rs = open(os.path.join("src", "shaders.rs"), "w")
rs.write('use crate::d3d12::Shader;\n\n\n')
for n, p,typ in output:
    rs.write(f'#[allow(dead_code)]\npub const {n.upper()}_{typ.upper()}: Shader = Shader {{\n    data: include_bytes!("../{p.replace(os.sep, "/")}"),\n    name: "{n}",\n}};\n\n')

