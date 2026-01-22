//! Hello World WASM application for SovelmaOS.
//!
//! This demonstrates basic WASM module functionality with the capability-based
//! security model. Capabilities must be granted at spawn time by the kernel.

#![no_std]

use sovelma_sdk::{print_str, yield_now};

/// Entry point for the WASM module.
///
/// Note: Initial capabilities are granted at spawn time by the kernel.
/// This module should receive any needed capabilities via the spawn mechanism.
#[no_mangle]
pub extern "C" fn _start() {
    print_str("Hello from WASM!\n");

    // Demonstrate cooperative scheduling
    yield_now();
    print_str("Yielded and resumed successfully!\n");

    // Note: To access files, the spawning code must grant a directory capability.
    // Example kernel code:
    //   let dir_cap = Capability::new(CapabilityType::Directory(handle), CapabilityRights::READ);
    //   engine.spawn_process_with_caps(wasm_bytes, vec![dir_cap])?;
}

/// Entry point that accepts an initial directory capability.
///
/// This is the preferred pattern: the kernel passes capabilities as arguments.
#[no_mangle]
pub extern "C" fn run_with_cap(dir_cap: i64) {
    print_str("Running with granted capability!\n");

    // Now we can use the capability to access files
    if dir_cap >= 0 {
        print_str("Directory capability received.\n");
        // Could call: sovelma_sdk::open(dir_cap, "some_file.txt")
    }

    yield_now();
    print_str("Done!\n");
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
