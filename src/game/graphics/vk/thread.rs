use ash::version::DeviceV1_0;
use ash::vk::{CommandPool, CommandPoolCreateFlags};
use crossbeam::queue::ArrayQueue;
use crossbeam::utils::Backoff;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast::*;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

#[allow(dead_code)]
pub struct Thread {
    pub command_pool: Arc<Mutex<ash::vk::CommandPool>>,
    destroying: Arc<AtomicBool>,
    work_received: AtomicBool,
    worker: JoinHandle<()>,
    task_queue: Arc<ArrayQueue<Box<dyn FnOnce() + Send + 'static>>>,
    sender: Sender<()>,
    notify: Arc<Notify>,
}

impl Thread {
    pub fn new(device: &ash::Device, queue_index: u32) -> Self {
        let task_queue = Arc::new(ArrayQueue::new(20));
        let queue = task_queue.clone();
        let (sender, receiver) = channel::<()>(100);
        let s1 = sender.clone();
        let destroying = Arc::new(AtomicBool::new(false));
        let d1 = destroying.clone();
        let pool_info = ash::vk::CommandPoolCreateInfo::builder()
            .flags(CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(queue_index)
            .build();
        let notify = Arc::new(Notify::new());
        let n1 = notify.clone();
        unsafe {
            let command_pool = device
                .create_command_pool(&pool_info, None)
                .expect("Failed to create command pool for thread.");
            Thread {
                destroying,
                worker: tokio::spawn(async move {
                    let s1 = s1;
                    let mut receiver = receiver;
                    let d1 = d1;
                    let notify = n1;
                    'outer: loop {
                        let mut work: Option<Box<dyn FnOnce() + Send>>;
                        while receiver.recv().await.is_ok() {
                            if d1.load(Ordering::SeqCst) {
                                break 'outer;
                            }
                            work = queue.pop();
                            if let Some(job) = work {
                                job();
                                s1.send(()).unwrap();
                            } else {
                                notify.notify();
                                break;
                            }
                        }
                    }
                }),
                task_queue: task_queue.clone(),
                sender,
                command_pool: Arc::new(Mutex::new(command_pool)),
                notify,
                work_received: AtomicBool::new(false),
            }
        }
    }

    pub fn add_job(&self, work: impl FnOnce() + Send + 'static) {
        let result = self.task_queue.push(Box::new(work));
        if result.is_err() {
            log::error!("Error pushing new job into the queue: Queue is full.");
            return;
        }
        self.work_received.store(true, Ordering::SeqCst);
        self.sender.send(()).unwrap();
    }

    pub async fn wait(&self) {
        if !self.work_received.load(Ordering::SeqCst) {
            return;
        }
        self.notify.notified().await;
        self.work_received.store(false, Ordering::SeqCst);
    }

    pub async fn dispose(&mut self) {
        self.destroying.store(true, Ordering::SeqCst);
        self.sender.send(()).unwrap();
        let worker = &mut self.worker;
        worker.await.expect("Failed to dispose worker thread.");
    }
}

pub struct ThreadPool {
    pub threads: Vec<Thread>,
    pub thread_count: usize,
}

impl ThreadPool {
    pub fn new(thread_count: usize, device: &ash::Device, queue_index: u32) -> Self {
        let mut threads = vec![];
        for _ in 0..thread_count {
            threads.push(Thread::new(device, queue_index));
        }
        ThreadPool {
            threads,
            thread_count,
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

    pub fn get_idle_command_pool(&self) -> Arc<Mutex<CommandPool>> {
        let backoff = Backoff::new();
        loop {
            if let Some(pool) = self
                .threads
                .iter()
                .find(|thread| thread.task_queue.is_empty())
            {
                return pool.command_pool.clone();
            }
            backoff.spin();
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        self.threads.clear();
    }
}
