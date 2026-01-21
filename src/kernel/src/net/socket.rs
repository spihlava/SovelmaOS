//! Socket abstraction layer.
//!
//! Provides high-level TCP and UDP socket types that wrap smoltcp sockets.

use super::stack::NetworkStack;
use super::NetError;
use smoltcp::iface::SocketHandle;
use smoltcp::wire::{IpAddress, IpEndpoint, Ipv4Address};

/// High-level TCP socket wrapper.
pub struct TcpSocket {
    handle: SocketHandle,
    local_port: u16,
}

impl TcpSocket {
    /// Create a new TCP socket.
    pub fn new(stack: &mut NetworkStack) -> Self {
        let handle = stack.tcp_socket();
        Self {
            handle,
            local_port: 0,
        }
    }

    /// Get the socket handle.
    pub fn handle(&self) -> SocketHandle {
        self.handle
    }

    /// Connect to a remote endpoint.
    pub fn connect(
        &mut self,
        stack: &mut NetworkStack,
        addr: Ipv4Address,
        port: u16,
    ) -> Result<(), NetError> {
        // Use ephemeral port for local binding
        self.local_port = ephemeral_port();
        let remote = IpEndpoint::new(IpAddress::Ipv4(addr), port);
        stack.tcp_connect(self.handle, remote, self.local_port)
    }

    /// Listen on a local port for incoming connections.
    pub fn listen(&mut self, stack: &mut NetworkStack, port: u16) -> Result<(), NetError> {
        self.local_port = port;
        stack.tcp_listen(self.handle, port)
    }

    /// Check if the socket is connected.
    pub fn is_connected(&self, stack: &mut NetworkStack) -> bool {
        let socket = stack.get_tcp_socket(self.handle);
        socket.is_active()
    }

    /// Check if socket can send data.
    pub fn can_send(&self, stack: &mut NetworkStack) -> bool {
        let socket = stack.get_tcp_socket(self.handle);
        socket.can_send()
    }

    /// Check if socket can receive data.
    pub fn can_recv(&self, stack: &mut NetworkStack) -> bool {
        let socket = stack.get_tcp_socket(self.handle);
        socket.can_recv()
    }

    /// Send data through the socket.
    pub fn send(&self, stack: &mut NetworkStack, data: &[u8]) -> Result<usize, NetError> {
        let socket = stack.get_tcp_socket(self.handle);
        socket.send_slice(data).map_err(|_| NetError::BufferFull)
    }

    /// Receive data from the socket.
    pub fn recv(&self, stack: &mut NetworkStack, buf: &mut [u8]) -> Result<usize, NetError> {
        let socket = stack.get_tcp_socket(self.handle);
        socket.recv_slice(buf).map_err(|_| NetError::IoError)
    }

    /// Close the socket.
    pub fn close(&self, stack: &mut NetworkStack) {
        stack.tcp_close(self.handle);
    }

    /// Get the local port.
    pub fn local_port(&self) -> u16 {
        self.local_port
    }
}

/// High-level UDP socket wrapper.
pub struct UdpSocket {
    handle: SocketHandle,
    local_port: u16,
}

impl UdpSocket {
    /// Create a new UDP socket.
    pub fn new(stack: &mut NetworkStack) -> Self {
        let handle = stack.udp_socket();
        Self {
            handle,
            local_port: 0,
        }
    }

    /// Get the socket handle.
    pub fn handle(&self) -> SocketHandle {
        self.handle
    }

    /// Bind the socket to a local port.
    pub fn bind(&mut self, stack: &mut NetworkStack, port: u16) -> Result<(), NetError> {
        self.local_port = port;
        stack.udp_bind(self.handle, port)
    }

    /// Send a datagram to a remote endpoint.
    pub fn send_to(
        &self,
        stack: &mut NetworkStack,
        data: &[u8],
        remote: IpEndpoint,
    ) -> Result<(), NetError> {
        let socket = stack.get_udp_socket(self.handle);
        socket
            .send_slice(data, remote)
            .map_err(|_| NetError::BufferFull)
    }

    /// Receive a datagram and get the sender's endpoint.
    pub fn recv_from(
        &self,
        stack: &mut NetworkStack,
        buf: &mut [u8],
    ) -> Result<(usize, IpEndpoint), NetError> {
        let socket = stack.get_udp_socket(self.handle);
        socket
            .recv_slice(buf)
            .map(|(len, meta)| (len, meta.endpoint))
            .map_err(|_| NetError::IoError)
    }

    /// Check if socket can send data.
    pub fn can_send(&self, stack: &mut NetworkStack) -> bool {
        let socket = stack.get_udp_socket(self.handle);
        socket.can_send()
    }

    /// Check if socket can receive data.
    pub fn can_recv(&self, stack: &mut NetworkStack) -> bool {
        let socket = stack.get_udp_socket(self.handle);
        socket.can_recv()
    }

    /// Get the local port.
    pub fn local_port(&self) -> u16 {
        self.local_port
    }
}

/// Counter for generating ephemeral ports.
static EPHEMERAL_PORT_COUNTER: spin::Mutex<u16> = spin::Mutex::new(49152);

/// Get the next ephemeral port number (49152-65535).
fn ephemeral_port() -> u16 {
    let mut counter = EPHEMERAL_PORT_COUNTER.lock();
    let port = *counter;
    *counter = if *counter == 65535 {
        49152
    } else {
        *counter + 1
    };
    port
}
