//! WASM Runtime integration for SovelmaOS.
//!
//! This module provides the WebAssembly runtime for user-space processes using
//! the `wasmi` interpreter. It implements fuel-based cooperative preemption
//! and integrates with the kernel's async task executor.
//!
//! # Architecture
//!
//! - **WasmEngine**: Shared engine configuration for all WASM modules.
//! - **WasmProcess**: A running WASM instance with its own store and capabilities.
//! - **WasmTask**: A Future adapter for running WASM functions as kernel tasks.
//!
//! # Security
//!
//! All processes are spawned with explicit capabilities via `spawn_process_with_caps`.
//! There is no ambient authority—processes can only access resources they've been
//! explicitly granted.
//!
//! # Preemption
//!
//! Fuel-based preemption is implemented at two levels:
//! 1. **wasmi fuel**: The interpreter tracks instruction fuel and traps on exhaustion.
//! 2. **Host fuel**: Host functions track a separate fuel counter and yield proactively.
//!
//! The host fuel mechanism ensures tasks yield cleanly (preserving the `ResumableInvocation`)
//! before wasmi's fuel runs out (which would terminate the task).

use alloc::boxed::Box;
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use wasmi::{core::TrapCode, Engine, Linker, Module, Store};

/// Fuel units granted per scheduler time slice.
///
/// This value controls how much computation a task can perform before
/// yielding to the scheduler. Higher values = longer time slices.
const FUEL_PER_SLICE: u64 = 10_000;

mod host;
pub use host::HostState;

use alloc::vec::Vec;
use sovelma_common::capability::Capability;

/// The shared WASM engine.
///
/// The engine holds the compilation cache and configuration shared by all
/// WASM instances. It is safe to clone (cheap Arc reference).
#[derive(Clone)]
pub struct WasmEngine {
    engine: Engine,
}

impl WasmEngine {
    /// Create a new WASM engine with fuel consumption enabled.
    pub fn new() -> Self {
        let mut config = wasmi::Config::default();
        config.consume_fuel(true);

        Self {
            engine: Engine::new(&config),
        }
    }

    /// Create a new process from WASM bytes with initial capabilities.
    ///
    /// This is the **preferred** method for spawning WASM processes as it enforces
    /// the object-capability security model. The process will only have access
    /// to resources explicitly granted via `initial_caps`.
    ///
    /// # Arguments
    ///
    /// * `wasm_bytes` - The WASM module bytecode
    /// * `initial_caps` - Capabilities to grant to the process at spawn time
    ///
    /// # Example
    ///
    /// ```ignore
    /// use sovelma_common::capability::{Capability, CapabilityType, CapabilityRights};
    ///
    /// let root_cap = Capability::new(
    ///     CapabilityType::Directory(root_handle.0 as u64),
    ///     CapabilityRights::READ,
    /// );
    /// let process = engine.spawn_process_with_caps(wasm_bytes, vec![root_cap])?;
    /// ```
    pub fn spawn_process_with_caps(
        &self,
        wasm_bytes: &[u8],
        initial_caps: Vec<Capability>,
    ) -> Result<WasmProcess, wasmi::Error> {
        let module = Module::new(&self.engine, wasm_bytes)?;
        let host_state = HostState::with_capabilities(initial_caps);
        let mut store = Store::new(&self.engine, host_state);
        let mut linker = <Linker<HostState>>::new(&self.engine);

        // Define host functions
        host::register_functions(&mut linker)?;

        let instance = linker.instantiate(&mut store, &module)?.start(&mut store)?;

        // Grant initial fuel
        if let Err(e) = store.add_fuel(FUEL_PER_SLICE) {
            // Log but don't fail - fuel is optional
            crate::println!("[WASM] Failed to add fuel: {:?}", e);
        }

        Ok(WasmProcess { store, instance })
    }

    /// Create a new process from WASM bytes without initial capabilities.
    ///
    /// # Warning
    ///
    /// This spawns a process with **no capabilities**. The process will not be
    /// able to access any resources. Use `spawn_process_with_caps` to grant
    /// initial capabilities.
    #[deprecated(
        since = "0.2.0",
        note = "Use spawn_process_with_caps to explicitly grant capabilities"
    )]
    pub fn spawn_process(&self, wasm_bytes: &[u8]) -> Result<WasmProcess, wasmi::Error> {
        self.spawn_process_with_caps(wasm_bytes, Vec::new())
    }
}

impl Default for WasmEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// A running WASM process.
///
/// Contains the wasmi store (with host state) and the instantiated module.
pub struct WasmProcess {
    store: Store<HostState>,
    instance: wasmi::Instance,
}

impl WasmProcess {
    /// Call a function exported by the module (blocking).
    ///
    /// This is a synchronous call that blocks until the function completes.
    /// For async execution, use `call_async` or `spawn_task`.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the exported function to call
    /// * `params` - Parameters to pass to the function
    ///
    /// # Returns
    ///
    /// The return values from the function, or an error if execution failed.
    pub fn call(
        &mut self,
        name: &str,
        params: &[wasmi::Value],
    ) -> Result<Box<[wasmi::Value]>, wasmi::Error> {
        let func = self.instance.get_func(&self.store, name).ok_or_else(|| {
            wasmi::Error::from(wasmi::core::Trap::from(
                wasmi::core::TrapCode::UnreachableCodeReached,
            ))
        })?;

        let mut results = [wasmi::Value::I32(0); 1];
        func.call(&mut self.store, params, &mut results)?;
        Ok(Box::new(results))
    }

    /// Call a function asynchronously.
    ///
    /// Returns a `Future` that drives the function execution, yielding to the
    /// scheduler when fuel is exhausted or `sp_sched_yield` is called.
    pub fn call_async<'a>(&'a mut self, name: &'a str) -> WasmCallFuture<'a> {
        WasmCallFuture {
            process: self,
            func_name: name,
            invocation: None,
        }
    }

    /// Spawn this process as a kernel task.
    ///
    /// The process will be driven by the executor, yielding cooperatively
    /// based on fuel consumption.
    pub fn spawn_task(self, name: &str, executor: &mut crate::task::executor::Executor) {
        use crate::task::{Priority, Task};

        let task = WasmTask {
            process: self,
            func_name: alloc::string::String::from(name),
            invocation: None,
        };

        executor.spawn(Task::with_priority(
            async move {
                match task.await {
                    Ok(()) => crate::println!("[WASM] Completed."),
                    Err(e) => crate::println!("[WASM] Error: {:?}", e),
                }
            },
            Priority::Normal,
        ));
    }
}

/// A Future that owns a WASM process and runs a function to completion.
///
/// This future drives the execution of a WASM function. It automatically:
/// - Replenishes wasmi fuel at the start of each poll cycle
/// - Resets host fuel for proactive yielding
/// - Handles yield traps by returning `Poll::Pending`
///
/// # Yielding
///
/// The task yields control when:
/// - The WASM code calls `sp_sched_yield`
/// - A host function's fuel check triggers `HostTrap::Yield`
///
/// # Termination
///
/// The task terminates (with error) when:
/// - An unrecoverable trap occurs (e.g., `OutOfFuel`, `Unreachable`)
/// - A host function returns a fatal error
pub struct WasmTask {
    process: WasmProcess,
    func_name: alloc::string::String,
    invocation: Option<wasmi::ResumableInvocation>,
}

impl Future for WasmTask {
    type Output = Result<(), wasmi::Error>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        // Refill wasmi fuel for this time slice
        if let Err(e) = this.process.store.add_fuel(FUEL_PER_SLICE) {
            crate::println!("[WASM] Failed to add fuel: {:?}", e);
        }

        // Reset host fuel for proactive yielding
        this.process.store.data_mut().fuel_remaining = FUEL_PER_SLICE;

        let result = match this.invocation.take() {
            None => {
                let func = this
                    .process
                    .instance
                    .get_func(&this.process.store, &this.func_name)
                    .ok_or_else(|| {
                        wasmi::Error::from(wasmi::core::Trap::from(
                            TrapCode::UnreachableCodeReached,
                        ))
                    })?;

                let mut results = [wasmi::Value::I32(0); 1];
                func.call_resumable(&mut this.process.store, &[], &mut results)
            }
            Some(invocation) => {
                let mut results = [wasmi::Value::I32(0); 1];
                invocation.resume(&mut this.process.store, &[], &mut results)
            }
        };

        match result {
            Ok(wasmi::ResumableCall::Finished) => Poll::Ready(Ok(())),
            Ok(wasmi::ResumableCall::Resumable(invocation)) => {
                this.invocation = Some(invocation);
                Poll::Pending
            }
            Err(e) => {
                // All errors terminate the task.
                // Proactive yielding via HostTrap::Yield returns Resumable, not Err.
                Poll::Ready(Err(e))
            }
        }
    }
}

/// A Future wrapper for a WASM function call.
///
/// Similar to `WasmTask`, but borrows the process instead of owning it.
/// Useful for one-off calls where the process will be reused.
pub struct WasmCallFuture<'a> {
    process: &'a mut WasmProcess,
    func_name: &'a str,
    invocation: Option<wasmi::ResumableInvocation>,
}

impl Future for WasmCallFuture<'_> {
    type Output = Result<(), wasmi::Error>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        // Refill wasmi fuel for this time slice
        if let Err(e) = this.process.store.add_fuel(FUEL_PER_SLICE) {
            crate::println!("[WASM] Failed to add fuel: {:?}", e);
        }

        // Reset host fuel for proactive yielding
        this.process.store.data_mut().fuel_remaining = FUEL_PER_SLICE;

        let result = match this.invocation.take() {
            None => {
                let func = this
                    .process
                    .instance
                    .get_func(&this.process.store, this.func_name)
                    .ok_or_else(|| {
                        wasmi::Error::from(wasmi::core::Trap::from(
                            TrapCode::UnreachableCodeReached,
                        ))
                    })?;

                let mut results = [wasmi::Value::I32(0); 1];
                func.call_resumable(&mut this.process.store, &[], &mut results)
            }
            Some(invocation) => {
                let mut results = [wasmi::Value::I32(0); 1];
                invocation.resume(&mut this.process.store, &[], &mut results)
            }
        };

        match result {
            Ok(wasmi::ResumableCall::Finished) => Poll::Ready(Ok(())),
            Ok(wasmi::ResumableCall::Resumable(invocation)) => {
                this.invocation = Some(invocation);
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}
