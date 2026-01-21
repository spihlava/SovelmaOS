//! Asynchronous task management.

use alloc::boxed::Box;
use core::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
    task::{Context, Poll},
};

pub mod executor;
pub mod keyboard;

/// A unique identifier for a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TaskId(u64);

impl TaskId {
    /// Create a new unique TaskId.
    fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        TaskId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// Task priority levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Idle = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// A wrapper around a future that represents a task.
pub struct Task {
    id: TaskId,
    priority: Priority,
    future: Pin<Box<dyn Future<Output = ()>>>,
}

impl Task {
    /// Create a new task from a future with Normal priority.
    pub fn new(future: impl Future<Output = ()> + 'static) -> Task {
        Self::with_priority(future, Priority::Normal)
    }

    /// Create a new task with specific priority.
    pub fn with_priority(future: impl Future<Output = ()> + 'static, priority: Priority) -> Task {
        Task {
            id: TaskId::new(),
            priority,
            future: Box::pin(future),
        }
    }

    /// Poll the task's future.
    fn poll(&mut self, context: &mut Context) -> Poll<()> {
        self.future.as_mut().poll(context)
    }
}
