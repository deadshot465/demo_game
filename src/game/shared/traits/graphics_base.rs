use crate::game::traits::Disposable;

pub trait GraphicsBase<
    BufferType: 'static + Disposable + Clone,
    CommandType: 'static + Clone,
    TextureType: 'static + Disposable + Clone,
>
{
    /// グラフィックシステムは初期化完了するのかどうか。<br />
    /// Is graphic system already initialized.
    fn is_initialized(&self) -> bool;

    /// 解放処理は始まったのかどうかを設定する。<br />
    /// Set and indicate if disposal has already begun.
    fn set_disposing(&mut self);

    /// 既に処理している全部のタスクを待つ。<br />
    /// Wait for all tasks that are being processed.
    unsafe fn wait_idle(&self);
}
