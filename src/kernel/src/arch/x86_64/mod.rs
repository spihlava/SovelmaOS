//! x86_64 architecture support.
//!
//! Provides VGA text mode output and serial port communication for x86_64 platforms.

pub mod serial;
pub mod vga;

pub use serial::SERIAL;
pub use vga::{Color, Writer, WRITER};

/// Halts the CPU until the next interrupt.
///
/// Used in idle loops to reduce power consumption.
#[inline]
pub fn hlt() {
    x86_64::instructions::hlt();
}

/// Halts the CPU in an infinite loop.
///
/// Used after unrecoverable errors (panics).
pub fn halt_loop() -> ! {
    loop {
        hlt();
    }
}
