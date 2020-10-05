use crate::game::traits::Disposable;

pub trait GraphicsBase<BufferType: 'static + Disposable + Clone, CommandType: 'static + Clone, TextureType: 'static + Disposable + Clone> {
    fn get_commands(&self) -> &Vec<CommandType>;
}