use crate::game::shared::structs::Vertex;
use glam::{Vec4};

pub struct SkinnedVertex {
    pub vertex: Vertex,
    pub joints: Option<Vec4>,
    pub weights: Option<Vec4>,
}

pub struct SkinnedModel {

}

impl SkinnedModel {
    /*pub fn load_skinned_mesh(file_name: &str, graphics: Weak<ShardedLock<GraphicsType>>,
                             position: Vec3A, scale: Vec3A, rotation: Vec3A, color: Vec4) -> Self {
        let raw_data = match std::fs::read(file_name) {
            Ok(d) => d,
            Err(e) => {
                log::error!("Error reading the model: {}", e.to_string());
                None
            }
        };
        let gltf = gltf::Gltf::from_slice(raw_data.as_slice()).unwrap();
        let blob = gltf.blob.as_ref().unwrap();

        let mut textures = vec![];
        for texture in gltf.textures() {
            match texture.source().source() {
                gltf::image::Source::View {
                    view, mime_type
                } => {
                    let slice = &blob[view.offset()..view.offset() + view.length() - 1];
                    let png = image::load_from_memory_with_format(slice, if mime_type == "image/jpeg" {
                        ImageFormat::Jpeg
                    } else {
                        ImageFormat::Png
                    });
                    let png = match png {
                        Ok(d) => d,
                        Err(e) => {
                            log::error!("Failed to load image from the memory.");
                            None
                        }
                    };
                    let image = match png.as_bgra8() {
                        Some(d) => d,
                        None => {
                            log::error!("Cannot convert the image to BGRA8 format.");
                        }
                    };
                    let buffer_size = image.height() * image.width() * 4;
                    let data = image.to_vec();
                    let _graphics = graphics.upgrade();
                    if let Some(g) = _graphics {
                        let lock = g.read().unwrap();
                        let img = lock.create_image(data.as_slice(), buffer_size as u64, image.width(), image.height());
                        textures.push(img);
                    }
                },
                _ => {
                    log::error!("The format has to be a binary GLTF.");
                }
            }
        }

        let scene = gltf.default_scene();
        let meshes: Vec<Mesh<BufferType, TextureType>>;
        if let Some(root_scene) = scene {
            meshes = Model::<GraphicsType, BufferType, CommandType, TextureType>::process_skinned_mesh_root_nodes(root_scene, blob, &textures);
        }
        else {
            meshes = Model::<GraphicsType, BufferType, CommandType, TextureType>::process_skinned_mesh_root_nodes(gltf.scenes().nth(0).unwrap(), blob, &textures);
        }

        let model = Model {
            position,
            scale,
            rotation,
            color,
            meshes: vec![],
            is_disposed: false,
            model_name: "".to_string(),
            model_index: 0,
            graphics: Default::default(),
            phantom: Default::default(),
            phantom_2: Default::default()
        };
        model
    }

    fn process_skinned_mesh_root_nodes(scene: Scene, buffer_data: &Vec<u8>, textures: &Vec<TextureType>) -> Vec<Mesh<BufferType, TextureType>> {
        let mut meshes = vec![];
        for node in scene.nodes() {
            let mut sub_meshes = Model::<GraphicsType, BufferType, CommandType, TextureType>::process_skinned_mesh_node(node, &buffer_data, textures);
            meshes.append(&mut sub_meshes);
        }
        meshes
    }

    fn process_skinned_mesh_node(node: Node, buffer_data: &Vec<u8>, textures: &Vec<TextureType>) -> Vec<Mesh<BufferType, TextureType>> {
        let mut meshes = vec![];
        let (t, r, s) = node.transform().decomposed();
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::from(s),
            Quat::from(r),
            Vec3::from(t)
        );
        if let Some(mesh) = node.mesh() {
            meshes.push(Model::<GraphicsType, BufferType, CommandType, TextureType>::process_skinned_mesh(&node, mesh, &buffer_data, transform.clone(), textures));
        }
        for _node in node.children() {
            let mut sub_meshes = Model::<GraphicsType, BufferType, CommandType, TextureType>::process_skinned_mesh_node(_node, &buffer_data, textures);
            let (t, r, s) = transform.clone().decomposed();
            let transform_matrix = Mat4::from_scale_rotation_translation(
                glam::Vec3::from(s),
                glam::Quat::from(r),
                glam::Vec3::from(t)
            );
            for mesh in sub_meshes.iter_mut() {
                for vertex in mesh.vertices.iter_mut() {
                    vertex.position = Vec3A::from(transform_matrix.transform_point3(glam::Vec3::from(vertex.position)));
                }
            }
            meshes.append(&mut submeshes);
        }
        meshes
    }

    fn process_skinned_mesh(node: &Node, mesh: gltf::Mesh, buffer_data: &Vec<u8>, mut local_transform: Transform, textures: &Vec<TextureType>) -> Mesh<BufferType, TextureType> {
        let mut root_joint = None;
        if let Some(skin) = node.skin() {
            let joints: Vec<_> = skin.joints().collect();
            if !joints.is_empty() {
                let reader = skin.reader(|buffer| {
                    match buffer.source() {
                        gltf::buffer::Source::Bin => (),
                        gltf::buffer::Source::Uri(uri) => {
                            log::error!("URI-based skins are not supported.");
                        }
                    }
                    Some(&buffer_data)
                });
                let ibm: Vec<Mat4> = reader.read_inverse_bind_matrices()
                    .unwrap()
                    .map(|r| r.into())
                    .collect();
                let node_to_joints_lookup: Vec<_> = joints.iter().map(|n| n.index()).collect();
                root_joint = Some()
            }
        }
        Mesh
    }

    fn process_skeleton(node: &Node, buffer_data: &[u8], node_to_joints_lookup: &[usize], inverse_bind_matrices: &[Mat4], local_transform: Mat4) -> Joint {
        let mut children = vec![];
        let node_index = node.index();
        let index = node_to_joints_lookup.iter()
            .enumerate()
            .find(|(_, x)| **x == node_index)
            .unwrap()
            .0;
        let name = node.name().unwrap_or("");
        let ibm = inverse_bind_matrices[index];
        let (t, r, s) = node.transform().decomposed();
        let node_transform = Mat4::from_scale_rotation_translation(
            Vec3::from(s),
            Quat::from(r),
            Vec3::from(t)
        );
        let pose_transform = local_transform * node_transform;
        for child in node.children() {
            children.push(Self::process_skeleton(node, buffer_data, node_to_joints_lookup, inverse_bind_matrices, pose_transform.clone()));
        }
        let ibm = ibm.clone();


        ()
    }*/
}