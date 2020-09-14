use glam::{Vec2, Vec3, Vec3A, Vec4, Mat4, Quat};
use crate::game::shared::traits::disposable::Disposable;
use std::sync::Weak;
use crate::game::shared::structs::{Mesh, Vertex, PushConstant};
use crate::game::graphics::vk::{Graphics, Buffer, Image};
use gltf::{Node, Scene};
use crate::game::traits::GraphicsBase;
use std::mem::ManuallyDrop;
use ash::vk::{CommandBuffer, PipelineBindPoint, ShaderStageFlags, IndexType};
use std::convert::TryFrom;
use ash::version::DeviceV1_0;
use crate::game::shared::enums::ShaderType;
use winapi::_core::marker::PhantomData;
use gltf::scene::Transform;
use crossbeam::sync::ShardedLock;

pub struct Model<GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>, BufferType: 'static + Disposable + Clone, CommandType: 'static, TextureType: 'static + Clone + Disposable> {
    pub position: Vec3A,
    pub scale: Vec3A,
    pub rotation: Vec3A,
    pub color: Vec4,
    pub meshes: Vec<Mesh<BufferType, TextureType>>,
    pub is_disposed: bool,
    pub model_name: String,
    pub model_index: usize,
    graphics: Weak<ShardedLock<GraphicsType>>,
    phantom: PhantomData<&'static CommandType>,
    phantom_2: PhantomData<&'static TextureType>,
}

impl<GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>, BufferType: 'static + Disposable + Clone, CommandType: 'static, TextureType: 'static + Clone + Disposable> Model<GraphicsType, BufferType, CommandType, TextureType> {
    pub fn new(file_name: &str, graphics: Weak<ShardedLock<GraphicsType>>,
               position: Vec3A, scale: Vec3A, rotation: Vec3A, color: Vec4) -> Self {
        let (document, buffer, image) = gltf::import(file_name)
            .expect("Failed to import the model.");

        let mut meshes: Vec<Mesh<BufferType, TextureType>>;
        if let Some(scene) = document.default_scene() {
            meshes = Self::process_root_nodes(scene, buffer, image);
        }
        else {
            meshes = Self::process_root_nodes(document.scenes().nth(0).unwrap(), buffer, image);
        }

        let _graphics = graphics.upgrade();
        if let Some(g) = _graphics {
            let lock = g.read().unwrap();
            for mesh in meshes.iter_mut() {
                let vertex_buffer = lock.create_vertex_buffer(mesh.vertices.as_slice());
                mesh.vertex_buffer = Some(ManuallyDrop::new(vertex_buffer));
                let index_buffer = lock.create_index_buffer(mesh.indices.as_slice());
                mesh.index_buffer = Some(ManuallyDrop::new(index_buffer));
            }
            drop(lock);
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
            model_index: 0,
            phantom: PhantomData,
            phantom_2: PhantomData,
        }
    }

    fn process_root_nodes(scene: Scene, buffer_data: Vec<gltf::buffer::Data>, image_data: Vec<gltf::image::Data>) -> Vec<Mesh<BufferType, TextureType>> {
        let mut meshes = vec![];
        for node in scene.nodes() {
            let mut submeshes = Self::process_node(node, &buffer_data, &image_data);
            meshes.append(&mut submeshes);
        }
        meshes
    }

    fn process_node(node: Node, buffer_data: &Vec<gltf::buffer::Data>, image_data: &Vec<gltf::image::Data>) -> Vec<Mesh<BufferType, TextureType>> {
        let mut meshes = vec![];
        let transform = node.transform();
        if let Some(mesh) = node.mesh() {
            meshes.push(Self::process_mesh(mesh, &buffer_data, transform.clone(), image_data));
        }
        for _node in node.children() {
            let mut submeshes = Self::process_node(_node, &buffer_data, image_data);
            let (t, r, s) = transform.clone().decomposed();
            let transform_matrix = Mat4::from_scale_rotation_translation(
                glam::Vec3::from(s),
                glam::Quat::from(r),
                glam::Vec3::from(t)
            );
            for mesh in submeshes.iter_mut() {
                for vertex in mesh.vertices.iter_mut() {
                    vertex.position = Vec3A::from(transform_matrix.transform_point3(glam::Vec3::from(vertex.position)));
                }
            }
            meshes.append(&mut submeshes);
        }
        meshes
    }

    fn process_mesh(mesh: gltf::Mesh, buffer_data: &Vec<gltf::buffer::Data>, mut local_transform: Transform, image_data: &Vec<gltf::image::Data>) -> Mesh<BufferType, TextureType> {
        let mut vertices: Vec<Vertex> = vec![];
        let mut indices: Vec<u32> = vec![];
        let (t, r, s) = local_transform.decomposed();
        let transform_matrix = Mat4::from_scale_rotation_translation(
            Vec3::from(s),
            Quat::from(r),
            Vec3::from(t)
        );

        for primitive in mesh.primitives() {
            let reader = primitive
                .reader(|buffer| Some(&buffer_data[buffer.index()]));

            primitive.material().pbr_metallic_roughness().base_color_texture()

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
                let vertex = Vertex::new(
                    Vec3A::from(transform_matrix.transform_point3(Vec3::from(positions[i]))),
                    normals[i],
                    tex_coord
                );
                vertices.push(vertex);
            }
        }

        Mesh {
            vertices,
            indices,
            vertex_buffer: None,
            index_buffer: None,
            is_disposed: false,
            texture: None,
        }
    }

    pub fn get_world_matrix(&self) -> Mat4 {
        let world = Mat4::identity();
        let scale = Mat4::from_scale(glam::Vec3::from(self.scale));
        let translation = Mat4::from_translation(glam::Vec3::from(self.position));
        let rotate = Mat4::from_rotation_ypr(self.rotation.y(), self.rotation.x(), self.rotation.z());
        world * translation * rotate * scale
    }
}

impl Model<Graphics, Buffer, CommandBuffer, Image> {
    pub fn render(&self, command_buffer: CommandBuffer) {
        let graphics = self.graphics.upgrade();
        if graphics.is_none() {
            return;
        }
        let graphics = graphics.unwrap();
        let lock = graphics.read().unwrap();
        let dynamic_offset = lock.dynamic_objects.dynamic_alignment *
            ash::vk::DeviceSize::try_from(self.model_index).unwrap();
        let dynamic_offset = u32::try_from(dynamic_offset).unwrap();
        unsafe {
            let pipeline = lock.pipeline.get_pipeline(ShaderType::BasicShader, 0);
            let pipeline_layout = lock.pipeline.get_pipeline_layout(ShaderType::BasicShader);
            lock.logical_device.cmd_bind_pipeline(
                command_buffer, PipelineBindPoint::GRAPHICS,
                pipeline
            );
            lock.logical_device.cmd_bind_descriptor_sets(
                command_buffer, PipelineBindPoint::GRAPHICS,
                pipeline_layout, 0,
                &[lock.descriptor_sets[0]], &[dynamic_offset, dynamic_offset]
            );
            let mut push_constant = lock.push_constant;
            for mesh in self.meshes.iter() {
                push_constant.object_color = self.color;
                let casted = bytemuck::cast::<PushConstant, [u8; 32]>(push_constant);
                lock.logical_device
                    .cmd_push_constants(command_buffer, pipeline_layout,
                                        ShaderStageFlags::FRAGMENT, 0, &casted[0..]);
                let vertex_buffers = [
                    mesh.get_vertex_buffer()
                ];
                lock.logical_device.cmd_bind_vertex_buffers(
                    command_buffer, 0, &vertex_buffers[0..], &[0]
                );
                lock.logical_device.cmd_bind_index_buffer(
                    command_buffer, mesh.get_index_buffer(), 0, IndexType::UINT32
                );
                lock.logical_device.cmd_draw_indexed(
                    command_buffer, u32::try_from(mesh.indices.len()).unwrap(),
                    1, 0, 0, 0
                );
            }
            drop(lock);
        }
    }
}

impl<GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>, BufferType: 'static + Disposable + Clone, CommandType: 'static, TextureType: 'static + Clone + Disposable> From<&Model<GraphicsType, BufferType, CommandType, TextureType>> for Model<GraphicsType, BufferType, CommandType, TextureType> {
    fn from(model: &Model<GraphicsType, BufferType, CommandType, TextureType>) -> Self {
        let mut _model = Model {
            position: model.position,
            scale: model.scale,
            rotation: model.rotation,
            color: model.color,
            graphics: model.graphics.clone(),
            meshes: model.meshes.to_vec(),
            is_disposed: true,
            model_name: model.model_name.clone(),
            model_index: 0,
            phantom: PhantomData,
            phantom_2: PhantomData,
        };

        _model.meshes.iter_mut()
            .for_each(|mesh| mesh.is_disposed = true);
        _model
    }
}

impl<GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>, BufferType: 'static + Disposable + Clone, CommandType: 'static, TextureType: 'static + Clone + Disposable> Drop for Model<GraphicsType, BufferType, CommandType, TextureType> {
    fn drop(&mut self) {
        log::info!("Dropping model...Model: {}, Model Index: {}", self.model_name.as_str(), self.model_index);
        if !self.is_disposed {
            self.dispose();
            log::info!("Successfully dropped model.");
        }
        else {
            log::warn!("Model is already dropped.");
        }
    }
}

impl<GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>, BufferType: 'static + Disposable + Clone, CommandType: 'static, TextureType: 'static + Clone + Disposable> Disposable for Model<GraphicsType, BufferType, CommandType, TextureType> {
    fn dispose(&mut self) {
        log::info!("Disposing model...Model: {}, Model Index: {}", self.model_name.as_str(), self.model_index);
        for mesh in self.meshes.iter_mut() {
            mesh.dispose();
        }
        self.is_disposed = true;
        log::info!("Successfully disposed model.");
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