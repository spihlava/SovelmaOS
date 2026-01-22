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

/// Yields execution to allow other tasks to run.
///
/// Returns `Pending` once, wakes itself, then returns `Ready`.
pub async fn yield_now() {
    YieldNow { yielded: false }.await
}

struct YieldNow {
    yielded: bool,
}

impl Future for YieldNow {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.yielded {
            Poll::Ready(())
        } else {
            self.yielded = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

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
    /// Lowest priority, runs only when no other tasks are ready.
    Idle = 0,
    /// Standard priority for most tasks.
    Normal = 1,
    /// High priority for latency-sensitive tasks.
    High = 2,
    /// Highest priority for critical system tasks.
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
