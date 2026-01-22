//! PCI configuration space access for x86_64.
//!
//! Provides port I/O based access to PCI configuration space for device
//! enumeration and configuration. Uses the legacy PCI mechanism (ports 0xCF8/0xCFC).
//!
//! # References
//!
//! - PCI Local Bus Specification, Section 3.2.2.3.2 "Configuration Mechanism #1"

use x86_64::instructions::port::{Port, PortWriteOnly};

/// PCI configuration address port (0xCF8).
const PCI_CONFIG_ADDRESS: u16 = 0x0CF8;

/// PCI configuration data port (0xCFC).
const PCI_CONFIG_DATA: u16 = 0x0CFC;

/// PCI vendor ID indicating "no device present".
pub const PCI_VENDOR_ID_NONE: u16 = 0xFFFF;

/// Intel vendor ID.
pub const PCI_VENDOR_INTEL: u16 = 0x8086;

/// Intel 82540EM (e1000) device ID - QEMU default.
pub const PCI_DEVICE_E1000_82540EM: u16 = 0x100E;

/// Intel 82545EM (e1000) device ID - alternate.
pub const PCI_DEVICE_E1000_82545EM: u16 = 0x100F;

/// Intel 82574L (e1000e) device ID.
pub const PCI_DEVICE_E1000E_82574L: u16 = 0x10D3;

/// PCI configuration space register offsets.
pub mod reg {
    /// Vendor ID (16-bit).
    pub const VENDOR_ID: u8 = 0x00;
    /// Device ID (16-bit).
    pub const DEVICE_ID: u8 = 0x02;
    /// Command register (16-bit).
    pub const COMMAND: u8 = 0x04;
    /// Status register (16-bit).
    pub const STATUS: u8 = 0x06;
    /// Revision ID (8-bit).
    pub const REVISION_ID: u8 = 0x08;
    /// Class code (24-bit: prog IF, subclass, class).
    pub const CLASS_CODE: u8 = 0x09;
    /// Header type (8-bit).
    pub const HEADER_TYPE: u8 = 0x0E;
    /// Base Address Register 0.
    pub const BAR0: u8 = 0x10;
    /// Base Address Register 1.
    pub const BAR1: u8 = 0x14;
    /// Base Address Register 2.
    pub const BAR2: u8 = 0x18;
    /// Interrupt line (8-bit).
    pub const INTERRUPT_LINE: u8 = 0x3C;
    /// Interrupt pin (8-bit).
    pub const INTERRUPT_PIN: u8 = 0x3D;
}

/// PCI command register bits.
pub mod cmd {
    /// Enable I/O space access.
    pub const IO_SPACE: u16 = 1 << 0;
    /// Enable memory space access.
    pub const MEM_SPACE: u16 = 1 << 1;
    /// Enable bus mastering.
    pub const BUS_MASTER: u16 = 1 << 2;
    /// Disable interrupt generation.
    pub const INTERRUPT_DISABLE: u16 = 1 << 10;
}

/// A PCI device address (bus, device, function).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PciAddress {
    /// Bus number (0-255).
    pub bus: u8,
    /// Device number (0-31).
    pub device: u8,
    /// Function number (0-7).
    pub function: u8,
}

impl PciAddress {
    /// Create a new PCI address.
    pub const fn new(bus: u8, device: u8, function: u8) -> Self {
        Self {
            bus,
            device,
            function,
        }
    }

    /// Build the 32-bit CONFIG_ADDRESS value for a given register offset.
    fn config_address(self, offset: u8) -> u32 {
        let bus32 = self.bus as u32;
        let dev32 = self.device as u32;
        let func32 = self.function as u32;
        let off32 = (offset & 0xFC) as u32; // Must be 4-byte aligned

        // Bit 31: Enable bit
        // Bits 23-16: Bus number
        // Bits 15-11: Device number
        // Bits 10-8: Function number
        // Bits 7-0: Register offset (low 2 bits always 0)
        0x8000_0000 | (bus32 << 16) | (dev32 << 11) | (func32 << 8) | off32
    }
}

/// Read a 32-bit value from PCI configuration space.
///
/// # Safety
///
/// This function performs raw port I/O. It is safe as long as:
/// - The PCI address refers to a valid device slot
/// - The offset is 4-byte aligned
pub fn read_config_u32(addr: PciAddress, offset: u8) -> u32 {
    let config_addr = addr.config_address(offset);

    // SAFETY: Port I/O to PCI config space is safe. The ports are well-defined
    // and reading from them does not corrupt memory.
    unsafe {
        let mut addr_port: PortWriteOnly<u32> = PortWriteOnly::new(PCI_CONFIG_ADDRESS);
        let mut data_port: Port<u32> = Port::new(PCI_CONFIG_DATA);

        addr_port.write(config_addr);
        data_port.read()
    }
}

/// Write a 32-bit value to PCI configuration space.
///
/// # Safety
///
/// This function performs raw port I/O. It modifies PCI configuration
/// which can have system-wide effects.
pub fn write_config_u32(addr: PciAddress, offset: u8, value: u32) {
    let config_addr = addr.config_address(offset);

    // SAFETY: Port I/O to PCI config space is architecturally defined.
    // Writing to configuration space is necessary for device setup.
    unsafe {
        let mut addr_port: PortWriteOnly<u32> = PortWriteOnly::new(PCI_CONFIG_ADDRESS);
        let mut data_port: Port<u32> = Port::new(PCI_CONFIG_DATA);

        addr_port.write(config_addr);
        data_port.write(value);
    }
}

/// Read a 16-bit value from PCI configuration space.
pub fn read_config_u16(addr: PciAddress, offset: u8) -> u16 {
    let dword = read_config_u32(addr, offset & 0xFC);
    let shift = ((offset & 2) * 8) as u32;
    ((dword >> shift) & 0xFFFF) as u16
}

/// Write a 16-bit value to PCI configuration space.
pub fn write_config_u16(addr: PciAddress, offset: u8, value: u16) {
    let aligned_offset = offset & 0xFC;
    let shift = ((offset & 2) * 8) as u32;

    let mut dword = read_config_u32(addr, aligned_offset);
    dword &= !(0xFFFF << shift);
    dword |= (value as u32) << shift;
    write_config_u32(addr, aligned_offset, dword);
}

/// Read an 8-bit value from PCI configuration space.
pub fn read_config_u8(addr: PciAddress, offset: u8) -> u8 {
    let dword = read_config_u32(addr, offset & 0xFC);
    let shift = ((offset & 3) * 8) as u32;
    ((dword >> shift) & 0xFF) as u8
}

/// Information about a discovered PCI device.
#[derive(Debug, Clone, Copy)]
pub struct PciDevice {
    /// PCI address (bus/device/function).
    pub addr: PciAddress,
    /// Vendor ID.
    pub vendor_id: u16,
    /// Device ID.
    pub device_id: u16,
    /// Class code (class << 16 | subclass << 8 | prog_if).
    pub class_code: u32,
    /// BAR0 value.
    pub bar0: u32,
    /// BAR1 value.
    pub bar1: u32,
    /// Interrupt line.
    pub irq: u8,
}

impl PciDevice {
    /// Read device information from a PCI address.
    ///
    /// Returns `None` if no device is present at this address.
    pub fn read(addr: PciAddress) -> Option<Self> {
        let vendor_id = read_config_u16(addr, reg::VENDOR_ID);
        if vendor_id == PCI_VENDOR_ID_NONE {
            return None;
        }

        let device_id = read_config_u16(addr, reg::DEVICE_ID);
        let class_code = read_config_u32(addr, reg::CLASS_CODE) >> 8;
        let bar0 = read_config_u32(addr, reg::BAR0);
        let bar1 = read_config_u32(addr, reg::BAR1);
        let irq = read_config_u8(addr, reg::INTERRUPT_LINE);

        Some(Self {
            addr,
            vendor_id,
            device_id,
            class_code,
            bar0,
            bar1,
            irq,
        })
    }

    /// Check if this is an e1000 network controller.
    pub fn is_e1000(&self) -> bool {
        self.vendor_id == PCI_VENDOR_INTEL
            && (self.device_id == PCI_DEVICE_E1000_82540EM
                || self.device_id == PCI_DEVICE_E1000_82545EM
                || self.device_id == PCI_DEVICE_E1000E_82574L)
    }

    /// Get the memory-mapped I/O base address from BAR0.
    ///
    /// Returns `None` if BAR0 is an I/O port or invalid.
    pub fn mmio_base(&self) -> Option<u64> {
        // Bit 0 = 0 means memory space
        if (self.bar0 & 1) != 0 {
            return None; // I/O space, not memory
        }

        // Bits 2:1 indicate type:
        // 00 = 32-bit
        // 10 = 64-bit
        let bar_type = (self.bar0 >> 1) & 0x3;
        let base = (self.bar0 & 0xFFFF_FFF0) as u64;

        match bar_type {
            0b00 => Some(base),
            0b10 => {
                // 64-bit BAR spans BAR0 and BAR1
                let high = self.bar1 as u64;
                Some((high << 32) | base)
            }
            _ => None,
        }
    }

    /// Enable bus mastering and memory space access for this device.
    pub fn enable(&self) {
        let current = read_config_u16(self.addr, reg::COMMAND);
        let new_cmd = current | cmd::MEM_SPACE | cmd::BUS_MASTER;
        write_config_u16(self.addr, reg::COMMAND, new_cmd);
    }
}

/// Scan all PCI buses for devices.
///
/// Calls the provided callback for each discovered device.
pub fn scan<F>(mut callback: F)
where
    F: FnMut(PciDevice),
{
    for bus in 0..=255u8 {
        for device in 0..32u8 {
            // Check function 0 first
            let addr = PciAddress::new(bus, device, 0);
            if let Some(dev) = PciDevice::read(addr) {
                callback(dev);

                // Check if multi-function device
                let header_type = read_config_u8(addr, reg::HEADER_TYPE);
                if (header_type & 0x80) != 0 {
                    // Multi-function: check functions 1-7
                    for function in 1..8u8 {
                        let addr = PciAddress::new(bus, device, function);
                        if let Some(dev) = PciDevice::read(addr) {
                            callback(dev);
                        }
                    }
                }
            }
        }
    }
}

/// Find the first e1000 network controller.
pub fn find_e1000() -> Option<PciDevice> {
    let mut result = None;
    scan(|dev| {
        if result.is_none() && dev.is_e1000() {
            result = Some(dev);
        }
    });
    result
}
