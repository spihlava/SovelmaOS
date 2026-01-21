# SovelmaOS Operating System Specification
## Version 0.1.0 - Draft

---

## 1. Overview

### 1.1 Purpose
SovelmaOS is a microkernel operating system designed for extreme fault tolerance and modularity, targeting ESP32 (RISC-V) and x86_64 platforms. The system prioritizes OTA-updatable modules while maintaining a stable, rarely-modified kernel.

### 1.2 Design Goals
| Priority | Goal |
|----------|------|
| 1 | Fault isolation - module crashes don't bring down the system |
| 2 | OTA updates - modules updatable without kernel reflash |
| 3 | Tiny footprint - kernel < 80KB on ESP32 |
| 4 | Network-first - TCP/IP available at boot |
| 5 | Single-user - no multi-user complexity |
| 6 | Dual-platform - same modules run on ESP32 and x86 |

### 1.3 Target Hardware

#### Primary: ESP32-C6
- CPU: RISC-V single-core 160MHz
- RAM: 512KB SRAM
- Flash: 4-16MB
- Connectivity: WiFi 6, BLE 5, 802.15.4 (Zigbee/Thread)

#### Secondary: x86_64
- For development, testing, and desktop variant
- QEMU or real hardware via UEFI boot

---

## 2. Architecture

### 2.1 System Layers

```
┌─────────────────────────────────────────────────────────────┐
│                     WASM MODULES                            │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐          │
│  │ net.wasm│ │ app.wasm│ │ gui.wasm│ │ ota.wasm│          │
│  └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘          │
│       └───────────┴───────────┴───────────┘                │
│                       │                                     │
│            ══════════╪══════════════════                   │
│                      │  HOST API                           │
│            ══════════╪══════════════════                   │
│                      ▼                                      │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                 WASM RUNTIME                          │  │
│  │              (wasm3 interpreter)                      │  │
│  └──────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│                   SovelmaOS KERNEL                             │
│                                                             │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐      │
│  │Scheduler │ │Capability│ │  Memory  │ │Interrupt │      │
│  │          │ │  System  │ │  Manager │ │  Router  │      │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘      │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐  │
│  │              BOOT SERVICES                            │  │
│  │  • smoltcp TCP/IP  • DHCP  • DNS  • littlefs         │  │
│  └──────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│                 PLATFORM HAL                                │
│         ESP32-C6 (esp-hal)  |  x86_64 (x86_64 crate)       │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 Memory Map (ESP32-C6)

```
0x0000_0000 ┌─────────────────────────┐
            │      ROM (Bootloader)   │ 384KB
0x0006_0000 ├─────────────────────────┤
            │      Kernel Code        │ ~64KB
0x0007_0000 ├─────────────────────────┤
            │      Kernel Data/BSS    │ ~16KB
0x0007_4000 ├─────────────────────────┤
            │      Capability Table   │ 4KB
0x0007_5000 ├─────────────────────────┤
            │      Network Buffers    │ 32KB
0x0007_D000 ├─────────────────────────┤
            │      WASM Runtime       │ 64KB
0x0008_D000 ├─────────────────────────┤
            │      WASM Module Arena  │ ~300KB
            │      (linear memories)  │
0x0007_FFFF └─────────────────────────┘

Flash Layout:
0x0000_0000 ┌─────────────────────────┐
            │      Bootloader         │ 64KB
0x0001_0000 ├─────────────────────────┤
            │      Kernel Image       │ 256KB
0x0005_0000 ├─────────────────────────┤
            │      Module Storage     │ 1MB (littlefs)
0x0015_0000 ├─────────────────────────┤
            │      Config/Data        │ 512KB (littlefs)
0x001D_0000 ├─────────────────────────┤
            │      OTA Staging        │ 512KB
0x0025_0000 └─────────────────────────┘
```

---

## 3. Kernel Specification

### 3.1 Scheduler

#### 3.1.1 Design
- Preemptive, priority-based
- 4 priority levels: CRITICAL, HIGH, NORMAL, IDLE
- Cooperative within WASM (WASM is single-threaded per module)
- Time slice: 10ms default

#### 3.1.2 Data Structures

```rust
pub struct Scheduler {
    /// Ready queues per priority level
    ready: [VecDeque<TaskId>; 4],
    /// Currently running task
    current: Option<TaskId>,
    /// All tasks
    tasks: Slab<Task>,
    /// System tick counter
    ticks: u64,
}

pub struct Task {
    pub id: TaskId,
    pub priority: Priority,
    pub state: TaskState,
    pub wasm_module: Option<ModuleId>,
    pub stack: &'static mut [u8],
    pub context: CpuContext,
    pub capabilities: CapabilitySet,
}

#[derive(Clone, Copy)]
pub enum Priority {
    Critical = 0,  // Interrupts, network
    High = 1,      // User-interactive
    Normal = 2,    // Background
    Idle = 3,      // Only when nothing else
}

#[derive(Clone, Copy)]
pub enum TaskState {
    Ready,
    Running,
    Blocked(BlockReason),
    Terminated,
}
```

#### 3.1.3 API

```rust
impl Scheduler {
    /// Create a new task
    pub fn spawn(&mut self, entry: fn(), priority: Priority, caps: CapabilitySet) -> TaskId;
    
    /// Yield current task
    pub fn yield_now(&mut self);
    
    /// Block current task
    pub fn block(&mut self, reason: BlockReason);
    
    /// Unblock a task
    pub fn unblock(&mut self, task: TaskId);
    
    /// Called by timer interrupt
    pub fn tick(&mut self);
    
    /// Select next task to run
    fn schedule(&mut self) -> Option<TaskId>;
}
```

### 3.2 Capability System

#### 3.2.1 Design Philosophy
- All access controlled by unforgeable capability tokens
- Capabilities granted at module load time
- Capabilities can be attenuated (reduced) but not amplified
- Revocation via generation counters

#### 3.2.2 Data Structures

```rust
/// A capability handle held by user code
#[derive(Clone, Copy)]
pub struct Cap {
    /// Index into kernel's capability table
    index: u16,
    /// Generation for revocation check
    generation: u16,
}

/// Kernel-side capability entry
pub struct CapEntry {
    pub object: CapObject,
    pub rights: Rights,
    pub generation: u16,
    pub owner: TaskId,
}

/// What the capability refers to
pub enum CapObject {
    Null,
    Memory { base: usize, size: usize },
    IpcEndpoint { id: u16 },
    Interrupt { irq: u8 },
    GpioPin { pin: u8 },
    UartPort { port: u8 },
    NetworkSocket { id: u16 },
    FileDescriptor { fd: u16 },
    WasmModule { id: u16 },
}

bitflags! {
    pub struct Rights: u16 {
        const READ    = 0x0001;
        const WRITE   = 0x0002;
        const EXECUTE = 0x0004;
        const GRANT   = 0x0008;  // Can derive child caps
        const REVOKE  = 0x0010;  // Can revoke child caps
    }
}
```

#### 3.2.3 API

```rust
impl CapabilityTable {
    /// Allocate a new capability
    pub fn create(&mut self, object: CapObject, rights: Rights, owner: TaskId) -> Cap;
    
    /// Look up and validate a capability
    pub fn lookup(&self, cap: Cap) -> Result<&CapEntry, CapError>;
    
    /// Derive a child capability with reduced rights
    pub fn derive(&mut self, parent: Cap, new_rights: Rights) -> Result<Cap, CapError>;
    
    /// Revoke a capability and all its children
    pub fn revoke(&mut self, cap: Cap) -> Result<(), CapError>;
    
    /// Check if capability grants specific rights
    pub fn check(&self, cap: Cap, required: Rights) -> Result<(), CapError>;
}
```

### 3.3 Memory Manager

#### 3.3.1 Design
- Static allocation only (no heap in kernel)
- Region-based allocation for WASM modules
- No MMU on ESP32 - rely on WASM sandboxing

#### 3.3.2 Data Structures

```rust
pub struct MemoryManager {
    /// Fixed regions allocated at boot
    regions: [MemRegion; MAX_REGIONS],
    /// Bitmap for WASM arena pages
    wasm_pages: Bitmap<WASM_ARENA_PAGES>,
}

pub struct MemRegion {
    pub base: usize,
    pub size: usize,
    pub kind: RegionKind,
    pub owner: Option<TaskId>,
}

pub enum RegionKind {
    Kernel,
    WasmLinear,
    NetworkBuffer,
    Framebuffer,
    DmaBuffer,
}
```

#### 3.3.3 API

```rust
impl MemoryManager {
    /// Allocate pages for WASM linear memory
    pub fn alloc_wasm_pages(&mut self, count: usize) -> Result<*mut u8, MemError>;
    
    /// Free WASM pages
    pub fn free_wasm_pages(&mut self, ptr: *mut u8, count: usize);
    
    /// Get a network buffer (from fixed pool)
    pub fn alloc_netbuf(&mut self) -> Result<NetBuf, MemError>;
    
    /// Return a network buffer
    pub fn free_netbuf(&mut self, buf: NetBuf);
}
```

### 3.4 Interrupt Router

#### 3.4.1 Design
- Minimal interrupt handlers in kernel
- Route to capability-holding tasks via IPC
- Deferred processing in tasks, not ISRs

#### 3.4.2 Data Structures

```rust
pub struct InterruptRouter {
    /// Registered handlers per IRQ
    handlers: [Option<IrqHandler>; MAX_IRQS],
}

pub struct IrqHandler {
    pub task: TaskId,
    pub endpoint: Cap,
    pub mask_on_trigger: bool,
}
```

#### 3.4.3 API

```rust
impl InterruptRouter {
    /// Register a task to handle an IRQ
    pub fn register(&mut self, irq: u8, handler: IrqHandler) -> Result<(), IrqError>;
    
    /// Unregister handler
    pub fn unregister(&mut self, irq: u8);
    
    /// Called from ISR - minimal work, just unblock task
    pub fn dispatch(&mut self, irq: u8);
    
    /// Task acknowledges interrupt, re-enables
    pub fn ack(&mut self, irq: u8);
}
```

---

## 4. WASM Runtime Specification

### 4.1 Runtime Selection
- **ESP32**: wasm3 (C interpreter, ~64KB)
- **x86**: wasmtime (JIT) or wasm3 for consistency

### 4.2 Module Loading

```rust
pub struct WasmRuntime {
    modules: Slab<LoadedModule>,
}

pub struct LoadedModule {
    pub id: ModuleId,
    pub name: String<32>,
    pub state: ModuleState,
    pub memory: *mut u8,
    pub memory_size: usize,
    pub capabilities: CapabilitySet,
    pub instance: wasm3::Instance,
}

pub enum ModuleState {
    Loading,
    Running,
    Paused,
    Crashed { error: WasmError },
    Terminated,
}
```

### 4.3 Host Functions (Syscall Interface)

All host functions take `&mut WasmContext` implicitly.

#### 4.3.1 Core

| Function | Signature | Description |
|----------|-----------|-------------|
| `sp_yield` | `() -> ()` | Yield to scheduler |
| `sp_sleep` | `(ms: u32) -> ()` | Sleep for milliseconds |
| `sp_time` | `() -> u64` | Get system time (ms since boot) |
| `sp_log` | `(level: u32, ptr: u32, len: u32) -> ()` | Log message |
| `sp_panic` | `(ptr: u32, len: u32) -> !` | Abort module |

#### 4.3.2 IPC

| Function | Signature | Description |
|----------|-----------|-------------|
| `sp_ipc_send` | `(endpoint: u32, ptr: u32, len: u32) -> i32` | Send message |
| `sp_ipc_recv` | `(endpoint: u32, buf: u32, len: u32) -> i32` | Receive message |
| `sp_ipc_poll` | `(endpoints: u32, count: u32, timeout: u32) -> i32` | Poll multiple |

#### 4.3.3 Filesystem

| Function | Signature | Description |
|----------|-----------|-------------|
| `sp_fs_open` | `(path: u32, len: u32, flags: u32) -> i32` | Open file |
| `sp_fs_read` | `(fd: u32, buf: u32, len: u32) -> i32` | Read from file |
| `sp_fs_write` | `(fd: u32, ptr: u32, len: u32) -> i32` | Write to file |
| `sp_fs_close` | `(fd: u32) -> i32` | Close file |
| `sp_fs_stat` | `(path: u32, len: u32, buf: u32) -> i32` | Get file info |

#### 4.3.4 Network

| Function | Signature | Description |
|----------|-----------|-------------|
| `sp_net_socket` | `(proto: u32) -> i32` | Create socket |
| `sp_net_connect` | `(sock: u32, addr: u32, port: u32) -> i32` | Connect |
| `sp_net_bind` | `(sock: u32, port: u32) -> i32` | Bind to port |
| `sp_net_listen` | `(sock: u32, backlog: u32) -> i32` | Listen |
| `sp_net_accept` | `(sock: u32) -> i32` | Accept connection |
| `sp_net_send` | `(sock: u32, ptr: u32, len: u32) -> i32` | Send data |
| `sp_net_recv` | `(sock: u32, buf: u32, len: u32) -> i32` | Receive data |
| `sp_net_close` | `(sock: u32) -> i32` | Close socket |

#### 4.3.5 GPIO

| Function | Signature | Description |
|----------|-----------|-------------|
| `sp_gpio_mode` | `(pin: u32, mode: u32) -> i32` | Set pin mode |
| `sp_gpio_read` | `(pin: u32) -> i32` | Read pin |
| `sp_gpio_write` | `(pin: u32, value: u32) -> i32` | Write pin |

#### 4.3.6 UART

| Function | Signature | Description |
|----------|-----------|-------------|
| `sp_uart_open` | `(port: u32, baud: u32) -> i32` | Open UART |
| `sp_uart_read` | `(port: u32, buf: u32, len: u32) -> i32` | Read bytes |
| `sp_uart_write` | `(port: u32, ptr: u32, len: u32) -> i32` | Write bytes |
| `sp_uart_close` | `(port: u32) -> i32` | Close UART |

### 4.4 Module Manifest

Each WASM module is accompanied by a TOML manifest:

```toml
[module]
name = "balboa-controller"
version = "1.0.0"
author = "Your Name"
description = "Balboa spa controller"

[capabilities]
# Required capabilities - module won't load without these
required = [
    "net:tcp",
    "uart:0",
]

# Optional capabilities - module works without these
optional = [
    "fs:read:/etc/",
    "fs:write:/var/log/",
]

[resources]
# Maximum linear memory (pages of 64KB)
max_memory_pages = 4
# Stack size
stack_size = 8192

[dependencies]
# Other modules this depends on (for IPC)
depends = []

[ota]
# Signature verification
signing_key = "ed25519:ABC123..."
```

---

## 5. Boot Sequence

### 5.1 ESP32-C6 Boot Flow

```
1. ROM Bootloader
   └─► Load second-stage bootloader from flash

2. Second-Stage Bootloader  
   └─► Verify kernel signature
   └─► Load kernel to RAM
   └─► Jump to kernel entry

3. Kernel Early Init (SovelmaOS_start)
   ├─► Initialize BSS
   ├─► Setup interrupt vectors
   ├─► Initialize memory manager
   └─► Initialize capability table

4. Kernel Main Init (SovelmaOS_main)
   ├─► Initialize scheduler
   ├─► Initialize HAL (clocks, GPIO, UART)
   ├─► Initialize network stack (smoltcp)
   ├─► Start DHCP client
   ├─► Initialize WASM runtime
   ├─► Mount littlefs
   └─► Load boot modules from /etc/boot.conf

5. Module Loading
   ├─► Load core modules (OTA manager, etc.)
   ├─► Verify signatures
   ├─► Grant capabilities per manifest
   └─► Start module tasks

6. Idle
   └─► Scheduler runs, system operational
```

### 5.2 Boot Configuration

`/etc/boot.conf`:
```toml
[network]
dhcp = true
# Or static:
# ip = "192.168.1.100"
# gateway = "192.168.1.1"
# dns = "192.168.1.1"

hostname = "SovelmaOS-spa"

[modules]
# Modules to load at boot
boot = [
    "/var/modules/ota-manager.wasm",
    "/var/modules/balboa.wasm",
]

[watchdog]
enabled = true
timeout_ms = 30000
```

---

## 6. Error Handling

### 6.1 Error Codes

```rust
#[repr(i32)]
pub enum SovelmaOSError {
    Ok = 0,
    
    // Generic
    InvalidArgument = -1,
    OutOfMemory = -2,
    NotFound = -3,
    AlreadyExists = -4,
    PermissionDenied = -5,
    
    // Capability
    InvalidCapability = -10,
    CapabilityRevoked = -11,
    InsufficientRights = -12,
    
    // IPC
    EndpointFull = -20,
    MessageTooLarge = -21,
    WouldBlock = -22,
    Timeout = -23,
    
    // Filesystem
    FileNotFound = -30,
    NotAFile = -31,
    NotADirectory = -32,
    FilesystemFull = -33,
    
    // Network
    ConnectionRefused = -40,
    ConnectionReset = -41,
    NetworkUnreachable = -42,
    
    // WASM
    ModuleInvalid = -50,
    ModuleTrap = -51,
    ModuleOom = -52,
}
```

### 6.2 Supervisor / Fault Recovery

```rust
pub struct Supervisor {
    policies: HashMap<ModuleId, RestartPolicy>,
    crash_log: CircularBuffer<CrashEvent>,
}

pub struct RestartPolicy {
    pub action: RestartAction,
    pub max_restarts: u32,
    pub window_ms: u32,
    pub backoff_base_ms: u32,
}

pub enum RestartAction {
    Restart,
    Ignore,
    Disable,
    Escalate,  // Notify another module
}

impl Supervisor {
    pub fn on_module_crash(&mut self, module: ModuleId, error: WasmError) {
        self.crash_log.push(CrashEvent { module, error, time: now() });
        
        let policy = self.policies.get(&module).unwrap_or(&DEFAULT_POLICY);
        let recent_crashes = self.count_recent_crashes(module, policy.window_ms);
        
        if recent_crashes >= policy.max_restarts {
            log::error!("Module {:?} exceeded restart limit", module);
            self.disable_module(module);
            return;
        }
        
        let delay = policy.backoff_base_ms * 2u32.pow(recent_crashes);
        self.schedule_restart(module, delay);
    }
}
```

---

## 7. Platform HAL

### 7.1 HAL Traits

```rust
pub trait SovelmaOSHal {
    // Time
    fn now_ms(&self) -> u64;
    fn delay_ms(&self, ms: u32);
    
    // Interrupts
    fn enable_interrupts(&self);
    fn disable_interrupts(&self);
    fn set_irq_handler(&self, irq: u8, handler: fn());
    
    // GPIO
    fn gpio_set_mode(&self, pin: u8, mode: GpioMode) -> Result<(), HalError>;
    fn gpio_read(&self, pin: u8) -> Result<bool, HalError>;
    fn gpio_write(&self, pin: u8, value: bool) -> Result<(), HalError>;
    
    // UART
    fn uart_init(&self, port: u8, config: UartConfig) -> Result<(), HalError>;
    fn uart_write(&self, port: u8, data: &[u8]) -> Result<usize, HalError>;
    fn uart_read(&self, port: u8, buf: &mut [u8]) -> Result<usize, HalError>;
    
    // Network (platform-specific driver)
    fn net_init(&self) -> Result<(), HalError>;
    fn net_poll(&self) -> Option<NetEvent>;
    fn net_send(&self, data: &[u8]) -> Result<(), HalError>;
    
    // Flash
    fn flash_read(&self, offset: usize, buf: &mut [u8]) -> Result<(), HalError>;
    fn flash_write(&self, offset: usize, data: &[u8]) -> Result<(), HalError>;
    fn flash_erase(&self, offset: usize, size: usize) -> Result<(), HalError>;
}
```

### 7.2 ESP32-C6 Implementation Notes

```rust
// Uses esp-hal crate
pub struct Esp32c6Hal {
    peripherals: Peripherals,
    wifi: EspWifi,
}

impl SovelmaOSHal for Esp32c6Hal {
    fn now_ms(&self) -> u64 {
        esp_hal::time::current_time().as_millis() as u64
    }
    
    // ... other implementations using esp-hal
}
```

---

## 8. Project Structure

```
SovelmaOS/
├── Cargo.toml
├── rust-toolchain.toml
├── .cargo/
│   └── config.toml
├── kernel/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── scheduler.rs
│   │   ├── capability.rs
│   │   ├── memory.rs
│   │   ├── interrupt.rs
│   │   ├── ipc.rs
│   │   ├── wasm/
│   │   │   ├── mod.rs
│   │   │   ├── runtime.rs
│   │   │   └── host_functions.rs
│   │   ├── fs/
│   │   │   ├── mod.rs
│   │   │   ├── vfs.rs
│   │   │   └── littlefs.rs
│   │   ├── net/
│   │   │   ├── mod.rs
│   │   │   ├── stack.rs
│   │   │   └── dhcp.rs
│   │   └── boot.rs
│   └── link.x              # Linker script
├── hal/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── traits.rs
│       ├── esp32c6/
│       │   ├── mod.rs
│       │   ├── gpio.rs
│       │   ├── uart.rs
│       │   ├── wifi.rs
│       │   └── flash.rs
│       └── x86_64/
│           └── mod.rs
├── modules/
│   ├── ota-manager/
│   │   ├── Cargo.toml
│   │   ├── manifest.toml
│   │   └── src/
│   │       └── lib.rs
│   └── example-app/
│       ├── Cargo.toml
│       ├── manifest.toml
│       └── src/
│           └── lib.rs
├── tools/
│   ├── SovelmaOS-flash/       # Flashing tool
│   └── SovelmaOS-sign/        # Module signing tool
└── docs/
    └── *.md
```

---

## 9. Build System

### 9.1 Toolchain Requirements

```toml
# rust-toolchain.toml
[toolchain]
channel = "nightly-2024-12-01"
components = ["rust-src", "rustfmt", "clippy"]
targets = ["riscv32imc-unknown-none-elf", "wasm32-unknown-unknown", "x86_64-unknown-none"]
```

### 9.2 Cargo Configuration

```toml
# .cargo/config.toml
[build]
target = "riscv32imc-unknown-none-elf"

[target.riscv32imc-unknown-none-elf]
runner = "espflash flash --monitor"
rustflags = ["-C", "link-arg=-Tlink.x"]

[target.wasm32-unknown-unknown]
runner = "wasmtime"

[env]
ESP_IDF_VERSION = "v5.2"
```

### 9.3 Build Commands

```bash
# Build kernel for ESP32-C6
cargo build --release -p SovelmaOS-kernel --target riscv32imc-unknown-none-elf

# Build a WASM module
cargo build --release -p example-app --target wasm32-unknown-unknown

# Flash to device
cargo run --release -p SovelmaOS-kernel

# Build for x86 (QEMU)
cargo build --release -p SovelmaOS-kernel --target x86_64-unknown-none
```

---

## 10. Testing Strategy

### 10.1 Unit Tests
- Run on host with `cargo test`
- Mock HAL for platform-independent testing

### 10.2 Integration Tests
- QEMU for x86 variant
- Wokwi ESP32 simulator
- Real hardware CI with self-hosted runner

### 10.3 Module Tests
- WASM modules tested in wasmtime on host
- Mock host functions

---

## Appendix A: References

- seL4 Microkernel: https://sel4.systems/
- wasm3: https://github.com/wasm3/wasm3
- smoltcp: https://github.com/smoltcp-rs/smoltcp
- esp-hal: https://github.com/esp-rs/esp-hal
- littlefs: https://github.com/littlefs-project/littlefs

---

## Appendix B: Glossary

| Term | Definition |
|------|------------|
| **Capability** | Unforgeable token granting specific rights to a resource |
| **Host Function** | Kernel function callable from WASM module |
| **Linear Memory** | WASM module's memory space |
| **Module** | A WASM binary loaded and executed by SovelmaOS |
| **HAL** | Hardware Abstraction Layer |

---

*Document Version: 0.1.0-draft*
*Last Updated: 2025-01-21*
