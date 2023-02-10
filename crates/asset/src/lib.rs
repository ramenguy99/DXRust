use std::path::Path;
use scene::{Scene, Deserialize};
use std::io::Read;

pub fn load_scene_from_file(path: &Path) -> Option<Scene> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut data: Vec<u8> = Vec::new();
    file.read_to_end(&mut data).ok()?;

    let mut view = &data[..];
    let mut total_size = 0;
    let mut buf: Vec<u8> = vec![0; 4 * 1024 * 1024 * 1024];
    loop {
        if view.len() == 0 {
            break;
        }
        let c_size = u32::from_le_bytes((&view[..4]).try_into().unwrap()) as usize;
        let u_size = u32::from_le_bytes((&view[4..8]).try_into().unwrap()) as usize;

        lz4::block::decompress_to_buffer(&view[4..4 + c_size], None, &mut buf[total_size..]).unwrap();

        view = &view[4 + c_size..];
        total_size += u_size;
    }

    let buf = &mut &buf[..total_size];
    let scene = Scene::deserialize(buf);
    assert!(buf.len() == 0);

    Some(scene)
}
