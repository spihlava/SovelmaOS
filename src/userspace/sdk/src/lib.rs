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
