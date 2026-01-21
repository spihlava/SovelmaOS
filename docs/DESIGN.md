# SovelmaOS Design Specification

## 1. Overview
SovelmaOS is a microkernel OS for ESP32 (RISC-V) and x86_64, prioritizing fault isolation via WebAssembly userspace.

## 2. Architecture

### 2.1 Layers
1. **WASM Modules (Ring 3)**: Applications (Network, GUI, Drivers) running in sandbox.
2. **Runtime (Ring 0)**: `wasmi` interpreter providing `HostFunctions` (syscalls).
3. **Kernel (Ring 0)**:
   - **Scheduler**: Priority-based, cooperative within modules.
   - **Capabilities**: Object-capability model for all resource access.
   - **Memory**: Static allocation, region-based for WASM arenas.
4. **HAL**: Hardware Abstraction Layer for ESP32/x86 compatibility.

### 2.2 Memory Layout (ESP32)
- **Kernel Code/Data**: ~80KB
- **WASM Runtime**: ~64KB
- **Module Arena**: ~300KB (Heap for WASM linear memories)

## 3. Core Subsystems

### 3.1 Capabilities
All resources (Memory, IPC, IRQ, Network) are guarded by `CapId` tokens.
- **Grant**: Kernel grants initial caps at boot based on manifest.
- **Revoke**: Generation-counter based revocation.

### 3.2 Scheduling
- **Tasks**: Kernel tasks mapped 1:1 to WASM instances.
- **Preemption**: Fuel-based preemption for WASM modules to ensure responsiveness.

### 3.3 Networking
- **Stack**: `smoltcp` running in kernel mode.
- **Interface**: Exposed to userspace via `sp_net_*` host functions.

## 4. WASM Userspace Interface

Modules interact with the kernel strictly through Host Functions.
- **System**: `sp_yield`, `sp_sleep`, `sp_log`
- **Network**: `sp_net_connect`, `sp_net_send`, `sp_net_recv`
- **Filesystem**: `sp_fs_open`, `sp_fs_read`, `sp_fs_size`, `sp_fs_close`
- **GPIO**: `sp_gpio_read`, `sp_gpio_write` (Cap-gated)

### 3.4 Filesystem
- **In-Memory**: Initial implementation is a RamFS.
- **Interface**: Path-based open, stateful or offset-based read.


## 5. Boot Sequence
1. **Bootloader**: Loads Kernel.
2. **Kernel Init**: Setup HAL, Allocator, Scheduler.
3. **Module Loader**: Loads `.wasm` blobs from Flash/FS.
4. **Execution**: Spawns tasks for each module.
