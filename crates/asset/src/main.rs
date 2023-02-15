#![allow(unused_imports)]

use std::path::Path;
use std::io::{Read, Write};

mod gltf;


use scene::{Scene, Serialize, Deserialize};


fn main() {
    let args: Vec<String> = std::env::args().collect();
    let input_path = Path::new(&args[1]);
    let textures_directory = Path::new(&args[2]);
    let output_path = Path::new(&args[3]);

    /*
    let vec: Vec<u8> = vec![0u8; 0x51d0a443];
    let mut file = std::fs::File::create(output_path).expect("Failed to create output file");
    for c in vec.chunks(1024 * 1024 * 1024) {
        let compressed = lz4::block::compress(&c, None, true).expect("Failed to compress scene");
        file.write_all(&compressed).expect("Failed to write file")
    }
    */


    if true {
        // let scene = gltf::import_file(input_path).expect("Failed to import scene");
        let scene = gltf::BistroImporter::import(input_path, textures_directory)
            .expect("Failed to import bistro");

        let mut base_none = 0;
        let mut spec_none = 0;
        let mut emis_none = 0;
        let mut norm_none = 0;

        let mut base_text = 0;
        let mut spec_text = 0;
        let mut emis_text = 0;
        let mut norm_text = 0;

        let mut base_const = 0;
        let mut spec_const = 0;
        let mut emis_const = 0;
        let mut norm_const = 0;

        use scene::MaterialParameter;

        for m in scene.meshes.iter() {
            match m.material.base_color {
                MaterialParameter::None       => base_none  += 1,
                MaterialParameter::Texture(_) => base_text  += 1,
                MaterialParameter::Vec4(_)    => base_const += 1,
                _ => unreachable!(),
            }

            match m.material.specular {
                MaterialParameter::None       => spec_none  += 1,
                MaterialParameter::Texture(_) => spec_text  += 1,
                MaterialParameter::Vec4(_)    => spec_const += 1,
                _ => unreachable!(),
            }

            match m.material.normal {
                MaterialParameter::None       => norm_none  += 1,
                MaterialParameter::Texture(_) => norm_text  += 1,
                MaterialParameter::Vec4(_)    => norm_const += 1,
                _ => unreachable!(),
            }

            match m.material.emissive {
                MaterialParameter::None       => emis_none  += 1,
                MaterialParameter::Texture(_) => emis_text  += 1,
                MaterialParameter::Vec4(_)    => emis_const += 1,
                _ => unreachable!(),
            }
        }
        println!("Scene: {} meshes and {} images", scene.meshes.len(), scene.images.len());
        println!("Base: {base_none:4} / {base_const:4} / {base_text:4}");
        println!("Spec: {spec_none:4} / {spec_const:4} / {spec_text:4}");
        println!("Norm: {norm_none:4} / {norm_const:4} / {norm_text:4}");
        println!("Emis: {emis_none:4} / {emis_const:4} / {emis_text:4}");

        let vec = scene.serialize();

        let mut file = std::fs::File::create(output_path).expect("Failed to create output file");
        for c in vec.chunks(1024 * 1024 * 1024) {
            let compressed = lz4::block::compress(&c, None, true).expect("Failed to compress scene");
            file.write_all(&(compressed.len() as u32).to_le_bytes()).expect("Failed to write file");
            file.write_all(&compressed).expect("Failed to write file")
        }
    }

/*
    else {
        let mut total_secs: f64 = 0.0;

        let mut file = std::fs::File::open(output_path).expect("Failed to open input file");
        let mut data: Vec<u8> = Vec::new();

        let begin = std::time::Instant::now();
        file.read_to_end(&mut data);

        let bytes = data.len();
        let secs = begin.elapsed().as_secs_f64();
        total_secs += secs;
        println!("Read {} bytes in {:.3}s ({:.3} GB/s)", bytes, secs, bytes as f64 / (1024. * 1024. * 1024. * secs));

        let begin = std::time::Instant::now();
        let mut view = &data[..];
        let mut total_size = 0;

        //let mut data: Vec<u8> = Vec::new();

        let mut buf: Vec<u8> = vec![0; 4 * 1024 * 1024 * 1024];
        loop {
            if view.len() == 0 {
                break;
            }

            //let u = decompress(view

            let c_size = u32::from_le_bytes((&view[..4]).try_into().unwrap()) as usize;
            let u_size = u32::from_le_bytes((&view[4..8]).try_into().unwrap()) as usize;

            lz4::block::decompress_to_buffer(&view[4..4 + c_size], None, &mut buf[total_size..]).unwrap();
            //data.extend_from_slice();
//            println!("{:x} {:?} {:x}", size, view.as_ptr(), view.len());

            view = &view[4 + c_size..];
            total_size += u_size;
        }

        let secs = begin.elapsed().as_secs_f64();
        total_secs += secs;
        println!("Decompressed {} bytes into {} bytes {:.3}s ({:.3} GB/s)", bytes, total_size, secs, total_size as f64 / (1024. * 1024. * 1024. * secs));

        let begin = std::time::Instant::now();

        let buf = &mut &buf[..total_size];
        let scene = Scene::deserialize(buf);
        assert!(buf.len() == 0);

        let secs = begin.elapsed().as_secs_f64();
        total_secs += secs;
        println!("Parsed scene in {:.3}s ({:.3} GB/s)", secs, total_size as f64 / (1024. * 1024. * 1024. * secs));

        println!("Total time {:.3}s", total_secs);
    }

    // let mut writer = std::io::BufWriter::new(std::fs::File::create(output_path).unwrap());




/*
    let begin = std::time::Instant::now();
    let args: Vec<String> = std::env::args().collect();
    let input_path = Path::new(&args[1]);
    let mut reader = std::io::BufReader::with_capacity(1024 * 1024, std::fs::File::open(input_path).unwrap());
    let scene = Scene::deserialize(&mut reader).unwrap();

    let secs = begin.elapsed().as_secs_f64();
    println!("Read {:.3}s", secs);
*/

    /*
    let begin = std::time::Instant::now();

    let args: Vec<String> = std::env::args().collect();
    let input_path = Path::new(&args[1]);
    let mut reader = std::fs::File::open(input_path).unwrap();

    let mut scene: Vec<u8> = Vec::new();
    reader.read_to_end(&mut scene);

    let bytes = scene.len();
    let secs = begin.elapsed().as_secs_f64();
    println!("Read {} bytes in {:.3}s ({:.3} GB/s)", bytes, secs, bytes as f64 / (1024. * 1024. * 1024. * secs));
    */
*/

}
