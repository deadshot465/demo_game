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
    pub command_pools: Vec<Arc<Mutex<ash::vk::CommandPool>>>,
    pub work_received: AtomicBool,
    destroying: Arc<AtomicBool>,
    notify: Receiver<()>,
    worker: Option<JoinHandle<anyhow::Result<()>>>,
    task_queue: Arc<ArrayQueue<Box<dyn FnOnce() + Send + 'static>>>,
    work_sender: Sender<()>,
}

impl Thread {
    pub fn new(device: &ash::Device, queue_index: u32, inflight_frame_count: usize) -> Self {
        let task_queue = Arc::new(ArrayQueue::new(1000));
        let (sender, receiver) = bounded(1000);
        let destroying = Arc::new(AtomicBool::new(false));
        let pool_info = ash::vk::CommandPoolCreateInfo::builder()
            .flags(CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(queue_index)
            .build();
        let (notify_sender, notify_receiver) = bounded(1000);
        let sender_clone = sender.clone();
        let destroying_clone = destroying.clone();
        let queue = task_queue.clone();
        unsafe {
            let mut command_pools = vec![];
            for _ in 0..inflight_frame_count {
                let command_pool = device
                    .create_command_pool(&pool_info, None)
                    .expect("Failed to create command pool for thread.");
                command_pools.push(Arc::new(Mutex::new(command_pool)));
            }
            Thread {
                destroying,
                work_received: AtomicBool::new(false),
                notify: notify_receiver,
                worker: Some(std::thread::spawn(move || {
                    let sender = sender_clone;
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
                                notify_sender.send(())?;
                                break;
                            }
                        }
                    }
                    Ok(())
                })),
                task_queue: task_queue.clone(),
                work_sender: sender,
                command_pools,
            }
        }
    }

    pub fn add_job(&self, work: impl FnOnce() + Send + 'static) -> anyhow::Result<()> {
        match self.task_queue.push(Box::new(work)) {
            Ok(_) => (),
            Err(_) => log::error!("Failed to push work into the queue."),
        }
        self.work_sender.send(())?;
        self.work_received.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub fn wait(&self) -> anyhow::Result<()> {
        self.notify.recv()?;
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
    pub inflight_frame_count: usize,
}

impl ThreadPool {
    pub fn new(
        thread_count: usize,
        inflight_frame_count: usize,
        device: &ash::Device,
        queue_index: u32,
    ) -> Self {
        let mut threads = vec![];
        for _ in 0..thread_count {
            threads.push(Thread::new(device, queue_index, inflight_frame_count));
        }
        ThreadPool {
            threads,
            thread_count,
            inflight_frame_count,
        }
    }

    pub fn set_thread_count(
        &mut self,
        thread_count: u32,
        inflight_frame_count: usize,
        device: &ash::Device,
        queue_index: u32,
    ) {
        self.threads.clear();
        for _ in 0..thread_count {
            self.threads
                .push(Thread::new(device, queue_index, inflight_frame_count));
        }
    }

    pub fn wait(&self) -> anyhow::Result<()> {
        for thread in self.threads.iter() {
            if !thread.work_received.load(Ordering::SeqCst) {
                continue;
            }
            thread.wait()?;
            thread.work_received.store(false, Ordering::SeqCst);
        }
        Ok(())
    }

    pub fn get_idle_command_pool(&self) -> Arc<Mutex<CommandPool>> {
        loop {
            if let Some(thread) = self
                .threads
                .iter()
                .find(|thread| (*thread).task_queue.is_empty())
            {
                return thread.command_pools[0].clone();
            }
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        self.wait().expect("Failed to wait on all threads.");
        self.threads.clear();
    }
}
