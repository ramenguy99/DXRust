# DXRust

A very much work in progress path tracer written in Rust using hardware accelerated raytracing through DXR.

![Editor](img/screenshot.png)

## Features
- DX12 and Win32 platform layers written from scratch
- Rasterization pipeline developed in parallel for comparison and debugging
- ImGui integration
- GLTF 3D models and texture loading
- Vector math library

## Todo
- Area light and BRDF multiple importance sampling.
- Path tracer integrator
- Shader hot reloading
- Much, much more...

## Build
Currently HLSL shaders are compiled to bytecode and included directly in the program executable as binary data.
A python script is used to build the shader binaries and generate the required rust code, a recent version of the [DXC compiler](https://github.com/microsoft/DirectXShaderCompiler/releases) `dxc.exe` must be in the path when running the script or the `DXC` variable in the script must be updated to point to the executable.

```
git clone --recursive https://github.com/ramenguy99/DXRust

cd DXRust
python shaders.py
cargo run --release PATH_TO_GLTF
```