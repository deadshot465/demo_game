use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::task::JoinHandle;
use tokio::sync::broadcast::*;
use ash::vk::CommandPoolCreateFlags;
use ash::version::DeviceV1_0;

#[allow(dead_code)]
pub struct Thread<F: 'static + FnOnce() + Send> {
    pub command_pool: ash::vk::CommandPool,
    destroying: Arc<AtomicBool>,
    worker: JoinHandle<()>,
    task_queue: Arc<Mutex<VecDeque<F>>>,
    sender: Sender<()>,
    receiver: Receiver<()>,
}

impl<F: 'static + FnOnce() + Send> Thread<F> {
    pub fn new(device: &ash::Device, queue_index: u32) -> Self {
        let task_queue = Arc::new(Mutex::new(VecDeque::new()));
        let queue = task_queue.clone();
        let (sender, receiver) = channel::<()>(50);
        let s1 = sender.clone();
        let r1 = sender.subscribe();
        let destroying = Arc::new(AtomicBool::new(false));
        let d1 = destroying.clone();
        let pool_info = ash::vk::CommandPoolCreateInfo::builder()
            .flags(CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(queue_index)
            .build();
        unsafe {
            let command_pool = device.create_command_pool(&pool_info, None)
                .expect("Failed to create command pool for thread.");
            Thread {
                destroying,
                worker: tokio::spawn(async move {
                    let s1 = s1;
                    let mut r1 = r1;
                    let d1 = d1;
                    'outer: loop {
                        let mut work: Option<F>;
                        while let Ok(_) = r1.recv().await {
                            if d1.load(Ordering::SeqCst) {
                                break 'outer;
                            }
                            let mut lock = queue.lock();
                            work = lock.pop_front();
                            drop(lock);
                            if let Some(job) = work {
                                job();
                                s1.send(()).unwrap();
                            }
                            else {
                                break;
                            }
                        }
                    }
                    ()
                }),
                task_queue: task_queue.clone(),
                sender,
                receiver,
                command_pool,
            }
        }
    }

    pub fn add_job(&mut self, work: F) {
        let mut lock = self.task_queue.lock();
        lock.push_back(work);
        drop(lock);
        self.sender.send(()).unwrap();
    }

    pub async fn wait(&self) {
        loop {
            let lock = self.task_queue.lock();
            if lock.is_empty() {
                break;
            }
            else {
                drop(lock);
                self.sender.send(()).unwrap();
            }
        }
    }

    pub async fn dispose(&mut self) {
        self.destroying.store(true, Ordering::SeqCst);
        self.sender.send(()).unwrap();
        let worker = &mut self.worker;
        worker.await
            .expect("Failed to dispose worker thread.");
    }
}

pub struct ThreadPool<F: 'static + FnOnce() + Send> {
    pub threads: Vec<Thread<F>>,
    pub thread_count: usize,
}

impl<F: 'static + FnOnce() + Send> ThreadPool<F> {
    pub fn new(thread_count: usize, device: &ash::Device, queue_index: u32) -> Self {
        let mut threads = vec![];
        for _ in 0..thread_count {
            threads.push(Thread::new(device, queue_index));
        }
        ThreadPool {
            threads,
            thread_count
        }
    }

    pub fn set_thread_count(&mut self, thread_count: u32, device: &ash::Device, queue_index: u32) {
        self.threads.clear();
        for _ in 0..thread_count {
            self.threads.push(Thread::new(device, queue_index));
        }
    }

    pub async fn wait(&self) {
        for thread in self.threads.iter() {
            thread.wait().await;
        }
    }
}

impl<F: 'static + FnOnce() + Send> Drop for ThreadPool<F> {
    fn drop(&mut self) {
        self.threads.clear();
    }
}