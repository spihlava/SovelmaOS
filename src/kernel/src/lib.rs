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

pub mod arch;

/// Initializes core kernel subsystems.
///
/// Called early in the boot process to set up essential services.
pub fn init() {
    #[cfg(target_arch = "x86_64")]
    {
        arch::x86_64::serial::init();
        arch::x86_64::vga::init();
    }
}
