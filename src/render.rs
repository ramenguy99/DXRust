use core::ptr::{null, null_mut};
use core::mem::{size_of, size_of_val};

use bytemuck::cast_slice;
use math::vec::{Vec2, Vec3, Vec4};

use scene::{Mesh, Scene};

use crate::d3d12::{self, ResourceDesc, ResourceBarrier};
use crate::shaders::{self, RayMeshInstance, RasterMeshInstance};
pub use crate::shaders::Constants as SceneConstants;
use crate::win32;

const MAX_SAMPLES: u32 = 1024;

pub trait Pipeline {
    fn resize(&mut self, d3d12: &d3d12::Context, width: u32, height:u32);

    fn render(&mut self, d3d12: &d3d12::Context, frame: &d3d12::Frame,
              _frame_index: u32, constants: &mut SceneConstants, reset: bool);
}

pub struct Raster {
    width: u32,
    height: u32,

    depth: d3d12::ID3D12Resource,
    depth_descriptor: d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
    #[allow(dead_code)]
    depth_heap: d3d12::DescriptorHeap,

    pso: d3d12::ID3D12PipelineState,
    rs: d3d12::ID3D12RootSignature,
    cs: d3d12::ID3D12CommandSignature,

    commands: d3d12::ID3D12Resource,

    positions: d3d12::ID3D12Resource,
    normals: d3d12::ID3D12Resource,
    uvs: d3d12::ID3D12Resource,
    indices: d3d12::ID3D12Resource,

    meshes_count: usize,
    vertices_count: usize,
    indices_count: usize,

    mesh_constants: d3d12::ID3D12Resource,

    constant_buffer: d3d12::PerFrameConstantBuffer,
}

#[repr(C)]
struct DrawArgs {
    index: u32,
    args: d3d12::D3D12_DRAW_INDEXED_ARGUMENTS,
}

impl Raster {
    pub fn init(window: &win32::Window, d3d12: &d3d12::Context,
            scene: &Scene) -> Self {

        let rs =
            d3d12.create_root_signature_from_shader(&shaders::MESH_VS)
                .expect("Failed to create root signature");

        let argument_descs = [
            d3d12::D3D12_INDIRECT_ARGUMENT_DESC {
                Type: d3d12::D3D12_INDIRECT_ARGUMENT_TYPE_CONSTANT,
                Anonymous: d3d12::D3D12_INDIRECT_ARGUMENT_DESC_0 {
                    Constant: d3d12::D3D12_INDIRECT_ARGUMENT_DESC_0_1 {
                        RootParameterIndex: 1,
                        DestOffsetIn32BitValues: 0,
                        Num32BitValuesToSet: 1,
                    },
                },
            },
            d3d12::D3D12_INDIRECT_ARGUMENT_DESC {
                Type: d3d12::D3D12_INDIRECT_ARGUMENT_TYPE_DRAW_INDEXED,
                ..Default::default()
            },
        ];

        let cs = d3d12.create_command_signature(&rs, size_of::<DrawArgs>()
                                                as u32, &argument_descs)
            .expect("Failed to create command signature");

        let mut commands_buf: Vec<DrawArgs> = Vec::new();
        let mut positions_buf: Vec<Vec3> = Vec::new();
        let mut normals_buf: Vec<Vec3> = Vec::new();
        let mut uvs_buf: Vec<Vec2> = Vec::new();
        let mut indices_buf: Vec<u32> = Vec::new();
        let mut mesh_constants_buf: Vec<RasterMeshInstance> = Vec::new();

        let mut current_vertex: u32 = 0;
        let mut current_index:  u32 = 0;

        for (i, m) in scene.meshes.iter().enumerate() {
            positions_buf.extend_from_slice(&m.positions);
            normals_buf  .extend_from_slice(&m.normals);
            uvs_buf      .extend_from_slice(&m.uvs);
            indices_buf  .extend_from_slice(&m.indices);

            let mut mesh_instance = RasterMeshInstance {
                transform: m.transform,
                ..Default::default()
            };

            macro_rules! material {
                ($m: ident, $index: ident, $value: ident, $default: expr) => {
                    match m.material.$m {
                        scene::MaterialParameter::Texture(v) => {
                            mesh_instance.$index = v;
                        },
                        scene::MaterialParameter::Vec4(v) => {
                            mesh_instance.$index  = u32::MAX;
                            mesh_instance.$value = v;
                        }
                        _ => {
                            mesh_instance.$index  = u32::MAX;
                            mesh_instance.$value = $default;
                        }
                    };
                };
            }

            material!(base_color, albedo_index,   albedo_value,   Vec4::new(1.0, 0.0, 1.0, 1.0));
            material!(specular,   specular_index, specular_value, Vec4::new(0.0, 1.0, 0.0, 0.0));
            material!(emissive,   emissive_index, emissive_value, Vec4::new(0.0, 0.0, 0.0, 0.0));

            mesh_constants_buf.push(mesh_instance);

            commands_buf.push(DrawArgs {
                index: i as u32,
                args: d3d12::D3D12_DRAW_INDEXED_ARGUMENTS {
                    IndexCountPerInstance: m.indices.len() as u32,
                    InstanceCount: 1,
                    StartIndexLocation: current_index,
                    BaseVertexLocation: current_vertex as i32,
                    StartInstanceLocation: 0,
                },
            });

            current_vertex = current_vertex.checked_add(m.positions.len() as u32)
                .expect("Overflow");
            current_index = current_index.checked_add(m.indices.len() as u32)
                .expect("Overflow");
        }

        assert!(current_vertex == positions_buf.len() as u32);
        assert!(current_index == indices_buf.len() as u32);

        let positions = d3d12.upload_buffer_sync( unsafe {
            core::slice::from_raw_parts(positions_buf.as_ptr() as *const u8,
                                        positions_buf.len() * size_of::<Vec3>())
            }, d3d12::D3D12_RESOURCE_STATE_GENERIC_READ)
            .expect("Failed to upload positions");

        let normals = d3d12.upload_buffer_sync( unsafe {
            core::slice::from_raw_parts(normals_buf.as_ptr() as *const u8,
                                        normals_buf.len() * size_of::<Vec3>())
            }, d3d12::D3D12_RESOURCE_STATE_GENERIC_READ)
            .expect("Failed to upload normals");

        let uvs = d3d12.upload_buffer_sync( unsafe {
            core::slice::from_raw_parts(uvs_buf.as_ptr() as *const u8,
                                        uvs_buf.len() * size_of::<Vec2>())
            }, d3d12::D3D12_RESOURCE_STATE_GENERIC_READ)
            .expect("Failed to upload normals");

        let indices = d3d12.upload_buffer_sync( unsafe {
            core::slice::from_raw_parts(indices_buf.as_ptr() as *const u8,
                                        indices_buf.len() * size_of::<u32>())
            }, d3d12::D3D12_RESOURCE_STATE_GENERIC_READ)
            .expect("Failed to upload indices");

        let mesh_constants = d3d12.upload_buffer_sync( unsafe {
            core::slice::from_raw_parts(mesh_constants_buf.as_ptr() as *const u8,
                                        mesh_constants_buf.len() *
                                        size_of::<RasterMeshInstance>())
            }, d3d12::D3D12_RESOURCE_STATE_GENERIC_READ)
            .expect("Failed to upload mesh constants");

        let mesh_desc = d3d12.alloc_csu_descriptor()
            .expect("Failed to alloc mesh constants descriptor");
        d3d12.create_shader_resource_view_structured_buffer(&mesh_constants,
                                            0, mesh_constants_buf.len() as u32,
                                            size_of::<RasterMeshInstance>() as u32,
                                            mesh_desc);


        let commands = d3d12.upload_buffer_sync( unsafe {
            core::slice::from_raw_parts(commands_buf.as_ptr() as *const u8,
                                        commands_buf.len() *
                                        size_of::<DrawArgs>())
            }, d3d12::D3D12_RESOURCE_STATE_GENERIC_READ)
            .expect("Failed to upload draw arguments");

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
                    d3d12::D3D12_INPUT_ELEMENT_DESC {
                        SemanticName: windows::core::PCSTR(b"TEXCOORD\0".as_ptr()),
                        SemanticIndex: 0,
                        Format: d3d12::DXGI_FORMAT_R32G32_FLOAT,
                        InputSlot: 2,
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
                    front_ccw: true,
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
            depth,
            depth_heap,
            depth_descriptor,
            rs,
            cs,
            commands,
            positions,
            normals,
            indices,
            uvs,
            vertices_count: positions_buf.len(),
            indices_count: indices_buf.len(),
            meshes_count: scene.meshes.len(),
            mesh_constants,
            constant_buffer,
            pso,
        }
    }
}

impl Pipeline for Raster {
    fn resize(&mut self, d3d12: &d3d12::Context, width: u32, height:u32) {
        self.width = width;
        self.height = height;

        self.depth = d3d12.create_depth_buffer(width, height)
            .expect("Failed to resize depth buffer");
        d3d12.create_depth_stencil_view(&self.depth, self.depth_descriptor);
    }

    fn render(&mut self, d3d12: &d3d12::Context, frame: &d3d12::Frame,
              frame_index: u32, constants: &mut SceneConstants, _reset: bool) {

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
            d3d12::D3D12_VERTEX_BUFFER_VIEW {
                BufferLocation: unsafe { self.uvs.GetGPUVirtualAddress() },
                SizeInBytes:    (self.vertices_count * size_of::<Vec2>()) as u32,
                StrideInBytes:  size_of::<Vec2>() as u32,
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
            command_list.SetDescriptorHeaps(&[Some(d3d12.csu_descriptor_heap.heap.clone())]);
            command_list.SetGraphicsRootSignature(&self.rs);
            command_list.SetGraphicsRootConstantBufferView(0,
                self.constant_buffer.get_gpu_virtual_address(frame_index));
            command_list.SetGraphicsRootShaderResourceView(2,
                self.mesh_constants.GetGPUVirtualAddress());
            command_list.SetGraphicsRootDescriptorTable(3,
                d3d12.csu_descriptor_heap.heap.GetGPUDescriptorHandleForHeapStart());

            command_list.IASetVertexBuffers(0, &vbv);
            command_list.IASetIndexBuffer(&ibv);
            command_list.IASetPrimitiveTopology(
                d3d12::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);

            command_list.RSSetViewports(&viewports);
            command_list.RSSetScissorRects(&scissor_rects);

            command_list.OMSetRenderTargets(1, &frame.render_target_descriptor,
                                            windows::Win32::Foundation::BOOL(0),
                                            &self.depth_descriptor);

            command_list.ExecuteIndirect(&self.cs, self.meshes_count as u32,
                                         &self.commands, 0, None, 0);


            command_list.Close().expect("Failed to close command list");
            d3d12::drop_barriers(barriers);
        }

        d3d12.execute_command_lists(&[Some(command_list.into())]);
    }
}

#[allow(dead_code)]
pub struct Ray {
    rs:                             d3d12::ID3D12RootSignature,
    state_object:                   d3d12::ID3D12StateObject,
    width:                          u32,
    height:                         u32,
    raygen_resource:                d3d12::ID3D12Resource,
    raygen_table:                   d3d12::D3D12_GPU_VIRTUAL_ADDRESS_RANGE,
    miss_resource:                  d3d12::ID3D12Resource,
    miss_table:                     d3d12::D3D12_GPU_VIRTUAL_ADDRESS_RANGE_AND_STRIDE,
    hit_resource:                   d3d12::ID3D12Resource,
    hit_table:                      d3d12::D3D12_GPU_VIRTUAL_ADDRESS_RANGE_AND_STRIDE,
    tlas:                           d3d12::ID3D12Resource,
    tlas_desc_handle:               d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
    blas:                           d3d12::ID3D12Resource,
    instances:                      d3d12::ID3D12Resource,
    normals:                        d3d12::ID3D12Resource,
    normals_desc_handle:            d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
    tangents:                       d3d12::ID3D12Resource,
    tangents_desc_handle:           d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
    uvs:                            d3d12::ID3D12Resource,
    uvs_desc_handle:                d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
    indices:                        d3d12::ID3D12Resource,
    indices_desc_handle:            d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
    uav:                            d3d12::ID3D12Resource,
    uav_desc_handle:                d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
    constant_buffer:                d3d12::PerFrameConstantBuffer,
    mesh_instances:                 d3d12::ID3D12Resource,
    mesh_instances_desc_handle:     d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,

    postprocess_rs:                 d3d12::ID3D12RootSignature,
    postprocess_pso:                d3d12::ID3D12PipelineState,
    postprocess_buffer:             d3d12::ID3D12Resource,
    postprocess_input_desc_handle:  d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,
    postprocess_output_desc_handle: d3d12::D3D12_CPU_DESCRIPTOR_HANDLE,

    samples: u32,
    max_samples: u32,
}

impl Ray {
    pub fn init(window: &win32::Window, d3d12: &d3d12::Context,
            scene: &Scene) -> Self {
        let rs = d3d12.create_root_signature_from_shader(&shaders::RAY_LIB)
            .expect("Failed to create root signature");

        let state_object = d3d12.create_dxr_state_object(&shaders::RAY_LIB)
            .expect("Failed to create state object");

        let uav = d3d12.create_resource(&d3d12::ResourceDesc::uav2d(
                d3d12::DXGI_FORMAT_R32G32B32A32_FLOAT,
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
        let mut positions_buf: Vec<Vec3> = Vec::new();
        let mut normals_buf: Vec<Vec3> = Vec::new();
        let mut tangents_buf: Vec<Vec3> = Vec::new();
        let mut uvs_buf: Vec<Vec2> = Vec::new();
        let mut indices_buf: Vec<u32> = Vec::new();
        let mut mesh_instances_buf: Vec<RayMeshInstance> = Vec::new();

        let mut scratch_size: u64 = 0;
        let mut blas_size:    u64 = 0;

        let mut geom_descs =
            vec![d3d12::D3D12_RAYTRACING_GEOMETRY_DESC::default(); scene.meshes.len()];

        let mut inputs =
            vec![d3d12::D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_INPUTS::default(); scene.meshes.len()];

        let mut info_sizes = vec![(0u64, 0u64); scene.meshes.len()];


        let mut current_vertex: u32 = 0;
        let mut current_index:  u32 = 0;

        for (i, m) in scene.meshes.iter().enumerate() {
            positions_buf.extend_from_slice(&m.positions);
            normals_buf  .extend_from_slice(&m.normals  );
            tangents_buf .extend_from_slice(&m.tangents );
            uvs_buf      .extend_from_slice(&m.uvs      );
            indices_buf  .extend_from_slice(&m.indices  );

            let mut mesh_instance = RayMeshInstance {
                vertex_offset: current_vertex,
                index_offset: current_index,
                ..Default::default()
            };

            macro_rules! material {
                ($m: ident, $index: ident, $value: ident, $default: expr) => {
                    match m.material.$m {
                        scene::MaterialParameter::Texture(v) => {
                            mesh_instance.$index = v;
                        },
                        scene::MaterialParameter::Vec4(v) => {
                            mesh_instance.$index  = u32::MAX;
                            mesh_instance.$value = v;
                        }
                        _ => {
                            mesh_instance.$index  = u32::MAX;
                            mesh_instance.$value = $default;
                        }
                    };
                };
            }

            material!(base_color, albedo_index,   albedo_value,   Vec4::new(1.0, 0.0, 1.0, 1.0));
            material!(specular,   specular_index, specular_value, Vec4::new(0.0, 1.0, 0.0, 0.0));
            material!(emissive,   emissive_index, emissive_value, Vec4::new(0.0, 0.0, 0.0, 0.0));
            if let scene::MaterialParameter::Texture(v) = m.material.normal {
                mesh_instance.normal_index = v;
            } else {
                mesh_instance.normal_index = u32::MAX;
            }

            mesh_instances_buf.push(mesh_instance);

            current_vertex = current_vertex.checked_add(m.positions.len() as u32)
                .expect("Overflow");
            current_index = current_index.checked_add(m.indices.len() as u32)
                .expect("Overflow");

            geom_descs[i] = d3d12::D3D12_RAYTRACING_GEOMETRY_DESC {
                Type: d3d12::D3D12_RAYTRACING_GEOMETRY_TYPE_TRIANGLES,
                Flags: d3d12::D3D12_RAYTRACING_GEOMETRY_FLAG_OPAQUE,
                Anonymous: d3d12::D3D12_RAYTRACING_GEOMETRY_DESC_0 {
                    Triangles: d3d12::D3D12_RAYTRACING_GEOMETRY_TRIANGLES_DESC {
                        VertexBuffer: d3d12::D3D12_GPU_VIRTUAL_ADDRESS_AND_STRIDE {
                            StartAddress: 0,
                            StrideInBytes: 12,
                        },
                        VertexFormat: d3d12::DXGI_FORMAT_R32G32B32_FLOAT,
                        VertexCount: m.positions.len() as u32,
                        IndexFormat: d3d12::DXGI_FORMAT_R32_UINT,
                        IndexCount: m.indices.len() as u32,
                        IndexBuffer: 0,
                        ..Default::default()
                    }
                },
            };

            inputs[i] = d3d12::D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_INPUTS {
                Type: d3d12::D3D12_RAYTRACING_ACCELERATION_STRUCTURE_TYPE_BOTTOM_LEVEL,
                Flags: d3d12::D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_NONE,
                DescsLayout: d3d12::D3D12_ELEMENTS_LAYOUT_ARRAY,
                NumDescs: 1,
                Anonymous: d3d12::D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_INPUTS_0 {
                    pGeometryDescs: &geom_descs[i],
                },
            };

            let mut info = Default::default();
            unsafe {
                d3d12.device.GetRaytracingAccelerationStructurePrebuildInfo(
                    &inputs[i], &mut info);
            }

            scratch_size += (info.ScratchDataSizeInBytes + 0xFF) & !0xFF;
            blas_size += (info.ResultDataMaxSizeInBytes + 0xFF) & !0xFF;

            info_sizes[i] = (info.ScratchDataSizeInBytes,
                             info.ResultDataMaxSizeInBytes);
        }

        assert!(current_vertex == positions_buf.len() as u32);
        assert!(current_index == indices_buf.len() as u32);

        let positions = d3d12.upload_buffer_sync(cast_slice(&positions_buf),
            d3d12::D3D12_RESOURCE_STATE_GENERIC_READ)
            .expect("Failed to upload positions");

        let normals = d3d12.upload_buffer_sync(cast_slice(&normals_buf),
            d3d12::D3D12_RESOURCE_STATE_GENERIC_READ)
            .expect("Failed to upload normals");

        let tangents = d3d12.upload_buffer_sync(cast_slice(&tangents_buf),
            d3d12::D3D12_RESOURCE_STATE_GENERIC_READ)
            .expect("Failed to upload tangents");

        let uvs = d3d12.upload_buffer_sync(cast_slice(&uvs_buf),
            d3d12::D3D12_RESOURCE_STATE_GENERIC_READ)
            .expect("Failed to upload uvs");

        let indices = d3d12.upload_buffer_sync(cast_slice(&indices_buf),
            d3d12::D3D12_RESOURCE_STATE_GENERIC_READ)
            .expect("Failed to upload indices");

        let mesh_instances = d3d12.upload_buffer_sync(
            cast_slice(&mesh_instances_buf),
            d3d12::D3D12_RESOURCE_STATE_GENERIC_READ)
            .expect("Failed to upload mesh instances");

        let blas_scratch = d3d12.create_resource(
            &d3d12::ResourceDesc::uav_buffer(scratch_size as usize),
            d3d12::D3D12_RESOURCE_STATE_COMMON,
            d3d12::D3D12_HEAP_TYPE_DEFAULT).expect("Failed to alloc blas scratch");

        let blas = d3d12.create_resource(
             &d3d12::ResourceDesc::uav_buffer(blas_size as usize),
             d3d12::D3D12_RESOURCE_STATE_RAYTRACING_ACCELERATION_STRUCTURE,
             d3d12::D3D12_HEAP_TYPE_DEFAULT).expect("Failed to alloc BLAS");

        let mut scratch_pointer = unsafe {
            blas_scratch.GetGPUVirtualAddress()
        };
        let scratch_end = scratch_pointer + scratch_size;
        let mut blas_pointer = unsafe {
            blas.GetGPUVirtualAddress()
        };
        let blas_end = blas_pointer + blas_size;
        let mut vertex_gpu_pointer = unsafe {
            positions.GetGPUVirtualAddress()
        };
        let mut index_gpu_pointer = unsafe {
            indices.GetGPUVirtualAddress()
        };

        for (i, m) in scene.meshes.iter().enumerate() {
            unsafe {
                let tris = &mut geom_descs[i].Anonymous.Triangles;
                tris.VertexBuffer.StartAddress = vertex_gpu_pointer;
                tris.IndexBuffer = index_gpu_pointer;

                vertex_gpu_pointer += (m.positions.len() * size_of::<Vec3>()) as u64;
                index_gpu_pointer += (m.indices.len() * size_of::<u32>()) as u64;

                let as_desc =
                    d3d12::D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_DESC {
                        Inputs: inputs[i],
                        DestAccelerationStructureData: blas_pointer,
                        ScratchAccelerationStructureData: scratch_pointer,
                        SourceAccelerationStructureData: 0,
                };

                scratch_pointer += (info_sizes[i].0 + 0xFF) & !0xFF;
                blas_pointer    += (info_sizes[i].1 + 0xFF) & !0xFF;

                assert!(scratch_pointer <= scratch_end);
                assert!(blas_pointer    <= blas_end   );

                d3d12.sync_command_list
                    .BuildRaytracingAccelerationStructure(&as_desc, &[]);
            }
        }
        assert!(scratch_pointer == scratch_end);
        assert!(blas_pointer    == blas_end   );

        let barriers = [d3d12::ResourceBarrier::uav(&blas)];
        unsafe {
            d3d12.sync_command_list.ResourceBarrier(&barriers);
            d3d12::drop_barriers(barriers);
        }

        let mut inputs = d3d12::D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_INPUTS {
            Type: d3d12::D3D12_RAYTRACING_ACCELERATION_STRUCTURE_TYPE_TOP_LEVEL,
            DescsLayout: d3d12::D3D12_ELEMENTS_LAYOUT_ARRAY,
            Flags: d3d12::D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_NONE,
            NumDescs: scene.meshes.len() as u32,
            ..Default::default()
        };

        let mut info = Default::default();

        unsafe {
            d3d12.device.GetRaytracingAccelerationStructurePrebuildInfo(
                &inputs, &mut info);
        }

        let tlas_scratch = d3d12.create_resource(
            &d3d12::ResourceDesc::uav_buffer(info.ScratchDataSizeInBytes
                                      as usize),
            d3d12::D3D12_RESOURCE_STATE_COMMON,
            d3d12::D3D12_HEAP_TYPE_DEFAULT).expect("Failed to alloc tlas scratch");

        let tlas = d3d12.create_resource(
            &d3d12::ResourceDesc::uav_buffer(info.ResultDataMaxSizeInBytes
                                      as usize),
            d3d12::D3D12_RESOURCE_STATE_RAYTRACING_ACCELERATION_STRUCTURE,
            d3d12::D3D12_HEAP_TYPE_DEFAULT).expect("Failed to alloc tlas");

        let instances = d3d12.create_resource(
            &d3d12::ResourceDesc::buffer(
                size_of::<d3d12::D3D12_RAYTRACING_INSTANCE_DESC>() *
                scene.meshes.len()),
            d3d12::D3D12_RESOURCE_STATE_GENERIC_READ,
            d3d12::D3D12_HEAP_TYPE_UPLOAD).expect("Failed to alloc instances");

        let instances_descs: &mut[d3d12::D3D12_RAYTRACING_INSTANCE_DESC] = unsafe {
            let mut ptr = null_mut();
            instances.Map(0, null(), &mut ptr)
                .expect("Failed to map instance descriptors");
            core::slice::from_raw_parts_mut(ptr as *mut d3d12::D3D12_RAYTRACING_INSTANCE_DESC,
                                            scene.meshes.len())
        };

        let mut blas_pointer = unsafe { blas.GetGPUVirtualAddress() };
        let blas_end = blas_pointer + blas_size;

        for (i, m) in scene.meshes.iter().enumerate() {
            let t = m.transform;
            instances_descs[i] = d3d12::D3D12_RAYTRACING_INSTANCE_DESC {
                Transform: [
                    t.e[0][0], t.e[1][0], t.e[2][0], t.e[3][0],
                    t.e[0][1], t.e[1][1], t.e[2][1], t.e[3][1],
                    t.e[0][2], t.e[1][2], t.e[2][2], t.e[3][2],
                ],
                _bitfield1: i as u32 | (0xFF << 24),
                _bitfield2: 0 | (d3d12::D3D12_RAYTRACING_INSTANCE_FLAG_NONE.0 << 24),
                AccelerationStructure: blas_pointer,
            };

            blas_pointer += (info_sizes[i].1 + 0xFF) & !0xFF;
            assert!(blas_pointer <= blas_end);
        }

        assert!(blas_pointer == blas_end);

        // Unmap instance descs
        core::mem::drop(instances_descs);
        unsafe { instances.Unmap(0, null()) };

        unsafe {
            inputs.Anonymous.InstanceDescs = instances.GetGPUVirtualAddress();
        }

        let tlas_desc = unsafe {
            d3d12::D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_DESC {
                Inputs: inputs,
                DestAccelerationStructureData: tlas.GetGPUVirtualAddress(),
                ScratchAccelerationStructureData: tlas_scratch.GetGPUVirtualAddress(),
                SourceAccelerationStructureData: 0,
            }
        };

        unsafe {
            d3d12.sync_command_list.BuildRaytracingAccelerationStructure(
                &tlas_desc, &[]);
        }

        d3d12.wait_sync_commands();

        let tlas_desc_handle = d3d12.alloc_csu_descriptor()
            .expect("failed to alloc csu descriptor for as");

        d3d12.create_shader_resource_view_as(&tlas, tlas_desc_handle);

        let indices_desc_handle = d3d12.alloc_csu_descriptor()
            .expect("Failed to alloc csu descriptor for indices");

        d3d12.create_shader_resource_view_buffer(&indices,
                                                 d3d12::DXGI_FORMAT_R32_UINT,
                                                 0, indices_buf.len() as u32,
                                                 indices_desc_handle);

        let normals_desc_handle = d3d12.alloc_csu_descriptor()
            .expect("Failed to alloc csu descriptor for normals");

        d3d12.create_shader_resource_view_buffer(&normals,
                                                 d3d12::DXGI_FORMAT_R32G32B32_FLOAT,
                                                 0, normals_buf.len() as u32,
                                                 normals_desc_handle);

        let tangents_desc_handle = d3d12.alloc_csu_descriptor()
            .expect("Failed to alloc csu descriptor for tangents");

        d3d12.create_shader_resource_view_buffer(&tangents,
                                                 d3d12::DXGI_FORMAT_R32G32B32_FLOAT,
                                                 0, tangents_buf.len() as u32,
                                                 tangents_desc_handle);

        let uvs_desc_handle = d3d12.alloc_csu_descriptor()
        .expect("Failed to alloc csu descriptor for uvs");

        d3d12.create_shader_resource_view_buffer(&uvs,
                                                 d3d12::DXGI_FORMAT_R32G32_FLOAT,
                                                 0, uvs_buf.len() as u32,
                                                 uvs_desc_handle);

        let mesh_instances_desc_handle = d3d12.alloc_csu_descriptor()
            .expect("Failed to alloc csu descriptor for instances");
        d3d12.create_shader_resource_view_structured_buffer(&mesh_instances,
                                            0, mesh_instances_buf.len() as u32,
                                            size_of::<RayMeshInstance>() as u32,
                                            mesh_instances_desc_handle);

        let constant_buffer = d3d12.create_per_frame_constant_buffer(
            size_of::<SceneConstants>())
            .expect("Failed to alloc per frame constant buffers");

        // Post processing
        let postprocess_rs = d3d12.create_root_signature_from_shader(
            &shaders::POSTPROCESS_CS)
            .expect("Failed to create root signature");

        let postprocess_pso =
            d3d12.create_compute_pipelinestate(&shaders::POSTPROCESS_CS, &postprocess_rs)
                .expect("Failed to initialize pipeline state");

        let postprocess_buffer = d3d12.create_resource(
            &ResourceDesc::uav2d(
                d3d12::DXGI_FORMAT_R8G8B8A8_UNORM,
                window.width(),
            window.height()),
            d3d12::D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
            d3d12::D3D12_HEAP_TYPE_DEFAULT)
            .expect("Failed to create postprocess buffer");

        let postprocess_input_desc_handle = d3d12.alloc_csu_descriptor()
        .expect("Failed to alloc csu descriptor for postprocess buffer");

        d3d12.create_shader_resource_view_tex2d(
            &uav, d3d12::DXGI_FORMAT_R32G32B32A32_FLOAT,
            postprocess_input_desc_handle);

        let postprocess_output_desc_handle = d3d12.alloc_csu_descriptor()
        .expect("Failed to alloc csu descriptor for postprocess buffer");

        d3d12.create_unordered_access_view(
            d3d12::D3D12_UAV_DIMENSION_TEXTURE2D,
            &postprocess_buffer, postprocess_output_desc_handle);

        Self {
            width: window.width(),
            height: window.height(),
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
            tlas,
            tlas_desc_handle,
            blas,
            instances,
            normals,
            normals_desc_handle,
            tangents,
            tangents_desc_handle,
            uvs,
            uvs_desc_handle,
            indices,
            indices_desc_handle,
            constant_buffer,
            mesh_instances,
            mesh_instances_desc_handle,

            postprocess_rs,
            postprocess_pso,
            postprocess_buffer,
            postprocess_input_desc_handle,
            postprocess_output_desc_handle,

            samples: 0,
            max_samples: MAX_SAMPLES,
        }
    }
}

impl Pipeline for Ray {
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
              frame_index: u32, constants: &mut SceneConstants, reset: bool) {
        let mut dispatch = false;
        if reset {
            self.samples = 0;
        }
        if self.samples < self.max_samples {
            self.samples += 1;
            constants.samples = self.samples;
            dispatch = true;
        }

        let command_list = d3d12.create_graphics_command_list(frame)
            .expect("Failed to create command list");

        unsafe {
            command_list.SetDescriptorHeaps(
                &[
                    Some(d3d12.csu_descriptor_heap.heap.clone()),
                ]);

            if dispatch {
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
                command_list.SetComputeRootDescriptorTable(
                    0, d3d12.csu_descriptor_heap.heap
                    .GetGPUDescriptorHandleForHeapStart());
                command_list.SetComputeRootConstantBufferView(1,
                    self.constant_buffer.get_gpu_virtual_address(frame_index));

                command_list.SetPipelineState1(&self.state_object);
                command_list.DispatchRays(&ray_desc);
            }

            let before_barriers = [
                ResourceBarrier::transition(&self.uav,
                    d3d12::D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
                    d3d12::D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE),
            ];
            command_list.ResourceBarrier(before_barriers.as_slice());

            // Postprocess compute shader
            command_list.SetComputeRootSignature(&self.postprocess_rs);
            command_list.SetComputeRootDescriptorTable(
                0, d3d12.csu_descriptor_heap.heap
                .GetGPUDescriptorHandleForHeapStart());
            command_list.SetComputeRoot32BitConstant(1, self.samples, 0);
            command_list.SetComputeRoot32BitConstant(1, constants.debug, 1);
            command_list.SetPipelineState(&self.postprocess_pso);
            command_list.Dispatch(self.width, self.height, 1);

            let mid_barriers = [
                d3d12::ResourceBarrier::transition(
                    &self.postprocess_buffer,
                    d3d12::D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
                    d3d12::D3D12_RESOURCE_STATE_COPY_SOURCE),

                d3d12::ResourceBarrier::transition(
                    frame.render_target_resource.as_ref().unwrap(),
                    d3d12::D3D12_RESOURCE_STATE_PRESENT,
                    d3d12::D3D12_RESOURCE_STATE_COPY_DEST),
            ];
            command_list.ResourceBarrier(mid_barriers.as_slice());

            command_list.CopyResource(frame.render_target_resource.as_ref()
                                      .unwrap(),
                                      &self.postprocess_buffer);

            let after_barriers = [
                d3d12::ResourceBarrier::transition(
                    &self.postprocess_buffer,
                    d3d12::D3D12_RESOURCE_STATE_COPY_SOURCE,
                    d3d12::D3D12_RESOURCE_STATE_UNORDERED_ACCESS),

                d3d12::ResourceBarrier::transition(
                    frame.render_target_resource.as_ref().unwrap(),
                    d3d12::D3D12_RESOURCE_STATE_COPY_DEST,
                    d3d12::D3D12_RESOURCE_STATE_RENDER_TARGET),

                ResourceBarrier::transition(&self.uav,
                    d3d12::D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE,
                    d3d12::D3D12_RESOURCE_STATE_UNORDERED_ACCESS),
            ];
            command_list.ResourceBarrier(after_barriers.as_slice());

            command_list.Close().expect("Failed to close command list");

            d3d12::drop_barriers(before_barriers);
            d3d12::drop_barriers(mid_barriers);
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

impl Pipeline for ClearState {
    fn resize(&mut self, _d3d12: &d3d12::Context, width: u32, height: u32) {
        self.width  = width;
        self.height = height;
    }

    fn render(&mut self, d3d12: &d3d12::Context, frame: &d3d12::Frame,
              _frame_index: u32, _constants: &mut SceneConstants, _reset: bool) {

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
