//! Serial port driver for x86_64.
//!
//! Provides serial output via COM1 (0x3F8) for debugging and logging.

use core::fmt::{self, Write};
use spin::Mutex;
use uart_16550::SerialPort;

/// COM1 I/O port address.
const COM1_PORT: u16 = 0x3F8;

/// Global serial port instance, lazily initialized.
///
/// Uses a spinlock for safe concurrent access from multiple contexts,
/// including interrupt handlers.
pub static SERIAL: spin::Once<Mutex<SerialPort>> = spin::Once::new();

/// Initializes the global serial port.
///
/// This function is idempotent - calling it multiple times has no effect
/// after the first successful initialization.
pub fn init() {
    SERIAL.call_once(|| {
        // SAFETY: COM1_PORT (0x3F8) is a well-known x86 serial port address.
        // We're running in kernel mode with full I/O port access.
        // The uart_16550 crate handles the port initialization sequence correctly.
        let mut serial = unsafe { SerialPort::new(COM1_PORT) };
        serial.init();
        Mutex::new(serial)
    });
}

/// Returns a reference to the serial port, initializing if necessary.
fn get_serial() -> &'static Mutex<SerialPort> {
    init();
    SERIAL.get().expect("serial port not initialized")
}

/// Prints to the serial port without a newline.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::arch::x86_64::serial::_print(format_args!($($arg)*))
    };
}

/// Prints to the serial port with a newline.
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($($arg:tt)*) => ($crate::serial_print!("{}\n", format_args!($($arg)*)))
}

/// Internal print function used by macros.
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    let serial = get_serial();
    serial.lock().write_fmt(args).expect("serial write failed");
}

/// A wrapper to implement HAL traits for the serial port.
pub struct SerialWrapper;

impl sovelma_hal::Serial for SerialWrapper {
    fn write_byte(&mut self, byte: u8) {
        get_serial().lock().send(byte);
    }

    fn read_byte(&mut self) -> Option<u8> {
        let mut port = get_serial().lock();
        if port.receive() != 0 {
            // Note: uart_16550's receive() is a bit simplistic,
            // but this works for basic HAL abstraction.
            Some(port.receive())
        } else {
            None
        }
    }
}
