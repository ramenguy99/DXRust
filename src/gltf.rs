use gltf::{
    Gltf,
    Document,
    buffer,
    image,
};

use std::path::Path;
use std::io::BufReader;
use std::fs::File;

use crate::mesh::{Mesh, Scene};
use math::{
    vec::Vec3,
    mat::Mat4,
    quat::Quat,
};


pub fn import_node(blob: &Vec<u8>, scene: &mut Scene, node: gltf::Node, parent: Mat4) 
    -> Option<()> {

    let local_transform = match node.transform() {
        gltf::scene::Transform::Matrix { matrix } => Mat4 { e: matrix },   
        gltf::scene::Transform::Decomposed { translation, rotation, scale } => 
            Mat4::translation(Vec3::from_slice(&translation)) * 
            Quat::from_slice(&rotation).to_mat4() *
            Mat4::scale3(Vec3::from_slice(&scale).into()),
    };

    let world_transform = parent * local_transform;

    if let Some(mesh) = node.mesh() {

        for primitive in mesh.primitives() {

            if let Some(acc) = primitive.indices() {
                assert!(acc.count() % 3 == 0);
            } else {
                panic!();
            }

            let reader = primitive.reader(|_buf| Some(&blob[..]));
            let positions = reader.read_positions().unwrap().map(|x| Vec3::from_slice(&x)).collect();
            let normals = reader.read_normals().unwrap().map(|x| Vec3::from_slice(&x)).collect();
            let indices = match reader.read_indices().unwrap() {
                gltf::mesh::util::ReadIndices::U8(it) => it.map(|x| x as u32).collect(),
                gltf::mesh::util::ReadIndices::U16(it) => it.map(|x| x as u32).collect(),
                gltf::mesh::util::ReadIndices::U32(it) => it.collect(),
            };

            scene.meshes.push(Mesh {
                positions,
                normals,
                indices,
                transform: world_transform,
            });
        }
    }

    for n in node.children() {
        import_node(blob, scene, n, world_transform)?;
    }

    Some(())
}


pub fn import_file(path: &Path) -> Option<Scene> {
    let mut scene = Scene::new();
    
    
    let reader = BufReader::new(File::open(path).ok()?);
    let gltf = Gltf::from_reader_without_validation(reader).ok()?;

    for s in gltf.scenes() {
        for n in s.nodes() {
            import_node(gltf.blob.as_ref()?, &mut scene, n, Mat4::identity())?;
        }
    }

    return Some(scene);
}


/*
#[derive(Debug)]
struct Stats {
    num_triangles: u64,
    num_primitives: u64,
    num_meshes: u64,

    mesh_refs: HashMap<usize, u64>,
    primitive_refs: HashMap<usize, u64>,
    buffer_refs: HashMap<usize, u64>,
    max_primitives_per_mesh: u64,
}

// Gltf file:
// document: scene information
// buffers: vertex/index buffers
// images: texture data
//
// Scene: tree of nodes
// Node: camera or mesh + transform
// Mesh: array of Primitives (+ morph target weights - useless for us)
// Primitive: geomtry data (mode, index buffer, attribute data)
//
// buffers: Vec<Data>
// data: Vec<u8>


fn print_node(node: gltf::Node, indent: u32, stats: &mut Stats) {
    for i in 0..indent {
//        print!(" ");
    }

    if let Some(mesh) = node.mesh() {
        stats.mesh_refs.entry(mesh.index()).and_modify(|e| { *e += 1 }).or_insert(1);

        for primitive in mesh.primitives() {
            stats.primitive_refs.entry(primitive.index()).and_modify(|e| { *e += 1 }).or_insert(1);

            assert!(matches!(primitive.mode(), gltf::mesh::Mode::Triangles));
            stats.num_primitives += 1;
            if let Some(acc) = primitive.indices() {
                stats.num_triangles += (acc.count() / 3) as u64;
                assert!(acc.count() % 3 == 0);
            } else {
                panic!();
            }

            let attrs = [
                primitive.get(&gltf::Semantic::Positions).unwrap().view().unwrap().offset(),
                primitive.get(&gltf::Semantic::Normals).unwrap().view().unwrap().offset(),
                primitive.get(&gltf::Semantic::TexCoords(0)).unwrap().view().unwrap().offset(),
                primitive.get(&gltf::Semantic::TexCoords(1)).unwrap().view().unwrap().offset(),
            ];
            
            for a in attrs {
                stats.buffer_refs.entry(a).and_modify(|e| { *e += 1 }).or_insert(1);
            }
        }
        stats.max_primitives_per_mesh = stats.max_primitives_per_mesh.max(mesh.primitives().count() as u64);
        stats.num_meshes += 1;
        assert!(mesh.weights() == None);
    }

//    println!("#{} - {} -> {}", node.index(), node.name().unwrap_or("<unknown>"), 
//             node.mesh().map(|x| x.name().unwrap_or("No name")).unwrap_or("No mesh"));

    for n in node.children() {
        print_node(n, indent + 1, stats);
    }
}


fn main() {
    let gltf = Gltf::open("../../Bistro/GLB/bistro.glb").unwrap();
    let mut stats = Stats {
        num_triangles: 0,
        num_primitives: 0,
        num_meshes: 0,
        mesh_refs: HashMap::new(),
        primitive_refs: HashMap::new(),
        buffer_refs: HashMap::new(),
        max_primitives_per_mesh: 0,
    };
    for scene in gltf.scenes() {
        for node in scene.nodes() {
            print_node(node, 0, &mut stats);
        }
    }

    println!("{} triangles - {} primitives ({}) - {} meshes ({})- {} buffers", 
             stats.num_triangles, stats.num_primitives, stats.primitive_refs.keys().count(), stats.num_meshes,
             stats.mesh_refs.keys().count(), stats.buffer_refs.keys().count());
    let max_mesh_refs = stats.mesh_refs.values().fold(0, |a, b| a.max(*b));
    let max_primitive_refs = stats.primitive_refs.values().fold(0, |a, b| a.max(*b));
    let max_buffer_refs = stats.buffer_refs.values().fold(0, |a, b| a.max(*b));
    println!("{max_mesh_refs} max mesh refs, {max_primitive_refs} max primitive refs, {} max primitives per mesh, {max_buffer_refs} max buffer refs", stats.max_primitives_per_mesh);
}
*/
