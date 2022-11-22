use crate::d3d12;
use crate::shaders;
use imgui::{Context, BackendFlags, Ui, DrawData, DrawIdx, DrawVert, DrawCmd,
    DrawCmdParams, TextureId};

use windows::{
    Win32::Foundation::{BOOL, RECT},
    core::PCSTR,
};

use windows::Win32::Graphics::{
    Direct3D::*,
    Direct3D12::*,
    Dxgi::Common::*,
};

use core::mem::size_of;
use core::slice;
use core::ptr::null;

const VERTEX_POSITION_OFFSET: u32 = 0;
const VERTEX_TEXCOORD_OFFSET: u32 = 8;
const VERTEX_COLOR_OFFSET: u32 = 16;

#[derive(Default)]
struct Frame {
    vertex_buffer: Option<d3d12::MappableResource>,
    vertex_buffer_count: usize,
    index_buffer: Option<d3d12::MappableResource>,
    index_buffer_count: usize,
}

pub struct Backend {
    // Kept here for holding a reference
    #[allow(dead_code)]
    heap: ID3D12DescriptorHeap,
    #[allow(dead_code)]
    font_atlas_resource: ID3D12Resource,

    root_signature: ID3D12RootSignature,
    pipeline_state: ID3D12PipelineState,

    frames: [Frame; d3d12::BACKBUFFER_COUNT as usize],
}

impl Backend {
    pub fn init(d3d12: &d3d12::Context, ctx: &mut Context,
                width: u32, height: u32) -> Option<Backend> {

        ctx.set_renderer_name(Some(String::from("windows-rs d3d12")));

        let io = ctx.io_mut();

        // Init backend
        io.backend_flags.insert(BackendFlags::HAS_MOUSE_CURSORS);
        io.backend_flags.insert(BackendFlags::HAS_SET_MOUSE_POS);

        // Init window
        //io.display_framebuffer_scale = [hidpi_factor as f32, hidpi_factor as f32];
        io.display_size = [width as f32, height as f32];

        // Init renderer
        io.backend_flags.insert(BackendFlags::RENDERER_HAS_VTX_OFFSET);

        // Create a descriptor heap for font atlas (1 descriptor)
        let heap: ID3D12DescriptorHeap = unsafe {
            d3d12.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                NumDescriptors: 1,
                Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
                ..Default::default()
            }).ok()?
        };

        // Get cpu and gpu descs
        let (cpu_desc, gpu_desc) = unsafe {
            (heap.GetCPUDescriptorHandleForHeapStart(),
             heap.GetGPUDescriptorHandleForHeapStart())
        };

        // Root signature
        let rs = d3d12.create_root_signature_from_shader(&shaders::IMGUI_VS)?;

        let input_elem_descs = [
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: PCSTR(b"POSITION\0".as_ptr()),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: VERTEX_POSITION_OFFSET,
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: PCSTR(b"TEXCOORD\0".as_ptr()),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: VERTEX_TEXCOORD_OFFSET,
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: PCSTR(b"COLOR\0".as_ptr()),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                InputSlot: 0,
                AlignedByteOffset: VERTEX_COLOR_OFFSET,
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
        ];

        // Pipeline state
        let mut pso_desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            NodeMask: 1,
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            pRootSignature: Some(rs.clone()),
            SampleMask: u32::MAX,
            NumRenderTargets: 1,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, ..Default::default() },
            Flags: D3D12_PIPELINE_STATE_FLAG_NONE,
            VS: shaders::IMGUI_VS.bytecode(),
            InputLayout: D3D12_INPUT_LAYOUT_DESC {
                pInputElementDescs: input_elem_descs.as_ptr(),
                NumElements: input_elem_descs.len() as u32,
            },
            PS: shaders::IMGUI_PS.bytecode(),
            BlendState: D3D12_BLEND_DESC {
                AlphaToCoverageEnable: BOOL(0),
                ..Default::default()
            },
            DepthStencilState:  D3D12_DEPTH_STENCIL_DESC {
                DepthEnable: BOOL(0),
                DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ALL,
                DepthFunc: D3D12_COMPARISON_FUNC_ALWAYS,
                StencilEnable: BOOL(0),
                FrontFace: D3D12_DEPTH_STENCILOP_DESC {
                    StencilFailOp: D3D12_STENCIL_OP_KEEP,
                    StencilDepthFailOp: D3D12_STENCIL_OP_KEEP,
                    StencilPassOp: D3D12_STENCIL_OP_KEEP,
                    StencilFunc: D3D12_COMPARISON_FUNC_ALWAYS,
                },
                BackFace: D3D12_DEPTH_STENCILOP_DESC {
                    StencilFailOp: D3D12_STENCIL_OP_KEEP,
                    StencilDepthFailOp: D3D12_STENCIL_OP_KEEP,
                    StencilPassOp: D3D12_STENCIL_OP_KEEP,
                    StencilFunc: D3D12_COMPARISON_FUNC_ALWAYS,
                },
                ..Default::default()
            },
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: D3D12_FILL_MODE_SOLID,
                CullMode: D3D12_CULL_MODE_NONE,
                FrontCounterClockwise: BOOL(0),
                DepthBias: D3D12_DEFAULT_DEPTH_BIAS,
                DepthBiasClamp: D3D12_DEFAULT_DEPTH_BIAS_CLAMP,
                SlopeScaledDepthBias: D3D12_DEFAULT_SLOPE_SCALED_DEPTH_BIAS,
                DepthClipEnable: BOOL(1),
                MultisampleEnable: BOOL(0),
                AntialiasedLineEnable: BOOL(0),
                ForcedSampleCount: 0,
                ConservativeRaster: D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF,
                ..Default::default()
            },
            ..Default::default()
        };

        pso_desc.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;

        pso_desc.BlendState.RenderTarget[0] = D3D12_RENDER_TARGET_BLEND_DESC {
            BlendEnable: BOOL(1),
            SrcBlend: D3D12_BLEND_SRC_ALPHA,
            DestBlend: D3D12_BLEND_INV_SRC_ALPHA,
            BlendOp: D3D12_BLEND_OP_ADD,
            SrcBlendAlpha: D3D12_BLEND_ONE,
            DestBlendAlpha: D3D12_BLEND_INV_SRC_ALPHA,
            BlendOpAlpha: D3D12_BLEND_OP_ADD,
            RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
            ..Default::default()
        };

        let pso = unsafe {
            d3d12.device.CreateGraphicsPipelineState(&pso_desc).ok()?
        };

        let font_atlas_format = DXGI_FORMAT_R8G8B8A8_UNORM;
        let font_atlas_resource = {
            let atlas = ctx.fonts().build_rgba32_texture();
            d3d12.upload_tex2d_sync(atlas.data, atlas.width, atlas.height,
                                font_atlas_format,
                                D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)?
        };

        d3d12.create_shader_resource_view_tex2d(&font_atlas_resource,
                                                font_atlas_format,
                                                cpu_desc);
        ctx.fonts().tex_id = TextureId::from(gpu_desc.ptr as usize);

        Some(Backend {
            heap,
            root_signature: rs,
            pipeline_state: pso,
            font_atlas_resource,
            frames: Default::default(),
        })
    }


    fn begin_frame(&mut self, ctx: &mut Context) {
        ctx.io_mut().delta_time = 1. / 144.;
    }

    fn setup_render_state(&self, draw_data: &DrawData, fr: &Frame,
                          frame: &d3d12::Frame,
                          command_list: &ID3D12GraphicsCommandList5)
        -> Option<()> {

        let mvp: [f32; 16] = {
            let l = draw_data.display_pos[0];
            let r = draw_data.display_pos[0] + draw_data.display_size[0];
            let t = draw_data.display_pos[1];
            let b = draw_data.display_pos[1] + draw_data.display_size[1];
            [
                2.0/(r-l),   0.0,           0.0,       0.0,
                0.0,         2.0/(t-b),     0.0,       0.0,
                0.0,         0.0,           0.5,       0.0,
                (r+l)/(l-r),  (t+b)/(b-t),  0.5,       1.0,
            ]
        };

        let vp = [ D3D12_VIEWPORT {
            Width: draw_data.display_size[0],
            Height: draw_data.display_size[1],
            MinDepth: 0.0,
            MaxDepth: 1.0,
            TopLeftX: 0.0,
            TopLeftY: 0.0,
        }];

        let vbv = [ D3D12_VERTEX_BUFFER_VIEW {
            BufferLocation: unsafe {
                fr.vertex_buffer.as_ref().unwrap().res.GetGPUVirtualAddress()
            },
            SizeInBytes: (fr.vertex_buffer_count *
                          size_of::<DrawVert>()) as u32,
            StrideInBytes: size_of::<DrawVert>() as u32,
        }];

        let ibv = D3D12_INDEX_BUFFER_VIEW {
            BufferLocation: unsafe {
                fr.index_buffer.as_ref().unwrap().res.GetGPUVirtualAddress()
            },
            SizeInBytes: (fr.index_buffer_count * size_of::<DrawIdx>()) as u32,
            Format: if size_of::<DrawIdx>() == 2 { DXGI_FORMAT_R16_UINT }
                    else { DXGI_FORMAT_R32_UINT}
        };

        unsafe {
            command_list.RSSetViewports(&vp);
            command_list.IASetVertexBuffers(0, &vbv);
            command_list.IASetIndexBuffer(&ibv);
            command_list.IASetPrimitiveTopology(
                D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            command_list.SetPipelineState(self.pipeline_state.clone());
            command_list.SetDescriptorHeaps(&[Some(self.heap.clone())]);
            command_list.SetGraphicsRootSignature(self.root_signature.clone());
            command_list.SetGraphicsRoot32BitConstants(0, 16,
                                                       mvp.as_ptr() as _, 0);
            command_list.OMSetBlendFactor(&[0., 0., 0., 0.]);
            command_list.OMSetRenderTargets(1, &frame.render_target_descriptor,
                                            BOOL(0), null());
        }

        Some(())
    }

    fn render_internal(&mut self, draw_data: &DrawData, d3d12: &d3d12::Context,
              frame: &d3d12::Frame, frame_index: u32) -> Option<()> {

        let fr = &mut self.frames[frame_index as usize];

        let vtx_count = draw_data.total_vtx_count as usize;
        let idx_count = draw_data.total_idx_count as usize;

        if fr.vertex_buffer.is_none() || fr.vertex_buffer_count < vtx_count {
            fr.vertex_buffer_count = vtx_count + 5000;

            let size = fr.vertex_buffer_count * size_of::<DrawVert>();
            let buf = d3d12.create_mappable_resource(size)?;
            fr.vertex_buffer = Some(buf);
        }

        if fr.index_buffer.is_none() || fr.index_buffer_count < idx_count {
            fr.index_buffer_count = idx_count + 10000;

            let size = fr.index_buffer_count.checked_mul(size_of::<DrawIdx>())?;
            let buf = d3d12.create_mappable_resource(size)?;
            fr.index_buffer = Some(buf);
        }

        assert!(fr.vertex_buffer_count >= vtx_count as usize);
        assert!(fr.index_buffer_count >= idx_count as usize);

        fr.vertex_buffer.as_ref().unwrap().write_with(|map| {
            let vtx_size = vtx_count * size_of::<DrawVert>();
            let mut vtx_it = &mut map[0..vtx_size];

            for list in draw_data.draw_lists() {
                let vbuf = unsafe {
                    slice::from_raw_parts(list.vtx_buffer().as_ptr() as *const u8,
                    list.vtx_buffer().len() *
                    size_of::<DrawVert>())
                };

                vtx_it[0..vbuf.len()].copy_from_slice(vbuf);
                vtx_it = &mut vtx_it[vbuf.len()..];
            }

            assert!(vtx_it.len() == 0);
        });

        fr.index_buffer.as_ref().unwrap().write_with(|map| {
            let idx_size = idx_count * size_of::<DrawIdx>();
            let mut idx_it = &mut map[0..idx_size];

            for list in draw_data.draw_lists() {
                let ibuf = unsafe {
                    slice::from_raw_parts(list.idx_buffer().as_ptr() as *const u8,
                    list.idx_buffer().len() *
                    size_of::<DrawIdx>())
                };

                idx_it[0..ibuf.len()].copy_from_slice(ibuf);
                idx_it = &mut idx_it[ibuf.len()..];
            }

            assert!(idx_it.len() == 0);
        });


        let command_list = d3d12.create_graphics_command_list(frame)?;
        let fr = &self.frames[frame_index as usize];
        self.setup_render_state(draw_data, fr, frame, &command_list)?;

        let global_vtx_offset: usize = 0;
        let global_idx_offset: usize = 0;
        let clip_off = draw_data.display_pos;

        for list in draw_data.draw_lists() {
            for cmd in list.commands() {
                use imgui::internal::RawWrapper;
                match cmd {
                    DrawCmd::Elements {
                        count,
                        cmd_params: DrawCmdParams {
                            clip_rect,
                            texture_id,
                            vtx_offset,
                            idx_offset,
                            ..
                        }
                    } => {
                        let clip_min = [clip_rect[0] - clip_off[0],
                                        clip_rect[1] - clip_off[1]];

                        let clip_max = [clip_rect[2] - clip_off[0],
                                        clip_rect[3] - clip_off[1]];
                        if clip_max[0] <= clip_min[0] ||
                            clip_max[1] <= clip_min[1] {
                            continue;
                        }

                        let rect = [ RECT {
                            left: clip_min[0] as i32,
                            top: clip_min[1] as i32,
                            right: clip_max[0] as i32,
                            bottom: clip_max[1] as i32,
                        }];

                        let tex = D3D12_GPU_DESCRIPTOR_HANDLE {
                            ptr: texture_id.id() as u64,
                        };

                        unsafe {
                            command_list.SetGraphicsRootDescriptorTable(1, tex);
                            command_list.RSSetScissorRects(&rect);
                            command_list.DrawIndexedInstanced(count as u32, 1,
                                (idx_offset + global_idx_offset) as u32,
                                (vtx_offset + global_vtx_offset) as i32, 0);
                        }
                    }

                    DrawCmd::ResetRenderState => {
                        self.setup_render_state(draw_data, fr, frame,
                                                &command_list);
                    },
                    DrawCmd::RawCallback{ callback, raw_cmd } =>{
                        unsafe { callback(list.raw(), raw_cmd) };
                    },
                }
            }
        }

        let barriers = [
            d3d12::ResourceBarrier::transition(
                frame.render_target_resource.as_ref().unwrap(),
                d3d12::D3D12_RESOURCE_STATE_RENDER_TARGET,
                d3d12::D3D12_RESOURCE_STATE_PRESENT),
        ];


        unsafe {
            command_list.ResourceBarrier(&barriers);
            command_list.Close().ok()?;

            d3d12::drop_barriers(barriers);
        }
        d3d12.execute_command_lists(&[Some(command_list.into())]);

        Some(())
    }

    pub fn render(&mut self, ctx: &mut Context,
                  d3d12: &d3d12::Context,
                  frame: &d3d12::Frame,
                  frame_index: u32) -> Option<()> {
        let draw_data = ctx.render();
        self.render_internal(&draw_data, d3d12, frame, frame_index)?;
        Some(())
    }

    pub fn frame<F: FnMut(&mut Ui)>(&mut self, ctx: &mut Context,
                                    mut func: F) {
        self.begin_frame(ctx);
        func(ctx.frame());
    }
}

