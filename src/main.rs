//#![windows_subsystem = "windows"]

use core::mem::{size_of, size_of_val};
use math::vec::Vec3;
use math::mat::{Mat4, self};
use mesh::Mesh;

mod win32;
mod d3d12;
mod shaders;
mod asset;
mod mesh;
mod imgui_impl;
mod gltf;

#[allow(unused_macros)]
macro_rules! debug_break {
    () => {
        unsafe {
            core::arch::asm!("int 3");
        }
    }
}


#[derive(Default, Clone, Copy)]
#[repr(C)]
struct SceneConstants {
    camera_position: Vec3,
    _padding0: f32,

    light_position: Vec3,
    _padding1: f32,

    diffuse_color: Vec3,
    film_dist: f32,

    projection: Mat4,
    view: Mat4,
    model: Mat4,
    normal: Mat4,
}

trait Scene {
    fn resize(&mut self, d3d12: &d3d12::Context, width: u32, height:u32);

    fn render(&mut self, d3d12: &d3d12::Context, frame: &d3d12::Frame, 
              _frame_index: u32, constants: &SceneConstants);
}

struct Raster {
    width: u32,
    height: u32,

    depth: d3d12::ID3D12Resource,
    depth_descriptor: d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
    #[allow(dead_code)]
    depth_heap: d3d12::DescriptorHeap,

    pso: d3d12::ID3D12PipelineState,
    rs: d3d12::ID3D12RootSignature,

    vertices_count: usize,
    indices_count: usize,
    positions: d3d12::ID3D12Resource,
    normals: d3d12::ID3D12Resource,
    indices: d3d12::ID3D12Resource,

    constant_buffer: d3d12::PerFrameConstantBuffer,
}

impl Raster {
    fn init(window: &win32::Window, d3d12: &d3d12::Context, 
                mesh: &Mesh) -> Self {

        let rs = 
            d3d12.create_root_signature_from_shader(&shaders::MESH_VS)
                .expect("Failed to create root signature");


        let positions = d3d12.upload_buffer_sync( unsafe {
            core::slice::from_raw_parts(mesh.positions.as_ptr() as *const u8, 
                                       mesh.positions.len() * size_of::<Vec3>())
            }, d3d12::D3D12_RESOURCE_STATE_GENERIC_READ)
            .expect("Failed to upload positions");

        let normals = d3d12.upload_buffer_sync( unsafe {
            core::slice::from_raw_parts(mesh.normals.as_ptr() as *const u8, 
                                       mesh.normals.len() * size_of::<Vec3>())
            }, d3d12::D3D12_RESOURCE_STATE_GENERIC_READ)
            .expect("Failed to upload normals");

        let indices = d3d12.upload_buffer_sync( unsafe {
            core::slice::from_raw_parts(mesh.indices.as_ptr() as *const u8, 
                                       mesh.indices.len() * size_of::<u32>())
            }, d3d12::D3D12_RESOURCE_STATE_GENERIC_READ)
            .expect("Failed to upload indices");

        let constant_buffer = d3d12.create_per_frame_constant_buffer(
            size_of::<SceneConstants>())
            .expect("Failed to create per frame constant buffers");

        let depth = d3d12.create_depth_buffer(window.width(), window.height())
            .expect("Failed to create depth buffer");

        let depth_heap = d3d12.create_descriptor_heap(
            1, d3d12::D3D12_DESCRIPTOR_HEAP_TYPE_DSV, false)
            .expect("Failed to create depth heap");

        let depth_descriptor = depth_heap.alloc_descriptor()
            .expect("Failed to alloc depth descriptor");

        d3d12.create_depth_stencil_view(&depth, depth_descriptor);

        let pso = d3d12.create_graphics_pipeline_state(
            &d3d12::GraphicsPipelineState {
                vs: Some(&shaders::MESH_VS),
                ps: Some(&shaders::MESH_PS),
                input_layout: &[
                    d3d12::D3D12_INPUT_ELEMENT_DESC {
                        SemanticName: windows::core::PCSTR(b"POSITION\0".as_ptr()),
                        SemanticIndex: 0,
                        Format: d3d12::DXGI_FORMAT_R32G32B32_FLOAT,
                        InputSlot: 0,
                        AlignedByteOffset: 0,
                        InputSlotClass: d3d12::D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                        InstanceDataStepRate: 0,
                    },
                    d3d12::D3D12_INPUT_ELEMENT_DESC {
                        SemanticName: windows::core::PCSTR(b"NORMAL\0".as_ptr()),
                        SemanticIndex: 0,
                        Format: d3d12::DXGI_FORMAT_R32G32B32_FLOAT,
                        InputSlot: 1,
                        AlignedByteOffset: 0,
                        InputSlotClass: d3d12::D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                        InstanceDataStepRate: 0,
                    },
                ],
                render_targets: &[
                    d3d12::RenderTargetState {
                        format: d3d12::DXGI_FORMAT_R8G8B8A8_UNORM,
                        blend_mode: d3d12::BlendMode::Default,
                    },
                ],
                rasterizer: d3d12::RasterizerState {
                    fill_mode: d3d12::D3D12_FILL_MODE_SOLID,
                    cull_mode: d3d12::D3D12_CULL_MODE_BACK,
                    front_ccw: false,
                    ..Default::default()
                },
                depth: d3d12::DepthState {
                    write: true,
                    test: true,
                    ..Default::default()
                },
                ..Default::default()
        }, &rs).expect("Failed to create graphics pso");

        Raster {
            width: window.width(),
            height: window.height(),
            vertices_count: mesh.positions.len(),
            indices_count: mesh.indices.len(),
            depth,
            depth_heap,
            depth_descriptor,
            rs,
            positions,
            normals,
            indices,
            constant_buffer,
            pso,
        }
    }
}

impl Scene for Raster {
    fn resize(&mut self, d3d12: &d3d12::Context, width: u32, height:u32) {
        self.width = width;
        self.height = height;

        self.depth = d3d12.create_depth_buffer(width, height)
            .expect("Failed to resize depth buffer");
        d3d12.create_depth_stencil_view(&self.depth, self.depth_descriptor);
    }

    fn render(&mut self, d3d12: &d3d12::Context, frame: &d3d12::Frame, 
              frame_index: u32, constants: &SceneConstants) {
        
        let vbv = [ 
            d3d12::D3D12_VERTEX_BUFFER_VIEW {
                BufferLocation: unsafe { self.positions.GetGPUVirtualAddress() },
                SizeInBytes:    (self.vertices_count * size_of::<Vec3>()) as u32,
                StrideInBytes:  size_of::<Vec3>() as u32,
            },
            d3d12::D3D12_VERTEX_BUFFER_VIEW {
                BufferLocation: unsafe { self.normals.GetGPUVirtualAddress() },
                SizeInBytes:    (self.vertices_count * size_of::<Vec3>()) as u32,
                StrideInBytes:  size_of::<Vec3>() as u32,
            },
        ];

        let ibv = d3d12::D3D12_INDEX_BUFFER_VIEW {
            BufferLocation: unsafe { self.indices.GetGPUVirtualAddress() },
            SizeInBytes:   (self.indices_count * size_of::<u32>()) as u32,
            Format: d3d12::DXGI_FORMAT_R32_UINT,
        };

        let viewports = [ d3d12::D3D12_VIEWPORT {
            Width: self.width as f32,
            Height: self.height as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
            TopLeftX: 0.0,
            TopLeftY: 0.0,
        }];

        let scissor_rects = [ d3d12::D3D12_RECT {
            left: 0,
            top: 0,
            right: self.width as i32,
            bottom: self.height as i32,
        }];

        let barriers = [
            d3d12::ResourceBarrier::transition(
                frame.render_target_resource.as_ref().unwrap(), 
                d3d12::D3D12_RESOURCE_STATE_PRESENT, 
                d3d12::D3D12_RESOURCE_STATE_RENDER_TARGET),
        ];

        let command_list = d3d12.create_graphics_command_list(frame)
            .expect("Failed to create command list");


        unsafe {
            let constant_data = core::slice::from_raw_parts(
                    constants as *const _ as *const u8, 
                    size_of_val(constants));

            self.constant_buffer.write(frame_index, constant_data)
                .expect("Failed to write constants");

            command_list.ResourceBarrier(&barriers);
            command_list.ClearRenderTargetView(frame.render_target_descriptor,
                                               [0.2, 0.1, 0.1, 1.0].as_ptr(), 
                                               &[]);
            command_list.ClearDepthStencilView(self.depth_descriptor,
                                               d3d12::D3D12_CLEAR_FLAG_DEPTH,
                                               1.0, 0, &[]);

            command_list.SetPipelineState(&self.pso);
            command_list.SetGraphicsRootSignature(&self.rs);
            command_list.SetGraphicsRootConstantBufferView(0,
                self.constant_buffer.get_gpu_virtual_address(frame_index));
            
            command_list.IASetVertexBuffers(0, &vbv);
            command_list.IASetIndexBuffer(&ibv);
            command_list.IASetPrimitiveTopology(
                d3d12::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            
            command_list.RSSetViewports(&viewports);
            command_list.RSSetScissorRects(&scissor_rects);

            command_list.OMSetRenderTargets(1, &frame.render_target_descriptor, 
                                            windows::Win32::Foundation::BOOL(0), 
                                            &self.depth_descriptor);

            command_list.DrawIndexedInstanced(self.indices_count as u32, 
                                              1, 0, 0, 0);

            command_list.Close().expect("Failed to close command list");
            d3d12::drop_barriers(barriers);
        }

        d3d12.execute_command_lists(&[Some(command_list.into())]);
    }
}

#[allow(dead_code)]
struct Ray {
    rs: d3d12::ID3D12RootSignature,
    state_object: d3d12::ID3D12StateObject,
    width: u32,
    height: u32,
    raygen_resource: d3d12::ID3D12Resource,
    raygen_table:    d3d12::D3D12_GPU_VIRTUAL_ADDRESS_RANGE,
    miss_resource:   d3d12::ID3D12Resource,
    miss_table:      d3d12::D3D12_GPU_VIRTUAL_ADDRESS_RANGE_AND_STRIDE,
    hit_resource:    d3d12::ID3D12Resource,
    hit_table:       d3d12::D3D12_GPU_VIRTUAL_ADDRESS_RANGE_AND_STRIDE,
    acceleration_structure: d3d12::AccelerationStructure,
    acceleration_structure_desc_handle: d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
    normals:         d3d12::ID3D12Resource,
    normals_desc_handle: d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
    indices:         d3d12::ID3D12Resource,
    indices_desc_handle: d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
    uav: d3d12::ID3D12Resource,
    uav_desc_handle: d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,

    constant_buffer: d3d12::PerFrameConstantBuffer,
}

impl Ray {
    fn init(window: &win32::Window, d3d12: &d3d12::Context, 
                mesh: &Mesh, transform: &Mat4) -> Self {
        let rs = d3d12.create_root_signature_from_shader(&shaders::RAY_LIB)
            .expect("Failed to create root signature");

        let state_object = d3d12.create_dxr_state_object(&shaders::RAY_LIB)
            .expect("Failed to create state object");

        let uav = d3d12.create_resource(&d3d12::ResourceDesc::uav2d(
                d3d12::DXGI_FORMAT_R8G8B8A8_UNORM, 
                window.width(), window.height()),
                d3d12::D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
                d3d12::D3D12_HEAP_TYPE_DEFAULT)
            .expect("failed to create uav resource");

        let uav_desc_handle = d3d12.alloc_csu_descriptor()
            .expect("Failed to alloc csu descriptor");

        d3d12.create_unordered_access_view(
            d3d12::D3D12_UAV_DIMENSION_TEXTURE2D, &uav, uav_desc_handle);

        use windows::core::Interface;
        let state_object_properties: d3d12::ID3D12StateObjectProperties =
            state_object.cast()
            .expect("Failed to query ID3D12StateObjectProperties");

        let make_shader_table = |name: &[u8]| {
            let name: Vec<u16> = name.iter().map(|x| *x as u16).collect();
            let mut data = [0u8; 
                d3d12::D3D12_RAYTRACING_SHADER_RECORD_BYTE_ALIGNMENT as usize];

            unsafe {
                data[0..d3d12::D3D12_SHADER_IDENTIFIER_SIZE_IN_BYTES as usize]
                    .copy_from_slice(core::slice::from_raw_parts(
                            state_object_properties.GetShaderIdentifier(
                                windows::core::PCWSTR(name.as_ptr())) as *const u8,
                                d3d12::D3D12_SHADER_IDENTIFIER_SIZE_IN_BYTES as usize));
            }
            let table = d3d12.upload_buffer_sync(&data, 
                            d3d12::D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE)
                        .expect("Failed to upload raygen table");
            table
        };

        let raygen_resource = make_shader_table(b"RayGeneration\0");
        let miss_resource   = make_shader_table(b"Miss\0");
        let hit_resource    = make_shader_table(b"HitGroup\0");

        let raygen_table = d3d12::D3D12_GPU_VIRTUAL_ADDRESS_RANGE {
            StartAddress: unsafe { raygen_resource.GetGPUVirtualAddress() },
            SizeInBytes:  d3d12::D3D12_SHADER_IDENTIFIER_SIZE_IN_BYTES as u64,
        };

        let miss_table = d3d12::D3D12_GPU_VIRTUAL_ADDRESS_RANGE_AND_STRIDE {
            StartAddress:  unsafe { miss_resource.GetGPUVirtualAddress() },
            SizeInBytes:   d3d12::D3D12_SHADER_IDENTIFIER_SIZE_IN_BYTES as u64,
            StrideInBytes: d3d12::D3D12_SHADER_IDENTIFIER_SIZE_IN_BYTES as u64,
        };

        let hit_table = d3d12::D3D12_GPU_VIRTUAL_ADDRESS_RANGE_AND_STRIDE {
            StartAddress:  unsafe { hit_resource.GetGPUVirtualAddress() },
            SizeInBytes:   d3d12::D3D12_SHADER_IDENTIFIER_SIZE_IN_BYTES as u64,
            StrideInBytes: d3d12::D3D12_SHADER_IDENTIFIER_SIZE_IN_BYTES as u64,
        };

        // Acceleration structures
        let positions = d3d12.upload_buffer_sync( unsafe {
            core::slice::from_raw_parts(mesh.positions.as_ptr() as *const u8, 
                                       mesh.positions.len() * size_of::<Vec3>())
            }, d3d12::D3D12_RESOURCE_STATE_GENERIC_READ)
            .expect("Failed to upload positions");

        let normals = d3d12.upload_buffer_sync( unsafe {
            core::slice::from_raw_parts(mesh.normals.as_ptr() as *const u8, 
                                       mesh.normals.len() * size_of::<Vec3>())
            }, d3d12::D3D12_RESOURCE_STATE_GENERIC_READ)
            .expect("Failed to upload normals");

        let indices = d3d12.upload_buffer_sync( unsafe {
            core::slice::from_raw_parts(mesh.indices.as_ptr() as *const u8, 
                                       mesh.indices.len() * size_of::<u32>())
            }, d3d12::D3D12_RESOURCE_STATE_GENERIC_READ)
            .expect("Failed to upload indices");


        let acceleration_structure = 
            d3d12.create_acceleration_structure(mesh.positions.len(), &positions,
                                                mesh.indices.len(), &indices,
                                                &transform)
            .expect("Failed to create acceleration structure");

        let acceleration_structure_desc_handle = d3d12.alloc_csu_descriptor()
            .expect("failed to alloc csu descriptor for as");

        d3d12.create_shader_resource_view_as(&acceleration_structure.tlas,
                                         acceleration_structure_desc_handle);

        let indices_desc_handle = d3d12.alloc_csu_descriptor()
            .expect("Failed to alloc csu descriptor for indices");

        d3d12.create_shader_resource_view_buffer(&indices,
                                                 d3d12::DXGI_FORMAT_R32_UINT,
                                                 0, mesh.indices.len() as u32,
                                                 indices_desc_handle);

        let normals_desc_handle = d3d12.alloc_csu_descriptor()
            .expect("Failed to alloc csu descriptor for normals");

        d3d12.create_shader_resource_view_buffer(&normals,
                                                 d3d12::DXGI_FORMAT_R32G32B32_FLOAT,
                                                 0, mesh.normals.len() as u32,
                                                 normals_desc_handle);


        let constant_buffer = d3d12.create_per_frame_constant_buffer(
            size_of::<SceneConstants>())
            .expect("Failed to alloc per frame constant buffers");
        
        Self {
            rs,
            state_object,
            uav,
            uav_desc_handle,
            raygen_table,
            raygen_resource,
            miss_table,
            miss_resource,
            hit_table,
            hit_resource,
            acceleration_structure,
            acceleration_structure_desc_handle,
            normals,
            normals_desc_handle,
            indices,
            indices_desc_handle,
            constant_buffer,
            width: window.width(),
            height: window.height(),
        }
    }
}

impl Scene for Ray {
    fn resize(&mut self, d3d12: &d3d12::Context, width: u32, height:u32) {
        self.width = width;
        self.height = height;

        self.uav = d3d12.create_resource(&d3d12::ResourceDesc::uav2d(
                d3d12::DXGI_FORMAT_R8G8B8A8_UNORM, width, height),
                d3d12::D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
                d3d12::D3D12_HEAP_TYPE_DEFAULT)
            .expect("Failed to create uav resource");

        d3d12.create_unordered_access_view(
            d3d12::D3D12_UAV_DIMENSION_TEXTURE2D, &self.uav, self.uav_desc_handle);
    }

    fn render(&mut self, d3d12: &d3d12::Context, frame: &d3d12::Frame, 
              frame_index: u32, constants: &SceneConstants) {

        let command_list = d3d12.create_graphics_command_list(frame)
            .expect("Failed to create command list");

        let before_barriers = [
            d3d12::ResourceBarrier::transition(
                &self.uav, 
                d3d12::D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
                d3d12::D3D12_RESOURCE_STATE_COPY_SOURCE),

                d3d12::ResourceBarrier::transition(
                    frame.render_target_resource.as_ref().unwrap(), 
                    d3d12::D3D12_RESOURCE_STATE_PRESENT, 
                    d3d12::D3D12_RESOURCE_STATE_COPY_DEST),
        ];

        let after_barriers = [
            d3d12::ResourceBarrier::transition(
                &self.uav, 
                d3d12::D3D12_RESOURCE_STATE_COPY_SOURCE,
                d3d12::D3D12_RESOURCE_STATE_UNORDERED_ACCESS),

                d3d12::ResourceBarrier::transition(
                    frame.render_target_resource.as_ref().unwrap(), 
                    d3d12::D3D12_RESOURCE_STATE_COPY_DEST, 
                    d3d12::D3D12_RESOURCE_STATE_RENDER_TARGET),
        ];

        unsafe {
            let ray_desc = d3d12::D3D12_DISPATCH_RAYS_DESC {
                Width: self.width,
                Height: self.height,
                Depth: 1,
                RayGenerationShaderRecord: self.raygen_table,
                MissShaderTable: self.miss_table,
                HitGroupTable: self.hit_table,
                ..Default::default()
            };

            let constant_data = core::slice::from_raw_parts(
                    constants as *const _ as *const u8, 
                    size_of_val(constants));

            self.constant_buffer.write(frame_index, constant_data)
                .expect("Failed to write constants");

            command_list.SetComputeRootSignature(&self.rs);
            command_list.SetDescriptorHeaps(
                &[Some(d3d12.csu_descriptor_heap.heap.clone())]);
            command_list.SetComputeRootDescriptorTable(
                0, d3d12.csu_descriptor_heap.heap
                .GetGPUDescriptorHandleForHeapStart());
            command_list.SetComputeRootConstantBufferView(1, 
                self.constant_buffer.get_gpu_virtual_address(frame_index));

            command_list.SetPipelineState1(&self.state_object);
            command_list.DispatchRays(&ray_desc); 
            command_list.ResourceBarrier(before_barriers.as_slice());
            command_list.CopyResource(frame.render_target_resource.as_ref()
                                      .unwrap(), 
                                      &self.uav);
            command_list.ResourceBarrier(after_barriers.as_slice());

            command_list.Close().expect("Failed to closa command list");

            d3d12::drop_barriers(before_barriers);
            d3d12::drop_barriers(after_barriers);
        }

        d3d12.execute_command_lists(&[Some(command_list.into())]);
    }
}

#[allow(dead_code)]
struct ClearState {
    rs: d3d12::ID3D12RootSignature,
    pso: d3d12::ID3D12PipelineState,
    uav: d3d12::ID3D12Resource,
    width: u32,
    height: u32,

    uav_desc_handle: d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
}

#[allow(dead_code)]
impl ClearState {
    fn init(window: &win32::Window, d3d12: &d3d12::Context, _mesh: &Mesh) 
        -> Self {
        let rs = d3d12.create_root_signature_from_shader(&shaders::CLEAR_CS)
            .expect("Failed to create root signature");

        let pso = d3d12.create_compute_pipelinestate(&shaders::CLEAR_CS, &rs)
            .expect("Failed to initialize pipeline state");

        let uav = d3d12.create_resource(&d3d12::ResourceDesc::uav2d(
                d3d12::DXGI_FORMAT_R8G8B8A8_UNORM, 
                window.width(), window.height()),
                d3d12::D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
                d3d12::D3D12_HEAP_TYPE_DEFAULT)
            .expect("Failed to create uav resource");

        let uav_desc_handle = d3d12.alloc_csu_descriptor()
            .expect("Failed to alloc csu descriptor");

        d3d12.create_unordered_access_view(
            d3d12::D3D12_UAV_DIMENSION_TEXTURE2D, &uav, uav_desc_handle);

        Self { 
            rs, 
            pso, 
            uav, 
            uav_desc_handle, 
            width: window.width(),
            height: window.height(),
        }
    }
}

impl Scene for ClearState {
    fn resize(&mut self, _d3d12: &d3d12::Context, width: u32, height: u32) {
        self.width  = width;
        self.height = height;
    }

    fn render(&mut self, d3d12: &d3d12::Context, frame: &d3d12::Frame, 
              _frame_index: u32, _constants: &SceneConstants) {

        let command_list = d3d12.create_graphics_command_list(frame)
            .expect("Failed to create command list");

        let before_barriers = [
            d3d12::ResourceBarrier::transition(
                &self.uav, 
                d3d12::D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
                d3d12::D3D12_RESOURCE_STATE_COPY_SOURCE),

                d3d12::ResourceBarrier::transition(
                    frame.render_target_resource.as_ref().unwrap(), 
                    d3d12::D3D12_RESOURCE_STATE_PRESENT, 
                    d3d12::D3D12_RESOURCE_STATE_COPY_DEST),
        ];

        let after_barriers = [
            d3d12::ResourceBarrier::transition(
                &self.uav, 
                d3d12::D3D12_RESOURCE_STATE_COPY_SOURCE,
                d3d12::D3D12_RESOURCE_STATE_UNORDERED_ACCESS),

            d3d12::ResourceBarrier::transition(
                frame.render_target_resource.as_ref().unwrap(), 
                d3d12::D3D12_RESOURCE_STATE_COPY_DEST, 
                d3d12::D3D12_RESOURCE_STATE_PRESENT),
        ];

        unsafe {
            command_list.SetComputeRootSignature(&self.rs);
            command_list.SetDescriptorHeaps(
                &[Some(d3d12.csu_descriptor_heap.heap.clone())]);
            command_list.SetComputeRootDescriptorTable(
                0, d3d12.csu_descriptor_heap.heap
                .GetGPUDescriptorHandleForHeapStart());

            command_list.SetPipelineState(&self.pso);
            command_list.Dispatch(self.width, self.height, 1); 
            command_list.ResourceBarrier(before_barriers.as_slice());
            command_list.CopyResource(&frame.render_target_resource, &self.uav);
            command_list.ResourceBarrier(after_barriers.as_slice());

            command_list.Close().expect("Failed to close command list");

            d3d12::drop_barriers(before_barriers);
            d3d12::drop_barriers(after_barriers);
        }

        d3d12.execute_command_lists(&[Some(command_list.into())]);
    }
}


fn main() {
    use std::path::Path;

    /*
    let mut asset_file = asset::AssetFile::from_file(
        &Path::new("res").join("models.asset"))
        .expect("Asset file not found");

    let mesh = asset_file.load_mesh("dragon").expect("Asset not found");
    */
    
    let scene = gltf::import_file(&Path::new("res").join("Bistro.glb"))
        .expect("GLB file not found");

    let mesh = &scene.meshes[16];

    println!("{}, {}, {}, {}, {:?}", scene.meshes.len(),
        mesh.positions.len(), mesh.normals.len(), mesh.indices.len(),
        mesh.transform,
        );

    /*
    let mesh = Mesh {
        positions: vec![
            Vec3::new(-0.5, -0.5, 0.5),
            Vec3::new( 0.5, -0.5, 0.5),
            Vec3::new( 0.0,  0.5, 0.5),
        ],
        normals: Vec::from([Vec3::new(0., 0., 1.0); 3]),
        indices: vec![0, 1, 2],
    };
    */

    let transform = mesh.transform *
        Mat4::rotation(Vec3::new(1.,0.,0.), core::f32::consts::PI * 0.5);
    /*
        Mat4::translation(Vec3::new(0., 0., -5.0)) *
        //Mat4::rotation(Vec3::new(0.,0.,1.), core::f32::consts::PI * 0.5) * 
        Mat4::rotation(Vec3::new(1.,0.,0.), core::f32::consts::PI * 0.5);
    */
    
    /*
    println!("{} positions, {} indices, max index: {}",
             mesh.positions.len(), mesh.indices.len(),
             mesh.indices.iter().fold(0, |b, x| b.max(*x)));
    */
    

    //let mut window = win32::create_window("Rust window", 1280, 720)
    let mut window = win32::create_window("Rust window", 1280, 720)
        .expect("Failed to create window");

    let mut d3d12 = d3d12::Context::init(&window)
        .expect("Failed to initialize D3D12");


    let mut imgui = imgui::Context::create();
    imgui.set_ini_filename(None);
    
    let mut imgui_impl = imgui_impl::Backend::init(&d3d12, &mut imgui, 
                                                   window.width(),
                                                   window.height())
        .expect("Failed to initialize imgui backend");


    let ray = Box::new(Ray::init(&window, &d3d12, &mesh, &transform));
    let raster = Box::new(Raster::init(&window, &d3d12, &mesh));

    let mut ray_scene:  Box<dyn Scene> = ray;
    let mut raster_scene: Box<dyn Scene> = raster;

    let mut scene: &mut Box<dyn Scene> = &mut raster_scene;

    let mut constants = SceneConstants {
        camera_position: Vec3::new(0., -30.0, 0.),
        light_position: Vec3::new(0., -30.0, 0.),
        diffuse_color: Vec3::new(0., 1., 0.),
        film_dist: 1.0,
        model: transform,
        normal: transform.to_normal_matrix(),
        ..Default::default()
    };

    'main: loop {
        while let Some(event) = window.poll_events() {
            let io = imgui.io_mut();

            use win32::{Event::*, MouseButton};
            match event {
                Quit => break 'main,
                KeyPress(Some('W')) => {
                    constants.camera_position.y += 1.0
                }
                KeyPress(Some('A')) => {
                    constants.camera_position.x += 1.0
                }
                KeyPress(Some('S')) => {
                    constants.camera_position.y -= 1.0
                }
                KeyPress(Some('D')) => {
                    constants.camera_position.x -= 1.0
                }
                KeyPress(Some('Q')) => {
                    constants.camera_position.z -= 1.0
                }
                KeyPress(Some('E')) => {
                    constants.camera_position.z += 1.0
                }
                KeyPress(Some('R')) => {
                    scene = &mut raster_scene;
                }
                KeyPress(Some('T')) => {
                    scene = &mut ray_scene;
                }

                MouseMove(x, y) => {
                    io.mouse_pos[0] = x as f32;
                    io.mouse_pos[1] = y as f32;
                }

                MouseLeave => {
                    io.mouse_pos[0] = f32::MAX;
                    io.mouse_pos[1] = f32::MAX;
                }

                MouseWheel(hor, vert) => {
                    io.mouse_wheel_h = hor;
                    io.mouse_wheel = vert;
                }

                MousePress(MouseButton::Left)   => io.mouse_down[0] = true,
                MousePress(MouseButton::Right)  => io.mouse_down[1] = true,
                MousePress(MouseButton::Middle) => io.mouse_down[2] = true,

                MouseRelease(MouseButton::Left)   => io.mouse_down[0] = false,
                MouseRelease(MouseButton::Right)  => io.mouse_down[1] = false,
                MouseRelease(MouseButton::Middle) => io.mouse_down[2] = false,

                Focus(in_focus) => io.app_focus_lost = !in_focus,

                Minimized => io.display_size = [0., 0.],
                Resize(width, height) => {
                    io.display_size = [width as f32, height as f32];
                    d3d12.resize(width, height).expect("Failed to resize");
                    scene.resize(&d3d12, width, height);
                },

                _ => println!("{:?}", event),
            }
        }


        {
            let (frame, index) = d3d12.begin_frame()
                .expect("Failed to begin frame");

            let aspect_ratio = window.height() as f32 / window.width() as f32;
            let near = 0.01;
            let far = 100.0;
            let fov = 2. * (1. / (constants.film_dist * 2.)).atan();

            constants.view = mat::lh::look_at(constants.camera_position, 
                                              constants.camera_position + 
                                              Vec3::new(0., 1., 0.),
                                              Vec3::new(0., 0., 1.));
            constants.projection = mat::lh::zo::perspective(
                fov, near, far, aspect_ratio);

            scene.render(&d3d12, &frame, index, &constants);

            
            imgui_impl.frame(&mut imgui, &d3d12, &frame, index, |ui| {
                ui.window("Hello world")
                    .size([300.0, 150.0], imgui::Condition::FirstUseEver)
                    .build(|| {
                        imgui::Drag::new("Film distance").range(0.1, 2.0)
                            .speed(0.01).build(&ui, &mut constants.film_dist);
                    });
                }
            );

            d3d12.end_frame(frame).expect("Failed to end frame");
        }

    }

    //Wait for the last frame we issued before shutting down
    d3d12.wait_idle();


}
