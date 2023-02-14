use image::io::Reader as ImageReader;
use gltf::{Gltf};

use std::path::{Path, PathBuf};
use std::io::{BufReader, Cursor};
use std::fs::File;
use std::collections::HashMap;

use scene::{Mesh, Scene, Image, Format, Material, MaterialParameter};
use math::{
    vec::{Vec2, Vec3, Vec4},
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

        // println!("Node: {}", node.index());

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
                base_color: MaterialParameter::Texture(mat.pbr_metallic_roughness()
                    .base_color_texture().unwrap()
                    .texture().source().index() as u32),
                normal: MaterialParameter::None,
                specular: MaterialParameter::None,
                emissive: MaterialParameter::None,
            };

            println!("{}",
                mat.pbr_metallic_roughness().base_color_texture().unwrap().texture().source().name().unwrap());
                // mat.emissive_factor());

            /*
            println!("{:?}", mat.pbr_metallic_roughness()
            .base_color_texture().unwrap()
            .texture().source().name());
        */

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
        for (i, n) in s.nodes().enumerate() {
            import_node(&blob, &mut scene, n, Mat4::identity())?;
        }
    }

    /*
    // println!("Importing images");

    for (i, img) in gltf.images().enumerate() {
        // println!("Image: {i}");

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
    */

    return Some(scene);
}

pub struct BistroImporter {
    scene: Scene,
    textures_map: HashMap<PathBuf, MaterialParameter>,
    current_texture: u32,
    texture_directory: PathBuf,
}

enum TextureType {
    BaseColor,
    Normal,
    Specular,
    Emissive,
}

impl BistroImporter {
    pub fn import(glb_path: &Path, texture_directory: &Path) -> Option<Scene> {
        let mut importer = BistroImporter {
            scene: Scene::new(),
            textures_map: HashMap::new(),
            current_texture: 0,
            texture_directory: texture_directory.to_path_buf(),
        };

        let reader = BufReader::new(File::open(glb_path).ok()?);
        let mut gltf = Gltf::from_reader_without_validation(reader).ok()?;
        let blob = gltf.blob.take().unwrap();

        for s in gltf.scenes() {
            for (i, n) in s.nodes().enumerate() {
                importer.import_node(&blob, n, Mat4::identity())?;
            }
        }

        Some(importer.scene)
    }

    fn import_node(&mut self, blob: &Vec<u8>, node: gltf::Node, parent: Mat4)
        -> Option<()> {

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

                let material = self.material(&primitive.material());

                self.scene.meshes.push(Mesh {
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
            self.import_node(blob, n, transform)?;
        }

        Some(())
    }

    fn material(&mut self, mat: &gltf::Material) -> Material {
        let base_color = mat.pbr_metallic_roughness()
            .base_color_texture().unwrap()
            .texture().source().name().unwrap();
        let name = base_color.split('-').nth(0).unwrap();
        let name = &name[..name.len() - 9];

        let base_color = format!("{}BaseColor.png", name);
        let normal     = format!("{}Normal.png",    name);
        let specular   = format!("{}Specular.png",  name);
        let emissive   = format!("{}Emissive.png",  name);

        let base_color = self.texture(&base_color, Format::RGBA8);
        let normal     = self.texture(&normal,     Format::RGBA8);
        let specular   = self.texture(&specular,   Format::RGBA8);
        let emissive   = self.texture(&emissive,   Format::RGBA8);

        Material {
            base_color,
            normal,
            specular,
            emissive,
        }
    }

    fn texture(&mut self, path: &str, format: Format) -> MaterialParameter {
        let path = self.texture_directory.join(path);

        if !path.exists() {
            println!("{path:?}");
            return MaterialParameter::None;
        }

        use std::collections::hash_map::Entry;
        match self.textures_map.entry(path) {
            Entry::Occupied(o) => *o.into_mut(),
            Entry::Vacant(v) => {
                use image::DynamicImage;

                let img = ImageReader::open(&v.key()).unwrap().decode().unwrap();
                let param = match format {
                    Format::RGBA8 => {
                        let img = img.into_rgba8();
                        if img.width() == 1 && img.height() == 1 {
                            let rgb = img.get_pixel(0, 0).0;
                            let v = Vec4::new(
                                rgb[0] as f32 / 255.0,
                                rgb[1] as f32 / 255.0,
                                rgb[2] as f32 / 255.0,
                                rgb[3] as f32 / 255.0
                            );
                            MaterialParameter::Vec4(v)
                        } else {
                            self.scene.images.push(Image {
                                width: img.width(),
                                height: img.height(),
                                data: img.into_raw(),
                                format,
                            });
                            let param = MaterialParameter::Texture(
                                self.current_texture);
                            self.current_texture += 1;
                            param
                        }
                    }
                };
                v.insert(param);
                param
            }
        }
    }
}