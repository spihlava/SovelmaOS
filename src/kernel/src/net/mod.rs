//! Network subsystem for SovelmaOS.
//!
//! Provides TCP/IP networking via smoltcp, including DHCP and DNS support.
//!
//! # Architecture
//!
//! - `device`: Virtual NIC driver for QEMU e1000
//! - `stack`: smoltcp Interface wrapper
//! - `socket`: Socket abstraction layer
//! - `dhcp`: DHCP client for automatic IP configuration
//! - `dns`: DNS resolver for hostname lookup

pub mod device;
pub mod dhcp;
pub mod dns;
pub mod socket;
pub mod stack;

pub use device::QemuE1000;
pub use dhcp::{DhcpClient, DhcpConfig, DhcpEvent};
pub use dns::{DnsResolver, DnsResult};
pub use socket::{TcpSocket, UdpSocket};
pub use stack::{NetConfig, NetworkStack};

pub use sovelma_common::net::NetError;

/// Initialize the network subsystem.
///
/// This should be called after memory initialization.
pub fn init() {
    // Network initialization will be done when creating NetworkStack
    // Device detection and setup happens there
}
