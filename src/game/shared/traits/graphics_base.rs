use crate::game::structs::Vertex;

pub trait GraphicsBase<BufferType: 'static + Drop + Clone, CommandType: 'static> {
    fn create_vertex_buffer(&self, vertices: &[Vertex]) -> BufferType;
    fn create_index_buffer(&self, indices: &[u32]) -> BufferType;
    fn get_commands(&self) -> &Vec<CommandType>;
}