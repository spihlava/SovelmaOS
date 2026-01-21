#![no_std]

use sovelma_common::capability::CapId;

extern "C" {
    fn print(ptr: *const u8, len: usize);
    fn sp_get_root() -> i32;
    fn sp_fs_open(dir_cap: i32, path_ptr: *const u8, path_len: usize) -> i32;
    fn sp_fs_read(file_cap: i32, buf_ptr: *mut u8, buf_len: usize, offset: i32) -> i32;
    fn sp_fs_mkdir(dir_cap: i32, path_ptr: *const u8, path_len: usize) -> i32;
    fn sp_fs_close(file_cap: i32);
    fn sp_sched_yield();
}

/// Print a message via the kernel console.
pub fn print_str(s: &str) {
    unsafe { print(s.as_ptr(), s.len()) };
}

pub fn get_root() -> i32 {
    unsafe { sp_get_root() }
}

pub fn open(dir_cap: i32, path: &str) -> i32 {
    unsafe { sp_fs_open(dir_cap, path.as_ptr(), path.len()) }
}

pub fn read(file_cap: i32, buf: &mut [u8], offset: usize) -> i32 {
    unsafe { sp_fs_read(file_cap, buf.as_mut_ptr(), buf.len(), offset as i32) }
}

pub fn mkdir(dir_cap: i32, path: &str) -> i32 {
    unsafe { sp_fs_mkdir(dir_cap, path.as_ptr(), path.len()) }
}

pub fn close(file_cap: i32) {
    unsafe { sp_fs_close(file_cap) }
}

pub fn yield_now() {
    unsafe { sp_sched_yield() }
}
