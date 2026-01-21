//! DHCP client for automatic IP configuration.
//!
//! Uses smoltcp's DHCP socket to acquire network configuration.

use super::stack::NetworkStack;
use alloc::vec::Vec;
use smoltcp::iface::SocketHandle;
use smoltcp::socket::dhcpv4::{self, Event as DhcpSocketEvent};
use smoltcp::time::{Duration, Instant};
use smoltcp::wire::{IpCidr, Ipv4Address, Ipv4Cidr};

/// DHCP client state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DhcpState {
    /// Not started yet.
    Idle,
    /// Discovering DHCP servers.
    Discovering,
    /// Requesting IP address.
    Requesting,
    /// IP address acquired.
    Configured,
    /// Using link-local address (DHCP failed).
    LinkLocal,
}

/// DHCP configuration acquired from server.
#[derive(Debug, Clone)]
pub struct DhcpConfig {
    /// Assigned IP address.
    pub ip: Ipv4Address,
    /// Subnet prefix length.
    pub prefix_len: u8,
    /// Default gateway.
    pub gateway: Option<Ipv4Address>,
    /// DNS server addresses.
    pub dns_servers: Vec<Ipv4Address>,
    /// Lease duration.
    pub lease_duration: Option<Duration>,
}

impl DhcpConfig {
    /// Get the IP address as a CIDR.
    pub fn cidr(&self) -> IpCidr {
        IpCidr::Ipv4(Ipv4Cidr::new(self.ip, self.prefix_len))
    }
}

/// Events emitted by the DHCP client.
#[derive(Debug, Clone)]
pub enum DhcpEvent {
    /// IP address configured successfully.
    Configured(DhcpConfig),
    /// DHCP lease lost or expired.
    Deconfigured,
    /// DHCP failed, using link-local address.
    LinkLocalFallback(Ipv4Address),
}

/// DHCP client for automatic network configuration.
pub struct DhcpClient {
    socket: Option<SocketHandle>,
    state: DhcpState,
    config: Option<DhcpConfig>,
    start_time: Option<Instant>,
    link_local_timeout: Duration,
}

impl DhcpClient {
    /// Create a new DHCP client.
    pub fn new() -> Self {
        Self {
            socket: None,
            state: DhcpState::Idle,
            config: None,
            start_time: None,
            // Fall back to link-local after 10 seconds
            link_local_timeout: Duration::from_secs(10),
        }
    }

    /// Start the DHCP discovery process.
    pub fn start(&mut self, stack: &mut NetworkStack, timestamp: Instant) {
        // Create DHCP socket
        let socket = dhcpv4::Socket::new();
        let handle = stack.sockets().add(socket);
        self.socket = Some(handle);
        self.state = DhcpState::Discovering;
        self.start_time = Some(timestamp);
    }

    /// Get the current state.
    pub fn state(&self) -> DhcpState {
        self.state
    }

    /// Get the current configuration, if any.
    pub fn config(&self) -> Option<&DhcpConfig> {
        self.config.as_ref()
    }

    /// Poll the DHCP client for events.
    ///
    /// Returns an event if the configuration changed.
    pub fn poll(&mut self, stack: &mut NetworkStack, timestamp: Instant) -> Option<DhcpEvent> {
        let handle = self.socket?;

        // Check for link-local fallback timeout
        if self.state == DhcpState::Discovering || self.state == DhcpState::Requesting {
            if let Some(start) = self.start_time {
                if timestamp - start > self.link_local_timeout {
                    return Some(self.fallback_to_link_local(stack));
                }
            }
        }

        let socket = stack.sockets().get_mut::<dhcpv4::Socket>(handle);

        match socket.poll() {
            None => None,
            Some(DhcpSocketEvent::Configured(config)) => {
                self.state = DhcpState::Configured;

                // Extract DNS servers (filter out None values if present)
                let dns_servers: Vec<Ipv4Address> = config.dns_servers.iter().copied().collect();

                let dhcp_config = DhcpConfig {
                    ip: config.address.address(),
                    prefix_len: config.address.prefix_len(),
                    gateway: config.router,
                    dns_servers: dns_servers.clone(),
                    lease_duration: None, // smoltcp handles renewal internally
                };

                // Apply configuration to network stack
                stack.set_ip_config(dhcp_config.cidr(), dhcp_config.gateway);
                stack.set_dns_servers(dns_servers);

                self.config = Some(dhcp_config.clone());
                Some(DhcpEvent::Configured(dhcp_config))
            }
            Some(DhcpSocketEvent::Deconfigured) => {
                self.state = DhcpState::Discovering;
                self.config = None;
                self.start_time = Some(timestamp);
                Some(DhcpEvent::Deconfigured)
            }
        }
    }

    /// Fall back to a link-local address when DHCP fails.
    fn fallback_to_link_local(&mut self, stack: &mut NetworkStack) -> DhcpEvent {
        self.state = DhcpState::LinkLocal;

        // Generate link-local address (169.254.x.x)
        // Use MAC address bytes for uniqueness
        let mac = stack.device().mac_address();
        let ip = Ipv4Address::new(169, 254, mac[4], mac[5]);

        let cidr = IpCidr::Ipv4(Ipv4Cidr::new(ip, 16));
        stack.set_ip_config(cidr, None);

        DhcpEvent::LinkLocalFallback(ip)
    }

    /// Request a renewal of the current lease.
    pub fn renew(&mut self, stack: &mut NetworkStack) {
        if let Some(handle) = self.socket {
            let socket = stack.sockets().get_mut::<dhcpv4::Socket>(handle);
            socket.reset();
            self.state = DhcpState::Discovering;
        }
    }
}

impl Default for DhcpClient {
    fn default() -> Self {
        Self::new()
    }
}
