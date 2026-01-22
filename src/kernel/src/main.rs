//! SovelmaOS Kernel Entry Point
//!
//! This is the main entry point for the SovelmaOS kernel.

#![no_std]
#![no_main]

extern crate alloc;

use ::x86_64::VirtAddr;
use alloc::{boxed::Box, sync::Arc, vec::Vec};
use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;
use smoltcp::time::Instant;
use sovelma_kernel::arch::x86_64::{self, vga::Color};
use sovelma_kernel::boot::{self, Status};
use sovelma_kernel::net::{
    DhcpClient, DhcpEvent, DnsResolver, NetConfig, NetworkDevice, NetworkStack,
};
use sovelma_kernel::terminal::{decode_scancode, Terminal};
use sovelma_kernel::{println, serial_println};

entry_point!(kernel_main);

/// Simple tick counter for timestamps.
static TICK_COUNT: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// Get current timestamp for smoltcp.
fn now() -> Instant {
    let ticks = TICK_COUNT.load(core::sync::atomic::Ordering::Relaxed);
    // Assume ~1ms per tick (rough approximation)
    Instant::from_millis(ticks as i64)
}

/// Increment the tick counter (called from main loop).
fn tick() {
    TICK_COUNT.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
}

/// Kernel entry point.
///
/// Called by the bootloader after setting up the initial environment.
fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // ========================================================================
    // Phase 1: Core Initialization (no logging yet)
    // ========================================================================
    sovelma_kernel::init();

    // Memory initialization
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { sovelma_kernel::memory::init_mapper(phys_mem_offset) };
    let mut frame_allocator =
        unsafe { sovelma_kernel::memory::BootInfoFrameAllocator::init(&boot_info.memory_map) };

    sovelma_kernel::allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    // Clear screen and show banner
    x86_64::vga::clear_screen();
    boot::banner::print_banner();

    // ========================================================================
    // Phase 2: Boot Logging - All messages use boot::log consistently
    // ========================================================================
    boot::log(Status::Ok, "Serial port initialized");
    boot::log(Status::Ok, "GDT loaded");
    boot::log(Status::Ok, "IDT configured");
    boot::log(Status::Ok, "Memory manager initialized");
    boot::log(Status::Ok, "Kernel heap ready");

    // Filesystem initialization
    boot::log_start("Initializing filesystem");
    const WASM_MAGIC: [u8; 8] = [0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
    sovelma_kernel::fs::ROOT_FS.add_file("hello.wasm", &WASM_MAGIC);
    boot::log_end(Status::Ok);
    boot::log_detail("RAM filesystem mounted at /");

    // Verify heap allocation
    let x = Box::new(42);
    let mut v = Vec::new();
    for i in 0..10 {
        v.push(i);
    }
    boot::log(
        Status::Ok,
        &alloc::format!("Heap verified (boxed={}, vec_len={})", *x, v.len()),
    );

    // Run kernel tests
    boot::log_start("Running kernel tests");
    sovelma_kernel::tests::run_all();
    boot::log_end(Status::Ok);

    // Test exception handling
    ::x86_64::instructions::interrupts::int3();
    boot::log(Status::Ok, "Exception handling verified");

    // ========================================================================
    // Phase 3: Network Initialization
    // ========================================================================
    boot::log_start("Probing network device");
    let device = NetworkDevice::probe(boot_info.physical_memory_offset);
    let is_real_nic = device.is_real();
    let mac = device.mac_address();
    boot::log_end(if is_real_nic {
        Status::Ok
    } else {
        Status::Warn
    });

    if is_real_nic {
        boot::log_detail("Intel e1000 detected via PCI");
    } else {
        boot::log_detail("No NIC found, using loopback");
    }
    boot::log_detail(&alloc::format!(
        "MAC: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0],
        mac[1],
        mac[2],
        mac[3],
        mac[4],
        mac[5]
    ));

    boot::log_start("Initializing network stack");
    let mut net_stack = NetworkStack::new(device, NetConfig::dhcp());
    boot::log_end(Status::Ok);

    // Initialize DHCP client
    let mut dhcp = DhcpClient::new();
    dhcp.start(&mut net_stack, now());
    boot::log(Status::Info, "DHCP discovery started");

    // Initialize DNS resolver (will be configured after DHCP completes)
    let dns = DnsResolver::new();

    // ========================================================================
    // Phase 4: Terminal Initialization
    // ========================================================================
    let terminal = Terminal::new();
    boot::log(Status::Ok, "Terminal initialized");

    // ========================================================================
    // Phase 5: WASM Engine
    // ========================================================================
    {
        use sovelma_kernel::wasm::WasmEngine;
        let _engine = WasmEngine::new();
        boot::log(Status::Ok, "WASM engine ready");
    }

    // ========================================================================
    // Boot Complete
    // ========================================================================
    println!();
    boot::log(Status::Ok, "Boot complete!");
    x86_64::vga::set_color(Color::Cyan, Color::Black);
    println!("\n Type 'help' for available commands.\n");
    x86_64::vga::set_color(Color::White, Color::Black);

    // ========================================================================
    // Phase 6: Executor and Async Tasks
    // ========================================================================
    let mut executor = sovelma_kernel::task::executor::Executor::new();

    // Wrap shared state
    let net_stack = Arc::new(spin::Mutex::new(net_stack));
    let dhcp = Arc::new(spin::Mutex::new(dhcp));
    let dns = Arc::new(spin::Mutex::new(dns));
    let terminal = Arc::new(spin::Mutex::new(terminal));

    // 1. Network Stack Poller Task
    {
        let net_stack = net_stack.clone();
        executor.spawn(sovelma_kernel::task::Task::new(async move {
            loop {
                tick();
                {
                    let mut stack = net_stack.lock();
                    stack.poll(now());
                }
                sovelma_kernel::task::yield_now().await;
            }
        }));
    }

    // 2. DHCP Task
    {
        let net_stack = net_stack.clone();
        let dhcp = dhcp.clone();
        let dns = dns.clone();
        executor.spawn(sovelma_kernel::task::Task::new(async move {
            loop {
                let event = {
                    let mut stack = net_stack.lock();
                    let mut d = dhcp.lock();
                    d.poll(&mut stack, now())
                };

                if let Some(e) = event {
                    let mut d_res = dns.lock();
                    let mut stack = net_stack.lock();
                    handle_dhcp_event(&e, &mut d_res, &mut stack);
                }
                sovelma_kernel::task::yield_now().await;
            }
        }));
    }

    // 3. Terminal/Keyboard Task
    {
        let terminal = terminal.clone();
        let net_stack = net_stack.clone();
        let dhcp = dhcp.clone();
        let dns = dns.clone();

        executor.spawn(sovelma_kernel::task::Task::new(async move {
            {
                let t = terminal.lock();
                t.prompt();
            }
            loop {
                if let Some(scancode) = get_scancode() {
                    if let Some(key) = decode_scancode(scancode) {
                        let mut t = terminal.lock();
                        if let Some(command) = t.handle_key(key) {
                            let mut stack = net_stack.lock();
                            let mut d = dhcp.lock();
                            let mut d_res = dns.lock();
                            command.execute(&mut stack, &mut d, &mut d_res, &t, now());
                            t.prompt();
                        }
                    }
                }
                sovelma_kernel::task::yield_now().await;
            }
        }));
    }

    // Run the executor
    executor.run();
}

/// Handle DHCP events with consistent logging.
fn handle_dhcp_event(event: &DhcpEvent, dns: &mut DnsResolver, stack: &mut NetworkStack) {
    match event {
        DhcpEvent::Configured(config) => {
            println!();
            boot::log(
                Status::Ok,
                &alloc::format!("DHCP: IP acquired {}/{}", config.ip, config.prefix_len),
            );
            if let Some(gw) = config.gateway {
                boot::log_detail(&alloc::format!("Gateway: {}", gw));
            }
            if !config.dns_servers.is_empty() {
                let dns_list: alloc::vec::Vec<_> = config
                    .dns_servers
                    .iter()
                    .map(|s| alloc::format!("{}", s))
                    .collect();
                boot::log_detail(&alloc::format!("DNS: {}", dns_list.join(", ")));
            }
            dns.init(stack);
            serial_println!("[DHCP] Configured: {}", config.ip);
        }
        DhcpEvent::Deconfigured => {
            println!();
            boot::log(Status::Warn, "DHCP: Lease expired, rediscovering...");
            serial_println!("[DHCP] Deconfigured");
        }
        DhcpEvent::LinkLocalFallback(ip) => {
            println!();
            boot::log(
                Status::Warn,
                &alloc::format!("DHCP: No server, using link-local {}", ip),
            );
            serial_println!("[DHCP] Link-local fallback: {}", ip);
        }
    }
}

/// Try to get a scancode from the keyboard queue.
fn get_scancode() -> Option<u8> {
    use sovelma_kernel::task::keyboard::SCANCODE_QUEUE;

    // Try to get from the async keyboard queue
    if let Some(queue) = SCANCODE_QUEUE.get() {
        queue.pop()
    } else {
        None
    }
}

/// Panic handler.
///
/// Called when the kernel encounters an unrecoverable error.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Use the already-initialized serial port
    serial_println!("KERNEL PANIC: {}", info);

    x86_64::vga::set_color(Color::LightRed, Color::Black);
    println!("\n\n!!! KERNEL PANIC !!!");
    x86_64::vga::set_color(Color::White, Color::Black);
    println!("{}", info);

    x86_64::halt_loop()
}
