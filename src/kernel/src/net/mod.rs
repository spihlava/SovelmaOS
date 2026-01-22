//! Network subsystem for SovelmaOS.
//!
//! Provides TCP/IP networking via smoltcp, including DHCP and DNS support.
//!
//! # Architecture
//!
//! - `e1000`: Real Intel e1000 NIC driver (PCI/MMIO)
//! - `device`: Loopback/fallback device for testing
//! - `stack`: smoltcp Interface wrapper
//! - `socket`: Socket abstraction layer
//! - `dhcp`: DHCP client for automatic IP configuration
//! - `dns`: DNS resolver for hostname lookup

pub mod device;
pub mod dhcp;
pub mod dns;
pub mod e1000;
pub mod socket;
pub mod stack;

pub use device::QemuE1000;
pub use dhcp::{DhcpClient, DhcpConfig, DhcpEvent};
pub use dns::{DnsResolver, DnsResult};
pub use e1000::E1000;
pub use socket::{TcpSocket, UdpSocket};
pub use stack::{NetConfig, NetworkStack};

pub use sovelma_common::net::NetError;

use smoltcp::phy::{Device, DeviceCapabilities, RxToken, TxToken};
use smoltcp::time::Instant;

/// Unified network device enum supporting multiple backends.
///
/// This allows the network stack to work with either a real e1000 driver
/// or the loopback device for testing.
pub enum NetworkDevice {
    /// Real Intel e1000 NIC driver.
    E1000(E1000),
    /// Loopback device for testing.
    Loopback(QemuE1000),
}

impl NetworkDevice {
    /// Probe for a real e1000 device, falling back to loopback.
    pub fn probe() -> Self {
        if let Some(e1000) = E1000::probe() {
            NetworkDevice::E1000(e1000)
        } else {
            NetworkDevice::Loopback(QemuE1000::new())
        }
    }

    /// Get the MAC address of the device.
    pub fn mac_address(&self) -> [u8; 6] {
        match self {
            NetworkDevice::E1000(dev) => dev.mac_address(),
            NetworkDevice::Loopback(dev) => dev.mac_address(),
        }
    }

    /// Check if this is a real hardware device.
    pub fn is_real(&self) -> bool {
        matches!(self, NetworkDevice::E1000(_))
    }
}

/// Receive token wrapper for NetworkDevice.
pub enum NetworkRxToken {
    /// E1000 receive token.
    E1000(e1000::E1000RxToken),
    /// Loopback receive token.
    Loopback(device::E1000RxToken),
}

/// Transmit token wrapper for NetworkDevice.
pub enum NetworkTxToken<'a> {
    /// E1000 transmit token.
    E1000(e1000::E1000TxToken<'a>),
    /// Loopback transmit token.
    Loopback(device::E1000TxToken<'a>),
}

impl RxToken for NetworkRxToken {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        match self {
            NetworkRxToken::E1000(token) => token.consume(f),
            NetworkRxToken::Loopback(token) => token.consume(f),
        }
    }
}

impl<'a> TxToken for NetworkTxToken<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        match self {
            NetworkTxToken::E1000(token) => token.consume(len, f),
            NetworkTxToken::Loopback(token) => token.consume(len, f),
        }
    }
}

impl Device for NetworkDevice {
    type RxToken<'a> = NetworkRxToken where Self: 'a;
    type TxToken<'a> = NetworkTxToken<'a> where Self: 'a;

    fn receive(&mut self, timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        match self {
            NetworkDevice::E1000(dev) => dev
                .receive(timestamp)
                .map(|(rx, tx)| (NetworkRxToken::E1000(rx), NetworkTxToken::E1000(tx))),
            NetworkDevice::Loopback(dev) => dev
                .receive(timestamp)
                .map(|(rx, tx)| (NetworkRxToken::Loopback(rx), NetworkTxToken::Loopback(tx))),
        }
    }

    fn transmit(&mut self, timestamp: Instant) -> Option<Self::TxToken<'_>> {
        match self {
            NetworkDevice::E1000(dev) => dev.transmit(timestamp).map(NetworkTxToken::E1000),
            NetworkDevice::Loopback(dev) => dev.transmit(timestamp).map(NetworkTxToken::Loopback),
        }
    }

    fn capabilities(&self) -> DeviceCapabilities {
        match self {
            NetworkDevice::E1000(dev) => dev.capabilities(),
            NetworkDevice::Loopback(dev) => dev.capabilities(),
        }
    }
}

/// Initialize the network subsystem.
///
/// This should be called after memory initialization.
pub fn init() {
    // Network initialization will be done when creating NetworkStack
    // Device detection and setup happens there
}
