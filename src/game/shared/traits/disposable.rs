pub trait Disposable: Drop {
    fn dispose(&mut self);
    fn is_disposed(&self) -> bool;
    fn get_name(&self) -> &str;
    fn set_name(&mut self, name: String) -> &str;
}
