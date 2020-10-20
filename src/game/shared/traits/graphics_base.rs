use crate::game::traits::Disposable;

pub trait GraphicsBase<
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Disposable + Clone,
>
{
    fn is_initialized(&self) -> bool;
    fn set_disposing(&mut self);
    unsafe fn wait_idle(&self);
}
