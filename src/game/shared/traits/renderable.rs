use crate::game::graphics::vk::{Pipeline, ThreadPool};
use crate::game::shared::structs::{ModelMetaData, PositionInfo, PushConstant};
use crate::game::shared::traits::Disposable;
use crate::game::traits::GraphicsBase;
use ash::vk::{CommandBufferInheritanceInfo, DescriptorSet};
use crossbeam::sync::ShardedLock;
use glam::Mat4;
use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicPtr, AtomicUsize};
use std::sync::Arc;

pub trait Renderable<GraphicsType, BufferType, CommandType, TextureType>: Disposable
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    fn update(&mut self, delta_time: f64);
    fn render(
        &self,
        inheritance_info: Arc<AtomicPtr<CommandBufferInheritanceInfo>>,
        push_constant: PushConstant,
        viewport: ash::vk::Viewport,
        scissor: ash::vk::Rect2D,
        device: Arc<ash::Device>,
        pipeline: Arc<ShardedLock<ManuallyDrop<Pipeline>>>,
        descriptor_set: DescriptorSet,
        thread_pool: Arc<ThreadPool>,
        frame_index: usize,
    );

    fn get_ssbo_index(&self) -> usize;
    fn get_model_metadata(&self) -> ModelMetaData;
    fn get_position_info(&self) -> PositionInfo;

    fn get_world_matrix(&self) -> Mat4 {
        let PositionInfo {
            position,
            scale,
            rotation,
        } = self.get_position_info();
        let world = Mat4::identity();
        let scale = Mat4::from_scale(glam::Vec3::from(scale));
        let translation = Mat4::from_translation(glam::Vec3::from(position));
        let rotate = Mat4::from_rotation_ypr(rotation.y, rotation.x, rotation.z);
        world * translation * rotate * scale
    }

    fn create_ssbo(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn dispose_ssbo(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn get_command_buffers(&self, frame_index: usize) -> Vec<CommandType>;
    fn set_position_info(&mut self, position_info: PositionInfo);
    fn set_model_metadata(&mut self, model_metadata: ModelMetaData);
    fn update_model_indices(&mut self, model_count: Arc<AtomicUsize>);
    fn set_ssbo_index(&mut self, ssbo_index: usize);

    fn box_clone(
        &self,
    ) -> Box<dyn Renderable<GraphicsType, BufferType, CommandType, TextureType> + Send + 'static>;
}

impl<GraphicsType, BufferType, CommandType, TextureType> Clone
    for Box<dyn Renderable<GraphicsType, BufferType, CommandType, TextureType> + Send + 'static>
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    fn clone(&self) -> Self {
        self.box_clone()
    }
}
