//! SovelmaOS Hardware Abstraction Layer (HAL) traits.
//!
//! This crate defines traits that abstract away platform-specific hardware details.

#![no_std]

/// Trait for a serial port or similar character-based communication channel.
pub trait Serial {
    /// Writes a single byte to the serial port.
    fn write_byte(&mut self, byte: u8);
    /// Reads a single byte from the serial port, if available.
    fn read_byte(&mut self) -> Option<u8>;
}

/// Trait for a text-based console output.
pub trait Console {
    /// Writes a string to the console.
    fn write_str(&mut self, s: &str);
    /// Clears the console screen.
    fn clear(&mut self);
}

/// Trait for controlling interrupts.
pub trait InterruptController {
    /// Globally enables interrupts.
    fn enable(&mut self);
    /// Globally disables interrupts.
    fn disable(&mut self);
    /// Signals the end of an interrupt to the controller.
    fn end_of_interrupt(&mut self, irq: u8);
}

/// Trait for a system timer.
pub trait Timer {
    /// Returns the number of ticks since the system started.
    fn current_ticks(&self) -> u64;
}
