# SovelmaOS Development Roadmap

## Completed (Audit P0/P1)

- [x] Modular architecture (`arch/x86_64/{vga,serial}.rs`)
- [x] Safety comments on all unsafe blocks
- [x] Named constants (VGA_BUFFER_ADDR, COM1_PORT)
- [x] Static serial/VGA initialization (spin::Once)
- [x] Removed dead ESP32 code
- [x] rust-toolchain.toml for reproducible builds
- [x] Consolidated cargo configs
- [x] Fixed VGA bounds checking
- [x] Verified QEMU boot

---

## Phase 1: Test Infrastructure

**Goal:** Enable test-driven development for kernel components.

### 1.1 Unit Test Framework

```
src/kernel/src/
├── lib.rs              # Add #![cfg_attr(test, feature(custom_test_frameworks))]
└── tests/
    └── mod.rs          # Test runner setup
```

**Tasks:**
1. Add custom test framework to `lib.rs`:
   ```rust
   #![cfg_attr(test, no_main)]
   #![cfg_attr(test, feature(custom_test_frameworks))]
   #![cfg_attr(test, test_runner(crate::tests::test_runner))]
   #![cfg_attr(test, reexport_test_harness_main = "test_main")]
   ```

2. Create `src/kernel/src/tests/mod.rs`:
   ```rust
   pub fn test_runner(tests: &[&dyn Fn()]) {
       serial_println!("Running {} tests", tests.len());
       for test in tests {
           test();
       }
       exit_qemu(QemuExitCode::Success);
   }
   ```

3. Add QEMU exit device support:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   #[repr(u32)]
   pub enum QemuExitCode {
       Success = 0x10,
       Failed = 0x11,
   }

   pub fn exit_qemu(exit_code: QemuExitCode) {
       use x86_64::instructions::port::Port;
       unsafe {
           let mut port = Port::new(0xf4);
           port.write(exit_code as u32);
       }
   }
   ```

4. Update `.cargo/config.toml` for test runner:
   ```toml
   [target.x86_64-unknown-none]
   runner = "bootimage runner"

   [target.'cfg(target_arch = "x86_64")'.runner]
   test-args = [
       "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
       "-serial", "stdio",
       "-display", "none"
   ]
   ```

5. Add first tests:
   - `tests/vga.rs` - VGA buffer write/scroll tests
   - `tests/serial.rs` - Serial output tests

**Verification:**
```bash
cargo test --lib
```

---

## Phase 2: Interrupt Handling

**Goal:** Handle CPU exceptions and hardware interrupts.

### 2.1 Interrupt Descriptor Table (IDT)

```
src/kernel/src/
├── arch/x86_64/
│   ├── mod.rs
│   ├── interrupts.rs   # NEW: IDT setup
│   └── gdt.rs          # NEW: GDT for TSS
└── lib.rs
```

**Tasks:**
1. Create `arch/x86_64/gdt.rs`:
   - Define GDT with kernel code/data segments
   - Add Task State Segment (TSS) for stack switching
   - Load GDT on init

2. Create `arch/x86_64/interrupts.rs`:
   - Define IDT with 256 entries
   - Implement exception handlers:
     - Division error (0)
     - Breakpoint (3)
     - Double fault (8) - **critical**
     - Page fault (14)
     - General protection fault (13)
   - Use `x86_64::structures::idt::InterruptDescriptorTable`

3. Add interrupt stack:
   ```rust
   pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

   lazy_static! {
       static ref TSS: TaskStateSegment = {
           let mut tss = TaskStateSegment::new();
           tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
               const STACK_SIZE: usize = 4096 * 5;
               static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
               let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
               stack_start + STACK_SIZE
           };
           tss
       };
   }
   ```

4. Add `lazy_static` dependency to Cargo.toml:
   ```toml
   lazy_static = { version = "1.4", features = ["spin_no_std"] }
   ```

5. Initialize in boot sequence:
   ```rust
   pub fn init() {
       gdt::init();
       interrupts::init_idt();
   }
   ```

**Verification:**
- Trigger breakpoint exception: `x86_64::instructions::interrupts::int3()`
- Verify handler prints message and continues
- Test double fault with stack overflow

---

## Phase 3: Hardware Interrupts (PIC)

**Goal:** Handle timer and keyboard interrupts.

### 3.1 Programmable Interrupt Controller

**Tasks:**
1. Add `pic8259` dependency:
   ```toml
   pic8259 = "0.10"
   ```

2. Create `arch/x86_64/pic.rs`:
   ```rust
   use pic8259::ChainedPics;
   use spin::Mutex;

   pub const PIC_1_OFFSET: u8 = 32;
   pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

   pub static PICS: Mutex<ChainedPics> = Mutex::new(
       unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) }
   );
   ```

3. Add interrupt handlers for:
   - Timer (IRQ 0 → vector 32)
   - Keyboard (IRQ 1 → vector 33)

4. Enable interrupts after IDT init:
   ```rust
   x86_64::instructions::interrupts::enable();
   ```

5. Implement keyboard scancode handling (basic):
   ```rust
   extern "x86-interrupt" fn keyboard_interrupt_handler(_: InterruptStackFrame) {
       use x86_64::instructions::port::Port;
       let mut port = Port::new(0x60);
       let scancode: u8 = unsafe { port.read() };
       serial_println!("Key: {}", scancode);
       // ... end of interrupt
   }
   ```

**Verification:**
- Timer ticks logged to serial
- Keyboard input echoed to serial

---

## Phase 4: Memory Management

**Goal:** Basic physical/virtual memory allocation.

### 4.1 Physical Memory

```
src/kernel/src/
├── memory/
│   ├── mod.rs
│   ├── frame_allocator.rs  # Physical frame allocation
│   └── paging.rs           # Page table management
```

**Tasks:**
1. Parse memory map from bootloader:
   ```rust
   use bootloader::bootinfo::{BootInfo, MemoryMap};

   #[no_mangle]
   pub extern "C" fn _start(boot_info: &'static BootInfo) -> ! {
       let memory_map = &boot_info.memory_map;
       // ...
   }
   ```

2. Implement bump allocator for frames:
   ```rust
   pub struct BootInfoFrameAllocator {
       memory_map: &'static MemoryMap,
       next: usize,
   }

   impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
       fn allocate_frame(&mut self) -> Option<PhysFrame> { ... }
   }
   ```

3. Create kernel heap:
   - Map heap pages (e.g., 100 KiB at 0x4444_4444_0000)
   - Implement `GlobalAlloc` trait
   - Add `linked_list_allocator` crate

4. Enable `alloc` crate:
   ```rust
   extern crate alloc;
   use alloc::{boxed::Box, vec::Vec};
   ```

**Verification:**
- `Box::new(42)` works
- `Vec::push()` works
- Memory exhaustion handled gracefully

---

## Phase 5: Basic Scheduler

**Goal:** Cooperative multitasking with async/await.

### 5.1 Async Executor

```
src/kernel/src/
├── task/
│   ├── mod.rs
│   ├── executor.rs     # Simple executor
│   └── keyboard.rs     # Async keyboard task
```

**Tasks:**
1. Create `Task` wrapper:
   ```rust
   pub struct Task {
       id: TaskId,
       future: Pin<Box<dyn Future<Output = ()>>>,
   }
   ```

2. Implement simple executor:
   ```rust
   pub struct Executor {
       tasks: BTreeMap<TaskId, Task>,
       task_queue: VecDeque<TaskId>,
       waker_cache: BTreeMap<TaskId, Waker>,
   }

   impl Executor {
       pub fn run(&mut self) -> ! {
           loop {
               self.run_ready_tasks();
               self.sleep_if_idle();
           }
       }
   }
   ```

3. Create async keyboard handler:
   ```rust
   pub async fn print_keypresses() {
       let mut scancodes = ScancodeStream::new();
       while let Some(scancode) = scancodes.next().await {
           // handle keypress
       }
   }
   ```

**Verification:**
- Multiple async tasks run concurrently
- Keyboard input processed asynchronously
- CPU sleeps when idle (HLT)

---

## Phase 6: HAL Trait System

**Goal:** Abstract platform differences behind traits.

### 6.1 HAL Crate

```
src/hal/
├── Cargo.toml
└── src/
    └── lib.rs          # Trait definitions
```

**Tasks:**
1. Create `sovelma-hal` crate:
   ```toml
   [package]
   name = "sovelma-hal"
   version = "0.1.0"
   edition = "2021"

   [dependencies]
   # none - pure trait definitions
   ```

2. Define core traits:
   ```rust
   pub trait Serial {
       fn write_byte(&mut self, byte: u8);
       fn read_byte(&mut self) -> Option<u8>;
   }

   pub trait Console {
       fn write_str(&mut self, s: &str);
       fn clear(&mut self);
       fn set_color(&mut self, fg: Color, bg: Color);
   }

   pub trait InterruptController {
       fn enable(&mut self);
       fn disable(&mut self);
       fn end_of_interrupt(&mut self, irq: u8);
   }

   pub trait Timer {
       fn current_ticks(&self) -> u64;
       fn set_alarm(&mut self, ticks: u64);
   }
   ```

3. Implement traits in `arch/x86_64/`:
   ```rust
   impl hal::Serial for SerialPort { ... }
   impl hal::Console for VgaWriter { ... }
   ```

4. Update kernel to use trait objects where appropriate

**Verification:**
- Kernel compiles with HAL traits
- x86_64 implementation passes all tests

---

## Phase 7: Capability System (Basic)

**Goal:** Access control for kernel resources.

### 7.1 Capability Table

```
src/kernel/src/
├── capability/
│   ├── mod.rs
│   ├── table.rs        # Capability storage
│   └── types.rs        # Capability definitions
```

**Tasks:**
1. Define capability types:
   ```rust
   #[derive(Debug, Clone, Copy)]
   pub enum CapabilityType {
       Memory { start: usize, size: usize, perms: Permissions },
       Serial { port: u16 },
       Timer,
       Interrupt { irq: u8 },
   }

   #[derive(Debug, Clone, Copy)]
   pub struct Capability {
       id: CapId,
       cap_type: CapabilityType,
       owner: TaskId,
   }
   ```

2. Implement capability table:
   ```rust
   pub struct CapabilityTable {
       caps: BTreeMap<CapId, Capability>,
       next_id: CapId,
   }

   impl CapabilityTable {
       pub fn grant(&mut self, task: TaskId, cap_type: CapabilityType) -> CapId;
       pub fn revoke(&mut self, cap_id: CapId) -> Result<(), CapError>;
       pub fn check(&self, task: TaskId, cap_id: CapId) -> bool;
   }
   ```

3. Integrate with syscall layer (future)

**Verification:**
- Capabilities can be created/revoked
- Access checks work correctly
- Invalid capability access denied

---

## Dependency Summary

Add to `Cargo.toml` as phases progress:

```toml
[dependencies]
# Phase 1 (existing)
spin = { version = "0.9", features = ["once", "mutex", "spin_mutex"] }

# Phase 2
lazy_static = { version = "1.4", features = ["spin_no_std"] }

# Phase 3
pic8259 = "0.10"

# Phase 4
linked_list_allocator = "0.10"

# Phase 5
crossbeam-queue = { version = "0.3", default-features = false, features = ["alloc"] }
conquer-once = { version = "0.4", default-features = false }
futures-util = { version = "0.3", default-features = false, features = ["alloc"] }

# Phase 7
# (no new deps - uses existing collections)
```

---

## Timeline Estimate

| Phase | Description | Complexity |
|-------|-------------|------------|
| 1 | Test Infrastructure | Low |
| 2 | IDT/Exceptions | Medium |
| 3 | PIC/Hardware IRQs | Medium |
| 4 | Memory Management | High |
| 5 | Basic Scheduler | High |
| 6 | HAL Traits | Medium |
| 7 | Capabilities | Medium |

**Recommended order:** 1 → 2 → 3 → 4 → 5 → 6 → 7

Each phase builds on the previous. Tests (Phase 1) are optional but highly recommended before proceeding.

---

## Quick Start Commands

```bash
# Build
cd src/kernel && cargo build --release

# Run in QEMU
cargo run --release

# Run tests (after Phase 1)
cargo test --lib

# Check for issues
cargo clippy -- -D warnings
```
