//! Host functions for WASM modules.
//!
//! Provides the bridge between the WASM sandbox and the SovelmaOS kernel.

use crate::println;
use alloc::collections::BTreeMap;

use sovelma_common::capability::{CapId, Capability, CapabilityRights, CapabilityType};
use wasmi::{Caller, Linker};

use core::fmt;

#[derive(Debug)]
pub enum HostTrap {
    Yield,
    Sleep(u64), // For future use
}

impl fmt::Display for HostTrap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HostTrap::Yield => write!(f, "Yield"),
            HostTrap::Sleep(ms) => write!(f, "Sleep({}ms)", ms),
        }
    }
}

impl wasmi::core::HostError for HostTrap {}

/// State shared between host and WASM instance.
pub struct HostState {
    /// Capabilities granted to this process.
    pub capabilities: BTreeMap<CapId, Capability>,
}

impl Default for HostState {
    fn default() -> Self {
        Self::new()
    }
}

impl HostState {
    /// Create a new host state.
    pub fn new() -> Self {
        Self {
            capabilities: BTreeMap::new(),
        }
    }

    /// Add a capability and return its ID.
    pub fn add_capability(&mut self, cap: Capability) -> CapId {
        let id = cap.id;
        self.capabilities.insert(id, cap);
        id
    }

    /// Get a capability if it exists and generation matches.
    pub fn get_capability(&self, id: CapId) -> Option<&Capability> {
        let cap = self.capabilities.get(&id)?;
        if cap.generation as u32 == id.generation() {
            Some(cap)
        } else {
            None
        }
    }

    /// Revoke a capability by ID.
    pub fn revoke(&mut self, id: CapId) {
        self.capabilities.remove(&id);
    }
}

/// Register host functions with the WASM linker.
pub fn register_functions(linker: &mut Linker<HostState>) -> Result<(), wasmi::Error> {
    // Example: print a message to the kernel console
    linker.func_wrap("env", "print", |caller: Caller<'_, HostState>| {
        let _host_state = caller.data();
        println!("[WASM] Host function 'print' called");
    })?;

    // File System API

    // sp_get_root() -> i64 (cap_id)
    linker.func_wrap(
        "env",
        "sp_get_root",
        |mut caller: Caller<'_, HostState>| -> i64 {
            use crate::fs::{FileSystem, ROOT_FS};

            let handle = match ROOT_FS.open("/") {
                Ok(h) => h,
                Err(_) => return -1,
            };

            let cap = Capability::new(
                CapabilityType::Directory(handle.0 as u64),
                CapabilityRights::READ | CapabilityRights::EXECUTE,
            );

            caller.data_mut().add_capability(cap).as_u64() as i64
        },
    )?;

    // sp_fs_open(dir_cap: i64, path_ptr: i32, path_len: i32) -> i64 (new_cap_id or -1)
    linker.func_wrap(
        "env",
        "sp_fs_open",
        |mut caller: Caller<'_, HostState>, dir_cap: i64, path_ptr: i32, path_len: i32| -> i64 {
            let memory = match caller.get_export("memory") {
                Some(wasmi::Extern::Memory(m)) => m,
                _ => return -2,
            };

            // Read path
            let mut buffer = alloc::vec![0u8; path_len as usize];
            if memory.read(&caller, path_ptr as usize, &mut buffer).is_err() {
                return -3;
            }
            let path = match core::str::from_utf8(&buffer) {
                Ok(s) => s,
                Err(_) => return -4,
            };

            let cap_id = CapId::from_u64(dir_cap as u64);

            // Use a block to borrow caller.data() and return handle/rights to drop borrow before mut borrow later
            let result = {
                let host_state = caller.data();
                if let Some(cap) = host_state.get_capability(cap_id) {
                    // Must be a directory
                    match cap.object {
                        CapabilityType::Directory(handle_val) => {
                            // Check READ right (arbitrary choice for now: need READ to open children?)
                            if cap.rights.contains(CapabilityRights::READ) {
                                use crate::fs::FileHandle;
                                let handle = FileHandle(handle_val as u32);
                                Ok(handle)
                            } else {
                                Err(-5) // Permission Denied
                            }
                        }
                        _ => Err(-6), // Not a directory
                    }
                } else {
                    Err(-1) // Cap not found or generation mismatch
                }
            };

            let dir_handle = match result {
                Ok(h) => h,
                Err(e) => return e,
            };

            // Perform FS operation
            // Use Global ROOT_FS (safe because Kernel context)
            use crate::fs::{FileSystem, ROOT_FS};
            let new_handle = match ROOT_FS.open_at(dir_handle, path) {
                Ok(h) => h,
                Err(_) => return -7, // FS open failed
            };

            // Determine type of new capability
            let is_dir = ROOT_FS.is_dir(new_handle);

            let cap_type = if is_dir {
                CapabilityType::Directory(new_handle.0 as u64)
            } else {
                CapabilityType::File(new_handle.0 as u64)
            };

            // Grant new Capability
            // Should inherit rights? For now grant READ|WRITE for files, READ|EXEC for dirs?
            // Simplification: Grant everything for new opens for now (unless we enforce ACLs in FS)
            let new_cap =
                Capability::new(cap_type, CapabilityRights::READ | CapabilityRights::WRITE);

            caller.data_mut().add_capability(new_cap).as_u64() as i64
        },
    )?;

    // sp_fs_read(file_cap: i64, buf_ptr: i32, buf_len: i32, offset: i32) -> i32 (bytes read or -1)
    linker.func_wrap(
        "env",
        "sp_fs_read",
        |mut caller: Caller<'_, HostState>,
         file_cap: i64,
         buf_ptr: i32,
         buf_len: i32,
         offset: i32|
         -> i32 {
            let memory = match caller.get_export("memory") {
                Some(wasmi::Extern::Memory(m)) => m,
                _ => return -2,
            };

            // Verify Capability
            let cap_id = CapId::from_u64(file_cap as u64);
            let result = {
                let host_state = caller.data();
                if let Some(cap) = host_state.get_capability(cap_id) {

                    match cap.object {
                        CapabilityType::File(handle_val) => {
                            if cap.rights.contains(CapabilityRights::READ) {
                                Ok(crate::fs::FileHandle(handle_val as u32))
                            } else {
                                Err(-5) // Permission Denied
                            }
                        }
                        _ => Err(-6), // Not a file
                    }
                } else {
                    Err(-1)
                }
            };

            let file_handle = match result {
                Ok(h) => h,
                Err(e) => return e,
            };

            // Perform Read
            use crate::fs::{FileSystem, ROOT_FS};
            let mut buffer = alloc::vec![0u8; buf_len as usize];
            let bytes_read = match ROOT_FS.read(file_handle, &mut buffer, offset as usize) {
                Ok(n) => n,
                Err(_) => return -1,
            };

            // Write to WASM memory
            if memory.write(&mut caller, buf_ptr as usize, &buffer[..bytes_read]).is_err() {
                return -3;
            }

            bytes_read as i32
        },
    )?;

    // sp_fs_size(file_cap: i64) -> i32
    linker.func_wrap(
        "env",
        "sp_fs_size",
        |caller: Caller<'_, HostState>, file_cap: i64| -> i32 {
            let cap_id = CapId::from_u64(file_cap as u64);
            let result = {
                let host_state = caller.data();
                if let Some(cap) = host_state.get_capability(cap_id) {
   
                    match cap.object {
                        CapabilityType::File(val) | CapabilityType::Directory(val) => {
                            Ok(crate::fs::FileHandle(val as u32))
                        }
                        _ => Err(-6),
                    }
                } else {
                    Err(-1)
                }
            };

            let handle = match result {
                Ok(h) => h,
                Err(e) => return e,
            };

            use crate::fs::{FileSystem, ROOT_FS};
            match ROOT_FS.size(handle) {
                Ok(s) => s as i32,
                Err(_) => -1,
            }
        },
    )?;

    // sp_fs_close(file_cap: i64)
    linker.func_wrap(
        "env",
        "sp_fs_close",
        |mut caller: Caller<'_, HostState>, file_cap: i64| {
            let cap_id = CapId::from_u64(file_cap as u64);

            // In a real capability system, we might just drop the capability.
            // But here we need to also close the underlying FS handle if it's a file.
            // So we need to retrieve the handle, then remove the cap.

            let handle_to_close = {
                let host_state = caller.data_mut();
                if let Some(cap) = host_state.capabilities.remove(&cap_id) {
   
                    match cap.object {
                        CapabilityType::File(val) | CapabilityType::Directory(val) => {
                            Some(crate::fs::FileHandle(val as u32))
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            };

            if let Some(handle) = handle_to_close {
                use crate::fs::{FileSystem, ROOT_FS};
                ROOT_FS.close(handle);
            }
        },
    )?;

    // sp_fs_mkdir(dir_cap: i64, path_ptr: i32, path_len: i32) -> i32 (0 on success, else error)
    linker.func_wrap(
        "env",
        "sp_fs_mkdir",
        |mut caller: Caller<'_, HostState>, dir_cap: i64, path_ptr: i32, path_len: i32| -> i32 {
            let memory = match caller.get_export("memory") {
                Some(wasmi::Extern::Memory(m)) => m,
                _ => return -2,
            };

            let mut buffer = alloc::vec![0u8; path_len as usize];
            if memory.read(&caller, path_ptr as usize, &mut buffer).is_err() {
                return -3;
            }
            let path = match core::str::from_utf8(&buffer) {
                Ok(s) => s,
                Err(_) => return -4,
            };

            let cap_id = CapId::from_u64(dir_cap as u64);
            let dir_handle = {
                let host_state = caller.data();
                match host_state.get_capability(cap_id) {
                    Some(cap) => {

                        match cap.object {
                            CapabilityType::Directory(val) => {
                                if cap.rights.contains(CapabilityRights::WRITE) {
                                    Ok(crate::fs::FileHandle(val as u32))
                                } else {
                                    Err(-5)
                                }
                            }
                            _ => Err(-6),
                        }
                    }
                    None => Err(-1),
                }
            };

            let handle = match dir_handle {
                Ok(h) => h,
                Err(e) => return e,
            };

            use crate::fs::{FileSystem, ROOT_FS};
            match ROOT_FS.mkdir_at(handle, path) {
                Ok(_) => 0,
                Err(_) => -7,
            }
        },
    )?;

    // sp_sched_yield()
    linker.func_wrap(
        "env",
        "sp_sched_yield",
        |_caller: Caller<'_, HostState>| -> Result<(), wasmi::core::Trap> {
            Err(wasmi::core::Trap::from(HostTrap::Yield))
        },
    )?;

    Ok(())
}
