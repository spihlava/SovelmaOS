//! WASM Runtime integration for SovelmaOS.
//!
//! Uses the wasmi interpreter for no_std compatible execution.

use crate::serial_println;
use alloc::boxed::Box;
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use wasmi::{Engine, Linker, Module, Store};

mod host;
pub use host::HostState;

/// The shared WASM engine.
#[derive(Clone)]
pub struct WasmEngine {
    engine: Engine,
}

impl WasmEngine {
    /// Create a new WASM engine.
    pub fn new() -> Self {
        let mut config = wasmi::Config::default();
        config.consume_fuel(true);

        Self {
            engine: Engine::new(&config),
        }
    }

    /// Create a new process from WASM bytes.
    pub fn spawn_process(&self, wasm_bytes: &[u8]) -> Result<WasmProcess, wasmi::Error> {
        let module = Module::new(&self.engine, wasm_bytes)?;
        let mut store = Store::new(&self.engine, HostState::new());
        let mut linker = <Linker<HostState>>::new(&self.engine);

        // Define host functions
        host::register_functions(&mut linker)?;

        // Instantiate and start
        // For strict async, we really want to split instantiate and start,
        // but start() usually runs the _start function which might be long running.
        // wasmi 0.31 doesn't easily let us wrap `start` in a resumable way if it's not a TypedFunc.
        // We will assume _start is short-lived or we accept it blocks for init.
        // The process then relies on the *message loop* being async, or we use "call" on a known entry point.
        
        // Strategy: Instantiate synchronously (Init). Then return a Process that can have methods called.
        // But if the user provides a raw module that runs main() in start(), we block.
        // For now, we follow standard instantiation.
        
        let instance = linker.instantiate(&mut store, &module)?.start(&mut store)?;
        
        // We grant some initial fuel
        store.add_fuel(10_000).ok();

        Ok(WasmProcess {
            store,
            instance,
            // In a real OS, we'd start "main" here if it exists and wrap it.
            // For this MVP, we assume the module exports a "run" or we leave it initialized.
            // If the goal is "run_module" (script style), we need to find the entry point.
            // Let's assume we call "frame_loop" or similar for GUI, or just implicit main.
            // If we just want to run the *test* (which is a script), we need to capture the execution of that script.
            // But `start` already ran it!
            // If the test module puts code in `start`, it's already done.
            // To be async, the valid pattern is: `start` sets up state, exports `step` or `run`.
            // OR we manually find `_start` and call it resumably.
        })
    }
}

impl Default for WasmEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// A running WASM process.
pub struct WasmProcess {
    store: Store<HostState>,
    instance: wasmi::Instance,
}

impl WasmProcess {
    /// Call a function exported by the module.
    pub fn call(&mut self, name: &str, params: &[wasmi::Value]) -> Result<Box<[wasmi::Value]>, wasmi::Error> {
        let func = self.instance.get_func(&self.store, name)
            .ok_or_else(|| wasmi::Error::from(wasmi::core::Trap::from(wasmi::core::TrapCode::UnreachableCodeReached)))?;
        
        // This is still blocking. To be async, we need a "CallFuture".
        // For MVP step 1, we expose this. WasmTask wrapper would use `call` in a loop? No.
        // We need `call_resumable`.
        
        let mut results = [wasmi::Value::I32(0); 1]; // Buffer
        func.call(&mut self.store, params, &mut results)?;
        Ok(Box::new(results)) // Simplify return
        
        // NOTE: Truly async requires `call_resumable` which is verbose to setup here.
        // We will refactor this to return a Future in the next iteration once the struct is in place.
    }
}

/// A Future wrapper for a WASM function call.
pub struct WasmCallFuture<'a> {
    process: &'a mut WasmProcess,
    func_name: &'a str,
}

impl Future for WasmCallFuture<'_> {
    type Output = Result<(), wasmi::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
         // This is where we would drive the interpreter.
         // Since wasmi 0.31 `call` is blocking unless we use `resumable`, we simulate yielding via fuel?
         // No, standard `call` doesn't return on fuel, it traps.
         // If we trap, we have to handle it.
         
         // Temporary Stub: Just run it.
         // In real implementation, this checks fuel remaining, adds fuel, runs, handles OutOfFuel trap by returning Pending.
         
         // Fix: Access func_name from self, not self.process
         let func_name = self.func_name;
         
         match self.process.call(func_name, &[]) {
            Ok(_) => Poll::Ready(Ok(())),
            Err(e) => Poll::Ready(Err(e)),
         }
    }
}
