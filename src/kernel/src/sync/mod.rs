//! Synchronization primitives for async kernel tasks.
//!
//! This module provides async-aware synchronization primitives that integrate
//! with the kernel's cooperative task executor. Unlike traditional blocking
//! primitives, these yield control to the scheduler when contended.
//!
//! # Primitives
//!
//! - [`AsyncMutex<T>`]: Exclusive lock that yields when contended
//! - [`Semaphore`]: Counting semaphore for limiting concurrent access
//!
//! # WASM Integration
//!
//! These primitives are exposed to WASM modules via host functions. The
//! [`registry`] module manages kernel-owned sync objects that WASM can
//! reference through capability handles.
//!
//! # Example
//!
//! ```ignore
//! use sovelma_kernel::sync::{AsyncMutex, Semaphore};
//!
//! // Mutex protecting shared state
//! let counter = AsyncMutex::new(0u32);
//! {
//!     let mut guard = counter.lock().await;
//!     *guard += 1;
//! }
//!
//! // Semaphore limiting concurrency
//! let sem = Semaphore::new(3);
//! sem.acquire().await;
//! // ... do work ...
//! sem.release();
//! ```

mod mutex;
pub mod registry;
mod semaphore;

pub use mutex::{AsyncMutex, AsyncMutexGuard, AsyncMutexLockFuture};
pub use semaphore::{Semaphore, SemaphoreAcquireFuture, SemaphorePermit};
