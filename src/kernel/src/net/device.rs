//! Virtual NIC device driver for QEMU e1000.
//!
//! Provides a smoltcp-compatible Device implementation for network I/O.
//! Currently implements a loopback device; real e1000 driver requires PCI enumeration.

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::time::Instant;
use spin::Mutex;

/// Maximum transmission unit (standard Ethernet).
const MTU: usize = 1500;

/// Maximum packet buffer size (MTU + Ethernet header).

/// Packet queue capacity.
const QUEUE_CAPACITY: usize = 16;

/// QEMU e1000 virtual network device.
///
/// Currently implements a loopback device for testing.
/// TODO: Implement actual e1000 MMIO driver with PCI enumeration.
pub struct QemuE1000 {
    rx_queue: Mutex<VecDeque<Vec<u8>>>,
    tx_queue: Mutex<VecDeque<Vec<u8>>>,
    mac_address: [u8; 6],
}

impl QemuE1000 {
    /// Create a new virtual network device.
    ///
    /// Uses a locally-administered MAC address for QEMU user networking.
    pub fn new() -> Self {
        Self {
            rx_queue: Mutex::new(VecDeque::with_capacity(QUEUE_CAPACITY)),
            tx_queue: Mutex::new(VecDeque::with_capacity(QUEUE_CAPACITY)),
            // Locally-administered MAC address (bit 1 of first byte set)
            mac_address: [0x52, 0x54, 0x00, 0x12, 0x34, 0x56],
        }
    }

    /// Get the MAC address of this device.
    pub fn mac_address(&self) -> [u8; 6] {
        self.mac_address
    }

    /// Inject a packet into the receive queue (for testing/loopback).
    pub fn inject_rx(&self, data: &[u8]) {
        let mut queue = self.rx_queue.lock();
        if queue.len() < QUEUE_CAPACITY {
            queue.push_back(data.to_vec());
        }
    }

    /// Drain transmitted packets (for testing/inspection).
    pub fn drain_tx(&self) -> Vec<Vec<u8>> {
        let mut queue = self.tx_queue.lock();
        queue.drain(..).collect()
    }
}

impl Default for QemuE1000 {
    fn default() -> Self {
        Self::new()
    }
}

/// Receive token for QemuE1000.
pub struct E1000RxToken {
    buffer: Vec<u8>,
}

impl RxToken for E1000RxToken {
    fn consume<R, F>(mut self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        f(&mut self.buffer)
    }
}

/// Transmit token for QemuE1000.
pub struct E1000TxToken<'a> {
    device: &'a QemuE1000,
}

impl<'a> TxToken for E1000TxToken<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buffer = alloc::vec![0u8; len];
        let result = f(&mut buffer);

        // Queue packet for transmission
        let mut queue = self.device.tx_queue.lock();
        if queue.len() < QUEUE_CAPACITY {
            queue.push_back(buffer);
        }

        result
    }
}

impl Device for QemuE1000 {
    type RxToken<'a> = E1000RxToken where Self: 'a;
    type TxToken<'a> = E1000TxToken<'a> where Self: 'a;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let mut rx_queue = self.rx_queue.lock();
        if let Some(buffer) = rx_queue.pop_front() {
            drop(rx_queue); // Release lock before creating tokens
            Some((E1000RxToken { buffer }, E1000TxToken { device: self }))
        } else {
            None
        }
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        let tx_queue = self.tx_queue.lock();
        if tx_queue.len() < QUEUE_CAPACITY {
            drop(tx_queue);
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
