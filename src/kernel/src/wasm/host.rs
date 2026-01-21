//! Host functions for WASM modules.
//!
//! Provides the bridge between the WASM sandbox and the SovelmaOS kernel.

use crate::capability::CapId;
use crate::println;
use alloc::vec::Vec;
use wasmi::{Caller, Linker};

/// State shared between host and WASM instance.
pub struct HostState {
    /// Capabilities granted to this process.
    pub capabilities: Vec<CapId>,
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
            capabilities: Vec::new(),
        }
    }
}

/// Register host functions with the WASM linker.
pub fn register_functions(linker: &mut Linker<HostState>) -> Result<(), wasmi::Error> {
    // Example: print a message to the kernel console
    linker.func_wrap("env", "print", |caller: Caller<'_, HostState>| {
        // Check for Serial or Console capability (hypothetical)
        let _host_state = caller.data();
        // In a real system, we'd check if host_state.capabilities contains a
        // Serial capability for the desired port.
        println!("[WASM] Host function 'print' called (Capability check would occur here)");
    })?;

    // Example: get a dummy temperature reading (gated by a hypothetical sensor capability)
    linker.func_wrap("env", "get_temp", |caller: Caller<'_, HostState>| -> i32 {
        let _host_state = caller.data();
        // Verification logic:
        // if !host_state.has_capability(CapabilityType::Sensor) { return -1; }

        println!("[WASM] Host function 'get_temp' called");
        42
    })?;

    Ok(())
}
