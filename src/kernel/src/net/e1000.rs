//! Intel e1000 (82540EM) network driver for QEMU.
//!
//! This module implements a real e1000 network driver that uses MMIO to
//! communicate with the hardware. It is designed for QEMU's e1000 emulation.
//!
//! # Hardware Overview
//!
//! The e1000 is a PCI network controller with:
//! - Memory-mapped I/O registers
//! - Descriptor ring buffers for TX/RX
//! - Interrupt-driven packet notification
//!
//! # Implementation Notes
//!
//! This driver uses polling mode (no interrupts) for simplicity. Packet
//! buffers are statically allocated in kernel memory.
//!
//! # References
//!
//! - Intel 8254x Software Developer's Manual
//! - OSDev Wiki: Intel Ethernet i8254x

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ptr::{read_volatile, write_volatile};
use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::time::Instant;

use crate::arch::x86_64::pci::{self, PciDevice};

/// Maximum transmission unit (standard Ethernet).
const MTU: usize = 1500;

/// Total packet buffer size (MTU + Ethernet header + padding).
const PACKET_BUFFER_SIZE: usize = 2048;

/// Number of transmit descriptors.
const TX_DESC_COUNT: usize = 32;

/// Number of receive descriptors.
const RX_DESC_COUNT: usize = 32;

// ============================================================================
// e1000 Register Offsets
// ============================================================================

mod regs {
    /// Device Control Register.
    pub const CTRL: u32 = 0x0000;
    /// Interrupt Cause Read.
    pub const ICR: u32 = 0x00C0;
    /// Interrupt Mask Clear.
    pub const IMC: u32 = 0x00D8;

    /// Receive Control Register.
    pub const RCTL: u32 = 0x0100;
    /// Receive Descriptor Base Low.
    pub const RDBAL: u32 = 0x2800;
    /// Receive Descriptor Base High.
    pub const RDBAH: u32 = 0x2804;
    /// Receive Descriptor Length.
    pub const RDLEN: u32 = 0x2808;
    /// Receive Descriptor Head.
    pub const RDH: u32 = 0x2810;
    /// Receive Descriptor Tail.
    pub const RDT: u32 = 0x2818;

    /// Transmit Control Register.
    pub const TCTL: u32 = 0x0400;
    /// Transmit IPG Register.
    pub const TIPG: u32 = 0x0410;
    /// Transmit Descriptor Base Low.
    pub const TDBAL: u32 = 0x3800;
    /// Transmit Descriptor Base High.
    pub const TDBAH: u32 = 0x3804;
    /// Transmit Descriptor Length.
    pub const TDLEN: u32 = 0x3808;
    /// Transmit Descriptor Head.
    pub const TDH: u32 = 0x3810;
    /// Transmit Descriptor Tail.
    pub const TDT: u32 = 0x3818;

    /// Receive Address Low (MAC address bytes 0-3).
    pub const RAL0: u32 = 0x5400;
    /// Receive Address High (MAC address bytes 4-5 + valid bit).
    pub const RAH0: u32 = 0x5404;

    /// Multicast Table Array (128 entries).
    pub const MTA_BASE: u32 = 0x5200;
}

/// Device Control register bits.
mod ctrl {
    /// Software reset. Self-clearing.
    pub const RST: u32 = 1 << 26;
    /// Set Link Up. Force link to be up.
    pub const SLU: u32 = 1 << 6;
    /// Auto-Speed Detection Enable.
    pub const ASDE: u32 = 1 << 5;
}

/// Receive Control register bits.
mod rctl {
    /// Receiver Enable.
    pub const EN: u32 = 1 << 1;
    /// Receive Buffer Size (00 = 2048 bytes).
    pub const BSIZE_2048: u32 = 0 << 16;
    /// Strip Ethernet CRC.
    pub const SECRC: u32 = 1 << 26;
    /// Broadcast Accept Mode.
    pub const BAM: u32 = 1 << 15;
}

/// Transmit Control register bits.
mod tctl {
    /// Transmitter Enable.
    pub const EN: u32 = 1 << 1;
    /// Pad Short Packets.
    pub const PSP: u32 = 1 << 3;
    /// Collision Threshold (bits 11:4).
    pub const CT_SHIFT: u32 = 4;
    /// Collision Distance (bits 21:12).
    pub const COLD_SHIFT: u32 = 12;
}

/// Transmit descriptor command bits.
mod txd_cmd {
    /// End of Packet.
    pub const EOP: u8 = 1 << 0;
    /// Insert FCS/CRC.
    pub const IFCS: u8 = 1 << 1;
    /// Report Status.
    pub const RS: u8 = 1 << 3;
}

/// Transmit descriptor status bits.
mod txd_stat {
    /// Descriptor Done.
    pub const DD: u8 = 1 << 0;
}

/// Receive descriptor status bits.
mod rxd_stat {
    /// Descriptor Done.
    pub const DD: u8 = 1 << 0;
}

// ============================================================================
// Descriptor Structures
// ============================================================================

/// Legacy transmit descriptor (16 bytes).
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
struct TxDesc {
    /// Buffer address (physical).
    addr: u64,
    /// Length of data.
    length: u16,
    /// Checksum offset.
    cso: u8,
    /// Command field.
    cmd: u8,
    /// Status field.
    status: u8,
    /// Checksum start field.
    css: u8,
    /// Special field.
    special: u16,
}

/// Legacy receive descriptor (16 bytes).
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
struct RxDesc {
    /// Buffer address (physical).
    addr: u64,
    /// Length of received data.
    length: u16,
    /// Packet checksum.
    checksum: u16,
    /// Status field.
    status: u8,
    /// Errors field.
    errors: u8,
    /// Special field.
    special: u16,
}

// ============================================================================
// Driver State
// ============================================================================

/// Intel e1000 network device driver.
///
/// Provides a smoltcp-compatible Device implementation for real networking
/// via QEMU's e1000 emulation.
pub struct E1000 {
    /// MMIO base address (virtual = physical in identity-mapped region).
    mmio_base: *mut u32,
    /// MAC address.
    mac_address: [u8; 6],
    /// Transmit descriptors (physically contiguous).
    tx_descs: Box<[TxDesc; TX_DESC_COUNT]>,
    /// Transmit buffers.
    tx_buffers: Box<[[u8; PACKET_BUFFER_SIZE]; TX_DESC_COUNT]>,
    /// Current transmit descriptor index.
    tx_cur: usize,
    /// Receive descriptors (physically contiguous).
    rx_descs: Box<[RxDesc; RX_DESC_COUNT]>,
    /// Receive buffers.
    rx_buffers: Box<[[u8; PACKET_BUFFER_SIZE]; RX_DESC_COUNT]>,
    /// Current receive descriptor index.
    rx_cur: usize,
}

// SAFETY: The E1000 driver contains a raw pointer to MMIO space. This is safe
// to send between threads because:
// 1. The MMIO region is fixed hardware - it doesn't move
// 2. Access is serialized through the spin::Mutex wrapper in NetworkStack
// 3. The kernel is single-core for now, and even with SMP, the mutex provides
//    the necessary synchronization
unsafe impl Send for E1000 {}

// SAFETY: Same reasoning as Send - access is serialized through a Mutex
unsafe impl Sync for E1000 {}

impl E1000 {
    /// Create a new e1000 driver from a PCI device.
    ///
    /// The `phys_mem_offset` is the virtual address offset where all physical
    /// memory is mapped (from the bootloader). This is needed to convert the
    /// PCI BAR physical address to a virtual address.
    ///
    /// Returns `None` if the device cannot be initialized.
    pub fn new(pci_dev: PciDevice, phys_mem_offset: u64) -> Option<Self> {
        // Get MMIO base address and convert to virtual address
        let mmio_phys = pci_dev.mmio_base()?;
        let mmio_base = (mmio_phys + phys_mem_offset) as *mut u32;

        // Enable PCI bus mastering and memory access
        pci_dev.enable();

        // Allocate descriptor rings and buffers
        // Note: In a real OS, these would need to be in DMA-accessible memory
        // For QEMU with identity mapping, heap memory works fine
        let tx_descs = Box::new([TxDesc::default(); TX_DESC_COUNT]);
        let tx_buffers = Box::new([[0u8; PACKET_BUFFER_SIZE]; TX_DESC_COUNT]);
        let rx_descs = Box::new([RxDesc::default(); RX_DESC_COUNT]);
        let rx_buffers = Box::new([[0u8; PACKET_BUFFER_SIZE]; RX_DESC_COUNT]);

        let mut dev = Self {
            mmio_base,
            mac_address: [0; 6],
            tx_descs,
            tx_buffers,
            tx_cur: 0,
            rx_descs,
            rx_buffers,
            rx_cur: 0,
        };

        dev.reset();
        dev.read_mac_address();
        dev.init_tx();
        dev.init_rx();
        dev.enable_interrupts();

        Some(dev)
    }

    /// Probe for and initialize an e1000 device.
    ///
    /// The `phys_mem_offset` is the virtual address offset where all physical
    /// memory is mapped (from the bootloader).
    ///
    /// Scans the PCI bus for an e1000 and initializes it if found.
    pub fn probe(phys_mem_offset: u64) -> Option<Self> {
        let pci_dev = pci::find_e1000()?;
        Self::new(pci_dev, phys_mem_offset)
    }

    /// Get the MAC address of this device.
    pub fn mac_address(&self) -> [u8; 6] {
        self.mac_address
    }

    // ========================================================================
    // Register Access
    // ========================================================================

    /// Read a 32-bit register.
    fn read_reg(&self, offset: u32) -> u32 {
        // SAFETY: We've verified mmio_base is valid from the PCI BAR.
        // Volatile read is required for MMIO.
        unsafe { read_volatile(self.mmio_base.byte_add(offset as usize)) }
    }

    /// Write a 32-bit register.
    fn write_reg(&self, offset: u32, value: u32) {
        // SAFETY: We've verified mmio_base is valid from the PCI BAR.
        // Volatile write is required for MMIO.
        unsafe { write_volatile(self.mmio_base.byte_add(offset as usize), value) }
    }

    // ========================================================================
    // Initialization
    // ========================================================================

    /// Reset the device.
    fn reset(&self) {
        // Disable interrupts
        self.write_reg(regs::IMC, 0xFFFF_FFFF);

        // Reset the device
        self.write_reg(regs::CTRL, ctrl::RST);

        // Wait for reset to complete (self-clearing bit)
        while (self.read_reg(regs::CTRL) & ctrl::RST) != 0 {
            core::hint::spin_loop();
        }

        // Disable interrupts again after reset
        self.write_reg(regs::IMC, 0xFFFF_FFFF);

        // Clear pending interrupts
        let _ = self.read_reg(regs::ICR);
    }

    /// Read MAC address from the device.
    fn read_mac_address(&mut self) {
        // Try reading from RAL0/RAH0 first (set by QEMU)
        let ral = self.read_reg(regs::RAL0);
        let rah = self.read_reg(regs::RAH0);

        // Check if valid (bit 31 of RAH is Address Valid)
        if (rah & (1 << 31)) != 0 || ral != 0 {
            self.mac_address[0] = ral as u8;
            self.mac_address[1] = (ral >> 8) as u8;
            self.mac_address[2] = (ral >> 16) as u8;
            self.mac_address[3] = (ral >> 24) as u8;
            self.mac_address[4] = rah as u8;
            self.mac_address[5] = (rah >> 8) as u8;
        } else {
            // Fallback: use QEMU's default MAC
            self.mac_address = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        }
    }

    /// Initialize transmit ring.
    fn init_tx(&mut self) {
        // Set up transmit descriptor buffer addresses
        for i in 0..TX_DESC_COUNT {
            self.tx_descs[i].addr = self.tx_buffers[i].as_ptr() as u64;
            self.tx_descs[i].status = txd_stat::DD; // Mark as available
        }

        // Program transmit descriptor base address
        let tx_desc_phys = self.tx_descs.as_ptr() as u64;
        self.write_reg(regs::TDBAL, tx_desc_phys as u32);
        self.write_reg(regs::TDBAH, (tx_desc_phys >> 32) as u32);

        // Program transmit descriptor length (in bytes)
        let tx_len = (TX_DESC_COUNT * core::mem::size_of::<TxDesc>()) as u32;
        self.write_reg(regs::TDLEN, tx_len);

        // Set head and tail
        self.write_reg(regs::TDH, 0);
        self.write_reg(regs::TDT, 0);

        // Configure TIPG (Inter Packet Gap) for IEEE 802.3
        // IPGT=10, IPGR1=10, IPGR2=10
        self.write_reg(regs::TIPG, 0x0060_200A);

        // Enable transmitter
        // CT=0x10 (collision threshold), COLD=0x40 (collision distance for FD)
        let tctl = tctl::EN | tctl::PSP | (0x10 << tctl::CT_SHIFT) | (0x40 << tctl::COLD_SHIFT);
        self.write_reg(regs::TCTL, tctl);
    }

    /// Initialize receive ring.
    fn init_rx(&mut self) {
        // Set up receive descriptor buffer addresses
        for i in 0..RX_DESC_COUNT {
            self.rx_descs[i].addr = self.rx_buffers[i].as_ptr() as u64;
            self.rx_descs[i].status = 0;
        }

        // Program receive descriptor base address
        let rx_desc_phys = self.rx_descs.as_ptr() as u64;
        self.write_reg(regs::RDBAL, rx_desc_phys as u32);
        self.write_reg(regs::RDBAH, (rx_desc_phys >> 32) as u32);

        // Program receive descriptor length (in bytes)
        let rx_len = (RX_DESC_COUNT * core::mem::size_of::<RxDesc>()) as u32;
        self.write_reg(regs::RDLEN, rx_len);

        // Set head and tail
        self.write_reg(regs::RDH, 0);
        // Tail points to last descriptor (not past it for RX)
        self.write_reg(regs::RDT, (RX_DESC_COUNT - 1) as u32);

        // Clear multicast table array
        for i in 0..128 {
            self.write_reg(regs::MTA_BASE + i * 4, 0);
        }

        // Enable receiver
        let rctl = rctl::EN
            | rctl::BAM           // Accept broadcast
            | rctl::BSIZE_2048    // 2048 byte buffers
            | rctl::SECRC; // Strip CRC
        self.write_reg(regs::RCTL, rctl);

        // Force link up
        let ctrl = self.read_reg(regs::CTRL);
        self.write_reg(regs::CTRL, ctrl | ctrl::SLU | ctrl::ASDE);
    }

    /// Enable (or in our case, acknowledge) interrupts.
    fn enable_interrupts(&self) {
        // For polling mode, we just clear any pending interrupts
        let _ = self.read_reg(regs::ICR);
    }

    // ========================================================================
    // Packet Transmission
    // ========================================================================

    /// Transmit a packet.
    ///
    /// Returns `true` if the packet was queued successfully.
    fn transmit_packet(&mut self, data: &[u8]) -> bool {
        if data.len() > PACKET_BUFFER_SIZE {
            return false;
        }

        let idx = self.tx_cur;
        let desc = &mut self.tx_descs[idx];

        // Wait for descriptor to be available (DD set)
        // SAFETY: TxDesc is repr(C, packed), reading status is safe
        if (desc.status & txd_stat::DD) == 0 {
            return false; // Descriptor still in use
        }

        // Copy data to buffer
        self.tx_buffers[idx][..data.len()].copy_from_slice(data);

        // Set up descriptor
        desc.length = data.len() as u16;
        desc.cmd = txd_cmd::EOP | txd_cmd::IFCS | txd_cmd::RS;
        desc.status = 0; // Clear DD - hardware will set it when done

        // Advance tail
        self.tx_cur = (self.tx_cur + 1) % TX_DESC_COUNT;
        self.write_reg(regs::TDT, self.tx_cur as u32);

        true
    }

    // ========================================================================
    // Packet Reception
    // ========================================================================

    /// Receive a packet.
    ///
    /// Returns the packet data if available, `None` otherwise.
    fn receive_packet(&mut self) -> Option<Vec<u8>> {
        let idx = self.rx_cur;
        let desc = &mut self.rx_descs[idx];

        // Check if descriptor has data
        if (desc.status & rxd_stat::DD) == 0 {
            return None;
        }

        // Get packet length and copy data
        let len = desc.length as usize;
        let data = self.rx_buffers[idx][..len].to_vec();

        // Reset descriptor for reuse
        desc.status = 0;

        // Advance to next descriptor
        let old_cur = self.rx_cur;
        self.rx_cur = (self.rx_cur + 1) % RX_DESC_COUNT;

        // Update tail to allow hardware to use this descriptor again
        self.write_reg(regs::RDT, old_cur as u32);

        Some(data)
    }
}

// ============================================================================
// smoltcp Device Implementation
// ============================================================================

/// Receive token for E1000.
pub struct E1000RxToken {
    buffer: Vec<u8>,
}

/// Transmit token for E1000.
pub struct E1000TxToken<'a> {
    device: &'a mut E1000,
}

impl RxToken for E1000RxToken {
    fn consume<R, F>(mut self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        f(&mut self.buffer)
    }
}

impl<'a> TxToken for E1000TxToken<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buffer = alloc::vec![0u8; len];
        let result = f(&mut buffer);
        self.device.transmit_packet(&buffer);
        result
    }
}

impl Device for E1000 {
    type RxToken<'a> = E1000RxToken where Self: 'a;
    type TxToken<'a> = E1000TxToken<'a> where Self: 'a;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        self.receive_packet()
            .map(|buffer| (E1000RxToken { buffer }, E1000TxToken { device: self }))
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        // Check if we have a free transmit descriptor
        let desc = &self.tx_descs[self.tx_cur];
        if (desc.status & txd_stat::DD) != 0 {
            Some(E1000TxToken { device: self })
        } else {
            None
        }
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ethernet;
        caps.max_transmission_unit = MTU;
        caps.max_burst_size = Some(1);
        caps
    }
}
