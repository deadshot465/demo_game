use crate::game::traits::Renderable;
use crate::game::{ResourceManager, UISystem};
use ash::vk::CommandPool;
use parking_lot::{Mutex, RwLock};
use std::cell::RefCell;
use std::mem::ManuallyDrop;
use std::rc::Rc;
use std::sync::{Arc, Weak};

pub type LockableRenderable<GraphicsType, BufferType, CommandType, TextureType> =
    Arc<Mutex<Box<dyn Renderable<GraphicsType, BufferType, CommandType, TextureType> + Send>>>;

pub type ResourceManagerHandle<GraphicsType, BufferType, CommandType, TextureType> =
    Arc<RwLock<ManuallyDrop<ResourceManager<GraphicsType, BufferType, CommandType, TextureType>>>>;

pub type ResourceManagerWeak<GraphicsType, BufferType, CommandType, TextureType> =
    Weak<RwLock<ManuallyDrop<ResourceManager<GraphicsType, BufferType, CommandType, TextureType>>>>;

pub type CommandData<CommandType> =
    std::collections::HashMap<usize, (Option<Arc<Mutex<CommandPool>>>, CommandType)>;

pub type UISystemHandle<GraphicsType, BufferType, CommandType, TextureType> =
    Option<Rc<RefCell<ManuallyDrop<UISystem<GraphicsType, BufferType, CommandType, TextureType>>>>>;
