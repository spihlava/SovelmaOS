#![no_std]

use sovelma_common::capability::CapId;

extern "C" {
    fn print_internal();
    fn get_temp_internal() -> i32;
}

/// Print a message via the kernel console.
pub fn print() {
    unsafe { print_internal() };
}

/// Get a temperature reading from the kernel.
pub fn get_temp() -> i32 {
    unsafe { get_temp_internal() }
}
