#![no_std]
#![no_main]

#[cfg(target_arch = "x86_64")]
use core::panic::PanicInfo;

// ---------------------------------------------------------------------------
// x86_64 Implementation
// ---------------------------------------------------------------------------

#[cfg(target_arch = "x86_64")]
mod x86_impl {
    use debug;
    use core::fmt::Write;
    use uart_16550::SerialPort;

    pub fn init_serial() -> SerialPort {
        let mut serial = unsafe { SerialPort::new(0x3F8) };
        serial.init();
        serial
    }
}

#[cfg(target_arch = "x86_64")]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    use core::fmt::Write;
    let mut serial = x86_impl::init_serial();
    
    let _ = writeln!(serial, "SovelmaOS v0.1.0 booting on x86_64...");
    
    // Ensure we don't return
    loop {
        x86_64::instructions::hlt();
    }
}

#[cfg(target_arch = "x86_64")]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    use core::fmt::Write;
    let mut serial = x86_impl::init_serial();
    let _ = writeln!(serial, "KERNEL PANIC");
    loop {
        x86_64::instructions::hlt();
    }
}

// ---------------------------------------------------------------------------
// ESP32-C6 (RISC-V) Implementation
// ---------------------------------------------------------------------------

#[cfg(target_arch = "riscv32")]
#[entry]
fn main() -> ! {
    use esp_hal::prelude::*;
    use esp_backtrace as _; // Register panic handler
    
    let peripherals = esp_hal::init(esp_hal::Config::default());
    
    // UART0 is standard log output on ESP32
    esp_println::println!("SovelmaOS v0.1.0 booting on ESP32-C6...");
    
    loop {
       esp_hal::delay::Delay::new().delay_millis(1000); 
    }
}

// Note: esp-backtrace handles panic for riscv32
