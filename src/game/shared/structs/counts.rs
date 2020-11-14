use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

pub struct Counts {
    pub model_count: Arc<AtomicUsize>,
    pub ssbo_count: AtomicUsize,
    pub entity_count: usize,
}

impl Default for Counts {
    fn default() -> Self {
        Self::new()
    }
}

impl Counts {
    pub fn new() -> Self {
        Counts {
            model_count: Arc::new(AtomicUsize::new(0)),
            ssbo_count: AtomicUsize::new(0),
            entity_count: 0,
        }
    }
}
