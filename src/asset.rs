
use std::path::Path;
use std::fs::File;
use std::io::{Read, SeekFrom, Seek};
use std::alloc::{alloc, Layout};

use math::mat::Mat4;
use crate::mesh::Mesh;

const ASSET_FILE_MAGIC: u32 =  0xAF00AF00;
const ASSET_NAME_LENGTH: usize = 80;
const ASSET_TAG_LENGTH: usize = 28;

#[repr(packed)]
#[allow(dead_code)]
#[derive(Debug, Copy, Clone)]
struct Header {
    magic: u32,
    version: u32,
    table_entry_count: u64,
}

#[repr(u32)]
#[allow(dead_code)]
#[derive(Debug, Copy, Clone, PartialEq)]
enum AssetType {
    None = 0,
    Mesh = 1,
    Image = 2,
    Sound = 3,
    Cubemap = 4,
}

#[repr(packed)]
#[allow(dead_code)]
#[derive(Debug, Copy, Clone)]
struct TableEntry {
    name: [u8; ASSET_NAME_LENGTH],
    tag: [u8; ASSET_TAG_LENGTH],
    typ: AssetType,
    offset: u64,
    size: u64,
}

#[repr(u32)]
#[derive(Debug, Copy, Clone)]
enum MeshFlags {
    Animated = 1,
}

#[repr(packed)]
#[allow(dead_code)]
#[derive(Debug, Copy, Clone)]
struct AssetMesh {
    flags: MeshFlags,
    vertices_count: u32,
    indices_count: u32,
    textures_count: u32,
    joints_count: u32,
    animations_count: u32,
    vertex_data_offset: u32,
    texture_assets_offset: u32,
    joints_offsets: u32,
    animations_offset: u32,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct AssetFile {
    file: File,
    header: Header,
    entries: Vec<TableEntry>,
}

impl AssetFile {
    unsafe fn read_vector<T>(file: &mut File, count: usize) -> Option<Vec<T>> {
        let layout = Layout::array::<T>(count).ok()?;
        assert!(layout.size() == count * core::mem::size_of::<T>());

        let mut table_buf = 
            core::slice::from_raw_parts_mut(alloc(layout), layout.size());

        file.read_exact(&mut table_buf).ok()?;

        Some(Vec::from_raw_parts(table_buf.as_mut_ptr() as *mut T, 
                                 count as usize,
                                 count as usize))
    }

    unsafe fn read_vector_at_offset<T>(&mut self, count: usize, 
                                       offset: u64) -> Option<Vec<T>> {
        self.file.seek(SeekFrom::Start(offset)).ok()?;
        Self::read_vector(&mut self.file, count)
    }

    pub fn from_file(path: &Path) -> Option<Self> {
        let mut file = File::open(path).ok()?;

        let mut header_buf = [0; core::mem::size_of::<Header>()];
        file.read_exact(&mut header_buf).ok()?;

        let header: Header = unsafe { core::mem::transmute(header_buf) };

        if header.magic != ASSET_FILE_MAGIC {
            return None;
        }

        let entries = unsafe { 
            Self::read_vector(&mut file, header.table_entry_count as usize)? 
        };


        Some(Self {
            file,
            header,
            entries,
        })
    }

    fn find_asset(&self, name: &str, typ: AssetType) 
        -> Option<(u64, u64)> {

        for e in self.entries.iter() {
            let n = unsafe {
                std::ffi::CStr::from_ptr(e.name.as_ptr() as *const _).to_str()
                    .unwrap()
            };

            let entry_type = e.typ;
            if n == name && entry_type == typ {
               return Some((e.offset, e.size)); 
            }
        }

        None
    }

    pub fn load_mesh(&mut self, name: &str) -> Option<Mesh> {
        let (offset, size) = self.find_asset(name, AssetType::Mesh)?;

        if size < core::mem::size_of::<AssetMesh>() as u64 {
            return None;
        }

        let mut info_buf = [0; core::mem::size_of::<AssetMesh>()];
        self.file.seek(SeekFrom::Start(offset)).ok()?;
        self.file.read_exact(&mut info_buf).ok()?;

        let info: AssetMesh = unsafe { core::mem::transmute(info_buf) };

        let vertices_count = info.vertices_count as u64;

        let positions_offset = offset + info.vertex_data_offset as u64;
        let normals_offset   = positions_offset + 12 * vertices_count;
        let tangents_offset  = normals_offset   + 12 * vertices_count;
        let uvs_offset       = tangents_offset  + 12 * vertices_count;
        let mut indices_offset = uvs_offset     +  8 * vertices_count;

        if (info.flags as u32 & MeshFlags::Animated as u32) != 0 && 
           info.joints_count > 0 {

            let weights_offset = indices_offset;
            let joints_offset = weights_offset + 16 * vertices_count;
            indices_offset = joints_offset + 16 * vertices_count;
        }

        if offset.checked_add(size)? <
            indices_offset + 4 * info.indices_count as u64 {
            return None;
        }

        let positions = unsafe { 
            self.read_vector_at_offset(vertices_count as usize, 
                                       positions_offset)? 
        };

        let normals = unsafe { 
            self.read_vector_at_offset(vertices_count as usize, 
                                       normals_offset)? 
        };

        let indices = unsafe { 
            self.read_vector_at_offset(info.indices_count as usize, 
                                       indices_offset)? 
        };

        Some(Mesh {
            positions,
            indices,
            normals,

            transform: Mat4::identity(),
        })
    }
}



