//! DNS resolver for hostname lookup.
//!
//! Provides asynchronous DNS resolution using smoltcp's DNS socket.

use super::stack::NetworkStack;
use super::NetError;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use smoltcp::iface::SocketHandle;
use smoltcp::socket::dns::{self, GetQueryResultError, StartQueryError};
use smoltcp::wire::{IpAddress, Ipv4Address};

/// Handle for tracking a pending DNS query.
#[derive(Clone, Copy)]
pub struct DnsQueryHandle {
    /// Internal socket handle for the query.
    pub handle: dns::QueryHandle,
    /// Query ID for tracking.
    pub id: u16,
}

impl core::fmt::Debug for DnsQueryHandle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DnsQueryHandle")
            .field("id", &self.id)
            .finish()
    }
}

/// Result of a DNS resolution.
#[derive(Debug, Clone)]
pub struct DnsResult {
    /// Original hostname queried.
    pub hostname: String,
    /// Resolved IP addresses.
    pub addresses: Vec<IpAddress>,
}

/// DNS resolver for hostname lookup.
pub struct DnsResolver {
    socket: Option<SocketHandle>,
    pending: Vec<(u16, dns::QueryHandle, String)>,
    next_id: u16,
}

impl DnsResolver {
    /// Create a new DNS resolver.
    pub fn new() -> Self {
        Self {
            socket: None,
            pending: Vec::new(),
            next_id: 1,
        }
    }

    /// Initialize the DNS resolver with the network stack.
    ///
    /// Must be called after DHCP completes or DNS servers are configured.
    pub fn init(&mut self, stack: &mut NetworkStack) {
        if self.socket.is_some() {
            return; // Already initialized
        }

        let servers = &stack.dns_servers;
        if servers.is_empty() {
            return; // No DNS servers configured
        }

        // Convert to smoltcp format
        let server_addrs: Vec<IpAddress> = servers.iter().map(|s| IpAddress::Ipv4(*s)).collect();

        let socket = dns::Socket::new(&server_addrs, Vec::new());
        let handle = stack.sockets().add(socket);
        self.socket = Some(handle);
    }

    /// Check if the resolver is initialized and ready.
    pub fn is_ready(&self) -> bool {
        self.socket.is_some()
    }

    /// Start a DNS query for a hostname.
    ///
    /// Returns a handle that can be used to check for results.
    pub fn resolve(
        &mut self,
        stack: &mut NetworkStack,
        hostname: &str,
    ) -> Result<DnsQueryHandle, NetError> {
        let socket_handle = self.socket.ok_or(NetError::DeviceNotReady)?;

        match stack.start_dns_query(socket_handle, hostname) {
            Ok(query_handle) => {
                let id = self.next_id;
                self.next_id = self.next_id.wrapping_add(1);
                self.pending.push((id, query_handle, hostname.to_string()));
                Ok(DnsQueryHandle {
                    handle: query_handle,
                    id,
                })
            }
            Err(StartQueryError::NoFreeSlot) => Err(NetError::BufferFull),
            Err(StartQueryError::InvalidName) => Err(NetError::DnsError),
            Err(StartQueryError::NameTooLong) => Err(NetError::DnsError),
        }
    }

    /// Poll for completed DNS queries.
    ///
    /// Returns results for any completed queries.
    pub fn poll(&mut self, stack: &mut NetworkStack) -> Vec<Result<DnsResult, NetError>> {
        let mut results = Vec::new();

        let socket_handle = match self.socket {
            Some(h) => h,
            None => return results,
        };

        let socket = stack.sockets().get_mut::<dns::Socket>(socket_handle);

        // Check each pending query
        let mut i = 0;
        while i < self.pending.len() {
            let (_, query_handle, _) = &self.pending[i];
            match socket.get_query_result(*query_handle) {
                Ok(addrs) => {
                    let (_, _, hostname) = self.pending.remove(i);
                    results.push(Ok(DnsResult {
                        hostname,
                        addresses: addrs.to_vec(),
                    }));
                    // Don't increment i since we removed an element
                }
                Err(GetQueryResultError::Pending) => {
                    i += 1; // Still waiting, check next
                }
                Err(GetQueryResultError::Failed) => {
                    let _ = self.pending.remove(i);
                    results.push(Err(NetError::DnsError));
                }
            }
        }

        results
    }

    /// Get result for a specific query (blocking check).
    pub fn get_result(
        &mut self,
        stack: &mut NetworkStack,
        query: DnsQueryHandle,
    ) -> Option<Result<DnsResult, NetError>> {
        let socket_handle = self.socket?;
        let socket = stack.sockets().get_mut::<dns::Socket>(socket_handle);

        match socket.get_query_result(query.handle) {
            Ok(addrs) => {
                // Find and remove from pending
                if let Some(pos) = self.pending.iter().position(|(id, _, _)| *id == query.id) {
                    let (_, _, hostname) = self.pending.remove(pos);
                    Some(Ok(DnsResult {
                        hostname,
                        addresses: addrs.to_vec(),
                    }))
                } else {
                    None
                }
            }
            Err(GetQueryResultError::Pending) => None,
            Err(GetQueryResultError::Failed) => {
                if let Some(pos) = self.pending.iter().position(|(id, _, _)| *id == query.id) {
                    let _ = self.pending.remove(pos);
                    Some(Err(NetError::DnsError))
                } else {
                    None
                }
            }
        }
    }

    /// Cancel a pending DNS query.
    pub fn cancel(&mut self, query: DnsQueryHandle) {
        if let Some(pos) = self.pending.iter().position(|(id, _, _)| *id == query.id) {
            self.pending.remove(pos);
        }
    }

    /// Get the number of pending queries.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

impl Default for DnsResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse an IPv4 address from a string.
///
/// Returns None if the string is not a valid IPv4 address.
pub fn parse_ipv4(s: &str) -> Option<Ipv4Address> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return None;
    }

    let octets: Vec<u8> = parts.iter().filter_map(|p| p.parse::<u8>().ok()).collect();

    if octets.len() != 4 {
        return None;
    }

    Some(Ipv4Address::new(octets[0], octets[1], octets[2], octets[3]))
}
