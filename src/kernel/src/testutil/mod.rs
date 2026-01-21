//! Test infrastructure for SovelmaOS kernel.
//!
//! Provides utilities for bare-metal testing with QEMU.
//!
//! # Usage
//!
//! Create a test binary in `tests/` that uses these utilities:
//!
//! ```rust,ignore
//! use sovelma_kernel::testutil::{QemuExitCode, exit_qemu, test_runner, Testable};
//! ```

use crate::serial_println;

/// QEMU exit codes for signaling test results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    /// All tests passed.
    Success = 0x10,
    /// One or more tests failed.
    Failed = 0x11,
}

/// Exit QEMU with the given exit code.
///
/// Uses the isa-debug-exit device configured on port 0xf4.
///
/// # Note
///
/// QEMU must be started with `-device isa-debug-exit,iobase=0xf4,iosize=0x04`.
/// The actual exit code will be `(value << 1) | 1`, so:
/// - `Success` (0x10) → exit code 33
/// - `Failed` (0x11) → exit code 35
pub fn exit_qemu(exit_code: QemuExitCode) {
    #[cfg(target_arch = "x86_64")]
    {
        use x86_64::instructions::port::Port;

        // SAFETY: Writing to the isa-debug-exit device port is safe when QEMU
        // is configured with this device. It triggers a QEMU exit.
        unsafe {
            let mut port = Port::new(0xf4);
            port.write(exit_code as u32);
        }
    }
}

/// Trait for types that can be run as tests.
pub trait Testable {
    /// Run the test and report results.
    fn run(&self);
}

impl<T: Fn()> Testable for T {
    fn run(&self) {
        serial_println!("test {} ... ", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

/// Custom test runner for bare-metal tests.
///
/// Runs all tests and exits QEMU with success if all pass.
///
/// # Example
///
/// ```rust,ignore
/// #![feature(custom_test_frameworks)]
/// #![test_runner(sovelma_kernel::testutil::test_runner)]
/// ```
pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

/// Panic handler for test binaries.
///
/// Reports test failure and exits QEMU with failure code.
///
/// # Usage
///
/// Use this in your test binary's panic handler:
///
/// ```rust,ignore
/// #[panic_handler]
/// fn panic(info: &PanicInfo) -> ! {
///     sovelma_kernel::testutil::test_panic_handler(info)
/// }
/// ```
pub fn test_panic_handler(info: &core::panic::PanicInfo) -> ! {
    serial_println!("[failed]");
    serial_println!("Error: {}", info);
    exit_qemu(QemuExitCode::Failed);
    crate::arch::x86_64::halt_loop()
}
