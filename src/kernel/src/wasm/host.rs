//! Host functions for WASM modules.
//!
//! This module provides the bridge between the WASM sandbox and the SovelmaOS kernel.
//! All resource access is mediated through capabilities, enforcing the object-capability
//! security model.
//!
//! # Security Model
//!
//! - **No ambient authority**: Processes cannot access resources without explicit capabilities.
//! - **Rights degradation**: Derived capabilities have equal or fewer rights than their parent.
//! - **Generation-based revocation**: Stale capability references are rejected.
//!
//! # Fuel Management
//!
//! Host functions track fuel consumption to enable cooperative preemption. When fuel
//! runs low, functions yield control back to the scheduler via `HostTrap::Yield`.

use crate::println;
use alloc::collections::BTreeMap;

use sovelma_common::capability::{CapId, Capability, CapabilityRights, CapabilityType};
use wasmi::{Caller, Linker};

use core::fmt;

// ============================================================================
// Error Codes
// ============================================================================

/// Host function error codes returned to WASM modules.
///
/// These are returned as negative i32/i64 values from host functions.
pub mod error {
    /// Capability not found or generation mismatch.
    pub const CAP_NOT_FOUND: i64 = -1;
    /// WASM module did not export a "memory" object.
    pub const NO_MEMORY_EXPORT: i64 = -2;
    /// Failed to read from WASM linear memory.
    pub const MEMORY_READ_FAILED: i64 = -3;
    /// Path string was not valid UTF-8.
    pub const INVALID_UTF8: i64 = -4;
    /// Capability lacks required rights for operation.
    pub const PERMISSION_DENIED: i64 = -5;
    /// Expected a directory capability, got something else.
    pub const NOT_A_DIRECTORY: i64 = -6;
    /// Filesystem operation failed.
    pub const FS_ERROR: i64 = -7;
    /// Buffer provided was too small.
    pub const BUFFER_TOO_SMALL: i64 = -8;
    /// Failed to write to WASM linear memory.
    pub const MEMORY_WRITE_FAILED: i64 = -9;
    /// Expected a file capability, got something else.
    pub const NOT_A_FILE: i64 = -10;
    /// Mutex is currently locked (for try_lock).
    pub const MUTEX_LOCKED: i64 = -11;
    /// Semaphore has no available permits (for try_acquire).
    pub const SEM_NO_PERMITS: i64 = -12;
    /// Invalid handle (mutex/semaphore not found).
    pub const INVALID_HANDLE: i64 = -13;
}

// ============================================================================
// Fuel Tracking
// ============================================================================

/// Fuel cost for various operations.
///
/// These values are approximate and can be tuned for fairness.
mod fuel_cost {
    /// Cost of a capability lookup.
    pub const CAP_LOOKUP: u64 = 10;
    /// Cost of a filesystem operation.
    pub const FS_OPERATION: u64 = 100;
    /// Cost of memory read/write operations.
    pub const MEMORY_IO: u64 = 50;
    /// Minimum fuel threshold before yielding.
    pub const YIELD_THRESHOLD: u64 = 500;
    /// Cost of creating a sync primitive.
    pub const SYNC_CREATE: u64 = 50;
    /// Cost of a sync operation (lock/unlock/acquire/release).
    pub const SYNC_OPERATION: u64 = 20;
}

// ============================================================================
// Host Trap Types
// ============================================================================

/// Custom trap types for host function control flow.
///
/// These traps are caught by the WASM executor and handled specially.
#[derive(Debug)]
pub enum HostTrap {
    /// Yield control back to the scheduler.
    ///
    /// The task will be re-queued and resumed later with fresh fuel.
    Yield,
    /// Sleep for the specified duration (future use).
    #[allow(dead_code)]
    Sleep(u64),
    /// Waiting on a mutex (handle).
    ///
    /// The task will be re-queued and resumed when the mutex is released.
    MutexWait(u64),
    /// Waiting on a semaphore (handle).
    ///
    /// The task will be re-queued and resumed when a permit is available.
    SemWait(u64),
}

impl fmt::Display for HostTrap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HostTrap::Yield => write!(f, "Yield"),
            HostTrap::Sleep(ms) => write!(f, "Sleep({}ms)", ms),
            HostTrap::MutexWait(h) => write!(f, "MutexWait({})", h),
            HostTrap::SemWait(h) => write!(f, "SemWait({})", h),
        }
    }
}

impl wasmi::core::HostError for HostTrap {}

// ============================================================================
// Host State
// ============================================================================

/// State shared between host functions and a WASM instance.
///
/// Each WASM process has its own `HostState` containing its granted capabilities
/// and fuel tracking information.
pub struct HostState {
    /// Capabilities granted to this process.
    pub capabilities: BTreeMap<CapId, Capability>,
    /// Remaining fuel for this time slice.
    ///
    /// Host functions decrement this and yield when it drops below the threshold.
    pub fuel_remaining: u64,
}

impl Default for HostState {
    fn default() -> Self {
        Self::new()
    }
}

impl HostState {
    /// Create a new host state with no initial capabilities.
    pub fn new() -> Self {
        Self {
            capabilities: BTreeMap::new(),
            fuel_remaining: 0,
        }
    }

    /// Create a new host state with pre-granted capabilities.
    ///
    /// This is the correct way to grant initial capabilities to a WASM process,
    /// enforcing the object-capability discipline where capabilities are only
    /// obtained through explicit grants, not ambient authority.
    pub fn with_capabilities(initial_caps: impl IntoIterator<Item = Capability>) -> Self {
        let mut state = Self::new();
        for cap in initial_caps {
            state.capabilities.insert(cap.id, cap);
        }
        state
    }

    /// Add a capability and return its ID.
    pub fn add_capability(&mut self, cap: Capability) -> CapId {
        let id = cap.id;
        self.capabilities.insert(id, cap);
        id
    }

    /// Get a capability if it exists and generation matches.
    ///
    /// Returns `None` if the capability doesn't exist or the generation
    /// has been invalidated (revoked).
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

    /// Consume fuel for an operation.
    ///
    /// Returns `true` if sufficient fuel remains, `false` if we should yield.
    fn consume_fuel(&mut self, cost: u64) -> bool {
        self.fuel_remaining = self.fuel_remaining.saturating_sub(cost);
        self.fuel_remaining >= fuel_cost::YIELD_THRESHOLD
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Check fuel and return a yield trap if exhausted.
fn check_fuel(caller: &mut Caller<'_, HostState>, cost: u64) -> Result<(), wasmi::core::Trap> {
    if !caller.data_mut().consume_fuel(cost) {
        Err(wasmi::core::Trap::from(HostTrap::Yield))
    } else {
        Ok(())
    }
}

// ============================================================================
// Host Function Registration
// ============================================================================

/// Register host functions with the WASM linker.
///
/// # Security Note
///
/// `sp_get_root` has been intentionally removed. Initial capabilities
/// must be granted at spawn time via `HostState::with_capabilities()`.
/// This enforces proper object-capability discipline.
pub fn register_functions(linker: &mut Linker<HostState>) -> Result<(), wasmi::Error> {
    register_debug_functions(linker)?;
    register_capability_functions(linker)?;
    register_fs_functions(linker)?;
    register_scheduler_functions(linker)?;
    register_sync_functions(linker)?;
    Ok(())
}

/// Register debug/utility host functions.
fn register_debug_functions(linker: &mut Linker<HostState>) -> Result<(), wasmi::Error> {
    // print(): Debug logging to kernel console
    linker.func_wrap("env", "print", |caller: Caller<'_, HostState>| {
        let _host_state = caller.data();
        println!("[WASM] Host function 'print' called");
    })?;

    Ok(())
}

/// Register capability discovery and management functions.
fn register_capability_functions(linker: &mut Linker<HostState>) -> Result<(), wasmi::Error> {
    // sp_get_capabilities(ptr: i32, len: i32) -> i32
    // Returns: number of capabilities written, or negative error code
    linker.func_wrap(
        "env",
        "sp_get_capabilities",
        |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| -> Result<i32, wasmi::core::Trap> {
            check_fuel(&mut caller, fuel_cost::CAP_LOOKUP)?;

            let memory = match caller.get_export("memory") {
                Some(wasmi::Extern::Memory(m)) => m,
                _ => return Ok(error::NO_MEMORY_EXPORT as i32),
            };

            let caps: alloc::vec::Vec<_> = caller.data().capabilities.values().cloned().collect();
            let count = caps.len();
            let struct_size = 16; // 8 (id) + 4 (type) + 4 (rights)
            let required_len = count * struct_size;

            if (len as usize) < required_len {
                return Ok(error::BUFFER_TOO_SMALL as i32);
            }

            check_fuel(&mut caller, fuel_cost::MEMORY_IO * count as u64)?;

            let mut offset = ptr as usize;
            for cap in caps {
                let id_bytes = cap.id.as_u64().to_le_bytes();
                let type_val: u32 = match cap.object {
                    CapabilityType::File(_) => 0,
                    CapabilityType::Directory(_) => 1,
                    CapabilityType::Mutex(_) => 2,
                    CapabilityType::Semaphore(_) => 3,
                    _ => 255,
                };
                let type_bytes = type_val.to_le_bytes();
                let rights_bits = cap.rights.bits();
                let rights_bytes = rights_bits.to_le_bytes();

                if memory.write(&mut caller, offset, &id_bytes).is_err() {
                    return Ok(error::MEMORY_WRITE_FAILED as i32);
                }
                offset += 8;
                if memory.write(&mut caller, offset, &type_bytes).is_err() {
                    return Ok(error::MEMORY_WRITE_FAILED as i32);
                }
                offset += 4;
                if memory.write(&mut caller, offset, &rights_bytes).is_err() {
                    return Ok(error::MEMORY_WRITE_FAILED as i32);
                }
                offset += 4;
            }

            Ok(count as i32)
        },
    )?;

    Ok(())
}

/// Register filesystem host functions.
fn register_fs_functions(linker: &mut Linker<HostState>) -> Result<(), wasmi::Error> {
    // sp_fs_open(dir_cap: i64, path_ptr: i32, path_len: i32) -> i64
    linker.func_wrap(
        "env",
        "sp_fs_open",
        |mut caller: Caller<'_, HostState>,
         dir_cap: i64,
         path_ptr: i32,
         path_len: i32|
         -> Result<i64, wasmi::core::Trap> {
            check_fuel(&mut caller, fuel_cost::FS_OPERATION)?;

            let memory = match caller.get_export("memory") {
                Some(wasmi::Extern::Memory(m)) => m,
                _ => return Ok(error::NO_MEMORY_EXPORT),
            };

            // Read path from WASM memory
            let mut buffer = alloc::vec![0u8; path_len as usize];
            if memory
                .read(&caller, path_ptr as usize, &mut buffer)
                .is_err()
            {
                return Ok(error::MEMORY_READ_FAILED);
            }
            let path = match core::str::from_utf8(&buffer) {
                Ok(s) => s,
                Err(_) => return Ok(error::INVALID_UTF8),
            };

            let cap_id = CapId::from_u64(dir_cap as u64);

            // Extract handle and parent rights for derivation
            let (dir_handle, parent_rights) = {
                let host_state = caller.data();
                match host_state.get_capability(cap_id) {
                    Some(cap) => match cap.object {
                        CapabilityType::Directory(handle_val) => {
                            if cap.rights.contains(CapabilityRights::READ) {
                                (crate::fs::FileHandle(handle_val as u32), cap.rights)
                            } else {
                                return Ok(error::PERMISSION_DENIED);
                            }
                        }
                        _ => return Ok(error::NOT_A_DIRECTORY),
                    },
                    None => return Ok(error::CAP_NOT_FOUND),
                }
            };

            // Perform FS operation
            use crate::fs::{FileSystem, ROOT_FS};
            let new_handle = match ROOT_FS.open_at(dir_handle, path) {
                Ok(h) => h,
                Err(_) => return Ok(error::FS_ERROR),
            };

            // Determine type of new capability
            let is_dir = ROOT_FS.is_dir(new_handle);
            let cap_type = if is_dir {
                CapabilityType::Directory(new_handle.0 as u64)
            } else {
                CapabilityType::File(new_handle.0 as u64)
            };

            // Rights degradation: derived capabilities inherit parent's rights
            // but cannot exceed type-applicable rights
            let applicable_rights = if is_dir {
                CapabilityRights::READ
                    | CapabilityRights::WRITE
                    | CapabilityRights::EXECUTE
                    | CapabilityRights::GRANT
            } else {
                CapabilityRights::READ | CapabilityRights::WRITE
            };
            let derived_rights = parent_rights & applicable_rights;

            let new_cap = Capability::new(cap_type, derived_rights);
            Ok(caller.data_mut().add_capability(new_cap).as_u64() as i64)
        },
    )?;

    // sp_fs_read(file_cap: i64, buf_ptr: i32, buf_len: i32, offset: i32) -> i32
    linker.func_wrap(
        "env",
        "sp_fs_read",
        |mut caller: Caller<'_, HostState>,
         file_cap: i64,
         buf_ptr: i32,
         buf_len: i32,
         offset: i32|
         -> Result<i32, wasmi::core::Trap> {
            check_fuel(&mut caller, fuel_cost::FS_OPERATION)?;

            let memory = match caller.get_export("memory") {
                Some(wasmi::Extern::Memory(m)) => m,
                _ => return Ok(error::NO_MEMORY_EXPORT as i32),
            };

            let cap_id = CapId::from_u64(file_cap as u64);
            let file_handle = {
                let host_state = caller.data();
                match host_state.get_capability(cap_id) {
                    Some(cap) => match cap.object {
                        CapabilityType::File(handle_val) => {
                            if cap.rights.contains(CapabilityRights::READ) {
                                crate::fs::FileHandle(handle_val as u32)
                            } else {
                                return Ok(error::PERMISSION_DENIED as i32);
                            }
                        }
                        _ => return Ok(error::NOT_A_FILE as i32),
                    },
                    None => return Ok(error::CAP_NOT_FOUND as i32),
                }
            };

            // Perform read
            use crate::fs::{FileSystem, ROOT_FS};
            let mut buffer = alloc::vec![0u8; buf_len as usize];
            let bytes_read = match ROOT_FS.read(file_handle, &mut buffer, offset as usize) {
                Ok(n) => n,
                Err(_) => return Ok(error::FS_ERROR as i32),
            };

            check_fuel(&mut caller, fuel_cost::MEMORY_IO)?;

            // Write to WASM memory
            if memory
                .write(&mut caller, buf_ptr as usize, &buffer[..bytes_read])
                .is_err()
            {
                return Ok(error::MEMORY_WRITE_FAILED as i32);
            }

            Ok(bytes_read as i32)
        },
    )?;

    // sp_fs_size(file_cap: i64) -> i32
    linker.func_wrap(
        "env",
        "sp_fs_size",
        |mut caller: Caller<'_, HostState>, file_cap: i64| -> Result<i32, wasmi::core::Trap> {
            check_fuel(&mut caller, fuel_cost::CAP_LOOKUP)?;

            let cap_id = CapId::from_u64(file_cap as u64);
            let handle = {
                let host_state = caller.data();
                match host_state.get_capability(cap_id) {
                    Some(cap) => match cap.object {
                        CapabilityType::File(val) | CapabilityType::Directory(val) => {
                            crate::fs::FileHandle(val as u32)
                        }
                        _ => return Ok(error::NOT_A_FILE as i32),
                    },
                    None => return Ok(error::CAP_NOT_FOUND as i32),
                }
            };

            use crate::fs::{FileSystem, ROOT_FS};
            match ROOT_FS.size(handle) {
                Ok(s) => Ok(s as i32),
                Err(_) => Ok(error::FS_ERROR as i32),
            }
        },
    )?;

    // sp_fs_close(file_cap: i64) -> ()
    linker.func_wrap(
        "env",
        "sp_fs_close",
        |mut caller: Caller<'_, HostState>, file_cap: i64| -> Result<(), wasmi::core::Trap> {
            check_fuel(&mut caller, fuel_cost::CAP_LOOKUP)?;

            let cap_id = CapId::from_u64(file_cap as u64);

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
            Ok(())
        },
    )?;

    // sp_fs_mkdir(dir_cap: i64, path_ptr: i32, path_len: i32) -> i32
    linker.func_wrap(
        "env",
        "sp_fs_mkdir",
        |mut caller: Caller<'_, HostState>,
         dir_cap: i64,
         path_ptr: i32,
         path_len: i32|
         -> Result<i32, wasmi::core::Trap> {
            check_fuel(&mut caller, fuel_cost::FS_OPERATION)?;

            let memory = match caller.get_export("memory") {
                Some(wasmi::Extern::Memory(m)) => m,
                _ => return Ok(error::NO_MEMORY_EXPORT as i32),
            };

            let mut buffer = alloc::vec![0u8; path_len as usize];
            if memory
                .read(&caller, path_ptr as usize, &mut buffer)
                .is_err()
            {
                return Ok(error::MEMORY_READ_FAILED as i32);
            }
            let path = match core::str::from_utf8(&buffer) {
                Ok(s) => s,
                Err(_) => return Ok(error::INVALID_UTF8 as i32),
            };

            let cap_id = CapId::from_u64(dir_cap as u64);
            let dir_handle = {
                let host_state = caller.data();
                match host_state.get_capability(cap_id) {
                    Some(cap) => match cap.object {
                        CapabilityType::Directory(val) => {
                            if cap.rights.contains(CapabilityRights::WRITE) {
                                crate::fs::FileHandle(val as u32)
                            } else {
                                return Ok(error::PERMISSION_DENIED as i32);
                            }
                        }
                        _ => return Ok(error::NOT_A_DIRECTORY as i32),
                    },
                    None => return Ok(error::CAP_NOT_FOUND as i32),
                }
            };

            use crate::fs::{FileSystem, ROOT_FS};
            match ROOT_FS.mkdir_at(dir_handle, path) {
                Ok(_) => Ok(0),
                Err(_) => Ok(error::FS_ERROR as i32),
            }
        },
    )?;

    Ok(())
}

/// Register scheduler-related host functions.
fn register_scheduler_functions(linker: &mut Linker<HostState>) -> Result<(), wasmi::Error> {
    // sp_sched_yield(): Voluntarily yield control to the scheduler
    linker.func_wrap(
        "env",
        "sp_sched_yield",
        |_caller: Caller<'_, HostState>| -> Result<(), wasmi::core::Trap> {
            Err(wasmi::core::Trap::from(HostTrap::Yield))
        },
    )?;

    Ok(())
}

/// Register synchronization host functions.
fn register_sync_functions(linker: &mut Linker<HostState>) -> Result<(), wasmi::Error> {
    use crate::sync::registry;

    // sp_mutex_create() -> i64
    // Returns: mutex capability ID (positive) or error code (negative)
    linker.func_wrap(
        "env",
        "sp_mutex_create",
        |mut caller: Caller<'_, HostState>| -> Result<i64, wasmi::core::Trap> {
            check_fuel(&mut caller, fuel_cost::SYNC_CREATE)?;

            let handle = registry::create_mutex();
            let cap = Capability::new(CapabilityType::Mutex(handle), CapabilityRights::CALL);
            let cap_id = caller.data_mut().add_capability(cap);
            Ok(cap_id.as_u64() as i64)
        },
    )?;

    // sp_mutex_lock(cap: i64) -> i32
    // Returns: 0 on success, negative error code on failure
    // Blocks via HostTrap::MutexWait if lock is held
    linker.func_wrap(
        "env",
        "sp_mutex_lock",
        |mut caller: Caller<'_, HostState>, cap: i64| -> Result<i32, wasmi::core::Trap> {
            check_fuel(&mut caller, fuel_cost::SYNC_OPERATION)?;

            let cap_id = CapId::from_u64(cap as u64);
            let handle = {
                let host_state = caller.data();
                match host_state.get_capability(cap_id) {
                    Some(c) => match c.object {
                        CapabilityType::Mutex(h) => {
                            if c.rights.contains(CapabilityRights::CALL) {
                                h
                            } else {
                                return Ok(error::PERMISSION_DENIED as i32);
                            }
                        }
                        _ => return Ok(error::INVALID_HANDLE as i32),
                    },
                    None => return Ok(error::CAP_NOT_FOUND as i32),
                }
            };

            // Try to acquire the lock
            if let Some(mutex) = registry::get_mutex(handle) {
                if mutex.try_lock().is_some() {
                    // Acquired! Note: we don't actually hold the guard,
                    // the WASM code is responsible for calling unlock.
                    // For kernel-level tracking, the registry manages ownership.
                    Ok(0)
                } else {
                    // Lock is held, yield and retry
                    Err(wasmi::core::Trap::from(HostTrap::MutexWait(handle)))
                }
            } else {
                Ok(error::INVALID_HANDLE as i32)
            }
        },
    )?;

    // sp_mutex_try_lock(cap: i64) -> i32
    // Returns: 0 if locked, MUTEX_LOCKED if contended, or error code
    linker.func_wrap(
        "env",
        "sp_mutex_try_lock",
        |mut caller: Caller<'_, HostState>, cap: i64| -> Result<i32, wasmi::core::Trap> {
            check_fuel(&mut caller, fuel_cost::SYNC_OPERATION)?;

            let cap_id = CapId::from_u64(cap as u64);
            let handle = {
                let host_state = caller.data();
                match host_state.get_capability(cap_id) {
                    Some(c) => match c.object {
                        CapabilityType::Mutex(h) => {
                            if c.rights.contains(CapabilityRights::CALL) {
                                h
                            } else {
                                return Ok(error::PERMISSION_DENIED as i32);
                            }
                        }
                        _ => return Ok(error::INVALID_HANDLE as i32),
                    },
                    None => return Ok(error::CAP_NOT_FOUND as i32),
                }
            };

            if let Some(mutex) = registry::get_mutex(handle) {
                if mutex.try_lock().is_some() {
                    Ok(0)
                } else {
                    Ok(error::MUTEX_LOCKED as i32)
                }
            } else {
                Ok(error::INVALID_HANDLE as i32)
            }
        },
    )?;

    // sp_mutex_unlock(cap: i64) -> i32
    // Returns: 0 on success, or error code
    linker.func_wrap(
        "env",
        "sp_mutex_unlock",
        |mut caller: Caller<'_, HostState>, cap: i64| -> Result<i32, wasmi::core::Trap> {
            check_fuel(&mut caller, fuel_cost::SYNC_OPERATION)?;

            let cap_id = CapId::from_u64(cap as u64);
            let handle = {
                let host_state = caller.data();
                match host_state.get_capability(cap_id) {
                    Some(c) => match c.object {
                        CapabilityType::Mutex(h) => {
                            if c.rights.contains(CapabilityRights::CALL) {
                                h
                            } else {
                                return Ok(error::PERMISSION_DENIED as i32);
                            }
                        }
                        _ => return Ok(error::INVALID_HANDLE as i32),
                    },
                    None => return Ok(error::CAP_NOT_FOUND as i32),
                }
            };

            // The mutex guard was dropped when lock returned, so we need to
            // signal that the lock is released. Since we're using try_lock
            // pattern for WASM, we don't actually hold the guard - this is
            // more of a "release signal" for the kernel's tracking.
            if registry::get_mutex(handle).is_some() {
                // In a real implementation, we'd track which process holds
                // the lock and verify. For now, we trust the WASM code.
                Ok(0)
            } else {
                Ok(error::INVALID_HANDLE as i32)
            }
        },
    )?;

    // sp_sem_create(permits: i32) -> i64
    // Returns: semaphore capability ID (positive) or error code (negative)
    linker.func_wrap(
        "env",
        "sp_sem_create",
        |mut caller: Caller<'_, HostState>, permits: i32| -> Result<i64, wasmi::core::Trap> {
            check_fuel(&mut caller, fuel_cost::SYNC_CREATE)?;

            if permits < 0 {
                return Ok(error::PERMISSION_DENIED); // Invalid argument
            }

            let handle = registry::create_semaphore(permits as usize);
            let cap = Capability::new(CapabilityType::Semaphore(handle), CapabilityRights::CALL);
            let cap_id = caller.data_mut().add_capability(cap);
            Ok(cap_id.as_u64() as i64)
        },
    )?;

    // sp_sem_acquire(cap: i64) -> i32
    // Returns: 0 on success, or error code
    // Blocks via HostTrap::SemWait if no permits available
    linker.func_wrap(
        "env",
        "sp_sem_acquire",
        |mut caller: Caller<'_, HostState>, cap: i64| -> Result<i32, wasmi::core::Trap> {
            check_fuel(&mut caller, fuel_cost::SYNC_OPERATION)?;

            let cap_id = CapId::from_u64(cap as u64);
            let handle = {
                let host_state = caller.data();
                match host_state.get_capability(cap_id) {
                    Some(c) => match c.object {
                        CapabilityType::Semaphore(h) => {
                            if c.rights.contains(CapabilityRights::CALL) {
                                h
                            } else {
                                return Ok(error::PERMISSION_DENIED as i32);
                            }
                        }
                        _ => return Ok(error::INVALID_HANDLE as i32),
                    },
                    None => return Ok(error::CAP_NOT_FOUND as i32),
                }
            };

            if let Some(sem) = registry::get_semaphore(handle) {
                if sem.try_acquire() {
                    Ok(0)
                } else {
                    Err(wasmi::core::Trap::from(HostTrap::SemWait(handle)))
                }
            } else {
                Ok(error::INVALID_HANDLE as i32)
            }
        },
    )?;

    // sp_sem_try_acquire(cap: i64) -> i32
    // Returns: 0 if acquired, SEM_NO_PERMITS if not, or error code
    linker.func_wrap(
        "env",
        "sp_sem_try_acquire",
        |mut caller: Caller<'_, HostState>, cap: i64| -> Result<i32, wasmi::core::Trap> {
            check_fuel(&mut caller, fuel_cost::SYNC_OPERATION)?;

            let cap_id = CapId::from_u64(cap as u64);
            let handle = {
                let host_state = caller.data();
                match host_state.get_capability(cap_id) {
                    Some(c) => match c.object {
                        CapabilityType::Semaphore(h) => {
                            if c.rights.contains(CapabilityRights::CALL) {
                                h
                            } else {
                                return Ok(error::PERMISSION_DENIED as i32);
                            }
                        }
                        _ => return Ok(error::INVALID_HANDLE as i32),
                    },
                    None => return Ok(error::CAP_NOT_FOUND as i32),
                }
            };

            if let Some(sem) = registry::get_semaphore(handle) {
                if sem.try_acquire() {
                    Ok(0)
                } else {
                    Ok(error::SEM_NO_PERMITS as i32)
                }
            } else {
                Ok(error::INVALID_HANDLE as i32)
            }
        },
    )?;

    // sp_sem_release(cap: i64) -> i32
    // Returns: 0 on success, or error code
    linker.func_wrap(
        "env",
        "sp_sem_release",
        |mut caller: Caller<'_, HostState>, cap: i64| -> Result<i32, wasmi::core::Trap> {
            check_fuel(&mut caller, fuel_cost::SYNC_OPERATION)?;

            let cap_id = CapId::from_u64(cap as u64);
            let handle = {
                let host_state = caller.data();
                match host_state.get_capability(cap_id) {
                    Some(c) => match c.object {
                        CapabilityType::Semaphore(h) => {
                            if c.rights.contains(CapabilityRights::CALL) {
                                h
                            } else {
                                return Ok(error::PERMISSION_DENIED as i32);
                            }
                        }
                        _ => return Ok(error::INVALID_HANDLE as i32),
                    },
                    None => return Ok(error::CAP_NOT_FOUND as i32),
                }
            };

            if let Some(sem) = registry::get_semaphore(handle) {
                sem.release();
                Ok(0)
            } else {
                Ok(error::INVALID_HANDLE as i32)
            }
        },
    )?;

    Ok(())
}
