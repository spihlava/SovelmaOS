# Architectural Decisions

## 1. Rust vs C for Microkernel
**Decision**: **Rust**.
- **Context**: While C is traditional for microkernels (seL4), and ~80% of kernel code is `unsafe`, Rust offers superior tooling (Cargo), better build system integration, and strong memory safety for the ~20% of logic (scheduler, capability checks) that is safe.
- **Benefit**: Faster development velocity, `smoltcp` integration, and stronger type system benefits.

## 2. WASM for Userspace
**Decision**: **WebAssembly (WASM)**.
- **Context**: Embedded devices often lack MMUs (ESP32-C3/C6 have PMP but not full paging mechanisms like x86). Native code requires complex relocation and linking.
- **Benefit**: 
    - **Isolation**: Memory safety without hardware MMU dependency.
    - **Portability**: Same binary runs on ESP32 and x86.
    - **OTA**: Safe updates; a crashing module cannot panic the kernel.

## 3. Dedicated Network Stack (smoltcp)
**Decision**: **In-Kernel smoltcp**.
- **Context**: Networking is critical for an IoT OS ("Network-first").
- **Benefit**: Available immediately at boot. No context-switching overhead for packet processing compared to a userspace driver.

## 4. Flash Filesystem
**Decision**: **littlefs**.
- **Context**: ESP32 uses NOR flash which requires wear leveling and power-loss protection.
- **Benefit**: Proven reliability on embedded flash. Exposed to WASM via file descriptors and capability checks.

## 5. Modular Kernel Architecture
**Decision**: **Separate arch-specific code into modules**.
- **Context**: The initial kernel implementation mixed platform-specific code (VGA, serial) with generic logic in a single file, violating the spec's modular structure.
- **Structure**:
  ```
  src/kernel/src/
  ├── lib.rs           # Public kernel API
  ├── main.rs          # Entry point only
  └── arch/
      ├── mod.rs       # Architecture abstraction
      └── x86_64/
          ├── mod.rs   # x86_64 platform module
          ├── vga.rs   # VGA text mode driver
          └── serial.rs # Serial port driver
  ```
- **Benefit**:
    - Clear separation of concerns
    - Easy to add new architectures (RISC-V, ARM)
    - Static initialization for panic-safe serial/VGA access
    - Documented unsafe blocks with safety invariants
