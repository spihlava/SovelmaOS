//! Boot logging with colored status indicators.
//!
//! Provides Linux-style boot messages with colored status brackets.

pub mod banner;

use crate::arch::x86_64::vga::{self, Color};
use crate::{print, println};

/// Boot status indicators.
#[derive(Debug, Clone, Copy)]
pub enum Status {
    /// Success - `[ OK ]` in green
    Ok,
    /// Failure - `[FAIL]` in red
    Fail,
    /// Warning - `[WARN]` in yellow
    Warn,
    /// Informational - `[INFO]` in cyan
    Info,
}

/// Log a boot stage with status.
///
/// Format: `[ OK ] Message text`
pub fn log(status: Status, message: &str) {
    print_status(status);
    println!(" {}", message);
}

/// Log an indented detail line (for sub-items).
///
/// Format: `       Detail text` (aligned with message after status)
pub fn log_detail(message: &str) {
    println!("       {}", message);
}

/// Log a section header.
///
/// Prints a blank line before the header for visual separation.
pub fn log_section(name: &str) {
    println!();
    vga::set_color(Color::LightCyan, Color::Black);
    println!("── {} ──", name);
    vga::set_color(Color::White, Color::Black);
}

fn print_status(status: Status) {
    let (text, color) = match status {
        Status::Ok => ("[ OK ]", Color::LightGreen),
        Status::Fail => ("[FAIL]", Color::LightRed),
        Status::Warn => ("[WARN]", Color::Yellow),
        Status::Info => ("[INFO]", Color::LightCyan),
    };
    vga::set_color(color, Color::Black);
    print!("{}", text);
    vga::set_color(Color::White, Color::Black);
}
