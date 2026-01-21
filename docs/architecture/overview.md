# SovelmaOS Architecture Overview

## Core Philosophy
SovelmaOS is a **Rust-based microkernel** that runs a **WASM userspace**. It is designed for extreme fault tolerance, ease of updates (OTA), and security on embedded devices (ESP32) and desktop (x86_64).

## System Architecture

```mermaid
graph TD
    UserSpace[WASM Userspace]
    Kernel[SovelmaOS Microkernel (Rust)]
    Hardware[Hardware (ESP32 / x86)]

    subgraph "Userspace (Sandboxed)"
        App1[App: Balboa Spa]
        App2[Service: Logger]
        App1 -- IPC --> App2
    end

    subgraph "Kernel Services"
        WASM[WASM Runtime (wasm3/wasmtime)]
        Net[Network Stack (smoltcp)]
        FS[Filesystem (littlefs)]
        Caps[Capability Manager]
    end

    UserSpace -- Host Functions --> Kernel
    Kernel -- HAL --> Hardware
```

## Key Components

| Component | Technology | Reasoning |
|-----------|------------|-----------|
| **Kernel** | Rust `no_std` | Memory safety, modern tooling, tiny footprint (~50KB). |
| **Runtime** | `wasm3` (ESP32) / `wasmtime` (x86) | Universal binary format, hardware-agnostic, built-in sandboxing. |
| **Network** | `smoltcp` | Rust-native, no heap allocation required, high performance. |
| **Filesystem**| `littlefs` (Flash) | Power-loss resilient, designed for microcontrollers. |
| **Isolation**| Capability Tokens | Granular access control (GPIO, Network, FS) per module. |

## Execution Model
1.  **Boot**: Kernel initializes HAL, Network, and WASM Runtime.
2.  **Load**: Kernel reads `.wasm` modules and Manifests (`.toml`) from storage.
3.  **Verify**: Capabilities are checked against the Manifest.
4.  **Run**: Modules execute in the runtime, calling Kernel via Host Functions.
