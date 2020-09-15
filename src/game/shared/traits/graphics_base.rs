use crate::game::structs::Vertex;
use crate::game::traits::Disposable;

pub trait GraphicsBase<BufferType: 'static + Drop + Clone, CommandType: 'static, TextureType: 'static + Disposable> {
    fn create_vertex_buffer(&self, vertices: &[Vertex]) -> BufferType;
    fn create_index_buffer(&self, indices: &[u32]) -> BufferType;
    fn get_commands(&self) -> &Vec<CommandType>;
    fn create_image(&self, image_data: &[u8], buffer_size: u64, width: u32, height: u32, format: gltf::image::Format) -> TextureType;
}