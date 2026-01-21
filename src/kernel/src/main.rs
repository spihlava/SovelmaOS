//! SovelmaOS Kernel Entry Point
//!
//! This is the main entry point for the SovelmaOS kernel.

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use sovelma_kernel::arch::x86_64::{self, vga::Color};
use sovelma_kernel::{print, println, serial_println};

/// Kernel entry point.
///
/// Called by the bootloader after setting up the initial environment.
#[no_mangle]
pub extern "C" fn _start() -> ! {
    sovelma_kernel::init();

    x86_64::vga::clear_screen();

    // Banner
    x86_64::vga::set_color(Color::Cyan, Color::Black);
    println!("  ____                 _              ___  ____  ");
    println!(" / ___|  _____   _____| |_ __ ___   / _ \\/ ___| ");
    println!(" \\___ \\ / _ \\ \\ / / _ \\ | '_ ` _ \\ | | | \\___ \\ ");
    println!("  ___) | (_) \\ V /  __/ | | | | | | | |_| |___) |");
    println!(" |____/ \\___/ \\_/ \\___|_|_| |_| |_|  \\___/|____/ ");
    println!();

    x86_64::vga::set_color(Color::White, Color::Black);
    println!(" SovelmaOS v0.1.0 booting...");
    println!(" ---------------------------");

    serial_println!("[OK] Serial initialized");

    // Boot milestones
    x86_64::vga::set_color(Color::LightGreen, Color::Black);
    print!(" [DONE] ");
    x86_64::vga::set_color(Color::White, Color::Black);
    println!("Serial port initialized (COM1)");

    x86_64::vga::set_color(Color::LightGreen, Color::Black);
    print!(" [DONE] ");
    x86_64::vga::set_color(Color::White, Color::Black);
    println!("VGA text buffer initialized");

    x86_64::vga::set_color(Color::LightGreen, Color::Black);
    print!(" [DONE] ");
    x86_64::vga::set_color(Color::White, Color::Black);
    println!("Kernel entry point reached");

    serial_println!("[OK] VGA text buffer initialized");
    serial_println!("SovelmaOS kernel entering idle loop...");

    println!();
    x86_64::vga::set_color(Color::Yellow, Color::Black);
    println!(" Kernel is now running in an idle loop.");

    x86_64::halt_loop()
}

/// Panic handler.
///
/// Called when the kernel encounters an unrecoverable error.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Use the already-initialized serial port
    serial_println!("KERNEL PANIC: {}", info);

    x86_64::vga::set_color(Color::LightRed, Color::Black);
    println!("\n\n!!! KERNEL PANIC !!!");
    x86_64::vga::set_color(Color::White, Color::Black);
    println!("{}", info);

    x86_64::halt_loop()
}
