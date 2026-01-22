//! A simple asynchronous task executor.
//!
//! This module provides a priority-based cooperative task executor for the kernel.
//! Tasks are organized into 4 priority levels and executed in order from highest
//! to lowest priority.

use super::{Task, TaskId};
use alloc::{collections::BTreeMap, sync::Arc};
use core::task::{Context, Poll, Waker};
use crossbeam_queue::ArrayQueue;
use futures_util::task::ArcWake;

/// Maximum number of tasks per priority queue.
const QUEUE_CAPACITY: usize = 100;

/// A simple executor that runs tasks to completion.
///
/// The executor maintains separate queues for each priority level and processes
/// them from highest (Critical) to lowest (Idle) priority.
pub struct Executor {
    /// All registered tasks, keyed by their unique ID.
    tasks: BTreeMap<TaskId, Task>,
    /// Priority queues: [Idle, Normal, High, Critical].
    task_queues: [Arc<ArrayQueue<TaskId>>; 4],
    /// Cached wakers for each task to avoid repeated allocations.
    waker_cache: BTreeMap<TaskId, Waker>,
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

impl Executor {
    /// Create a new executor with empty task queues.
    pub fn new() -> Self {
        Executor {
            tasks: BTreeMap::new(),
            task_queues: [
                Arc::new(ArrayQueue::new(QUEUE_CAPACITY)), // Idle
                Arc::new(ArrayQueue::new(QUEUE_CAPACITY)), // Normal
                Arc::new(ArrayQueue::new(QUEUE_CAPACITY)), // High
                Arc::new(ArrayQueue::new(QUEUE_CAPACITY)), // Critical
            ],
            waker_cache: BTreeMap::new(),
        }
    }

    /// Spawn a new task on the executor.
    ///
    /// If a task with the same ID already exists (should never happen due to
    /// atomic ID generation), the spawn is silently ignored to prevent panics.
    ///
    /// If the priority queue is full, the task is dropped and a warning is logged.
    pub fn spawn(&mut self, task: Task) {
        let task_id = task.id;
        let priority = task.priority as usize;

        // Defense in depth: check for duplicate IDs (should never happen)
        if self.tasks.contains_key(&task_id) {
            #[cfg(debug_assertions)]
            crate::println!("BUG: Duplicate task ID {:?}, ignoring spawn", task_id);
            return;
        }

        self.tasks.insert(task_id, task);

        // If queue is full, remove the task and log a warning
        if self.task_queues[priority].push(task_id).is_err() {
            self.tasks.remove(&task_id);
            crate::println!(
                "WARNING: Executor queue {} full, dropping task {:?}",
                priority,
                task_id
            );
        }
    }

    /// Run all ready tasks.
    ///
    /// Iterates through priority queues from Critical (3) down to Idle (0),
    /// polling each task until it either completes or yields.
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
    ///
    /// This function never returns under normal operation (diverging `-> !`).
    pub fn run(&mut self) -> ! {
        loop {
            self.run_ready_tasks();
            self.sleep_if_idle();
        }
    }

    /// Sleep the CPU if no tasks are ready.
    ///
    /// Uses x86_64 HLT instruction to reduce power consumption while waiting
    /// for interrupts to wake the processor.
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

/// Internal waker implementation for tasks.
///
/// When a task is woken, its ID is pushed back onto its priority queue
/// so it will be polled again.
struct TaskWaker {
    task_id: TaskId,
    task_queue: Arc<ArrayQueue<TaskId>>,
}

impl TaskWaker {
    /// Create a new `Waker` for the given task.
    #[allow(clippy::new_ret_no_self)]
    fn new(task_id: TaskId, task_queue: Arc<ArrayQueue<TaskId>>) -> Waker {
        futures_util::task::waker(Arc::new(TaskWaker {
            task_id,
            task_queue,
        }))
    }
}

impl ArcWake for TaskWaker {
    /// Wake the task by re-queuing it.
    ///
    /// If the queue is full, the wake is silently dropped. This can happen
    /// under extreme load but is safe—the task will be woken again later.
    fn wake_by_ref(arc_self: &Arc<Self>) {
        // Silently drop if queue is full to avoid kernel panic
        let _ = arc_self.task_queue.push(arc_self.task_id);
    }
}
