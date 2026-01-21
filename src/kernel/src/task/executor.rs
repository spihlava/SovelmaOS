//! A simple asynchronous task executor.

use super::{Task, TaskId};
use alloc::{collections::BTreeMap, sync::Arc};
use core::task::{Context, Poll, Waker};
use crossbeam_queue::ArrayQueue;
use futures_util::task::ArcWake;

/// A simple executor that runs tasks to completion.
pub struct Executor {
    tasks: BTreeMap<TaskId, Task>,
    task_queues: [Arc<ArrayQueue<TaskId>>; 4],
    waker_cache: BTreeMap<TaskId, Waker>,
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

impl Executor {
    /// Create a new executor.
    pub fn new() -> Self {
        Executor {
            tasks: BTreeMap::new(),
            task_queues: [
                Arc::new(ArrayQueue::new(100)), // Idle
                Arc::new(ArrayQueue::new(100)), // Normal
                Arc::new(ArrayQueue::new(100)), // High
                Arc::new(ArrayQueue::new(100)), // Critical
            ],
            waker_cache: BTreeMap::new(),
        }
    }

    /// Spawn a new task on the executor.
    pub fn spawn(&mut self, task: Task) {
        let task_id = task.id;
        let priority = task.priority as usize;
        if self.tasks.insert(task_id, task).is_some() {
            panic!("task with same ID already in tasks");
        }
        self.task_queues[priority]
            .push(task_id)
            .expect("queue full");
    }

    /// Run all ready tasks.
    fn run_ready_tasks(&mut self) {
        // Iterate queues from Critical (3) down to Idle (0)
        for priority in (0..4).rev() {
            let queue = &self.task_queues[priority];

            // Process all tasks in this priority level before moving lower
            while let Some(task_id) = queue.pop() {
                let task = match self.tasks.get_mut(&task_id) {
                    Some(task) => task,
                    None => continue, // task no longer exists
                };

                let waker = self
                    .waker_cache
                    .entry(task_id)
                    .or_insert_with(|| TaskWaker::new(task_id, self.task_queues[priority].clone()));

                let mut context = Context::from_waker(waker);
                match task.poll(&mut context) {
                    Poll::Ready(()) => {
                        // task done -> remove it and its cached waker
                        self.tasks.remove(&task_id);
                        self.waker_cache.remove(&task_id);
                    }
                    Poll::Pending => {}
                }
            }
        }
    }

    /// Run the executor until all tasks are finished.
    pub fn run(&mut self) -> ! {
        loop {
            self.run_ready_tasks();
            self.sleep_if_idle();
        }
    }

    /// Sleep the CPU if no tasks are ready.
    /// Sleep the CPU if no tasks are ready.
    fn sleep_if_idle(&self) {
        use x86_64::instructions::interrupts;

        interrupts::disable();
        // Check all queues
        let is_empty = self.task_queues.iter().all(|q| q.is_empty());
        if is_empty {
            interrupts::enable_and_hlt();
        } else {
            interrupts::enable();
        }
    }
}

struct TaskWaker {
    task_id: TaskId,
    task_queue: Arc<ArrayQueue<TaskId>>,
}

impl TaskWaker {
    #[allow(clippy::new_ret_no_self)]
    fn new(task_id: TaskId, task_queue: Arc<ArrayQueue<TaskId>>) -> Waker {
        futures_util::task::waker(Arc::new(TaskWaker {
            task_id,
            task_queue,
        }))
    }
}

impl ArcWake for TaskWaker {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self
            .task_queue
            .push(arc_self.task_id)
            .expect("task_queue full");
    }
}
