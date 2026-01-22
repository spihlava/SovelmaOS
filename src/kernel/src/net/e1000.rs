//! Intel e1000 (82540EM) network driver for QEMU.
//!
//! This module implements a real e1000 network driver using MMIO.
//! Designed for the SovelmaOS kernel.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ptr::{read_volatile, write_volatile};
use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::time::Instant;

use crate::arch::x86_64::pci::{self, PciDevice};

const MTU: usize = 1500;
const PACKET_BUFFER_SIZE: usize = 2048;
const TX_DESC_COUNT: usize = 64;
const RX_DESC_COUNT: usize = 64;

// Registers
const REG_CTRL: u32 = 0x0000;
const REG_STATUS: u32 = 0x0008;
const REG_ICR: u32 = 0x00C0;
const REG_IMC: u32 = 0x00D8;
const REG_RCTL: u32 = 0x0100;
const REG_RDBAL: u32 = 0x2800;
const REG_RDBAH: u32 = 0x2804;
const REG_RDLEN: u32 = 0x2808;
const REG_RDH: u32 = 0x2810;
const REG_RDT: u32 = 0x2818;
const REG_TCTL: u32 = 0x0400;
const REG_TIPG: u32 = 0x0410;
const REG_TDBAL: u32 = 0x3800;
const REG_TDBAH: u32 = 0x3804;
const REG_TDLEN: u32 = 0x3808;
const REG_TDH: u32 = 0x3810;
const REG_TDT: u32 = 0x3818;
const REG_RAL0: u32 = 0x5400;
const REG_RAH0: u32 = 0x5404;

// Bits
const CTRL_RST: u32 = 1 << 26;
const CTRL_SLU: u32 = 1 << 6;
const RCTL_EN: u32 = 1 << 1;
const RCTL_BAM: u32 = 1 << 15;
const TCTL_EN: u32 = 1 << 1;
const TCTL_PSP: u32 = 1 << 3;

// Descriptor Status Bits
const TX_DD: u8 = 1 << 0;
const RX_DD: u8 = 1 << 0;
const RX_EOP: u8 = 1 << 1;

#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
struct TxDesc {
    addr: u64,
    length: u16,
    cso: u8,
    cmd: u8,
    status: u8,
    css: u8,
    special: u16,
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
struct RxDesc {
    addr: u64,
    length: u16,
    checksum: u16,
    status: u8,
    errors: u8,
    special: u16,
}

pub struct E1000 {
    mmio_base: *mut u32,
    phys_mem_offset: u64,
    mac_address: [u8; 6],
    tx_descs: Box<[TxDesc; TX_DESC_COUNT]>,
    tx_buffers: Box<[[u8; PACKET_BUFFER_SIZE]; TX_DESC_COUNT]>,
    tx_cur: usize,
    rx_descs: Box<[RxDesc; RX_DESC_COUNT]>,
    rx_buffers: Box<[[u8; PACKET_BUFFER_SIZE]; RX_DESC_COUNT]>,
    rx_cur: usize,
}

unsafe impl Send for E1000 {}
unsafe impl Sync for E1000 {}

impl E1000 {
    pub fn new(pci_dev: PciDevice, phys_mem_offset: u64) -> Option<Self> {
        pci_dev.enable();
        let mmio_phys = pci_dev.mmio_base()?;
        let mmio_base = (mmio_phys + phys_mem_offset) as *mut u32;

        let tx_descs = Box::new([TxDesc::default(); TX_DESC_COUNT]);
        let tx_buffers = Box::new([[0u8; PACKET_BUFFER_SIZE]; TX_DESC_COUNT]);
        let rx_descs = Box::new([RxDesc::default(); RX_DESC_COUNT]);
        let rx_buffers = Box::new([[0u8; PACKET_BUFFER_SIZE]; RX_DESC_COUNT]);

        let mut dev = Self {
            mmio_base,
            phys_mem_offset,
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
        dev.write_mac_address();
        dev.init_tx();
        dev.init_rx();
        
        let status = dev.read_reg(REG_STATUS);
        crate::serial_println!("[e1000] Driver Initialized. MAC: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}, Status: {:#x}",
            dev.mac_address[0], dev.mac_address[1], dev.mac_address[2],
            dev.mac_address[3], dev.mac_address[4], dev.mac_address[5], status);

        Some(dev)
    }

    pub fn probe(phys_mem_offset: u64) -> Option<Self> {
        pci::find_e1000().and_then(|pci| Self::new(pci, phys_mem_offset))
    }

    fn read_reg(&self, offset: u32) -> u32 {
        unsafe { read_volatile(self.mmio_base.byte_add(offset as usize)) }
    }

    fn write_reg(&self, offset: u32, value: u32) {
        unsafe { write_volatile(self.mmio_base.byte_add(offset as usize), value) }
    }

    fn virt_to_phys(&self, virt_addr: u64) -> u64 {
        virt_addr - self.phys_mem_offset
    }

    fn reset(&mut self) {
        self.write_reg(REG_IMC, 0xFFFF_FFFF);
        self.write_reg(REG_CTRL, CTRL_RST);
        for _ in 0..20_000 { core::hint::spin_loop(); }
    }

    fn read_mac_address(&mut self) {
        let ral = self.read_reg(REG_RAL0);
        let rah = self.read_reg(REG_RAH0);
        if (rah & (1 << 31)) != 0 || ral != 0 {
            self.mac_address[0] = ral as u8;
            self.mac_address[1] = (ral >> 8) as u8;
            self.mac_address[2] = (ral >> 16) as u8;
            self.mac_address[3] = (ral >> 24) as u8;
            self.mac_address[4] = rah as u8;
            self.mac_address[5] = (rah >> 8) as u8;
        } else {
            // Standard Locally Administered MAC for virtualization
            self.mac_address = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
        }
    }

    fn write_mac_address(&self) {
        let ral = u32::from_le_bytes([self.mac_address[0], self.mac_address[1], self.mac_address[2], self.mac_address[3]]);
        let rah = u32::from_le_bytes([self.mac_address[4], self.mac_address[5], 0, 0]) | (1 << 31);
        self.write_reg(REG_RAL0, ral);
        self.write_reg(REG_RAH0, rah);
    }

    fn init_tx(&mut self) {
        for i in 0..TX_DESC_COUNT {
            let addr = self.tx_buffers[i].as_ptr() as u64;
            self.tx_descs[i].addr = self.virt_to_phys(addr);
            self.tx_descs[i].status = TX_DD;
        }
        let phys = self.virt_to_phys(self.tx_descs.as_ptr() as u64);
        self.write_reg(REG_TDBAL, phys as u32);
        self.write_reg(REG_TDBAH, (phys >> 32) as u32);
        self.write_reg(REG_TDLEN, (TX_DESC_COUNT * 16) as u32);
        self.write_reg(REG_TDH, 0);
        self.write_reg(REG_TDT, 0);
        self.write_reg(REG_TIPG, 0x0060_200A);
        self.write_reg(REG_TCTL, TCTL_EN | TCTL_PSP | (0x10 << 4) | (0x40 << 12));
    }

    fn init_rx(&mut self) {
        for i in 0..RX_DESC_COUNT {
            let addr = self.rx_buffers[i].as_ptr() as u64;
            self.rx_descs[i].addr = self.virt_to_phys(addr);
            self.rx_descs[i].status = 0;
        }
        let phys = self.virt_to_phys(self.rx_descs.as_ptr() as u64);
        self.write_reg(REG_RDBAL, phys as u32);
        self.write_reg(REG_RDBAH, (phys >> 32) as u32);
        self.write_reg(REG_RDLEN, (RX_DESC_COUNT * 16) as u32);
        self.write_reg(REG_RDH, 0);
        // RDT should be one less than RDH initially to show all descriptors are available
        self.write_reg(REG_RDT, (RX_DESC_COUNT - 1) as u32);
        
        // Enable with Broadcast Accept, Multicast Promisc, Unicast Promisc
        self.write_reg(REG_RCTL, RCTL_EN | RCTL_BAM | (1 << 3) | (1 << 4));
        
        // Force link status up
        let ctrl = self.read_reg(REG_CTRL);
        self.write_reg(REG_CTRL, ctrl | CTRL_SLU);
    }

    pub fn transmit_raw(&mut self, data: &[u8]) -> bool {
        if data.len() > PACKET_BUFFER_SIZE { return false; }
        let idx = self.tx_cur;
        let desc = &mut self.tx_descs[idx];
        
        if (desc.status & TX_DD) == 0 { return false; }
        
        self.tx_buffers[idx][..data.len()].copy_from_slice(data);
        desc.length = data.len() as u16;
        desc.cmd = (1 << 0) | (1 << 1) | (1 << 3); // EOP | IFCS | RS
        desc.status = 0;
        
        self.tx_cur = (self.tx_cur + 1) % TX_DESC_COUNT;
        self.write_reg(REG_TDT, self.tx_cur as u32);
        
        crate::serial_println!("[e1000] Transmitting {} bytes", data.len());
        true
    }

    pub fn receive_raw(&mut self) -> Option<Vec<u8>> {
        let idx = self.rx_cur;
        let desc = &mut self.rx_descs[idx];
        
        if (desc.status & RX_DD) == 0 {
             // Debug: check if head has moved
             let rdh = self.read_reg(REG_RDH);
             if rdh != (self.rx_cur as u32) {
                  // This is normal if we are lagging, but if it stays different and DD=0, something is wrong.
             }
             return None;
        }
        
        let len = desc.length as usize;
        let data = self.rx_buffers[idx][..len].to_vec();
        
        crate::serial_println!("[e1000] Received {} bytes (status={:#x})", len, desc.status);
        
        desc.status = 0;
        self.rx_cur = (self.rx_cur + 1) % RX_DESC_COUNT;
        // Hardware uses up to RDT. So we move RDT to the one we just processed.
        self.write_reg(REG_RDT, idx as u32);
        
        Some(data)
    }

    pub fn mac_address(&self) -> [u8; 6] { self.mac_address }
}

impl Device for E1000 {
    type RxToken<'a> = E1000RxToken where Self: 'a;
    type TxToken<'a> = E1000TxToken<'a> where Self: 'a;

    fn receive(&mut self, _: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        self.receive_raw().map(|b| (E1000RxToken { buffer: b }, E1000TxToken { dev: self }))
    }

    fn transmit(&mut self, _: Instant) -> Option<Self::TxToken<'_>> {
        if (self.tx_descs[self.tx_cur].status & TX_DD) != 0 {
            Some(E1000TxToken { dev: self })
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

pub struct E1000RxToken { buffer: Vec<u8> }
impl RxToken for E1000RxToken {
    fn consume<R, F>(mut self, f: F) -> R where F: FnOnce(&mut [u8]) -> R { f(&mut self.buffer) }
}

pub struct E1000TxToken<'a> { dev: &'a mut E1000 }
impl<'a> TxToken for E1000TxToken<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R where F: FnOnce(&mut [u8]) -> R {
        let mut b = alloc::vec![0u8; len];
        let r = f(&mut b);
        self.dev.transmit_raw(&b);
        r
    }
}
