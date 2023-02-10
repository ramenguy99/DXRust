use std::path::Path;
use std::io::{Read, Write};

mod gltf;


use scene::{Scene, Serialize, Deserialize};


fn main() {
    let args: Vec<String> = std::env::args().collect();
    let input_path = Path::new(&args[1]);
    let output_path = Path::new(&args[2]);

    /*
    let vec: Vec<u8> = vec![0u8; 0x51d0a443];
    let mut file = std::fs::File::create(output_path).expect("Failed to create output file");
    for c in vec.chunks(1024 * 1024 * 1024) {
        let compressed = lz4::block::compress(&c, None, true).expect("Failed to compress scene");
        file.write_all(&compressed).expect("Failed to write file")
    }
    */


    if false {
        let scene = gltf::import_file(input_path).expect("Failed to import scene");

        let vec = scene.serialize();

        let mut file = std::fs::File::create(output_path).expect("Failed to create output file");
        for c in vec.chunks(1024 * 1024 * 1024) {
            let compressed = lz4::block::compress(&c, None, true).expect("Failed to compress scene");
            file.write_all(&(compressed.len() as u32).to_le_bytes());
            file.write_all(&compressed).expect("Failed to write file")
        }
    }


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

}
