//! Support for the primary and secondary 8259 Programmable Interrupt Controllers (PICs).

use pic8259::ChainedPics;
use spin::Mutex;

/// The offset of the first PIC (master).
///
/// IRQs 0..7 are mapped to interrupts 32..39.
pub const PIC_1_OFFSET: u8 = 32;

/// The offset of the second PIC (slave).
///
/// IRQs 8..15 are mapped to interrupts 40..47.
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

/// The global instance of the chained PICs.
pub static PICS: Mutex<ChainedPics> =
    Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

/// Possible IRQ indices.
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    /// Timer interrupt.
    Timer = PIC_1_OFFSET,
    /// Keyboard interrupt.
    Keyboard,
}

impl InterruptIndex {
    /// Returns the internal u8 value.
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Returns the internal usize value.
    pub fn as_usize(self) -> usize {
        usize::from(self as u8)
    }
}
