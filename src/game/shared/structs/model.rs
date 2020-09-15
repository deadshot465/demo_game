use glam::{Vec2, Vec3, Vec3A, Vec4, Mat4, Quat};
use crate::game::shared::traits::disposable::Disposable;
use std::sync::{Arc,Weak};
use crate::game::shared::structs::{Mesh, Vertex, PushConstant, Primitive, SamplerResource};
use crate::game::graphics::vk::{Graphics, Buffer, Image};
use gltf::{Node, Scene};
use crate::game::traits::GraphicsBase;
use std::mem::ManuallyDrop;
use ash::vk::{CommandBuffer, PipelineBindPoint, ShaderStageFlags, IndexType, DeviceSize};
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
}

impl<GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>, BufferType: 'static + Disposable + Clone, CommandType: 'static, TextureType: 'static + Clone + Disposable> Model<GraphicsType, BufferType, CommandType, TextureType> {
    fn process_model(document: gltf::Document, buffer: Vec<gltf::buffer::Data>) -> Vec<Mesh<BufferType, TextureType>> {
        let meshes = if let Some(scene) = document.default_scene() {
            Self::process_root_nodes(scene, buffer)
        }
        else {
            Self::process_root_nodes(document.scenes().nth(0).unwrap(), buffer)
        };
        println!("Mesh count: {}", meshes.len());
        for (index, mesh) in meshes.iter().enumerate() {
            println!("Mesh {} primitive count: {}", index, mesh.primitives.len());
        }
        meshes
    }

    fn create_model(file_name: &str, graphics: Weak<ShardedLock<GraphicsType>>,
               position: Vec3A, scale: Vec3A, rotation: Vec3A, color: Vec4) -> Self {
        let (document, buffer, image) = gltf::import(file_name)
            .expect("Failed to import the model.");

        let mut meshes = Self::process_model(document, buffer);

        let _graphics = graphics.upgrade();
        if let Some(g) = _graphics {
            let lock = g.read().unwrap();
            for (index, mesh) in meshes.iter_mut().enumerate() {
                log::info!("Creating buffer for mesh {}...", index);
                let vertices = mesh.primitives.iter()
                    .map(|p| &p.vertices)
                    .flatten()
                    .map(|v| *v)
                    .collect::<Vec<_>>();
                let indices = mesh.primitives.iter()
                    .map(|p| &p.indices)
                    .flatten()
                    .map(|i| *i)
                    .collect::<Vec<_>>();
                let vertex_buffer = lock.create_vertex_buffer(vertices.as_slice());
                mesh.vertex_buffer = Some(ManuallyDrop::new(vertex_buffer));
                let index_buffer = lock.create_index_buffer(indices.as_slice());
                mesh.index_buffer = Some(ManuallyDrop::new(index_buffer));

                let mut texture_indices = mesh.primitives.iter()
                    .filter_map(|p| {
                        p.texture_index
                    })
                    .collect::<Vec<_>>();
                texture_indices.sort();
                texture_indices.dedup();
                println!("Texture indices: {:?}", texture_indices.as_slice());
                use gltf::image::Format;
                for i in texture_indices.iter() {
                    let img = image.get(*i);
                    if let Some(_img) = img {
                        let buffer_size = _img.width * _img.height * 4;
                        match _img.format {
                            Format::R8G8B8 | Format::B8G8R8 => {
                                let pixels = &_img.pixels;
                                let mut rgba_pixels: Vec<u8> = vec![];
                                let mut rgba_index = 0;
                                let mut rgb_index = 0;
                                rgba_pixels.resize(buffer_size as usize, 0);
                                for _ in 0..(_img.width * _img.height) {
                                    rgba_pixels[rgba_index] = pixels[rgb_index];
                                    rgba_pixels[rgba_index + 1] = pixels[rgb_index + 1];
                                    rgba_pixels[rgba_index + 2] = pixels[rgb_index + 2];
                                    rgba_pixels[rgba_index + 3] = 255;
                                    rgba_index += 4;
                                    rgb_index += 3;
                                }
                                let texture = lock.create_image(rgba_pixels.as_slice(), buffer_size as u64, _img.width as u32, _img.height as u32, match _img.format {
                                    Format::B8G8R8 => Format::B8G8R8A8,
                                    Format::R8G8B8 => Format::R8G8B8A8,
                                    _ => _img.format
                                });
                                mesh.texture.push(ManuallyDrop::new(texture));
                            },
                            Format::R8G8B8A8 | Format::B8G8R8A8 => {
                                let texture = lock.create_image(_img.pixels.as_slice(), buffer_size as u64, _img.width as u32, _img.height as u32, _img.format);
                                mesh.texture.push(ManuallyDrop::new(texture));
                            },
                            _ => {
                                unimplemented!("Unsupported image format: {:?}", _img.format);
                            }
                        }
                    }
                }
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
        }
    }

    fn process_root_nodes(scene: Scene, buffer_data: Vec<gltf::buffer::Data>) -> Vec<Mesh<BufferType, TextureType>> {
        let mut meshes = Vec::with_capacity(150);
        for node in scene.nodes() {
            let mut submeshes = Self::process_node(node, &buffer_data, Mat4::identity());
            meshes.append(&mut submeshes);
        }
        meshes
    }

    fn process_node(node: Node, buffer_data: &Vec<gltf::buffer::Data>, local_transform: Mat4) -> Vec<Mesh<BufferType, TextureType>> {
        let mut meshes = Vec::with_capacity(150);
        let (t, r, s) = node.transform().decomposed();
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::from(s),
            Quat::from(r),
            Vec3::from(t)
        );
        let transform = local_transform * transform;
        if let Some(mesh) = node.mesh() {
            meshes.push(Self::process_mesh(mesh, &buffer_data, transform.clone()));
        }
        for _node in node.children() {
            let mut submeshes = Self::process_node(_node, &buffer_data, transform.clone());
            meshes.append(&mut submeshes);
        }
        meshes
    }

    fn process_mesh(mesh: gltf::Mesh, buffer_data: &Vec<gltf::buffer::Data>, local_transform: Mat4) -> Mesh<BufferType, TextureType> {
        let mut primitives = Vec::with_capacity(5);

        for primitive in mesh.primitives() {
            let reader = primitive
                .reader(|buffer| Some(&buffer_data[buffer.index()]));

            let positions = reader.read_positions();
            let normals = reader.read_normals();
            let uvs = reader.read_tex_coords(0);
            let indices = reader.read_indices()
                .unwrap()
                .into_u32()
                .collect::<Vec<_>>();

            let mut vertices = match (positions, normals, uvs) {
                (Some(positions), Some(normals), Some(uvs)) => {
                    let _vertices = positions
                        .zip(normals)
                        .zip(uvs.into_f32())
                        .map(|((pos, normal), uv)| {
                            Vertex {
                                position: Vec3A::from(pos),
                                normal: Vec3A::from(normal),
                                uv: Vec2::from(uv)
                            }
                        })
                        .collect::<Vec<_>>();
                    _vertices
                },
                (Some(positions), Some(normals), None) => {
                    let _vertices = positions
                        .zip(normals)
                        .map(|(pos, normal)| {
                            Vertex {
                                position: Vec3A::from(pos),
                                normal: Vec3A::from(normal),
                                uv: Vec2::new(0.0, 0.0)
                            }
                        })
                        .collect::<Vec<_>>();
                    _vertices
                },
                (positions, normals, uvs) => {
                    unimplemented!("Unsupported combination of values. Positions: {}, Normals: {}, UVs: {}", positions.is_some(), normals.is_some(), uvs.is_some());
                }
            };

            let texture_index = primitive.material()
                .pbr_metallic_roughness()
                .base_color_texture()
                .map(|x| x.texture().index());

            for vertex in vertices.iter_mut() {
                vertex.position = Vec3A::from(local_transform.transform_point3(Vec3::from(vertex.position)));
            }

            primitives.push(Primitive {
                vertices,
                indices,
                texture_index,
                is_disposed: false,
            });
        }

        Mesh {
            primitives,
            vertex_buffer: None,
            index_buffer: None,
            texture: vec![],
            is_disposed: false,
            sampler_resource: None,
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
    pub fn new(file_name: &str, graphics: Weak<ShardedLock<Graphics>>,
               position: Vec3A, scale: Vec3A, rotation: Vec3A, color: Vec4) -> Self {
        let mut model = Self::create_model(file_name, graphics, position, scale, rotation, color);
        model
    }

    pub fn create_sampler_resource(&mut self) {
        let _graphics = self.graphics.upgrade().unwrap();
        let lock = _graphics.read().unwrap();
        for mesh in self.meshes.iter_mut() {
            if !mesh.texture.is_empty() {
                mesh.create_sampler_resource(
                    Arc::downgrade(&lock.logical_device),
                    lock.sampler_descriptor_set_layout,
                    lock.descriptor_pool
                );
            }
        }
        drop(lock);
    }

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
            let mut push_constant = lock.push_constant;
            for mesh in self.meshes.iter() {
                let shader_type = if !mesh.texture.is_empty() {
                    ShaderType::BasicShader
                } else {
                    ShaderType::BasicShaderWithoutTexture
                };
                let pipeline = lock.pipeline.get_pipeline(shader_type, 0);
                lock.logical_device.cmd_bind_pipeline(
                    command_buffer, PipelineBindPoint::GRAPHICS,
                    pipeline
                );
                let pipeline_layout = lock.pipeline.get_pipeline_layout(shader_type);
                push_constant.object_color = self.color;
                let casted = bytemuck::cast::<PushConstant, [u8; 32]>(push_constant);
                lock.logical_device
                    .cmd_push_constants(command_buffer, pipeline_layout,
                                        ShaderStageFlags::FRAGMENT, 0, &casted[0..]);
                let vertex_buffers = [
                    mesh.get_vertex_buffer()
                ];
                let index_buffer = mesh.get_index_buffer();
                lock.logical_device.cmd_bind_descriptor_sets(
                    command_buffer, PipelineBindPoint::GRAPHICS,
                    pipeline_layout, 0,
                    &[lock.descriptor_sets[0]], &[dynamic_offset, dynamic_offset]
                );
                if !mesh.texture.is_empty() {
                    match mesh.sampler_resource.as_ref() {
                        Some(res) => {
                            match res {
                                SamplerResource::DescriptorSet(set) => {
                                    lock.logical_device
                                        .cmd_bind_descriptor_sets(
                                            command_buffer, PipelineBindPoint::GRAPHICS,
                                            pipeline_layout, 1, &[*set], &[]
                                        );
                                }
                            }
                        },
                        None => (),
                    }
                }

                let mut vertex_offset_index = 0;
                let mut index_offset_index = 0;
                for primitive in mesh.primitives.iter() {
                    lock.logical_device.cmd_bind_vertex_buffers(
                        command_buffer, 0, &vertex_buffers[0..], &[0]
                    );
                    lock.logical_device.cmd_bind_index_buffer(
                        command_buffer, index_buffer, 0, IndexType::UINT32
                    );
                    lock.logical_device.cmd_draw_indexed(
                        command_buffer, u32::try_from(primitive.indices.len()).unwrap(),
                        1, index_offset_index, vertex_offset_index, 0
                    );
                    vertex_offset_index += primitive.vertices.len() as i32;
                    index_offset_index += primitive.indices.len() as u32;
                }
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