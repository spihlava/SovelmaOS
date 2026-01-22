//! Network stack wrapper around smoltcp.
//!
//! Provides a high-level interface for TCP/IP networking.

use super::{NetError, NetworkDevice};
use alloc::vec::Vec;
use smoltcp::iface::{Config, Interface, SocketHandle, SocketSet};
use smoltcp::socket::tcp;
use smoltcp::socket::udp;
use smoltcp::socket::icmp;
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr, IpEndpoint, Ipv4Address};

/// Maximum number of sockets in the socket set.
const MAX_SOCKETS: usize = 16;

/// TCP socket receive buffer size.
const TCP_RX_BUFFER_SIZE: usize = 4096;

/// TCP socket transmit buffer size.
const TCP_TX_BUFFER_SIZE: usize = 4096;

/// UDP socket receive buffer metadata slots.
const UDP_RX_META_SIZE: usize = 8;

/// UDP socket transmit metadata slots.
const UDP_TX_META_SIZE: usize = 8;

/// UDP socket buffer size.
const UDP_BUFFER_SIZE: usize = 2048;

/// Network configuration options.
#[derive(Clone)]
pub enum NetConfig {
    /// Use DHCP to acquire IP configuration.
    Dhcp,
    /// Use static IP configuration.
    Static {
        /// IP address with prefix length (e.g., 192.168.1.100/24).
        ip: IpCidr,
        /// Default gateway.
        gateway: Option<Ipv4Address>,
        /// DNS server addresses.
        dns_servers: Vec<Ipv4Address>,
    },
}

impl NetConfig {
    /// Create a DHCP configuration.
    pub fn dhcp() -> Self {
        NetConfig::Dhcp
    }

    /// Create a static IP configuration.
    pub fn static_ip(ip: IpCidr, gateway: Option<Ipv4Address>, dns: Vec<Ipv4Address>) -> Self {
        NetConfig::Static {
            ip,
            gateway,
            dns_servers: dns,
        }
    }
}

/// Network stack managing smoltcp interface and sockets.
pub struct NetworkStack {
    device: NetworkDevice,
    interface: Interface,
    sockets: SocketSet<'static>,
    config: NetConfig,
    /// DNS server addresses for resolver.
    pub dns_servers: Vec<Ipv4Address>,
}

impl NetworkStack {
    /// Create a new network stack with the given device and configuration.
    pub fn new(device: NetworkDevice, config: NetConfig) -> Self {
        let mac = device.mac_address();
        let hardware_addr = HardwareAddress::Ethernet(EthernetAddress(mac));

        let iface_config = Config::new(hardware_addr);

        // Create a dummy device for interface initialization
        // The real device will be used during poll()
        let mut dummy = super::QemuE1000::new();
        let interface = Interface::new(iface_config, &mut dummy, Instant::from_millis(0));

        // Pre-allocate socket storage
        let sockets = SocketSet::new(Vec::with_capacity(MAX_SOCKETS));

        let dns_servers = match &config {
            NetConfig::Dhcp => Vec::new(),
            NetConfig::Static { dns_servers, .. } => dns_servers.clone(),
        };

        let mut stack = Self {
            device,
            interface,
            sockets,
            config,
            dns_servers,
        };

        // Apply static configuration if provided
        if let NetConfig::Static { ip, gateway, .. } = &stack.config.clone() {
            stack.interface.update_ip_addrs(|addrs| {
                addrs.push(*ip).ok();
            });
            if let Some(gw) = gateway {
                stack
                    .interface
                    .routes_mut()
                    .add_default_ipv4_route(*gw)
                    .ok();
            }
        }

        stack
    }

    /// Poll the network stack, processing any pending I/O.
    ///
    /// This should be called regularly in the main loop.
    pub fn poll(&mut self, timestamp: Instant) {
        self.interface
            .poll(timestamp, &mut self.device, &mut self.sockets);
    }

    /// Get the current IP address, if configured.
    pub fn ip_address(&self) -> Option<IpAddress> {
        self.interface.ip_addrs().first().map(|cidr| cidr.address())
    }

    /// Get the current IPv4 address, if configured.
    pub fn ipv4_address(&self) -> Option<Ipv4Address> {
        match self.ip_address() {
            Some(IpAddress::Ipv4(v4)) => Some(v4),
            _ => None,
        }
    }

    /// Check if an IP address is configured.
    pub fn has_ip(&self) -> bool {
        !self.interface.ip_addrs().is_empty()
    }

    /// Set the IP configuration.
    pub fn set_ip_config(&mut self, ip: IpCidr, gateway: Option<Ipv4Address>) {
        self.interface.update_ip_addrs(|addrs| {
            addrs.clear();
            addrs.push(ip).ok();
        });
        if let Some(gw) = gateway {
            self.interface.routes_mut().add_default_ipv4_route(gw).ok();
        }
    }

    /// Set DNS servers.
    pub fn set_dns_servers(&mut self, servers: Vec<Ipv4Address>) {
        self.dns_servers = servers;
    }

    /// Create a new TCP socket and return its handle.
    pub fn tcp_socket(&mut self) -> SocketHandle {
        let rx_buffer = tcp::SocketBuffer::new(alloc::vec![0; TCP_RX_BUFFER_SIZE]);
        let tx_buffer = tcp::SocketBuffer::new(alloc::vec![0; TCP_TX_BUFFER_SIZE]);
        let socket = tcp::Socket::new(rx_buffer, tx_buffer);
        self.sockets.add(socket)
    }

    /// Create a new UDP socket and return its handle.
    pub fn udp_socket(&mut self) -> SocketHandle {
        let rx_buffer = udp::PacketBuffer::new(
            alloc::vec![udp::PacketMetadata::EMPTY; UDP_RX_META_SIZE],
            alloc::vec![0; UDP_BUFFER_SIZE],
        );
        let tx_buffer = udp::PacketBuffer::new(
            alloc::vec![udp::PacketMetadata::EMPTY; UDP_TX_META_SIZE],
            alloc::vec![0; UDP_BUFFER_SIZE],
        );
        let socket = udp::Socket::new(rx_buffer, tx_buffer);
        self.sockets.add(socket)
    }

    /// Create a new ICMP socket and return its handle.
    pub fn icmp_socket(&mut self) -> SocketHandle {
        let rx_buffer = icmp::PacketBuffer::new(
            alloc::vec![icmp::PacketMetadata::EMPTY; 4],
            alloc::vec![0; 1024],
        );
        let tx_buffer = icmp::PacketBuffer::new(
            alloc::vec![icmp::PacketMetadata::EMPTY; 4],
            alloc::vec![0; 1024],
        );
        let socket = icmp::Socket::new(rx_buffer, tx_buffer);
        self.sockets.add(socket)
    }

    /// Get a TCP socket by handle.
    pub fn get_tcp_socket(&mut self, handle: SocketHandle) -> &mut tcp::Socket<'static> {
        self.sockets.get_mut::<tcp::Socket>(handle)
    }

    /// Get a UDP socket by handle.
    pub fn get_udp_socket(&mut self, handle: SocketHandle) -> &mut udp::Socket<'static> {
        self.sockets.get_mut::<udp::Socket>(handle)
    }

    /// Connect a TCP socket to a remote endpoint.
    pub fn tcp_connect(
        &mut self,
        handle: SocketHandle,
        remote: IpEndpoint,
        local_port: u16,
    ) -> Result<(), NetError> {
        let socket = self.sockets.get_mut::<tcp::Socket>(handle);
        let cx = self.interface.context();
        socket
            .connect(cx, remote, local_port)
            .map_err(|_| NetError::ConnectionRefused)
    }

    /// Bind a TCP socket to listen on a local port.
    pub fn tcp_listen(&mut self, handle: SocketHandle, port: u16) -> Result<(), NetError> {
        let socket = self.sockets.get_mut::<tcp::Socket>(handle);
        socket.listen(port).map_err(|_| NetError::IoError)
    }

    /// Close a TCP socket.
    pub fn tcp_close(&mut self, handle: SocketHandle) {
        let socket = self.sockets.get_mut::<tcp::Socket>(handle);
        socket.close();
    }

    /// Bind a UDP socket to a local port.
    pub fn udp_bind(&mut self, handle: SocketHandle, port: u16) -> Result<(), NetError> {
        let socket = self.sockets.get_mut::<udp::Socket>(handle);
        socket.bind(port).map_err(|_| NetError::IoError)
    }

    /// Get access to the underlying device.
    pub fn device(&self) -> &NetworkDevice {
        &self.device
    }

    /// Get mutable access to the underlying device.
    pub fn device_mut(&mut self) -> &mut NetworkDevice {
        &mut self.device
    }

    /// Get access to the raw interface (for DHCP client).
    pub fn interface(&self) -> &Interface {
        &self.interface
    }

    /// Get mutable access to the raw interface (for DHCP client).
    pub fn interface_mut(&mut self) -> &mut Interface {
        &mut self.interface
    }

    /// Start a DNS query (avoids borrow checker issues).
    pub fn start_dns_query(
        &mut self,
        handle: SocketHandle,
        hostname: &str,
    ) -> Result<smoltcp::socket::dns::QueryHandle, smoltcp::socket::dns::StartQueryError> {
        let socket = self.sockets.get_mut::<smoltcp::socket::dns::Socket>(handle);
        socket.start_query(
            self.interface.context(),
            hostname,
            smoltcp::wire::DnsQueryType::A,
        )
    }

    /// Get access to the socket set.
    pub fn sockets(&mut self) -> &mut SocketSet<'static> {
        &mut self.sockets
    }

    /// Check for received ICMP packets and print replies.
    pub fn check_icmp(&mut self) {
        let mut buffer = [0u8; 1024];
        for (_handle, socket) in self.sockets.iter_mut() {
            if let smoltcp::socket::Socket::Icmp(socket) = socket {
                if socket.can_recv() {
                    match socket.recv_slice(&mut buffer) {
                        Ok((len, source)) => {
                            let icmp_packet = smoltcp::wire::Icmpv4Packet::new_unchecked(&buffer[..len]);
                            if let Ok(icmp_repr) = smoltcp::wire::Icmpv4Repr::parse(&icmp_packet, &Default::default()) {
                                if let smoltcp::wire::Icmpv4Repr::EchoReply { ident, seq_no, .. } = icmp_repr {
                                    crate::println!("\n[Network] Ping reply from {}: ident={}, seq={}", source, ident, seq_no);
                                }
                            }
                        }
                        Err(_) => {}
                    }
                }
            }
        }
    }
}
