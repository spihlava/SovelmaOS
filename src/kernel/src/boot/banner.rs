//! Boot banner and branding.

use crate::arch::x86_64::vga::{self, Color};
use crate::println;

/// Print the SovelmaOS boot banner.
pub fn print_banner() {
    vga::set_color(Color::Cyan, Color::Black);
    println!("  ____                 _              ___  ____  ");
    println!(" / ___|  _____   _____| |_ __ ___   / _ \\/ ___| ");
    println!(" \\___ \\ / _ \\ \\ / / _ \\ | '_ ` _ \\ | | | \\___ \\ ");
    println!("  ___) | (_) \\ V /  __/ | | | | | || |_| |___) |");
    println!(" |____/ \\___/ \\_/ \\___|_|_| |_| |_| \\___/|____/ ");
    println!();
    vga::set_color(Color::White, Color::Black);
    println!(" SovelmaOS v0.1.0");
    println!();
}
