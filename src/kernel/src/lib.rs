//! SovelmaOS Kernel
//!
//! A capability-based microkernel targeting x86_64 platforms.
//!
//! # Architecture
//!
//! The kernel is structured into the following modules:
//! - `arch`: Platform-specific code (VGA, serial, interrupts)
//!
//! # Safety
//!
//! This is a `#![no_std]` kernel. All unsafe code is documented with safety
//! invariants explaining why the usage is correct.

#![no_std]
#![warn(missing_docs)]
#![feature(abi_x86_interrupt)]
#![feature(raw_ref_op)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::testutil::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

pub mod allocator;
pub mod arch;
pub mod boot;
pub mod capability;
pub mod fs;
pub mod memory;
pub mod net;
pub mod sync;
pub mod task;
pub mod terminal;
pub mod tests;
pub mod wasm;

/// Test infrastructure for the kernel.
///
/// Provides QEMU exit device support and test runner utilities.
/// Used by integration test binaries.
pub mod testutil;

/// Initializes core kernel subsystems.
///
/// Called early in the boot process to set up essential services.
pub fn init() {
    #[cfg(target_arch = "x86_64")]
    {
        arch::x86_64::serial::init();
        arch::x86_64::vga::init();
        arch::x86_64::gdt::init();
        arch::x86_64::interrupts::init_idt();
    }
}
