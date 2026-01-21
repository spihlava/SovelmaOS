#![no_std]
#![no_main]

#[cfg(target_arch = "x86_64")]
use core::panic::PanicInfo;

// ---------------------------------------------------------------------------
// x86_64 Implementation
// ---------------------------------------------------------------------------

#[cfg(target_arch = "x86_64")]
mod x86_impl {
    use core::fmt::{self, Write};
    use core::ptr;
    use uart_16550::SerialPort;

    pub fn init_serial() -> SerialPort {
        let mut serial = unsafe { SerialPort::new(0x3F8) };
        serial.init();
        serial
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    pub enum Color {
        Black = 0,
        Blue = 1,
        Green = 2,
        Cyan = 3,
        Red = 4,
        Magenta = 5,
        Brown = 6,
        LightGray = 7,
        DarkGray = 8,
        LightBlue = 9,
        LightGreen = 10,
        LightCyan = 11,
        LightRed = 12,
        Pink = 13,
        Yellow = 14,
        White = 15,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(transparent)]
    struct ColorCode(u8);

    impl ColorCode {
        fn new(foreground: Color, background: Color) -> ColorCode {
            ColorCode((background as u8) << 4 | (foreground as u8))
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(C)]
    struct ScreenChar {
        ascii_character: u8,
        color_code: ColorCode,
    }

    const BUFFER_HEIGHT: usize = 25;
    const BUFFER_WIDTH: usize = 80;

    struct Buffer {
        chars: [[ScreenChar; BUFFER_WIDTH]; BUFFER_HEIGHT],
    }

    pub struct Writer {
        column_position: usize,
        color_code: ColorCode,
        buffer: *mut Buffer,
    }

    impl Writer {
        pub fn set_color(&mut self, foreground: Color, background: Color) {
            self.color_code = ColorCode::new(foreground, background);
        }

        pub fn write_byte(&mut self, byte: u8) {
            match byte {
                b'\n' => self.new_line(),
                byte => {
                    if self.column_position >= BUFFER_WIDTH {
                        self.new_line();
                    }

                    let row = BUFFER_HEIGHT - 1;
                    let col = self.column_position;

                    let color_code = self.color_code;
                    unsafe {
                        ptr::write_volatile(
                            &mut (*self.buffer).chars[row][col],
                            ScreenChar {
                                ascii_character: byte,
                                color_code,
                            },
                        );
                    }
                    self.column_position += 1;
                }
            }
        }

        fn new_line(&mut self) {
            for row in 1..BUFFER_HEIGHT {
                for col in 0..BUFFER_WIDTH {
                    unsafe {
                        let character = ptr::read_volatile(&(*self.buffer).chars[row][col]);
                        ptr::write_volatile(&mut (*self.buffer).chars[row - 1][col], character);
                    }
                }
            }
            self.clear_row(BUFFER_HEIGHT - 1);
            self.column_position = 0;
        }

        fn clear_row(&mut self, row: usize) {
            let blank = ScreenChar {
                ascii_character: b' ',
                color_code: self.color_code,
            };
            for col in 0..BUFFER_WIDTH {
                unsafe {
                    ptr::write_volatile(&mut (*self.buffer).chars[row][col], blank);
                }
            }
        }

        pub fn clear_screen(&mut self) {
            for row in 0..BUFFER_HEIGHT {
                self.clear_row(row);
            }
        }
    }

    impl fmt::Write for Writer {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            for byte in s.bytes() {
                self.write_byte(byte);
            }
            Ok(())
        }
    }

    pub fn get_vga_writer() -> Writer {
        Writer {
            column_position: 0,
            color_code: ColorCode::new(Color::White, Color::Black),
            buffer: 0xb8000 as *mut Buffer,
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    use core::fmt::Write;
    use x86_impl::Color;

    let mut serial = x86_impl::init_serial();
    let mut vga = x86_impl::get_vga_writer();
    vga.clear_screen();

    // Banner
    vga.set_color(Color::Cyan, Color::Black);
    let _ = writeln!(vga, "  ____                 _              ___  ____  ");
    let _ = writeln!(vga, " / ___|  _____   _____| |_ __ ___   / _ \\/ ___| ");
    let _ = writeln!(vga, " \\___ \\ / _ \\ \\ / / _ \\ | '_ ` _ \\ | | | \\___ \\ ");
    let _ = writeln!(vga, "  ___) | (_) \\ V /  __/ | | | | | | | |_| |___) |");
    let _ = writeln!(vga, " |____/ \\___/ \\_/ \\___|_|_| |_| |_|  \\___/|____/ ");
    let _ = writeln!(vga, "                                                 ");
    
    vga.set_color(Color::White, Color::Black);
    let _ = writeln!(vga, " SovelmaOS v0.1.0 booting...");
    let _ = writeln!(vga, " ---------------------------");
    
    let _ = writeln!(serial, "[OK] Serial initialized");
    
    // Milestones
    vga.set_color(Color::LightGreen, Color::Black);
    let _ = write!(vga, " [DONE] ");
    vga.set_color(Color::White, Color::Black);
    let _ = writeln!(vga, "Serial port initialized (0x3F8)");

    vga.set_color(Color::LightGreen, Color::Black);
    let _ = write!(vga, " [DONE] ");
    vga.set_color(Color::White, Color::Black);
    let _ = writeln!(vga, "VGA text buffer initialized (0xB8000)");

    vga.set_color(Color::LightGreen, Color::Black);
    let _ = write!(vga, " [DONE] ");
    vga.set_color(Color::White, Color::Black);
    let _ = writeln!(vga, "Kernel entry point reached");

    let _ = writeln!(serial, "[OK] VGA text buffer initialized");
    let _ = writeln!(serial, "SovelmaOS kernel starting loop...");
    
    let _ = writeln!(vga, " ");
    vga.set_color(Color::Yellow, Color::Black);
    let _ = writeln!(vga, " Kernel is now running in an infinite loop.");
    
    // Ensure we don't return
    loop {
        x86_64::instructions::hlt();
    }
}

#[cfg(target_arch = "x86_64")]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    use core::fmt::Write;
    use x86_impl::Color;

    let mut serial = x86_impl::init_serial();
    let _ = writeln!(serial, "KERNEL PANIC: {}", info);
    
    let mut vga = x86_impl::get_vga_writer();
    vga.set_color(Color::LightRed, Color::Black);
    let _ = writeln!(vga, "\n\n!!! KERNEL PANIC !!!");
    vga.set_color(Color::White, Color::Black);
    let _ = writeln!(vga, "{}", info);
    
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
