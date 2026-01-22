//! Async-aware mutex for cooperative multitasking.
//!
//! This module provides an async mutex that yields to the scheduler when
//! contended, integrating with the kernel's async executor.

use alloc::sync::Arc;
use core::{
    cell::UnsafeCell,
    future::Future,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    task::{Context, Poll, Waker},
};
use crossbeam_queue::ArrayQueue;

/// Maximum number of waiters per mutex.
const MAX_WAITERS: usize = 100;

/// An async-aware mutex that yields to the scheduler when contended.
///
/// Unlike a spin lock, this mutex allows tasks waiting for the lock to yield
/// control back to the executor, enabling other tasks to make progress.
///
/// # Example
///
/// ```ignore
/// let mutex = AsyncMutex::new(0u32);
///
/// // In an async context:
/// let guard = mutex.lock().await;
/// *guard += 1;
/// // guard is dropped here, releasing the lock
/// ```
pub struct AsyncMutex<T> {
    /// The protected data.
    data: UnsafeCell<T>,
    /// Lock state: false = unlocked, true = locked.
    locked: AtomicBool,
    /// FIFO queue of waiters to wake.
    waiters: ArrayQueue<Waker>,
}

// Safety: The mutex provides synchronized access to T.
// Send + Sync is safe because we use atomic operations for the lock state
// and only allow access through the guard.
unsafe impl<T: Send> Send for AsyncMutex<T> {}
unsafe impl<T: Send> Sync for AsyncMutex<T> {}

impl<T> AsyncMutex<T> {
    /// Create a new unlocked mutex protecting the given data.
    pub fn new(data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
            locked: AtomicBool::new(false),
            waiters: ArrayQueue::new(MAX_WAITERS),
        }
    }

    /// Create a new mutex wrapped in an Arc for shared ownership.
    pub fn new_shared(data: T) -> Arc<Self> {
        Arc::new(Self::new(data))
    }

    /// Attempt to acquire the lock without blocking.
    ///
    /// Returns `Some(guard)` if the lock was acquired, `None` if it's held
    /// by another task.
    pub fn try_lock(&self) -> Option<AsyncMutexGuard<'_, T>> {
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(AsyncMutexGuard { mutex: self })
        } else {
            None
        }
    }

    /// Acquire the lock asynchronously.
    ///
    /// Returns a future that resolves to a guard when the lock is acquired.
    /// If the lock is already held, the future will yield to the scheduler
    /// until the lock becomes available.
    pub fn lock(&self) -> AsyncMutexLockFuture<'_, T> {
        AsyncMutexLockFuture {
            mutex: self,
            registered: false,
        }
    }

    /// Wake the next waiter in the queue, if any.
    fn wake_next(&self) {
        if let Some(waker) = self.waiters.pop() {
            waker.wake();
        }
    }
}

/// RAII guard that releases the mutex when dropped.
pub struct AsyncMutexGuard<'a, T> {
    mutex: &'a AsyncMutex<T>,
}

impl<T> Deref for AsyncMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // Safety: We hold the lock, so we have exclusive access.
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T> DerefMut for AsyncMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safety: We hold the lock, so we have exclusive access.
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<T> Drop for AsyncMutexGuard<'_, T> {
    fn drop(&mut self) {
        // Release the lock
        self.mutex.locked.store(false, Ordering::Release);
        // Wake the next waiter
        self.mutex.wake_next();
    }
}

/// Future returned by `AsyncMutex::lock()`.
pub struct AsyncMutexLockFuture<'a, T> {
    mutex: &'a AsyncMutex<T>,
    registered: bool,
}

impl<'a, T> Future for AsyncMutexLockFuture<'a, T> {
    type Output = AsyncMutexGuard<'a, T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        // Fast path: try to acquire immediately
        if this
            .mutex
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            return Poll::Ready(AsyncMutexGuard { mutex: this.mutex });
        }

        // Slow path: register waker and retry
        if !this.registered {
            // Push may fail if queue is full, but we still try
            let _ = this.mutex.waiters.push(cx.waker().clone());
            this.registered = true;
        }

        // Double-check after registration to avoid lost wakeup
        if this
            .mutex
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            return Poll::Ready(AsyncMutexGuard { mutex: this.mutex });
        }

        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mutex_uncontended() {
        let mutex = AsyncMutex::new(42);

        // try_lock should succeed
        let guard = mutex.try_lock().expect("should acquire lock");
        assert_eq!(*guard, 42);
        drop(guard);

        // Should be able to lock again
        let guard2 = mutex.try_lock().expect("should acquire lock again");
        assert_eq!(*guard2, 42);
    }

    #[test]
    fn test_mutex_try_lock_fails_when_locked() {
        let mutex = AsyncMutex::new(42);

        let _guard = mutex.try_lock().expect("should acquire lock");

        // Second try_lock should fail
        assert!(mutex.try_lock().is_none());
    }

    #[test]
    fn test_mutex_guard_deref_mut() {
        let mutex = AsyncMutex::new(0u32);

        {
            let mut guard = mutex.try_lock().expect("should acquire lock");
            *guard = 100;
        }

        let guard = mutex.try_lock().expect("should acquire lock");
        assert_eq!(*guard, 100);
    }
}
