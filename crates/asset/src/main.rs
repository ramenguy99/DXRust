#![allow(unused_imports)]

use std::path::Path;
use std::io::{Read, Write};

mod gltf;

use scene::{Scene, Serialize, Deserialize};

const IMPORT_BISTRO: bool = true;

fn print_scene_stats(scene: &Scene) {
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
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let input_path = Path::new(&args[1]);
    let textures_directory = Path::new(&args[2]);
    let output_path = Path::new(&args[3]);

    let scene = if IMPORT_BISTRO {
        gltf::BistroImporter::import(input_path, textures_directory)
            .expect("Failed to import bistro")
    } else {
        gltf::import_file(input_path).expect("Failed to import scene")
    };

    print_scene_stats(&scene);

    let vec = scene.serialize();
    let mut file = std::fs::File::create(output_path).expect("Failed to create output file");
    for c in vec.chunks(1024 * 1024 * 1024) {
        let compressed = lz4::block::compress(&c, None, true).expect("Failed to compress scene");
        file.write_all(&(compressed.len() as u32).to_le_bytes()).expect("Failed to write file");
        file.write_all(&compressed).expect("Failed to write file")
    }
}
