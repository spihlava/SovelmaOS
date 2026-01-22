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
    /// In progress - `[    ]` in gray
    Run,
}

/// Log a boot stage with status.
pub fn log(status: Status, message: &str) {
    print_status(status);
    println!(" {}", message);
}

/// Log start of an operation (shows `[    ]`).
///
/// Follow with `log_end()` to complete the line.
pub fn log_start(message: &str) {
    print_status(Status::Run);
    print!(" {}...", message);
}

/// Complete a previous `log_start()` with final status.
///
/// Overwrites the in-progress indicator with the final status.
pub fn log_end(status: Status) {
    print!("\r");
    print_status(status);
    println!();
}

/// Log an indented detail line (for sub-items).
pub fn log_detail(message: &str) {
    println!("       {}", message);
}

fn print_status(status: Status) {
    let (text, color) = match status {
        Status::Ok => ("[ OK ]", Color::LightGreen),
        Status::Fail => ("[FAIL]", Color::LightRed),
        Status::Warn => ("[WARN]", Color::Yellow),
        Status::Info => ("[INFO]", Color::LightCyan),
        Status::Run => ("[    ]", Color::DarkGray),
    };
    vga::set_color(color, Color::Black);
    print!("{}", text);
    vga::set_color(Color::White, Color::Black);
}
