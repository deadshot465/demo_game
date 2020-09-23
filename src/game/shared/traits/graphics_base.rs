use crate::game::traits::Disposable;

pub trait GraphicsBase<BufferType: 'static + Drop + Clone, CommandType: 'static + Clone, TextureType: 'static + Disposable> {
    fn get_commands(&self) -> &Vec<CommandType>;
}