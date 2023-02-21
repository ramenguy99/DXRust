use crate::win32::Window;
use windows::core::{Interface, PCSTR};
use windows::Win32::Foundation::{HANDLE, BOOL, CloseHandle, RECT};
use windows::Win32::System::{
    Threading::{WaitForSingleObjectEx, CreateEventA},
    WindowsProgramming::INFINITE
};

use core::ptr::{null, null_mut};
use core::ffi::c_void;
use core::cell::Cell;
use core::mem::size_of;

use bytemuck::{Pod, cast_slice};


use math::mat::Mat4;
pub use windows::Win32::Graphics::{
    Direct3D::*,
    Direct3D12::*,
    Dxgi::*,
    Dxgi::Common::*,
};

#[no_mangle]
pub static D3D12SDKVersion: u32 = 606;

#[no_mangle]
pub static D3D12SDKPath: &[u8] =
    b"..\\..\\agility\\agility\\build\\native\\bin\\x64\\";

pub const BACKBUFFER_COUNT: u32 = 3;
const GPU_VALIDATION_ENABLED: bool = false;
const CSU_DESCRIPTOR_HEAP_COUNT: usize = 1024;

#[allow(non_camel_case_types)]
pub type D3D12_GPU_VIRTUAL_ADDRESS = u64;
#[allow(non_camel_case_types)]
pub type D3D12_RECT = RECT;

pub struct Shader<'a> {
    pub name: &'static str,
    pub data: &'a [u8],
}

impl<'a> Shader<'a> {
    pub fn bytecode(&self) -> D3D12_SHADER_BYTECODE {
        D3D12_SHADER_BYTECODE {
            pShaderBytecode: self.data.as_ptr() as *const c_void,
            BytecodeLength: self.data.len(),
        }
    }
}

pub struct MappableResource {
    pub res: ID3D12Resource,
    size: usize,
}

pub struct DescriptorHeap {
    pub heap: ID3D12DescriptorHeap,
    increment: usize,
    top: Cell<D3D12_CPU_DESCRIPTOR_HANDLE>,
    end: D3D12_CPU_DESCRIPTOR_HANDLE,
}

#[allow(dead_code)]
struct AccelerationStructureBuffers {
    scratch: ID3D12Resource,
    acceleration_structure: ID3D12Resource,
    instance_desc: Option<ID3D12Resource>,
}

#[allow(dead_code)]
pub struct AccelerationStructure {
    pub tlas: ID3D12Resource,
    pub blas: ID3D12Resource,
}

pub struct PerFrameConstantBuffer {
    resource: ID3D12Resource,
    size: usize,
    map: &'static mut[u8],
}

#[allow(dead_code)]
pub struct Frame {
    pub render_target_resource: Option<ID3D12Resource>,
    pub render_target_descriptor: D3D12_CPU_DESCRIPTOR_HANDLE,
    command_allocator: ID3D12CommandAllocator,
    fence_value: Cell<u64>,
}

#[allow(dead_code)]
pub struct Context {
    pub device: ID3D12Device5,
    debug_interface: ID3D12Debug1,
    command_queue: ID3D12CommandQueue,
    factory: IDXGIFactory6,
    swapchain: IDXGISwapChain3,
    frames: Vec<Frame>,
    fence: ID3D12Fence,
    fence_event: HANDLE,
    fence_value: Cell<u64>,
    sync_queue: ID3D12CommandQueue,
    sync_allocator: ID3D12CommandAllocator,
    pub sync_command_list: ID3D12GraphicsCommandList5,
    sync_fence: ID3D12Fence,
    sync_fence_value: Cell<u64>,
    rtv_descriptor_heap: ID3D12DescriptorHeap,

    pub csu_descriptor_heap: DescriptorHeap,
}

impl Context {

    #[allow(unused)]
    unsafe extern "system" fn message_callback(
        _category: D3D12_MESSAGE_CATEGORY,
        _severity: D3D12_MESSAGE_SEVERITY,
        _id: D3D12_MESSAGE_ID,
        _pdescription: windows::core::PCSTR,
        _pcontext: *mut ::core::ffi::c_void) {

    }

    pub fn init(window: &Window) -> Option<Self> {
        let mut debug_interface: Option<ID3D12Debug1> = None;

        unsafe { D3D12GetDebugInterface(&mut debug_interface).ok()? }

        let debug_interface = debug_interface?;

        unsafe {
            debug_interface.EnableDebugLayer();
            if GPU_VALIDATION_ENABLED {
                debug_interface.SetEnableGPUBasedValidation(true);
            }
        }

        let mut device: Option<ID3D12Device5> = None;

        unsafe {
            D3D12CreateDevice(None, D3D_FEATURE_LEVEL_12_1, &mut device).ok()?;
        }

        let device = device?;


        let info_queue: ID3D12InfoQueue = device.cast().unwrap();
        unsafe {
            info_queue.SetBreakOnSeverity(D3D12_MESSAGE_SEVERITY_ERROR,
                                          true).ok()?;
            info_queue.SetBreakOnSeverity(D3D12_MESSAGE_SEVERITY_CORRUPTION,
                                          true).ok()?;

            /* Looks like this is supported only on Windows 11
            let mut cookie: u32 = 0;
            info_queue.RegisterMessageCallback(Some(Context::message_callback),
                D3D12_MESSAGE_CALLBACK_FLAG_NONE, null(), &mut cookie).unwrap();
            */
        }


        let command_queue: ID3D12CommandQueue = unsafe {
            device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
                Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
                ..Default::default()
            }).ok()?
        };

        let factory: IDXGIFactory6 = unsafe { CreateDXGIFactory().ok()? };

        let swapchain_desc = DXGI_SWAP_CHAIN_DESC1 {
            BufferCount: BACKBUFFER_COUNT,
            Width: window.width() as u32,
            Height: window.height() as u32,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, ..Default::default() },
            ..Default::default()
        };
        let swapchain: IDXGISwapChain3 = unsafe {
            factory.CreateSwapChainForHwnd(&command_queue,
                                           window.handle,
                                           &swapchain_desc,
                                           core::ptr::null(),
                                           None).ok()?
        }.cast().ok()?;

        let rtv_descriptor_heap: ID3D12DescriptorHeap = unsafe {
            device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                NumDescriptors: BACKBUFFER_COUNT,
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                ..Default::default()
            }).ok()?
        };

        let (rtv_heap_start, rtv_heap_increment) = unsafe {
            (rtv_descriptor_heap.GetCPUDescriptorHandleForHeapStart(),
            device.GetDescriptorHandleIncrementSize(
                D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize)
        };

        let mut frames = Vec::new();
        for i in 0..BACKBUFFER_COUNT {
            let render_target_resource: Option<ID3D12Resource> = unsafe {
                Some(swapchain.GetBuffer(i).ok()?)
            };

            let render_target_descriptor = D3D12_CPU_DESCRIPTOR_HANDLE {
                ptr: rtv_heap_start.ptr + i as usize * rtv_heap_increment
            };

            unsafe {
                device.CreateRenderTargetView(&render_target_resource, null(),
                                              render_target_descriptor);
            }

            let command_allocator: ID3D12CommandAllocator = unsafe {
                device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)
                    .ok()?
            };

            frames.push(Frame {
                render_target_resource,
                render_target_descriptor,
                command_allocator,
                fence_value: Cell::new(0),
            });
        }

        let fence: ID3D12Fence = unsafe {
            device.CreateFence(0, D3D12_FENCE_FLAG_NONE).ok()?
        };

        let fence_event: HANDLE = unsafe {
            CreateEventA(null(), BOOL(0), BOOL(0), PCSTR(null())).ok()?
        };


        let sync_queue: ID3D12CommandQueue = unsafe {
            device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
                Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
                ..Default::default()
            }).ok()?
        };

        let sync_allocator: ID3D12CommandAllocator = unsafe {
            device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)
                .ok()?
        };

        let sync_command_list: ID3D12GraphicsCommandList5 = unsafe {
            device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT,
                                     &sync_allocator, None).ok()?
        };

        let sync_fence: ID3D12Fence = unsafe {
            device.CreateFence(0, D3D12_FENCE_FLAG_NONE).ok()?
        };

        let csu_descriptor_heap: ID3D12DescriptorHeap = unsafe {
            device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                NumDescriptors: CSU_DESCRIPTOR_HEAP_COUNT as u32,
                Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
                ..Default::default()
            }).ok()?
        };

        let csu_descriptor_top = unsafe {
            csu_descriptor_heap.GetCPUDescriptorHandleForHeapStart()
        };

        let csu_descriptor_increment = unsafe {
            device.GetDescriptorHandleIncrementSize(
                D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as usize
        };

        let csu_descriptor_end = D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: csu_descriptor_top.ptr.checked_add(
                CSU_DESCRIPTOR_HEAP_COUNT * csu_descriptor_increment)?
        };

        Some(Self{
            device,
            debug_interface,
            command_queue,
            factory,
            swapchain,
            frames,
            rtv_descriptor_heap,
            fence,
            fence_event,
            fence_value: Cell::new(0),
            sync_queue,
            sync_allocator,
            sync_command_list,
            sync_fence_value: Cell::new(0),
            sync_fence,
            csu_descriptor_heap: DescriptorHeap {
                heap: csu_descriptor_heap,
                top: Cell::new(csu_descriptor_top),
                end: csu_descriptor_end,
                increment: csu_descriptor_increment,
            },
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Option<()> {
        self.wait_idle();

        // Drop all render target views
        for f in self.frames.iter_mut() {
            f.render_target_resource = None;
        }

        // Resize swapchain
        unsafe {
            self.swapchain.ResizeBuffers(0, width, height,
                                         DXGI_FORMAT_UNKNOWN, 0).unwrap();
        }

        // Recreate render target views
        for (i, f) in self.frames.iter_mut().enumerate() {
            f.render_target_resource = unsafe {
                Some(self.swapchain.GetBuffer(i as u32).ok()?)
            };

            unsafe {
                self.device.CreateRenderTargetView(f.render_target_resource
                                                   .as_ref().unwrap(),
                                                   null(),
                                                   f.render_target_descriptor);
            }
        }
        Some(())
    }

    pub fn create_root_signature_from_shader(&self, shader: &Shader)
        -> Option<ID3D12RootSignature> {
        unsafe { self.device.CreateRootSignature(1, shader.data).ok() }
    }

    pub fn create_command_signature(&self, rs: &ID3D12RootSignature, stride: u32,
                                    descs: &[D3D12_INDIRECT_ARGUMENT_DESC])
        -> Option<ID3D12CommandSignature> {
        unsafe {
            let mut cs: Option<ID3D12CommandSignature> = None;
            self.device.CreateCommandSignature(&D3D12_COMMAND_SIGNATURE_DESC {
                ByteStride: stride,
                NumArgumentDescs: descs.len() as u32,
                pArgumentDescs: descs.as_ptr(),
                NodeMask: 0,
            }, rs, &mut cs).ok()?;
            cs
        }
    }

    pub fn create_compute_pipelinestate(&self, shader: &Shader,
                                        rs: &ID3D12RootSignature)
        -> Option<ID3D12PipelineState> {
        unsafe{
            self.device.CreateComputePipelineState(
                &D3D12_COMPUTE_PIPELINE_STATE_DESC {
                    pRootSignature: Some(rs.clone()),
                    CS: shader.bytecode(),
                    ..Default::default()
                }).ok()
        }
    }

    pub fn create_resource(&self, resource_desc: &ResourceDesc,
                           initial_state: D3D12_RESOURCE_STATES,
                           heap_type: D3D12_HEAP_TYPE)
        -> Option<ID3D12Resource> {
        let mut result: Option<ID3D12Resource> = None;
        unsafe {
            self.device.CreateCommittedResource(
                &D3D12_HEAP_PROPERTIES {
                    Type: heap_type,
                    ..Default::default()
                },
                D3D12_HEAP_FLAG_NONE,
                &resource_desc.0,
                initial_state,
                null(),
                &mut result
            ).ok();
        }

        result
    }

    pub fn create_unordered_access_view(&self, dimension: D3D12_UAV_DIMENSION,
                                   resource: &ID3D12Resource,
                                   descriptor: D3D12_CPU_DESCRIPTOR_HANDLE) {
        unsafe {
            self.device.CreateUnorderedAccessView(resource, None,
                &D3D12_UNORDERED_ACCESS_VIEW_DESC {
                    ViewDimension: dimension,
                    ..Default::default()
                }, descriptor);
        }
    }

    pub fn create_shader_resource_view_as(&self,
                       resource: &ID3D12Resource,
                       descriptor: D3D12_CPU_DESCRIPTOR_HANDLE) {
        unsafe {
            self.device.CreateShaderResourceView(None,
                &D3D12_SHADER_RESOURCE_VIEW_DESC {
                    ViewDimension:
                        D3D12_SRV_DIMENSION_RAYTRACING_ACCELERATION_STRUCTURE,
                    Shader4ComponentMapping:
                        D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
                    Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                        RaytracingAccelerationStructure:
                            D3D12_RAYTRACING_ACCELERATION_STRUCTURE_SRV {
                                Location: resource.GetGPUVirtualAddress(),
                            }
                    },
                    ..Default::default()
                }, descriptor);
        }
    }

    pub fn create_shader_resource_view_buffer(&self, resource: &ID3D12Resource,
                                  format: DXGI_FORMAT, first: u64, count: u32,
                                  descriptor: D3D12_CPU_DESCRIPTOR_HANDLE) {
        unsafe {
            self.device.CreateShaderResourceView(resource,
                &D3D12_SHADER_RESOURCE_VIEW_DESC {
                    Format: format,
                    ViewDimension: D3D12_SRV_DIMENSION_BUFFER,
                    Shader4ComponentMapping:
                        D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
                    Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                        Buffer: D3D12_BUFFER_SRV {
                            FirstElement: first,
                            NumElements: count,
                            StructureByteStride: 0,
                            Flags: D3D12_BUFFER_SRV_FLAG_NONE,
                        }
                    },
                    ..Default::default()
                }, descriptor);
        }
    }

    pub fn create_shader_resource_view_structured_buffer(&self,
         resource: &ID3D12Resource, first: u64, count: u32, stride: u32,
         descriptor: D3D12_CPU_DESCRIPTOR_HANDLE) {
        unsafe {
            self.device.CreateShaderResourceView(resource,
                &D3D12_SHADER_RESOURCE_VIEW_DESC {
                    Format: DXGI_FORMAT_UNKNOWN,
                    ViewDimension: D3D12_SRV_DIMENSION_BUFFER,
                    Shader4ComponentMapping:
                        D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
                    Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                        Buffer: D3D12_BUFFER_SRV {
                            FirstElement: first,
                            NumElements: count,
                            StructureByteStride: stride,
                            Flags: D3D12_BUFFER_SRV_FLAG_NONE,
                        }
                    },
                    ..Default::default()
                }, descriptor);
        }
    }


    pub fn create_shader_resource_view_tex2d(&self, resource: &ID3D12Resource,
                                     format: DXGI_FORMAT,
                                     descriptor: D3D12_CPU_DESCRIPTOR_HANDLE) {
        unsafe {
            self.device.CreateShaderResourceView(resource,
                &D3D12_SHADER_RESOURCE_VIEW_DESC {
                    Format: format,
                    ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
                    Shader4ComponentMapping:
                        D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
                    Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                        Texture2D: D3D12_TEX2D_SRV {
                            MipLevels: 1,
                            MostDetailedMip: 0,
                            ..Default::default()
                        }
                    },
                    ..Default::default()
                }, descriptor);
        }
    }

    pub fn create_depth_stencil_view(&self, resource: &ID3D12Resource,
                                     descriptor: D3D12_CPU_DESCRIPTOR_HANDLE) {
        unsafe {
            self.device.CreateDepthStencilView(resource,
               &D3D12_DEPTH_STENCIL_VIEW_DESC {
                   Format: DXGI_FORMAT_D32_FLOAT,
                   ViewDimension: D3D12_DSV_DIMENSION_TEXTURE2D,
                   Flags: D3D12_DSV_FLAG_NONE,
                   Anonymous: D3D12_DEPTH_STENCIL_VIEW_DESC_0 {
                       Texture2D: D3D12_TEX2D_DSV {
                           MipSlice: 0,
                       },
                   }
               }, descriptor);
        }
    }


    pub fn alloc_csu_descriptor(&self) ->
        Option<D3D12_CPU_DESCRIPTOR_HANDLE> {
        self.csu_descriptor_heap.alloc_descriptor()
    }

    pub fn create_graphics_command_list(&self, frame: &Frame) ->
        Option<ID3D12GraphicsCommandList5> {

        unsafe {
            self.device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT,
                                          &frame.command_allocator, None).ok()
        }
    }

    pub fn execute_command_lists(&self,
                                 command_lists: &[Option<ID3D12CommandList>]) {
        unsafe {
            self.command_queue.ExecuteCommandLists(command_lists);
        }
    }

    pub fn begin_frame(&self) -> Option<(&Frame, u32)> {
        let index = unsafe { self.swapchain.GetCurrentBackBufferIndex()};
        let frame = self.frames.get(index as usize)?;

        // Set fence_value + 1 as the target value for this frame.
        // We wait for the fence to reach this value before rendering
        // again to this frame.
        self.fence_value.set(self.fence_value.get() + 1);
        frame.fence_value.set(self.fence_value.get());

        unsafe { frame.command_allocator.Reset().ok()? };

        Some((frame, index))
    }

    pub fn end_frame(&self, frame: &Frame, vsync: bool) -> Option<()> {
        unsafe {
            self.swapchain.Present(if vsync {1} else {0}, 0).ok()?;

            // Signal the fence with the target value for this frame.
            self.command_queue.Signal(&self.fence, frame.fence_value.get()).ok()?;

            let frame_index = self.swapchain.GetCurrentBackBufferIndex()
                as usize;
            let next_frame = &self.frames[frame_index];

            // Wait for the fence to reach the target value for the next frame.
            if self.fence.GetCompletedValue() < next_frame.fence_value.get() {
                self.fence.SetEventOnCompletion(next_frame.fence_value.get(),
                                                self.fence_event).ok();
                WaitForSingleObjectEx(self.fence_event, INFINITE, BOOL(0));
            }
        }

        Some(())
    }

    pub fn wait_idle(&self) -> Option<()> {
        unsafe {
            if self.fence.GetCompletedValue() < self.fence_value.get() {
                self.fence.SetEventOnCompletion(self.fence_value.get(),
                                                self.fence_event).ok()?;
                WaitForSingleObjectEx(self.fence_event, INFINITE, BOOL(0));
            }
        }

        Some(())
    }


    pub fn create_dxr_state_object(&self, shader: &Shader)
        -> Option<ID3D12StateObject> {

        let library_desc = D3D12_DXIL_LIBRARY_DESC {
            DXILLibrary: shader.bytecode(),
            ..Default::default()
        };

        let library_subobject = D3D12_STATE_SUBOBJECT {
            Type: D3D12_STATE_SUBOBJECT_TYPE_DXIL_LIBRARY,
            pDesc: &library_desc as *const _ as *const c_void,
        };

        let state_object_desc = D3D12_STATE_OBJECT_DESC {
            Type: D3D12_STATE_OBJECT_TYPE_RAYTRACING_PIPELINE,
            NumSubobjects: 1,
            pSubobjects: &library_subobject,
        };

        unsafe {
            self.device.CreateStateObject(&state_object_desc).ok()
        }
    }

    // pub fn readback_tex2d_sync(&self, resource: &ID3D12Resource,
    //     width: u32, height: u32, data: &mut [u8]) -> Option<()> {

    //     self.
    //     Some(())
    // }

    pub fn upload_tex2d_sync(&self, data: &[u8], width: u32, height: u32,
                          format: DXGI_FORMAT, state: D3D12_RESOURCE_STATES)
    -> Option<ID3D12Resource> {

        let upload_pitch = (width * 4 + D3D12_TEXTURE_DATA_PITCH_ALIGNMENT - 1)
            & !(D3D12_TEXTURE_DATA_PITCH_ALIGNMENT - 1);
        let upload_size = height as usize * upload_pitch as usize;

        let upload_resource =
            self.create_mappable_resource(upload_size, D3D12_HEAP_TYPE_UPLOAD)?;

        upload_resource.write_with(|map| {
            for y in 0..height as usize {
                let map_start = y * upload_pitch as usize;
                let map_end = map_start + width as usize * 4;

                let data_start = y * width as usize * 4;
                let data_end = data_start + width as usize * 4;

                map[map_start..map_end]
                    .copy_from_slice(&data[data_start..data_end]);
            }
        });

        let dest_resource =
            self.create_resource(&ResourceDesc::tex2d(format, width,
                                                      height,
                                                      D3D12_RESOURCE_FLAG_NONE),
                                 D3D12_RESOURCE_STATE_COPY_DEST,
                                 D3D12_HEAP_TYPE_DEFAULT)?;

        let upload_loc = D3D12_TEXTURE_COPY_LOCATION {
            pResource: Some(upload_resource.res),
            Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
            Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                PlacedFootprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                    Footprint: D3D12_SUBRESOURCE_FOOTPRINT {
                        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                        Width: width,
                        Height: height,
                        Depth: 1,
                        RowPitch: upload_pitch,
                    },
                    ..Default::default()
                }
            }
        };


        let dest_loc = D3D12_TEXTURE_COPY_LOCATION {
            pResource: Some(dest_resource.clone()),
            Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
            Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                SubresourceIndex: 0,
            }
        };

        unsafe {
            self.sync_command_list.CopyTextureRegion(&dest_loc, 0, 0, 0,
                                                     &upload_loc, null());
            let barriers = [
                ResourceBarrier::transition(&dest_resource,
                                            D3D12_RESOURCE_STATE_COPY_DEST,
                                            state)
            ];
            self.sync_command_list.ResourceBarrier(&barriers);
            drop_barriers(barriers);

        }

        self.wait_sync_commands();

        Some(dest_resource)
    }

    pub fn upload_buffer_sync(&self, data: &[u8], state: D3D12_RESOURCE_STATES)
        -> Option<ID3D12Resource> {

        if data.len() == 0 {
            return None;
        }


        let upload_buffer = self.create_mappable_resource(data.len(),
                                                          D3D12_HEAP_TYPE_UPLOAD)?;
        upload_buffer.write_with(|map| map.copy_from_slice(data));

        let dest_buffer =
            self.create_resource(&ResourceDesc::buffer(data.len()),
                                D3D12_RESOURCE_STATE_COMMON,
                                D3D12_HEAP_TYPE_DEFAULT)?;
        unsafe {
            self.sync_command_list.CopyBufferRegion(&dest_buffer, 0,
                                                    &upload_buffer.res, 0,
                                                    data.len() as u64);
            let barriers = [
                ResourceBarrier::transition(&dest_buffer,
                                            D3D12_RESOURCE_STATE_COPY_DEST,
                                            state)
            ];
            self.sync_command_list.ResourceBarrier(&barriers);
            drop_barriers(barriers);

        }

        self.wait_sync_commands();

        Some(dest_buffer)
    }

    pub fn wait_sync_commands(&self) -> Option<()> {
        unsafe {
            let event: HANDLE =
                CreateEventA(null(), BOOL(0), BOOL(0), PCSTR(null())).ok()?;

            self.sync_command_list.Close().ok()?;
            self.sync_queue.ExecuteCommandLists(
                &[Some(self.sync_command_list.clone().into())]);

            self.sync_fence_value.set(self.sync_fence_value.get() + 1);
            self.sync_queue.Signal(&self.sync_fence,
                                   self.sync_fence_value.get()).ok()?;
            self.sync_fence.SetEventOnCompletion(self.sync_fence_value.get(),
                                                 event).ok()?;
            WaitForSingleObjectEx(event, INFINITE, BOOL(0));
            CloseHandle(event);

            self.sync_allocator.Reset().ok()?;
            self.sync_command_list.Reset(&self.sync_allocator, None).ok()?;
        }

        Some(())
    }

    #[allow(dead_code)]
    fn create_blas(&self,
                   positions_count: usize, positions_buffer: &ID3D12Resource,
                   indices_count: usize, index_buffer: &ID3D12Resource)
        -> Option<AccelerationStructureBuffers> {

        let geom_desc = unsafe {
            D3D12_RAYTRACING_GEOMETRY_DESC {
                Type: D3D12_RAYTRACING_GEOMETRY_TYPE_TRIANGLES,
                Flags: D3D12_RAYTRACING_GEOMETRY_FLAG_OPAQUE,
                Anonymous: D3D12_RAYTRACING_GEOMETRY_DESC_0 {
                    Triangles: D3D12_RAYTRACING_GEOMETRY_TRIANGLES_DESC {
                        VertexBuffer: D3D12_GPU_VIRTUAL_ADDRESS_AND_STRIDE {
                            StartAddress:
                                positions_buffer.GetGPUVirtualAddress(),
                            StrideInBytes: 12,
                        },
                        VertexFormat: DXGI_FORMAT_R32G32B32_FLOAT,
                        VertexCount: positions_count as u32,
                        IndexFormat: DXGI_FORMAT_R32_UINT,
                        IndexCount: indices_count as u32,
                        IndexBuffer: index_buffer.GetGPUVirtualAddress(),
                        ..Default::default()
                    }
                },
            }
        };

        let inputs = D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_INPUTS {
            Type: D3D12_RAYTRACING_ACCELERATION_STRUCTURE_TYPE_BOTTOM_LEVEL,
            Flags: D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_NONE,
            DescsLayout: D3D12_ELEMENTS_LAYOUT_ARRAY,
            NumDescs: 1,
            Anonymous: D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_INPUTS_0 {
                pGeometryDescs: &geom_desc,
            },
        };

        let mut info = Default::default();
        unsafe {
            self.device.GetRaytracingAccelerationStructurePrebuildInfo(
                &inputs, &mut info);
        }

        let buffers = AccelerationStructureBuffers {
            scratch: self.create_resource(
                         &ResourceDesc::uav_buffer(
                             info.ScratchDataSizeInBytes as usize),
                         D3D12_RESOURCE_STATE_COMMON,
                         D3D12_HEAP_TYPE_DEFAULT)?,

            acceleration_structure: self.create_resource(
                         &ResourceDesc::uav_buffer(
                             info.ResultDataMaxSizeInBytes as usize),
                         D3D12_RESOURCE_STATE_RAYTRACING_ACCELERATION_STRUCTURE,
                         D3D12_HEAP_TYPE_DEFAULT)?,
            instance_desc: None,
        };


        let as_desc = unsafe {
            D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_DESC {
                Inputs: inputs,
                DestAccelerationStructureData:
                    buffers.acceleration_structure.GetGPUVirtualAddress(),
                ScratchAccelerationStructureData:
                    buffers.scratch.GetGPUVirtualAddress(),
                SourceAccelerationStructureData: 0,
            }
        };

        unsafe {
            self.sync_command_list
                .BuildRaytracingAccelerationStructure(&as_desc, &[]);
        }

        let barriers = [ResourceBarrier::uav(&buffers.acceleration_structure)];
        unsafe {
            self.sync_command_list.ResourceBarrier(&barriers);
            drop_barriers(barriers);
        }

        Some(buffers)
    }

    #[allow(dead_code)]
    fn create_tlas(&self, blas: &ID3D12Resource, transform: &Mat4)
        -> Option<AccelerationStructureBuffers> {

        let mut inputs = D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_INPUTS {
            Type: D3D12_RAYTRACING_ACCELERATION_STRUCTURE_TYPE_TOP_LEVEL,
            DescsLayout: D3D12_ELEMENTS_LAYOUT_ARRAY,
            Flags: D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_NONE,
            NumDescs: 1,
            ..Default::default()
        };

        let mut info = Default::default();

        unsafe {
            self.device.GetRaytracingAccelerationStructurePrebuildInfo(
                &inputs, &mut info);
        }

        // Create the buffers
        let buffers = AccelerationStructureBuffers {
            scratch: self.create_resource(
                         &ResourceDesc::uav_buffer(info.ScratchDataSizeInBytes
                                                   as usize),
                         D3D12_RESOURCE_STATE_COMMON,
                         D3D12_HEAP_TYPE_DEFAULT)?,

            acceleration_structure: self.create_resource(
                         &ResourceDesc::uav_buffer(info.ResultDataMaxSizeInBytes
                                                   as usize),
                         D3D12_RESOURCE_STATE_RAYTRACING_ACCELERATION_STRUCTURE,
                         D3D12_HEAP_TYPE_DEFAULT)?,

            instance_desc: Some(self.create_resource(
                         &ResourceDesc::buffer(
                             size_of::<D3D12_RAYTRACING_INSTANCE_DESC>()),
                         D3D12_RESOURCE_STATE_GENERIC_READ,
                         D3D12_HEAP_TYPE_UPLOAD)?),
        };

        let instance_desc: &mut D3D12_RAYTRACING_INSTANCE_DESC = unsafe {
            let mut ptr = null_mut();
            buffers.instance_desc.as_ref().unwrap().Map(0, null(), &mut ptr)
                .ok()?;
            &mut (*(ptr as *mut D3D12_RAYTRACING_INSTANCE_DESC))
        };

        instance_desc._bitfield1 = 0 | 0xFF << 24 ;
        instance_desc._bitfield2 = 0 |
            D3D12_RAYTRACING_INSTANCE_FLAG_NONE.0 << 24;

        let t = transform;
        let transf = [
            t.e[0][0], t.e[1][0], t.e[2][0], t.e[3][0],
            t.e[0][1], t.e[1][1], t.e[2][1], t.e[3][1],
            t.e[0][2], t.e[1][2], t.e[2][2], t.e[3][2],
        ];
        instance_desc.Transform.copy_from_slice(&transf);
        instance_desc.AccelerationStructure = unsafe {
            blas.GetGPUVirtualAddress()
        };

        // Unmap
        core::mem::drop(instance_desc);
        unsafe { buffers.instance_desc.as_ref().unwrap().Unmap(0, null()) };


        unsafe {
            inputs.Anonymous.InstanceDescs =
                buffers.instance_desc.as_ref().unwrap().GetGPUVirtualAddress();
        }

        // Create the TLAS
        let as_desc = unsafe {
            D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_DESC {
                Inputs: inputs,
                DestAccelerationStructureData:
                    buffers.acceleration_structure.GetGPUVirtualAddress(),
                ScratchAccelerationStructureData:
                    buffers.scratch.GetGPUVirtualAddress(),
                SourceAccelerationStructureData: 0,
            }
        };

        unsafe {
            self.sync_command_list.BuildRaytracingAccelerationStructure(
                &as_desc, &[]);
        }

        let barriers = [ResourceBarrier::uav(&buffers.acceleration_structure)];
        unsafe {
            self.sync_command_list.ResourceBarrier(&barriers);
            drop_barriers(barriers);
        }

        Some(buffers)
    }

    #[allow(dead_code)]
    pub fn create_acceleration_structure(&self,
                                         positions_count: usize,
                                         positions_buffer: &ID3D12Resource,
                                         indices_count: usize,
                                         index_buffer: &ID3D12Resource,
                                         transform: &Mat4)
        -> Option<AccelerationStructure> {

        let blas = self.create_blas(positions_count, positions_buffer,
                                    indices_count, index_buffer)?;
        let tlas = self.create_tlas(&blas.acceleration_structure, transform)?;
        self.wait_sync_commands();

        Some(AccelerationStructure {
            blas: blas.acceleration_structure,
            tlas: tlas.acceleration_structure,
        })
    }

    pub fn create_per_frame_constant_buffer(&self, wanted_size: usize)
        -> Option<PerFrameConstantBuffer> {

        // Align to 256 bytes
        let size = wanted_size + (wanted_size.wrapping_neg() & 0xFF);
        assert!(size % 256 == 0 && size >= wanted_size);

        let total_size = size * BACKBUFFER_COUNT as usize;

        let resource = self.create_resource(&ResourceDesc::buffer(total_size),
                                            D3D12_RESOURCE_STATE_GENERIC_READ,
                                            D3D12_HEAP_TYPE_UPLOAD)?;
        let map = unsafe {
            let mut ptr: *mut c_void = null_mut();
            resource.Map(0, &D3D12_RANGE { Begin: 0, End: 0 }, &mut ptr).ok()?;
            core::slice::from_raw_parts_mut(ptr as *mut u8, total_size)
        };

        Some(PerFrameConstantBuffer {
            resource,
            map,
            size,
        })
    }

    pub fn create_mappable_resource(&self, size: usize,
        heap_type: D3D12_HEAP_TYPE) -> Option<MappableResource> {
        let res = self.create_resource(&ResourceDesc::buffer(size),
                                       D3D12_RESOURCE_STATE_COMMON,
                                       heap_type)?;
        Some(MappableResource {
            res,
            size,
        })
    }

    pub fn create_depth_buffer(&self, width: u32, height: u32)
        -> Option<ID3D12Resource> {
        let mut result: Option<ID3D12Resource> = None;
        unsafe {
            self.device.CreateCommittedResource(
                &D3D12_HEAP_PROPERTIES {
                    Type: D3D12_HEAP_TYPE_DEFAULT,
                    ..Default::default()
                },
                D3D12_HEAP_FLAG_NONE,
                &ResourceDesc::depth_buffer(width, height).0,
                D3D12_RESOURCE_STATE_DEPTH_WRITE,
                &D3D12_CLEAR_VALUE {
                    Format: DXGI_FORMAT_D32_FLOAT,
                    Anonymous: D3D12_CLEAR_VALUE_0 {
                        DepthStencil: D3D12_DEPTH_STENCIL_VALUE {
                            Depth: 1.0,
                            Stencil: 0,
                        },
                    }
                },
                &mut result
            ).ok();
        }
        result
    }

    pub fn create_descriptor_heap(&self, count: usize,
                                  typ: D3D12_DESCRIPTOR_HEAP_TYPE,
                                  shader_visible: bool)
        -> Option<DescriptorHeap> {

        let heap: ID3D12DescriptorHeap = unsafe {
            self.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: typ,
                NumDescriptors: count as u32,
                Flags: if shader_visible {
                    D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE
                } else {
                    D3D12_DESCRIPTOR_HEAP_FLAG_NONE
                },
                ..Default::default()
            }).ok()?
        };

        let top = unsafe {
            heap.GetCPUDescriptorHandleForHeapStart()
        };

        let increment = unsafe {
            self.device.GetDescriptorHandleIncrementSize(
                D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as usize
        };

        let end = D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: top.ptr + count * increment
        };

        Some(DescriptorHeap {
            heap: heap,
            top: Cell::new(top),
            end: end,
            increment: increment,
        })
    }
}

impl PerFrameConstantBuffer {
    pub fn write(&mut self, index: u32, data: &[u8]) -> Option<()> {
        if data.len() > self.size {
            return None;
        }

        let base = self.size * index as usize;
        self.map[base..base + data.len()].copy_from_slice(data);

        Some(())
    }

    pub fn get_gpu_virtual_address(&self, index: u32) -> D3D12_GPU_VIRTUAL_ADDRESS {
        unsafe { self.resource.GetGPUVirtualAddress() +
            self.size as u64 * index as u64 }
    }
}

pub struct ResourceDesc(D3D12_RESOURCE_DESC);
impl ResourceDesc {
    pub fn tex2d(format: DXGI_FORMAT, width: u32, height: u32,
                 flags: D3D12_RESOURCE_FLAGS) -> Self {
        Self(D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
            Alignment: 0,
            Width: width.into(),
            Height: height,
            DepthOrArraySize: 1,
            Format: format,
            Flags: flags,
            MipLevels: 1,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
        })
    }

    pub fn depth_buffer(width: u32, height: u32) -> Self {
        Self::tex2d(DXGI_FORMAT_D32_FLOAT, width, height,
                    D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL)
    }

    pub fn uav2d(format: DXGI_FORMAT, width: u32, height: u32) -> Self {
        Self::tex2d(format, width, height,
                    D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS)
    }

    pub fn buffer(size: usize) -> Self {
        Self(D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
            Alignment: 0,
            Width: size as u64,
            Height: 1,
            DepthOrArraySize: 1,
            MipLevels: 1,
            Format: DXGI_FORMAT_UNKNOWN,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
            Flags: D3D12_RESOURCE_FLAG_NONE,
        })
    }

    pub fn uav_buffer(size: usize) -> Self {
        Self(D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
            Alignment: 0,
            Width: size as u64,
            Height: 1,
            DepthOrArraySize: 1,
            MipLevels: 1,
            Format: DXGI_FORMAT_UNKNOWN,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
            Flags: D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS,
        })
    }

}

pub struct ResourceBarrier;

impl ResourceBarrier {
    pub fn transition(resource: &ID3D12Resource, before: D3D12_RESOURCE_STATES,
                      after: D3D12_RESOURCE_STATES) -> D3D12_RESOURCE_BARRIER {
        D3D12_RESOURCE_BARRIER {
            Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
            Anonymous: D3D12_RESOURCE_BARRIER_0 {
                Transition: core::mem::ManuallyDrop::new(
                    D3D12_RESOURCE_TRANSITION_BARRIER {
                        pResource: Some(resource.clone()),
                        Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                        StateBefore: before,
                        StateAfter: after,
                    }
                )
            }
        }
    }

    pub fn uav(resource: &ID3D12Resource) -> D3D12_RESOURCE_BARRIER {
        D3D12_RESOURCE_BARRIER {
            Type: D3D12_RESOURCE_BARRIER_TYPE_UAV,
            Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
            Anonymous: D3D12_RESOURCE_BARRIER_0 {
                UAV: core::mem::ManuallyDrop::new(
                    D3D12_RESOURCE_UAV_BARRIER {
                        pResource: Some(resource.clone()),
                    }
                )
            }
        }
    }
}

pub unsafe fn drop_barrier(b: D3D12_RESOURCE_BARRIER) {
    match b.Type{
        D3D12_RESOURCE_BARRIER_TYPE_TRANSITION => {
            core::mem::ManuallyDrop::into_inner(b.Anonymous.Transition);
        },
        D3D12_RESOURCE_BARRIER_TYPE_UAV => {
            core::mem::ManuallyDrop::into_inner(b.Anonymous.UAV);
        },
        _ => unimplemented!()
    }
}

pub unsafe fn drop_barriers<const N: usize>(barriers:
                                            [D3D12_RESOURCE_BARRIER; N]) {
    for b in barriers {
        drop_barrier(b);
    }
}



impl MappableResource {
    unsafe fn map(&self, begin: usize, end: usize) -> Option<&mut [u8]> {
        let read_range = D3D12_RANGE { Begin: begin, End: end };
        let mut ptr: *mut c_void = null_mut();

        self.res.Map(0, &read_range, &mut ptr).ok()?;
        Some(core::slice::from_raw_parts_mut(ptr as *mut u8, self.size))
    }

    unsafe fn unmap(&self) {
        self.res.Unmap(0, null_mut());
    }

    pub fn write_with<F: FnOnce(&mut [u8])>(&self, func: F) -> Option<()> {
        let map = unsafe { self.map(0, self.size)? };
        func(map);
        unsafe { self.unmap(); }
        Some(())
    }


    pub fn read_with<T: Pod, F: FnOnce(&[T])>(&self, func: F) -> Option<()> {
        let map = unsafe { self.map(0, self.size)? };
        func(cast_slice(map));
        unsafe { self.unmap(); }
        Some(())
    }
}

#[allow(dead_code)]
#[derive(Default)]
pub enum BlendMode {
    #[default]
    Default,
    AlphaBlending,
}

pub struct RenderTargetState {
    pub format: DXGI_FORMAT,
    pub blend_mode: BlendMode,
}

impl Default for RenderTargetState {
    fn default() -> Self {
        Self {
            format: DXGI_FORMAT_R8G8B8A8_UNORM,
            blend_mode: BlendMode::default(),
        }
    }
}

pub struct DepthState {
    pub format: DXGI_FORMAT,
    pub test: bool,
    pub write: bool,
    pub func: D3D12_COMPARISON_FUNC,
}

impl Default for DepthState {
    fn default() -> Self {
        Self {
            test: true,
            write: true,
            func: D3D12_COMPARISON_FUNC_LESS,
            format: DXGI_FORMAT_D32_FLOAT,
        }
    }
}

pub struct RasterizerState {
    pub fill_mode: D3D12_FILL_MODE,
    pub cull_mode: D3D12_CULL_MODE,
    pub front_ccw: bool,
}

impl Default for RasterizerState {
    fn default() -> Self {
        Self {
            fill_mode: D3D12_FILL_MODE_SOLID,
            cull_mode: D3D12_CULL_MODE_BACK,
            front_ccw: true,
        }
    }
}


pub struct GraphicsPipelineState<'a> {
    pub vs: Option<&'a Shader<'a>>,
    pub ps: Option<&'a Shader<'a>>,
    pub rasterizer: RasterizerState,
    pub render_targets: &'a [RenderTargetState],
    pub depth: DepthState,
    pub input_layout: &'a [D3D12_INPUT_ELEMENT_DESC],
    pub primitive_topology: D3D12_PRIMITIVE_TOPOLOGY_TYPE,
}

impl<'a> Default for GraphicsPipelineState<'a> {
    fn default() -> Self {
        Self {
            vs: Default::default(),
            ps: Default::default(),
            rasterizer: Default::default(),
            render_targets: Default::default(),
            depth: Default::default(),
            input_layout: Default::default(),
            primitive_topology: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        }
    }
}

impl Context {
    pub fn create_graphics_pipeline_state(&self, state: &GraphicsPipelineState,
                                          rs: &ID3D12RootSignature)
        -> Option<ID3D12PipelineState> {

        let mut pso_desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: Some(rs.clone()),
            VS: state.vs.map(|x| x.bytecode()).unwrap_or_default(),
            PS: state.ps.map(|x| x.bytecode()).unwrap_or_default(),
            BlendState: D3D12_BLEND_DESC {
                AlphaToCoverageEnable: BOOL(0),
                ..Default::default()
            },
            SampleMask: u32::MAX,
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: state.rasterizer.fill_mode,
                CullMode: state.rasterizer.cull_mode,
                FrontCounterClockwise: BOOL(state.rasterizer.front_ccw as _),
                DepthBias: D3D12_DEFAULT_DEPTH_BIAS,
                DepthBiasClamp: D3D12_DEFAULT_DEPTH_BIAS_CLAMP,
                SlopeScaledDepthBias: D3D12_DEFAULT_SLOPE_SCALED_DEPTH_BIAS,
                DepthClipEnable: BOOL(1),
                MultisampleEnable: BOOL(0),
                AntialiasedLineEnable: BOOL(0),
                ForcedSampleCount: 0,
                ConservativeRaster: D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF,
            },
            DepthStencilState: D3D12_DEPTH_STENCIL_DESC {
                DepthEnable: BOOL(state.depth.test as i32),
                DepthWriteMask: if state.depth.write {
                    D3D12_DEPTH_WRITE_MASK_ALL
                } else {
                    D3D12_DEPTH_WRITE_MASK_ZERO
                },
                DepthFunc: state.depth.func,
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
            InputLayout: D3D12_INPUT_LAYOUT_DESC {
                pInputElementDescs: state.input_layout.as_ptr(),
                NumElements:        state.input_layout.len() as u32,
            },
            PrimitiveTopologyType: state.primitive_topology,
            NumRenderTargets: state.render_targets.len() as u32,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, ..Default::default() },
            DSVFormat: state.depth.format,
            NodeMask: 0,
            ..Default::default()
        };

        for (i, b) in state.render_targets.iter().enumerate() {
            let rt = pso_desc.BlendState.RenderTarget.get_mut(i)?;
            *rt = match &b.blend_mode {
                BlendMode::Default => D3D12_RENDER_TARGET_BLEND_DESC {
                    BlendEnable: BOOL(0),
                    LogicOpEnable: BOOL(0),
                    SrcBlend: D3D12_BLEND_ONE,
                    DestBlend: D3D12_BLEND_ZERO,
                    BlendOp: D3D12_BLEND_OP_ADD,
                    SrcBlendAlpha: D3D12_BLEND_ONE,
                    DestBlendAlpha: D3D12_BLEND_ZERO,
                    BlendOpAlpha: D3D12_BLEND_OP_ADD,
                    RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
                    ..Default::default()
                },

                BlendMode::AlphaBlending => D3D12_RENDER_TARGET_BLEND_DESC {
                    BlendEnable: BOOL(1),
                    LogicOpEnable: BOOL(0),
                    SrcBlend: D3D12_BLEND_SRC_ALPHA,
                    DestBlend: D3D12_BLEND_INV_SRC_ALPHA,
                    BlendOp: D3D12_BLEND_OP_ADD,
                    SrcBlendAlpha: D3D12_BLEND_ONE,
                    DestBlendAlpha: D3D12_BLEND_INV_SRC_ALPHA,
                    BlendOpAlpha: D3D12_BLEND_OP_ADD,
                    RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
                    ..Default::default()
                },
            };

            *pso_desc.RTVFormats.get_mut(i)? = b.format;
        }


        unsafe {
            self.device.CreateGraphicsPipelineState(&pso_desc).ok()
        }
    }
}

impl DescriptorHeap {
    pub fn alloc_descriptor(&self) -> Option<D3D12_CPU_DESCRIPTOR_HANDLE> {

        let top = self.top.get();

        if top.ptr >= self.end.ptr {
            None
        } else {
            self.top.set(D3D12_CPU_DESCRIPTOR_HANDLE {
                ptr: top.ptr + self.increment,
            });
            Some(top)
        }
    }

    pub fn offset(&self) -> usize {
        unsafe {
            (self.top.get().ptr - self.heap.GetCPUDescriptorHandleForHeapStart()
                .ptr) / self.increment
        }
    }
}
