use ash::version::DeviceV1_0;
use ash::vk::{CommandPool, CommandPoolCreateFlags};
use crossbeam::channel::*;
use crossbeam::queue::ArrayQueue;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

#[allow(dead_code)]
pub struct Thread {
    pub command_pool: Arc<Mutex<ash::vk::CommandPool>>,
    destroying: Arc<AtomicBool>,
    work_received: AtomicBool,
    worker: Option<JoinHandle<anyhow::Result<()>>>,
    task_queue: Arc<ArrayQueue<Box<dyn FnOnce() + Send + 'static>>>,
    work_sender: Sender<()>,
    complete_receiver: Receiver<()>,
}

impl Thread {
    pub fn new(device: &ash::Device, queue_index: u32) -> Self {
        let task_queue = Arc::new(ArrayQueue::new(100));
        let (sender, receiver) = bounded::<()>(100);
        let (complete_sender, complete_receiver) = bounded::<()>(100);
        let destroying = Arc::new(AtomicBool::new(false));
        let pool_info = ash::vk::CommandPoolCreateInfo::builder()
            .flags(CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(queue_index)
            .build();
        let sender_clone = sender.clone();
        let destroying_clone = destroying.clone();
        let queue = task_queue.clone();
        unsafe {
            let command_pool = device
                .create_command_pool(&pool_info, None)
                .expect("Failed to create command pool for thread.");
            Thread {
                destroying,
                worker: Some(std::thread::spawn(move || {
                    let sender = sender_clone;
                    let complete_sender = complete_sender;
                    let receiver = receiver;
                    let destroying = destroying_clone;
                    'outer: loop {
                        let mut work: Option<Box<dyn FnOnce() + Send>>;
                        while receiver.recv().is_ok() {
                            if destroying.load(Ordering::SeqCst) {
                                break 'outer;
                            }
                            work = queue.pop();
                            if let Some(job) = work {
                                job();
                                sender.send(())?;
                            } else {
                                complete_sender.send(())?;
                                break;
                            }
                        }
                    }
                    Ok(())
                })),
                task_queue: task_queue.clone(),
                work_sender: sender,
                command_pool: Arc::new(Mutex::new(command_pool)),
                complete_receiver,
                work_received: AtomicBool::new(false),
            }
        }
    }

    pub fn add_job(&self, work: impl FnOnce() + Send + 'static) -> anyhow::Result<()> {
        match self.task_queue.push(Box::new(work)) {
            Ok(_) => (),
            Err(_) => log::error!("Failed to push work into the queue."),
        }
        self.work_received.store(true, Ordering::SeqCst);
        self.work_sender.send(())?;
        Ok(())
    }

    pub fn wait(&self) -> anyhow::Result<()> {
        if !self.work_received.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.complete_receiver.recv()?;
        self.work_received.store(false, Ordering::SeqCst);
        Ok(())
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        self.destroying.store(true, Ordering::SeqCst);
        self.work_sender
            .send(())
            .expect("Failed to send work to the worker thread.");
        if let Some(worker) = self.worker.take() {
            match worker.join() {
                Ok(_) => (),
                Err(_) => log::error!("Failed to join work thread."),
            }
        }
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

    pub fn wait(&self) -> anyhow::Result<()> {
        for thread in self.threads.iter() {
            thread.wait()?;
        }
        Ok(())
    }

    pub fn get_idle_command_pool(&self) -> Arc<Mutex<CommandPool>> {
        loop {
            if let Some(pool) = self
                .threads
                .iter()
                .find(|thread| thread.task_queue.is_empty())
            {
                return pool.command_pool.clone();
            }
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        self.threads.clear();
    }
}
