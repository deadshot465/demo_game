use glam::{Vec2, Vec3A, Vec4, Mat4};
use crate::game::shared::traits::disposable::Disposable;
use std::sync::{Arc, RwLock};
use crate::game::shared::structs::{Mesh, Vertex};
use crate::game::graphics::vk::{Graphics, Buffer};
use gltf::{Node, Scene};

pub struct Model<GraphicsType: 'static, BufferType: 'static + Disposable + Clone> {
    pub position: Vec3A,
    pub scale: Vec3A,
    pub rotation: Vec3A,
    pub color: Vec4,
    graphics: Arc<RwLock<GraphicsType>>,
    pub meshes: Vec<Mesh<BufferType>>,
    pub is_disposed: bool,
    pub model_name: String,
    pub model_index: usize,
}

impl<GraphicsType: 'static, BufferType: 'static + Disposable + Clone> Model<GraphicsType, BufferType> {
    pub fn new(file_name: &str, graphics: Arc<RwLock<GraphicsType>>,
               position: Vec3A, scale: Vec3A, rotation: Vec3A, color: Vec4) -> Self {
        let (document, buffer, _image) = gltf::import(file_name)
            .expect("Failed to import the model.");

        let meshes: Vec<Mesh<BufferType>>;
        if let Some(scene) = document.default_scene() {
            meshes = Model::<GraphicsType, BufferType>::process_root_nodes(scene, buffer);
        }
        else {
            meshes = Model::<GraphicsType, BufferType>::process_root_nodes(document.scenes().nth(0).unwrap(), buffer);
        }

        let x: f32 = rotation.x();
        let y: f32 = rotation.y();
        let z: f32 = rotation.z();

        Model {
            position,
            scale,
            rotation: Vec3A::new(x.to_radians(), y.to_radians(), z.to_radians()),
            color,
            graphics,
            meshes,
            is_disposed: false,
            model_name: file_name.to_string(),
            model_index: 0
        }
    }

    fn process_root_nodes(scene: Scene, buffer_data: Vec<gltf::buffer::Data>) -> Vec<Mesh<BufferType>> {
        let mut meshes = vec![];
        for node in scene.nodes() {
            let mut submeshes = Model::<GraphicsType, BufferType>::process_node(node, &buffer_data);
            meshes.append(&mut submeshes);
        }
        meshes
    }

    fn process_node(node: Node, buffer_data: &Vec<gltf::buffer::Data>) -> Vec<Mesh<BufferType>> {
        let mut meshes = vec![];
        if let Some(mesh) = node.mesh() {
            meshes.push(Model::<GraphicsType, BufferType>::process_mesh(mesh, &buffer_data));
        }
        for _node in node.children() {
            let mut submeshes = Model::<GraphicsType, BufferType>::process_node(_node, &buffer_data);
            meshes.append(&mut submeshes);
        }
        meshes
    }

    fn process_mesh(mesh: gltf::Mesh, buffer_data: &Vec<gltf::buffer::Data>) -> Mesh<BufferType> {
        let mut vertices: Vec<Vertex> = vec![];
        let mut indices: Vec<u32> = vec![];

        for primitive in mesh.primitives() {
            let reader = primitive
                .reader(|buffer| Some(&buffer_data[buffer.index()]));

            let mut positions: Vec<Vec3A> = vec![];
            let mut normals: Vec<Vec3A> = vec![];
            let mut tex_coords: Vec<Vec2> = vec![];

            if let Some(iter) = reader.read_positions() {
                for position in iter {
                    positions.push(glam::f32::vec3a(position[0], position[1], position[2]));
                }
            }
            if let Some(iter) = reader.read_normals() {
                for normal in iter {
                    normals.push(glam::f32::vec3a(normal[0], normal[1], normal[2]));
                }
            }
            if let Some(iter) = reader.read_tex_coords(0) {
                let mut has_texcoord = false;
                for tex_coord in iter.into_f32() {
                    has_texcoord = true;
                    tex_coords.push(Vec2::new(tex_coord[0], tex_coord[1]));
                }
                if !has_texcoord {
                    tex_coords.push(Vec2::new(0.0, 0.0));
                }
            }
            if let Some(iter) = reader.read_indices() {
                for index in iter.into_u32() {
                    indices.push(index);
                }
            }

            println!("Positions: {}", positions.len());
            println!("Normals: {}", normals.len());
            println!("TexCoords: {}", tex_coords.len());
            println!("Indices: {}", indices.len());

            for i in 0..positions.len() {
                let tex_coord = if let Some(_tex_coord) = tex_coords.get(i) {
                    *_tex_coord
                } else {
                    Vec2::new(0.0, 0.0)
                };
                vertices.push(Vertex::new(positions[i], normals[i], tex_coord));
            }
        }
        Mesh {
            vertices,
            indices,
            vertex_buffer: None,
            index_buffer: None,
            is_disposed: false
        }
    }

    pub fn get_world_matrix(&self) -> Mat4 {
        let world = Mat4::identity();
        let scale = Mat4::from_scale(glam::Vec3::from(self.scale));
        let rotation_x = Mat4::from_rotation_x(self.rotation.x());
        let rotation_y = Mat4::from_rotation_y(self.rotation.y());
        let rotation_z = Mat4::from_rotation_z(self.rotation.z());
        let translation = Mat4::from_translation(glam::Vec3::from(self.position));
        let rotate = rotation_z * rotation_y * rotation_x;
        scale * rotate * translation * world
    }
}

impl<GraphicsType: 'static, BufferType: 'static + Disposable + Clone> From<&Model<GraphicsType, BufferType>> for Model<GraphicsType, BufferType> {
    fn from(model: &Model<GraphicsType, BufferType>) -> Self {
        Model {
            position: model.position,
            scale: model.scale,
            rotation: model.rotation,
            color: model.color,
            graphics: model.graphics.clone(),
            meshes: model.meshes.to_vec(),
            is_disposed: false,
            model_name: model.model_name.clone(),
            model_index: 0
        }
    }
}

impl<GraphicsType: 'static, BufferType: 'static + Disposable + Clone> Drop for Model<GraphicsType, BufferType> {
    fn drop(&mut self) {
        if !self.is_disposed {
            self.dispose();
        }
    }
}

impl<GraphicsType: 'static, BufferType: 'static + Disposable + Clone> Disposable for Model<GraphicsType, BufferType> {
    fn dispose(&mut self) {
        for mesh in self.meshes.iter_mut() {
            mesh.dispose();
        }
        self.is_disposed = true;
    }

    fn is_disposed(&self) -> bool {
        self.is_disposed
    }

    fn get_name(&self) -> &str {
        self.model_name.as_str()
    }

    fn set_name(&mut self, name: String) -> &str {
        self.model_name = name;
        self.model_name.as_str()
    }
}