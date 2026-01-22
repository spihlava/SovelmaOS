//! Global registry for kernel-managed synchronization objects.
//!
//! This module provides thread-safe registries for mutexes and semaphores
//! that are exposed to WASM modules via host functions.

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::{Mutex, Once};

use super::{AsyncMutex, Semaphore};

/// Global registry for mutexes accessible from WASM.
static MUTEX_REGISTRY: Once<Mutex<BTreeMap<u64, Arc<AsyncMutex<()>>>>> = Once::new();

/// Global registry for semaphores accessible from WASM.
static SEM_REGISTRY: Once<Mutex<BTreeMap<u64, Arc<Semaphore>>>> = Once::new();

/// Next handle ID for mutexes.
static NEXT_MUTEX_ID: AtomicU64 = AtomicU64::new(1);

/// Next handle ID for semaphores.
static NEXT_SEM_ID: AtomicU64 = AtomicU64::new(1);

/// Initialize the sync registries.
fn init_registries() {
    MUTEX_REGISTRY.call_once(|| Mutex::new(BTreeMap::new()));
    SEM_REGISTRY.call_once(|| Mutex::new(BTreeMap::new()));
}

/// Get the mutex registry, initializing if needed.
fn mutex_registry() -> &'static Mutex<BTreeMap<u64, Arc<AsyncMutex<()>>>> {
    init_registries();
    MUTEX_REGISTRY.get().expect("mutex registry initialized")
}

/// Get the semaphore registry, initializing if needed.
fn sem_registry() -> &'static Mutex<BTreeMap<u64, Arc<Semaphore>>> {
    init_registries();
    SEM_REGISTRY.get().expect("sem registry initialized")
}

/// Create a new mutex and return its handle.
pub fn create_mutex() -> u64 {
    let handle = NEXT_MUTEX_ID.fetch_add(1, Ordering::Relaxed);
    let mutex = Arc::new(AsyncMutex::new(()));
    mutex_registry().lock().insert(handle, mutex);
    handle
}

/// Get a mutex by handle.
pub fn get_mutex(handle: u64) -> Option<Arc<AsyncMutex<()>>> {
    mutex_registry().lock().get(&handle).cloned()
}

/// Destroy a mutex by handle.
pub fn destroy_mutex(handle: u64) -> bool {
    mutex_registry().lock().remove(&handle).is_some()
}

/// Create a new semaphore and return its handle.
pub fn create_semaphore(permits: usize) -> u64 {
    let handle = NEXT_SEM_ID.fetch_add(1, Ordering::Relaxed);
    let sem = Arc::new(Semaphore::new(permits));
    sem_registry().lock().insert(handle, sem);
    handle
}

/// Get a semaphore by handle.
pub fn get_semaphore(handle: u64) -> Option<Arc<Semaphore>> {
    sem_registry().lock().get(&handle).cloned()
}

/// Destroy a semaphore by handle.
pub fn destroy_semaphore(handle: u64) -> bool {
    sem_registry().lock().remove(&handle).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mutex_registry() {
        let h1 = create_mutex();
        let h2 = create_mutex();

        assert_ne!(h1, h2);
        assert!(get_mutex(h1).is_some());
        assert!(get_mutex(h2).is_some());
        assert!(get_mutex(9999).is_none());

        assert!(destroy_mutex(h1));
        assert!(get_mutex(h1).is_none());
        assert!(!destroy_mutex(h1)); // Already destroyed
    }

    #[test]
    fn test_semaphore_registry() {
        let h1 = create_semaphore(3);
        let h2 = create_semaphore(1);

        assert_ne!(h1, h2);

        let sem1 = get_semaphore(h1).unwrap();
        assert_eq!(sem1.available_permits(), 3);

        let sem2 = get_semaphore(h2).unwrap();
        assert_eq!(sem2.available_permits(), 1);

        assert!(destroy_semaphore(h1));
        assert!(get_semaphore(h1).is_none());
    }
}
