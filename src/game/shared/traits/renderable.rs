use crate::game::graphics::vk::{Pipeline, ThreadPool};
use crate::game::shared::structs::{ModelMetaData, PositionInfo, PushConstant};
use crate::game::shared::traits::Disposable;
use crate::game::traits::GraphicsBase;
use ash::vk::{CommandBufferInheritanceInfo, DescriptorSet};
use crossbeam::sync::ShardedLock;
use glam::{EulerRot, Mat4};
use slotmap::{DefaultKey, Key};
use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicPtr, AtomicUsize};
use std::sync::Arc;

/// 描画できるオブジェクト<br />
/// Renderable objects.  
pub trait Renderable<GraphicsType, BufferType, CommandType, TextureType>: Disposable
where
    GraphicsType: 'static + GraphicsBase<BufferType, CommandType, TextureType>,
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Clone + Disposable,
{
    fn box_clone(
        &self,
    ) -> Box<dyn Renderable<GraphicsType, BufferType, CommandType, TextureType> + Send + 'static>;

    /// モデルのSSBOを作成する。<br />
    /// Create SSBO for the model.
    fn create_ssbo(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    /// モデルのSSBOを解放する。<br />
    /// Release SSBO for the model.
    fn dispose_ssbo(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    /// このモデルを描画するためのコマンドバッファを取得する。<br />
    /// Obtain command buffers for rendering this model.
    fn get_command_buffers(&self, frame_index: usize) -> Vec<CommandType>;

    /// このモデルが配属されたエンティティを取得する。<br />
    /// Get the entity this model belongs to.
    fn get_entity(&self) -> DefaultKey {
        DefaultKey::null()
    }

    /// モデルのメタデータを取得する。<br />
    /// Obtain model's metadata.
    fn get_model_metadata(&self) -> ModelMetaData;

    /// モデルの位置などの情報を取得する。<br />
    /// Get position info of the model.
    fn get_position_info(&self) -> PositionInfo;

    /// 主なSSBOの中にこのモデルのインデックスを取得する。<br />
    /// Get the index of this model inside the primary SSBO.
    fn get_ssbo_index(&self) -> usize;

    /// ワールド行列を取得する。<br />
    /// Get world matrix of this model.
    fn get_world_matrix(&self) -> Mat4 {
        let PositionInfo {
            position,
            scale,
            rotation,
        } = self.get_position_info();
        let world = Mat4::IDENTITY;
        let scale = Mat4::from_scale(glam::Vec3::from(scale));
        let translation = Mat4::from_translation(glam::Vec3::from(position));
        let rotate = Mat4::from_euler(EulerRot::YXZ, rotation.y, rotation.x, rotation.z);
        world * translation * rotate * scale
    }

    /// モデルを描画する。<br />
    /// Render this model.
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

    /// モデルのメタデータを設定する。<br />
    /// Set this model's metadata.
    fn set_model_metadata(&mut self, model_metadata: ModelMetaData);

    /// モデルの位置情報を設定する。<br />
    /// Set position info of this model.
    fn set_position_info(&mut self, position_info: PositionInfo);

    /// 主なSSBOの中にこのモデルのインデックスを設定する。<br />
    /// Set the index of this model inside the primary SSBO.
    fn set_ssbo_index(&mut self, ssbo_index: usize);

    /// モデルを更新する。<br />
    /// Update this model.
    fn update(&mut self, delta_time: f64);

    /// モデルのインデックスを更新する。<br />
    /// Update this model's index.
    fn update_model_indices(&mut self, model_count: Arc<AtomicUsize>);
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
