pub trait Disposable: Drop {
    /// メモリーを解放する。<br />
    /// Manually dispose the memory.
    fn dispose(&mut self);

    /// このメモリーは既に解放されたのかどうか。<br />
    /// Is this memory already disposed.
    fn is_disposed(&self) -> bool;

    /// このリソースの名前。<br />
    /// The name of this disposable resource.
    fn get_name(&self) -> &str;

    /// このリソースの名前を設定する。<br />
    /// Set the name of this resource.
    fn set_name(&mut self, name: String) -> &str;
}
