//! SovelmaOS Userspace SDK
//!
//! Provides safe wrappers around kernel host functions for WASM modules.
//!
//! # Capability-Based Security
//!
//! SovelmaOS uses an object-capability security model. Processes do not have
//! ambient authority to access resources. Instead, capabilities must be
//! explicitly granted at spawn time by the kernel.
//!
//! Initial capabilities are passed to the process and can be accessed via
//! a well-known memory location or passed as arguments to the entry point.

#![no_std]

extern "C" {
    fn print(ptr: *const u8, len: usize);
    fn sp_fs_open(dir_cap: i64, path_ptr: *const u8, path_len: usize) -> i64;
    fn sp_fs_read(file_cap: i64, buf_ptr: *mut u8, buf_len: usize, offset: i32) -> i32;
    fn sp_fs_mkdir(dir_cap: i64, path_ptr: *const u8, path_len: usize) -> i32;
    fn sp_fs_close(file_cap: i64);
    fn sp_sched_yield();

    // Sync primitives
    fn sp_mutex_create() -> i64;
    fn sp_mutex_lock(cap: i64) -> i32;
    fn sp_mutex_try_lock(cap: i64) -> i32;
    fn sp_mutex_unlock(cap: i64) -> i32;
    fn sp_sem_create(permits: i32) -> i64;
    fn sp_sem_acquire(cap: i64) -> i32;
    fn sp_sem_try_acquire(cap: i64) -> i32;
    fn sp_sem_release(cap: i64) -> i32;
}

/// Print a message via the kernel console.
pub fn print_str(s: &str) {
    unsafe { print(s.as_ptr(), s.len()) };
}

// Note: get_root() has been removed. Capabilities are now granted at spawn time.
// Access your initial capabilities through the mechanism provided by the kernel
// (e.g., passed as arguments to your entry point or via a well-known memory location).

/// Open a file or directory relative to a directory capability.
///
/// # Arguments
/// * `dir_cap` - A directory capability ID (must have READ permission)
/// * `path` - Relative path to open
///
/// # Returns
/// * Positive value: New capability ID for the opened file/directory
/// * Negative value: Error code
pub fn open(dir_cap: i64, path: &str) -> i64 {
    unsafe { sp_fs_open(dir_cap, path.as_ptr(), path.len()) }
}

/// Read data from a file capability.
///
/// # Arguments
/// * `file_cap` - A file capability ID (must have READ permission)
/// * `buf` - Buffer to read into
/// * `offset` - Byte offset to start reading from
///
/// # Returns
/// * Positive value: Number of bytes read
/// * Negative value: Error code
pub fn read(file_cap: i64, buf: &mut [u8], offset: usize) -> i32 {
    unsafe { sp_fs_read(file_cap, buf.as_mut_ptr(), buf.len(), offset as i32) }
}

/// Create a directory relative to a directory capability.
///
/// # Arguments
/// * `dir_cap` - A directory capability ID (must have WRITE permission)
/// * `path` - Relative path of directory to create
///
/// # Returns
/// * 0: Success
/// * Negative value: Error code
pub fn mkdir(dir_cap: i64, path: &str) -> i32 {
    unsafe { sp_fs_mkdir(dir_cap, path.as_ptr(), path.len()) }
}

/// Close a file or directory capability.
///
/// # Arguments
/// * `file_cap` - The capability ID to close
pub fn close(file_cap: i64) {
    unsafe { sp_fs_close(file_cap) }
}

/// Yield execution to the scheduler.
///
/// This allows other tasks to run. The current task will be rescheduled
/// to continue execution later.
pub fn yield_now() {
    unsafe { sp_sched_yield() }
}

// ============================================================================
// Synchronization Primitives
// ============================================================================

/// A handle to a kernel mutex.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MutexHandle(pub i64);

/// A handle to a kernel semaphore.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemHandle(pub i64);

/// Error codes for sync operations.
pub mod sync_error {
    /// Mutex is currently locked (for try_lock).
    pub const MUTEX_LOCKED: i32 = -11;
    /// Semaphore has no available permits.
    pub const SEM_NO_PERMITS: i32 = -12;
    /// Invalid handle.
    pub const INVALID_HANDLE: i32 = -13;
}

/// Create a new mutex.
///
/// # Returns
/// * `Ok(MutexHandle)` - Handle to the created mutex
/// * `Err(i32)` - Error code on failure
pub fn mutex_create() -> Result<MutexHandle, i32> {
    let result = unsafe { sp_mutex_create() };
    if result < 0 {
        Err(result as i32)
    } else {
        Ok(MutexHandle(result))
    }
}

/// Lock a mutex, blocking until the lock is acquired.
///
/// # Arguments
/// * `handle` - The mutex handle
///
/// # Returns
/// * `Ok(())` - Lock acquired
/// * `Err(i32)` - Error code
pub fn mutex_lock(handle: MutexHandle) -> Result<(), i32> {
    let result = unsafe { sp_mutex_lock(handle.0) };
    if result == 0 {
        Ok(())
    } else {
        Err(result)
    }
}

/// Try to lock a mutex without blocking.
///
/// # Arguments
/// * `handle` - The mutex handle
///
/// # Returns
/// * `Ok(true)` - Lock acquired
/// * `Ok(false)` - Lock is held by another task
/// * `Err(i32)` - Error code
pub fn mutex_try_lock(handle: MutexHandle) -> Result<bool, i32> {
    let result = unsafe { sp_mutex_try_lock(handle.0) };
    match result {
        0 => Ok(true),
        -11 => Ok(false), // MUTEX_LOCKED
        err => Err(err),
    }
}

/// Unlock a mutex.
///
/// # Arguments
/// * `handle` - The mutex handle
///
/// # Returns
/// * `Ok(())` - Lock released
/// * `Err(i32)` - Error code
pub fn mutex_unlock(handle: MutexHandle) -> Result<(), i32> {
    let result = unsafe { sp_mutex_unlock(handle.0) };
    if result == 0 {
        Ok(())
    } else {
        Err(result)
    }
}

/// Create a new semaphore with the given number of permits.
///
/// # Arguments
/// * `permits` - Initial (and maximum) permit count
///
/// # Returns
/// * `Ok(SemHandle)` - Handle to the created semaphore
/// * `Err(i32)` - Error code on failure
pub fn sem_create(permits: u32) -> Result<SemHandle, i32> {
    let result = unsafe { sp_sem_create(permits as i32) };
    if result < 0 {
        Err(result as i32)
    } else {
        Ok(SemHandle(result))
    }
}

/// Acquire a permit from a semaphore, blocking until one is available.
///
/// # Arguments
/// * `handle` - The semaphore handle
///
/// # Returns
/// * `Ok(())` - Permit acquired
/// * `Err(i32)` - Error code
pub fn sem_acquire(handle: SemHandle) -> Result<(), i32> {
    let result = unsafe { sp_sem_acquire(handle.0) };
    if result == 0 {
        Ok(())
    } else {
        Err(result)
    }
}

/// Try to acquire a permit without blocking.
///
/// # Arguments
/// * `handle` - The semaphore handle
///
/// # Returns
/// * `Ok(true)` - Permit acquired
/// * `Ok(false)` - No permits available
/// * `Err(i32)` - Error code
pub fn sem_try_acquire(handle: SemHandle) -> Result<bool, i32> {
    let result = unsafe { sp_sem_try_acquire(handle.0) };
    match result {
        0 => Ok(true),
        -12 => Ok(false), // SEM_NO_PERMITS
        err => Err(err),
    }
}

/// Release a permit back to a semaphore.
///
/// # Arguments
/// * `handle` - The semaphore handle
///
/// # Returns
/// * `Ok(())` - Permit released
/// * `Err(i32)` - Error code
pub fn sem_release(handle: SemHandle) -> Result<(), i32> {
    let result = unsafe { sp_sem_release(handle.0) };
    if result == 0 {
        Ok(())
    } else {
        Err(result)
    }
}
