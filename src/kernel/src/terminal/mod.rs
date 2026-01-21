//! Terminal subsystem for SovelmaOS.
//!
//! Provides a command-line interface with keyboard input handling.
//!
//! # Architecture
//!
//! - `shell`: Command-line shell with input handling
//! - `commands`: Built-in shell commands

pub mod commands;
pub mod shell;

pub use commands::Command;
pub use shell::Terminal;

use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};

/// Global keyboard decoder instance.
static KEYBOARD: spin::Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> =
    spin::Mutex::new(Keyboard::new(
        ScancodeSet1::new(),
        layouts::Us104Key,
        HandleControl::Ignore,
    ));

/// Decode a PS/2 scancode to a key event.
///
/// Returns the decoded key if a complete key event was received.
pub fn decode_scancode(scancode: u8) -> Option<DecodedKey> {
    let mut keyboard = KEYBOARD.lock();
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        keyboard.process_keyevent(key_event)
    } else {
        None
    }
}
