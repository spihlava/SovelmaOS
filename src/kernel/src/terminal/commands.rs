//! Built-in shell commands.
//!
//! Provides commands for network operations, system info, and more.

use crate::arch::x86_64::vga::{self, Color};
use crate::net::dns::parse_ipv4;
use crate::net::{DhcpClient, DnsResolver, NetworkStack};
use crate::{print, println};
use alloc::string::{String, ToString};
use smoltcp::time::Instant;
use smoltcp::wire::IpAddress;

/// Shell command types.
#[derive(Debug, Clone)]
pub enum Command {
    /// Display help information.
    Help,
    /// Clear the screen.
    Clear,
    /// Show network configuration.
    Ifconfig,
    /// DHCP operations.
    Dhcp(DhcpAction),
    /// DNS lookup.
    Dns {
        /// The hostname to resolve.
        hostname: String,
    },
    /// Establish TCP connection.
    Connect {
        /// The hostname or IP address to connect to.
        host: String,
        /// The port number to connect to.
        port: u16,
    },
    /// Echo text.
    Echo {
        /// The text to echo.
        text: String,
    },
    /// Show system info.
    Sysinfo,
    /// Run a test WASM module.
    WasmTest {
        /// The file to run.
        file: String,
    },
    /// Unknown command.
    Unknown(String),
}

/// DHCP sub-commands.
#[derive(Debug, Clone)]
pub enum DhcpAction {
    /// Show DHCP status.
    Status,
    /// Request new lease.
    Renew,
    /// Release current lease.
    Release,
}

impl Command {
    /// Parse a command from input.
    pub fn parse(cmd: &str, args: &[&str]) -> Option<Command> {
        match cmd.to_lowercase().as_str() {
            "help" | "?" => Some(Command::Help),
            "clear" | "cls" => Some(Command::Clear),
            "ifconfig" | "ip" => Some(Command::Ifconfig),
            "dhcp" => {
                let action = args.first().map(|s| s.to_lowercase());
                let action = match action.as_deref() {
                    Some("renew") => DhcpAction::Renew,
                    Some("release") => DhcpAction::Release,
                    _ => DhcpAction::Status,
                };
                Some(Command::Dhcp(action))
            }
            "dns" | "nslookup" | "resolve" => {
                if let Some(hostname) = args.first() {
                    Some(Command::Dns {
                        hostname: hostname.to_string(),
                    })
                } else {
                    println!("Usage: dns <hostname>");
                    None
                }
            }
            "connect" | "nc" => {
                if args.len() >= 2 {
                    if let Ok(port) = args[1].parse::<u16>() {
                        Some(Command::Connect {
                            host: args[0].to_string(),
                            port,
                        })
                    } else {
                        println!("Invalid port number");
                        None
                    }
                } else {
                    println!("Usage: connect <host> <port>");
                    None
                }
            }
            "echo" => {
                let text = args.join(" ");
                Some(Command::Echo { text })
            }
            "sysinfo" | "info" => Some(Command::Sysinfo),
            "wasm-test" | "wasm" => {
                let file = args.first().unwrap_or(&"hello.wasm").to_string();
                Some(Command::WasmTest { file })
            }
            "" => None,
            _ => Some(Command::Unknown(cmd.to_string())),
        }
    }

    /// Execute a command.
    pub fn execute(
        self,
        stack: &mut NetworkStack,
        dhcp: &mut DhcpClient,
        dns: &mut DnsResolver,
        terminal: &super::Terminal,
        timestamp: Instant,
    ) {
        match self {
            Command::Help => cmd_help(),
            Command::Clear => terminal.clear(),
            Command::Ifconfig => cmd_ifconfig(stack, dhcp),
            Command::Dhcp(action) => cmd_dhcp(action, stack, dhcp, timestamp),
            Command::Dns { hostname } => cmd_dns(&hostname, stack, dns),
            Command::Connect { host, port } => cmd_connect(&host, port, stack, dns),
            Command::Echo { text } => println!("{}", text),
            Command::Sysinfo => cmd_sysinfo(),
            Command::WasmTest { file } => cmd_wasm_test(&file),
            Command::Unknown(cmd) => {
                vga::set_color(Color::LightRed, Color::Black);
                println!("Unknown command: {}", cmd);
                vga::set_color(Color::White, Color::Black);
                println!("Type 'help' for available commands.");
            }
        }
    }
}

/// Display help information.
fn cmd_help() {
    println!();
    vga::set_color(Color::Cyan, Color::Black);
    println!("SovelmaOS Shell Commands");
    println!("========================");
    vga::set_color(Color::White, Color::Black);
    println!();
    println!("  help          Show this help message");
    println!("  clear         Clear the screen");
    println!("  ifconfig      Show network configuration");
    println!("  dhcp [renew]  Show DHCP status or request new lease");
    println!("  dns <host>    Resolve hostname to IP address");
    println!("  connect <host> <port>  Open TCP connection");
    println!("  echo <text>   Echo text to console");
    println!("  sysinfo       Show system information");
    println!("  wasm-test     Run a simple WASM module test");
    println!();
}

/// Show network configuration.
fn cmd_ifconfig(stack: &NetworkStack, dhcp: &DhcpClient) {
    println!();
    vga::set_color(Color::Cyan, Color::Black);
    println!("Network Configuration");
    println!("---------------------");
    vga::set_color(Color::White, Color::Black);

    // MAC address
    let mac = stack.device().mac_address();
    print!("  MAC:     ");
    vga::set_color(Color::Yellow, Color::Black);
    println!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    );
    vga::set_color(Color::White, Color::Black);

    // IP address
    print!("  IP:      ");
    if let Some(ip) = stack.ip_address() {
        vga::set_color(Color::LightGreen, Color::Black);
        println!("{}", ip);
    } else {
        vga::set_color(Color::LightRed, Color::Black);
        println!("Not configured");
    }
    vga::set_color(Color::White, Color::Black);

    // Gateway
    print!("  Gateway: ");
    if let Some(config) = dhcp.config() {
        if let Some(gw) = config.gateway {
            vga::set_color(Color::Yellow, Color::Black);
            println!("{}", gw);
        } else {
            println!("None");
        }
    } else {
        println!("None");
    }
    vga::set_color(Color::White, Color::Black);

    // DNS servers
    print!("  DNS:     ");
    if !stack.dns_servers.is_empty() {
        vga::set_color(Color::Yellow, Color::Black);
        for (i, server) in stack.dns_servers.iter().enumerate() {
            if i > 0 {
                print!(", ");
            }
            print!("{}", server);
        }
        println!();
    } else {
        println!("None");
    }
    vga::set_color(Color::White, Color::Black);

    // DHCP state
    print!("  DHCP:    ");
    vga::set_color(Color::Yellow, Color::Black);
    println!("{:?}", dhcp.state());
    vga::set_color(Color::White, Color::Black);
    println!();
}

/// Handle DHCP commands.
fn cmd_dhcp(
    action: DhcpAction,
    stack: &mut NetworkStack,
    dhcp: &mut DhcpClient,
    _timestamp: Instant,
) {
    match action {
        DhcpAction::Status => {
            println!("DHCP State: {:?}", dhcp.state());
            if let Some(config) = dhcp.config() {
                println!("  IP: {}/{}", config.ip, config.prefix_len);
                if let Some(gw) = config.gateway {
                    println!("  Gateway: {}", gw);
                }
                if !config.dns_servers.is_empty() {
                    print!("  DNS: ");
                    for (i, dns) in config.dns_servers.iter().enumerate() {
                        if i > 0 {
                            print!(", ");
                        }
                        print!("{}", dns);
                    }
                    println!();
                }
            }
        }
        DhcpAction::Renew => {
            println!("Requesting DHCP renewal...");
            dhcp.renew(stack);
        }
        DhcpAction::Release => {
            println!("DHCP release not yet implemented");
        }
    }
}

/// Handle DNS lookup.
fn cmd_dns(hostname: &str, stack: &mut NetworkStack, dns: &mut DnsResolver) {
    // Check if it's already an IP address
    if let Some(ip) = parse_ipv4(hostname) {
        println!("{} -> {}", hostname, ip);
        return;
    }

    // Initialize DNS resolver if needed
    if !dns.is_ready() {
        dns.init(stack);
    }

    if !dns.is_ready() {
        vga::set_color(Color::LightRed, Color::Black);
        println!("DNS resolver not ready (no DNS servers configured)");
        vga::set_color(Color::White, Color::Black);
        return;
    }

    print!("Resolving {}... ", hostname);

    match dns.resolve(stack, hostname) {
        Ok(_handle) => {
            // In a real implementation, we'd poll for the result
            // For now, just indicate the query was started
            println!("(query started)");
            println!("Use the main loop to poll for DNS results.");
        }
        Err(e) => {
            vga::set_color(Color::LightRed, Color::Black);
            println!("Failed: {}", e);
            vga::set_color(Color::White, Color::Black);
        }
    }
}

/// Handle TCP connect.
fn cmd_connect(host: &str, port: u16, stack: &mut NetworkStack, _dns: &mut DnsResolver) {
    // Parse or resolve the host
    let ip = if let Some(ip) = parse_ipv4(host) {
        ip
    } else {
        // Would need async DNS resolution here
        vga::set_color(Color::LightRed, Color::Black);
        println!("DNS resolution for connect not yet implemented.");
        println!("Please use an IP address directly.");
        vga::set_color(Color::White, Color::Black);
        return;
    };

    println!("Connecting to {}:{}...", ip, port);

    let handle = stack.tcp_socket();
    let remote = smoltcp::wire::IpEndpoint::new(IpAddress::Ipv4(ip), port);
    let local_port = 49152 + (ip.0[3] as u16 % 1000); // Simple ephemeral port

    match stack.tcp_connect(handle, remote, local_port) {
        Ok(()) => {
            vga::set_color(Color::LightGreen, Color::Black);
            println!("Connection initiated to {}:{}", ip, port);
            vga::set_color(Color::White, Color::Black);
            println!("Use the main loop to check connection state.");
        }
        Err(e) => {
            vga::set_color(Color::LightRed, Color::Black);
            println!("Connection failed: {}", e);
            vga::set_color(Color::White, Color::Black);
        }
    }
}

/// Show system information.
fn cmd_sysinfo() {
    println!();
    vga::set_color(Color::Cyan, Color::Black);
    println!("SovelmaOS System Information");
    println!("============================");
    vga::set_color(Color::White, Color::Black);
    println!("  Version:    0.1.0");
    println!("  Arch:       x86_64");
    println!("  Platform:   QEMU");

    // Could add more system info here:
    // - Memory usage
    // - Uptime
    // - CPU info
    // - Interrupt counts
    println!();
}
/// Run a simple WASM module test.
fn cmd_wasm_test(filename: &str) {
    use crate::fs::{FileSystem, ROOT_FS};
    use crate::wasm::WasmEngine;
    use alloc::vec;

    println!();
    vga::set_color(Color::Cyan, Color::Black);
    println!("WASM Runtime Test executing '{}'", filename);
    println!("-----------------");
    vga::set_color(Color::White, Color::Black);

    // Open file
    let handle = match ROOT_FS.open(filename) {
        Ok(h) => h,
        Err(e) => {
            vga::set_color(Color::LightRed, Color::Black);
            println!("Failed to open file: {:?}", e);
            vga::set_color(Color::White, Color::Black);
            return;
        }
    };

    // Read file
    let size = ROOT_FS.size(handle).unwrap_or(0);
    let mut buffer = vec![0u8; size];
    if let Err(e) = ROOT_FS.read(handle, &mut buffer, 0) {
        vga::set_color(Color::LightRed, Color::Black);
        println!("Failed to read file: {:?}", e);
        vga::set_color(Color::White, Color::Black);
        return;
    }

    ROOT_FS.close(handle);

    let engine = WasmEngine::new();

    // Use spawn_process_with_caps to be safe/compliant, even if caps are empty for now.
    // In a real test, we might want to grant some caps.
    match engine.spawn_process_with_caps(&buffer, vec![]) {
        Ok(mut process) => {
            vga::set_color(Color::LightGreen, Color::Black);
            println!("WASM process spawned successfully!");
            vga::set_color(Color::White, Color::Black);

            println!("Executing _start...");
            match process.call("_start", &[]) {
                Ok(_) => {
                    vga::set_color(Color::LightGreen, Color::Black);
                    println!("_start completed successfully!");
                }
                Err(e) => {
                    vga::set_color(Color::LightRed, Color::Black);
                    println!("Execution failed: {:?}", e);
                }
            }
        }
        Err(e) => {
            vga::set_color(Color::LightRed, Color::Black);
            println!("WASM test failed: {:?}", e);
        }
    }
    vga::set_color(Color::White, Color::Black);
    println!();
}
