//! Counting semaphore for limiting concurrent access.
//!
//! This module provides an async-aware counting semaphore that integrates
//! with the kernel's async executor.

use core::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll, Waker},
};
use crossbeam_queue::ArrayQueue;

/// Maximum number of waiters per semaphore.
const MAX_WAITERS: usize = 100;

/// A counting semaphore for limiting concurrent access to a resource.
///
/// The semaphore maintains a permit count. Tasks can acquire permits
/// (decrementing the count) or release permits (incrementing the count).
/// When no permits are available, tasks that try to acquire will yield
/// until a permit becomes available.
///
/// # Example
///
/// ```ignore
/// // Limit to 3 concurrent accesses
/// let sem = Semaphore::new(3);
///
/// // In an async context:
/// sem.acquire().await;
/// // ... do work with limited concurrency ...
/// sem.release();
/// ```
pub struct Semaphore {
    /// Current number of available permits.
    permits: AtomicUsize,
    /// Maximum permits (for bounds checking on release).
    max_permits: usize,
    /// FIFO queue of waiters to wake.
    waiters: ArrayQueue<Waker>,
}

// Safety: Semaphore uses atomic operations and is safe to share across tasks.
unsafe impl Send for Semaphore {}
unsafe impl Sync for Semaphore {}

impl Semaphore {
    /// Create a new semaphore with the given number of initial permits.
    ///
    /// The `permits` value is both the initial count and the maximum.
    pub fn new(permits: usize) -> Self {
        Self {
            permits: AtomicUsize::new(permits),
            max_permits: permits,
            waiters: ArrayQueue::new(MAX_WAITERS),
        }
    }

    /// Create a new semaphore with separate initial and maximum permit counts.
    pub fn with_max(initial: usize, max: usize) -> Self {
        debug_assert!(initial <= max, "initial permits cannot exceed max");
        Self {
            permits: AtomicUsize::new(initial),
            max_permits: max,
            waiters: ArrayQueue::new(MAX_WAITERS),
        }
    }

    /// Get the current number of available permits.
    pub fn available_permits(&self) -> usize {
        self.permits.load(Ordering::Relaxed)
    }

    /// Attempt to acquire a permit without blocking.
    ///
    /// Returns `true` if a permit was acquired, `false` if none available.
    pub fn try_acquire(&self) -> bool {
        loop {
            let current = self.permits.load(Ordering::Relaxed);
            if current == 0 {
                return false;
            }
            if self
                .permits
                .compare_exchange_weak(current, current - 1, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                return true;
            }
            // CAS failed, retry
        }
    }

    /// Acquire a permit asynchronously.
    ///
    /// Returns a future that resolves when a permit has been acquired.
    /// If no permits are available, the future will yield to the scheduler
    /// until one becomes available.
    pub fn acquire(&self) -> SemaphoreAcquireFuture<'_> {
        SemaphoreAcquireFuture {
            semaphore: self,
            registered: false,
        }
    }

    /// Release a permit back to the semaphore.
    ///
    /// This increments the permit count and wakes any waiting tasks.
    /// The permit count will not exceed the maximum.
    pub fn release(&self) {
        loop {
            let current = self.permits.load(Ordering::Relaxed);
            let new_val = core::cmp::min(current + 1, self.max_permits);
            if current == new_val {
                // Already at max, just wake waiters
                break;
            }
            if self
                .permits
                .compare_exchange_weak(current, new_val, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
            // CAS failed, retry
        }

        // Wake the next waiter
        if let Some(waker) = self.waiters.pop() {
            waker.wake();
        }
    }
}

/// Future returned by `Semaphore::acquire()`.
pub struct SemaphoreAcquireFuture<'a> {
    semaphore: &'a Semaphore,
    registered: bool,
}

impl Future for SemaphoreAcquireFuture<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        // Fast path: try to acquire immediately
        if this.semaphore.try_acquire() {
            return Poll::Ready(());
        }

        // Slow path: register waker and retry
        if !this.registered {
            let _ = this.semaphore.waiters.push(cx.waker().clone());
            this.registered = true;
        }

        // Double-check after registration to avoid lost wakeup
        if this.semaphore.try_acquire() {
            return Poll::Ready(());
        }

        Poll::Pending
    }
}

/// RAII guard that releases a semaphore permit when dropped.
///
/// Use this when you want automatic permit release on scope exit.
pub struct SemaphorePermit<'a> {
    semaphore: &'a Semaphore,
}

impl<'a> SemaphorePermit<'a> {
    /// Create a new permit guard (assumes permit already acquired).
    pub fn new(semaphore: &'a Semaphore) -> Self {
        Self { semaphore }
    }
}

impl Drop for SemaphorePermit<'_> {
    fn drop(&mut self) {
        self.semaphore.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semaphore_try_acquire() {
        let sem = Semaphore::new(2);

        // Should succeed twice
        assert!(sem.try_acquire());
        assert!(sem.try_acquire());

        // Third should fail
        assert!(!sem.try_acquire());
    }

    #[test]
    fn test_semaphore_release() {
        let sem = Semaphore::new(1);

        assert!(sem.try_acquire());
        assert!(!sem.try_acquire());

        sem.release();
        assert!(sem.try_acquire());
    }

    #[test]
    fn test_semaphore_max_permits() {
        let sem = Semaphore::new(2);

        // Release without acquiring should not exceed max
        sem.release();
        sem.release();
        sem.release();

        assert_eq!(sem.available_permits(), 2);
    }

    #[test]
    fn test_semaphore_with_max() {
        let sem = Semaphore::with_max(1, 5);

        assert_eq!(sem.available_permits(), 1);
        assert!(sem.try_acquire());
        assert!(!sem.try_acquire());

        // Can release up to max
        sem.release();
        sem.release();
        sem.release();
        assert_eq!(sem.available_permits(), 4);
    }
}
