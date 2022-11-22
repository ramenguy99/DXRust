use image::io::Reader as ImageReader;
use gltf::{Gltf};

use std::path::Path;
use std::io::{BufReader, Cursor};
use std::fs::File;

use crate::scene::{Mesh, Scene, Image, Format, Material};
use math::{
    vec::{Vec2, Vec3},
    mat::Mat4,
    quat::Quat,
};


pub fn import_node(blob: &Vec<u8>, scene: &mut Scene, node: gltf::Node,
    parent: Mat4) -> Option<()> {

    let local_transform = match node.transform() {
        gltf::scene::Transform::Matrix { matrix } => Mat4 { e: matrix },
        gltf::scene::Transform::Decomposed { translation, rotation, scale } =>
            Mat4::translation(Vec3::from_slice(&translation)) *
            Quat::from_slice(&rotation).to_mat4() *
            Mat4::scale3(Vec3::from_slice(&scale).into()),
    };

    let transform = parent * local_transform;

    if let Some(mesh) = node.mesh() {

        for primitive in mesh.primitives() {

            if let Some(acc) = primitive.indices() {
                assert!(acc.count() % 3 == 0);
            } else {
                panic!();
            }

            let reader = primitive.reader(|_buf| Some(&blob[..]));
            let positions = reader.read_positions().unwrap()
                .map(|x| Vec3::from_slice(&x)).collect();
            let normals = reader.read_normals().unwrap()
                .map(|x| Vec3::from_slice(&x)).collect();
            let uvs = reader.read_tex_coords(0).unwrap().into_f32()
                .map(|x| Vec2::from_slice(&x)).collect();

            use gltf::mesh::util::ReadIndices;
            let indices = match reader.read_indices().unwrap() {
                ReadIndices::U8(it) => it.map(|x| x as u32).collect(),
                ReadIndices::U16(it) => it.map(|x| x as u32).collect(),
                ReadIndices::U32(it) => it.collect(),
            };

            let mat = primitive.material();

            let material = Material {
                albedo_texture: mat.pbr_metallic_roughness()
                    .base_color_texture().unwrap()
                    .texture().source().index() as u32,
            };

            scene.meshes.push(Mesh {
                positions,
                normals,
                uvs,
                indices,
                transform: transform,
                material,
            });
        }
    }

    use gltf::khr_lights_punctual::Kind;
    if let Some(light) = node.light() {
        match light.kind() {
            Kind::Directional => println!("Directional"),
            Kind::Point => println!("Point"),
            Kind::Spot { inner_cone_angle, outer_cone_angle } =>
                println!("Spot {inner_cone_angle}, {outer_cone_angle}"),
        }
    }

    for n in node.children() {
        import_node(blob, scene, n, transform)?;
    }

    Some(())
}


pub fn import_file(path: &Path) -> Option<Scene> {
    let mut scene = Scene::new();


    let reader = BufReader::new(File::open(path).ok()?);
    let mut gltf = Gltf::from_reader_without_validation(reader).ok()?;
    let blob = gltf.blob.take().unwrap();

    for s in gltf.scenes() {
        for n in s.nodes() {
            import_node(&blob, &mut scene, n, Mat4::identity())?;
        }
    }

    for (_i, img) in gltf.images().enumerate() {
        match img.source() {
            gltf::image::Source::View { view, mime_type: _ } => {
                let begin = view.offset();
                let end = begin + view.length();
                let encoded_image = &blob[begin..end];
                let enc = ImageReader::new(Cursor::new(encoded_image))
                    .with_guessed_format().unwrap();
                let img = enc.decode().unwrap();
                let img = img.to_rgba8();
                scene.images.push(Image {
                    width: img.width(),
                    height: img.height(),
                    data: img.into_raw(),
                    format: Format::RGBA8,
                });
            },
            _ => panic!(),
        }
    }

    return Some(scene);
}

