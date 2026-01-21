//! VGA text mode driver for x86_64.
//!
//! Provides colored text output to the VGA text buffer at 0xB8000.

use core::fmt::{self, Write};
use core::ptr;
use spin::Mutex;

/// VGA text buffer memory-mapped I/O address.
const VGA_BUFFER_ADDR: usize = 0xB8000;

/// Number of rows in VGA text mode.
const BUFFER_HEIGHT: usize = 25;

/// Number of columns in VGA text mode.
const BUFFER_WIDTH: usize = 80;

/// VGA color codes.
///
/// Standard 16-color VGA palette for text mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    /// Black color.
    Black = 0,
    /// Blue color.
    Blue = 1,
    /// Green color.
    Green = 2,
    /// Cyan color.
    Cyan = 3,
    /// Red color.
    Red = 4,
    /// Magenta color.
    Magenta = 5,
    /// Brown color.
    Brown = 6,
    /// Light gray color.
    LightGray = 7,
    /// Dark gray color.
    DarkGray = 8,
    /// Light blue color.
    LightBlue = 9,
    /// Light green color.
    LightGreen = 10,
    /// Light cyan color.
    LightCyan = 11,
    /// Light red color.
    LightRed = 12,
    /// Pink color.
    Pink = 13,
    /// Yellow color.
    Yellow = 14,
    /// White color.
    White = 15,
}

/// Combined foreground and background color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct ColorCode(u8);

impl ColorCode {
    /// Creates a new color code from foreground and background colors.
    const fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

/// A single character cell in the VGA buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

/// The VGA text buffer layout.
#[repr(transparent)]
struct Buffer {
    chars: [[ScreenChar; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

/// Global VGA writer instance.
///
/// Uses a spinlock for safe concurrent access.
pub static WRITER: spin::Once<Mutex<Writer>> = spin::Once::new();

/// Initializes the global VGA writer.
///
/// Idempotent - safe to call multiple times.
pub fn init() {
    WRITER.call_once(|| Mutex::new(Writer::new()));
}

/// Returns a reference to the VGA writer, initializing if necessary.
fn get_writer() -> &'static Mutex<Writer> {
    init();
    WRITER.get().expect("VGA writer not initialized")
}

/// VGA text mode writer.
///
/// Manages cursor position and color state for writing to the VGA buffer.
pub struct Writer {
    /// Current column position (0 to BUFFER_WIDTH-1).
    column_position: usize,
    /// Current color code for new characters.
    color_code: ColorCode,
    /// Pointer to the VGA buffer.
    ///
    /// SAFETY: This pointer is valid for the lifetime of the kernel.
    /// The VGA buffer at 0xB8000 is always mapped in x86 real/protected mode.
    buffer: *mut Buffer,
}

// SAFETY: Writer only accesses the VGA buffer through volatile operations.
// The buffer is memory-mapped hardware that exists for the kernel's lifetime.
// Access is synchronized through the WRITER spinlock.
unsafe impl Send for Writer {}

impl Writer {
    /// Creates a new VGA writer.
    fn new() -> Self {
        Writer {
            column_position: 0,
            color_code: ColorCode::new(Color::White, Color::Black),
            // SAFETY: VGA_BUFFER_ADDR (0xB8000) is the standard VGA text buffer
            // address on x86 systems. This memory is always present and mapped
            // when running on x86 hardware or in QEMU.
            buffer: VGA_BUFFER_ADDR as *mut Buffer,
        }
    }

    /// Sets the foreground and background colors for subsequent writes.
    pub fn set_color(&mut self, foreground: Color, background: Color) {
        self.color_code = ColorCode::new(foreground, background);
    }

    /// Writes a single byte to the VGA buffer.
    ///
    /// Handles newlines and automatic line wrapping.
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                // Check bounds BEFORE writing to prevent overflow
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;

                // SAFETY: We've verified col < BUFFER_WIDTH above, and row is
                // constant at BUFFER_HEIGHT - 1. The buffer pointer was validated
                // at construction time. Using volatile write because the VGA buffer
                // is memory-mapped I/O that may be read by hardware at any time.
                unsafe {
                    ptr::write_volatile(
                        &mut (*self.buffer).chars[row][col],
                        ScreenChar {
                            ascii_character: byte,
                            color_code: self.color_code,
                        },
                    );
                }
                self.column_position += 1;
            }
        }
    }

    /// Scrolls the screen up by one line.
    fn new_line(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                // SAFETY: row is in range [1, BUFFER_HEIGHT), col is in range [0, BUFFER_WIDTH).
                // row-1 is in range [0, BUFFER_HEIGHT-1). All indices are valid.
                // Using volatile operations because VGA buffer is memory-mapped I/O.
                unsafe {
                    let character = ptr::read_volatile(&(*self.buffer).chars[row][col]);
                    ptr::write_volatile(&mut (*self.buffer).chars[row - 1][col], character);
                }
            }
        }
        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
    }

    /// Clears a single row by filling it with spaces.
    fn clear_row(&mut self, row: usize) {
        debug_assert!(row < BUFFER_HEIGHT, "row index out of bounds");

        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };
        for col in 0..BUFFER_WIDTH {
            // SAFETY: row is asserted to be < BUFFER_HEIGHT, col is in [0, BUFFER_WIDTH).
            // Using volatile write because VGA buffer is memory-mapped I/O.
            unsafe {
                ptr::write_volatile(&mut (*self.buffer).chars[row][col], blank);
            }
        }
    }

    /// Clears the entire screen.
    pub fn clear_screen(&mut self) {
        for row in 0..BUFFER_HEIGHT {
            self.clear_row(row);
        }
        self.column_position = 0;
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            match byte {
                // Printable ASCII or newline
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                // Non-printable: show placeholder
                _ => self.write_byte(0xfe),
            }
        }
        Ok(())
    }
}

impl sovelma_hal::Console for Writer {
    fn write_str(&mut self, s: &str) {
        let _ = <Self as fmt::Write>::write_str(self, s);
    }

    fn clear(&mut self) {
        self.clear_screen();
    }
}

/// Prints to the VGA buffer without a newline.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::arch::x86_64::vga::_print(format_args!($($arg)*))
    };
}

/// Prints to the VGA buffer with a newline.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)))
}

/// Internal print function used by macros.
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    let writer = get_writer();
    writer.lock().write_fmt(args).expect("vga write failed");
}

/// Sets the VGA output color.
pub fn set_color(foreground: Color, background: Color) {
    get_writer().lock().set_color(foreground, background);
}

/// Clears the VGA screen.
pub fn clear_screen() {
    get_writer().lock().clear_screen();
}
