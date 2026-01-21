//! System-wide error types for SovelmaOS.

use core::fmt;

/// Network subsystem error types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NetError {
    /// Device not initialized
    DeviceNotReady,
    /// No IP address configured
    NoAddress,
    /// Connection refused
    ConnectionRefused,
    /// Connection timed out
    Timeout,
    /// Socket buffer full
    BufferFull,
    /// Invalid address format
    InvalidAddress,
    /// DNS resolution failed
    DnsError,
    /// DHCP failed to acquire lease
    DhcpFailed,
    /// Generic I/O error
    IoError,
}

impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetError::DeviceNotReady => write!(f, "network device not ready"),
            NetError::NoAddress => write!(f, "no IP address configured"),
            NetError::ConnectionRefused => write!(f, "connection refused"),
            NetError::Timeout => write!(f, "connection timed out"),
            NetError::BufferFull => write!(f, "socket buffer full"),
            NetError::InvalidAddress => write!(f, "invalid address format"),
            NetError::DnsError => write!(f, "DNS error"),
            NetError::DhcpFailed => write!(f, "DHCP failed to acquire lease"),
            NetError::IoError => write!(f, "I/O error"),
        }
    }
}
