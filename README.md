# SovelmaOS

**State-of-the-Art Microkernel OS for ESP32 (RISC-V) and x86_64**

SovelmaOS is a formally separated microkernel architecture featuring a Rust-based kernel and a WASM-based userspace.

## Project Structure

- `src/kernel`: The core kernel (Ring 0), managing capability-based security, memory, and task scheduling.
- `src/userspace`: WASM application layer (Ring 3 equivalent).
  - `sdk`: `sovelma-sdk` crate for WASM apps to access host functions.
  - `apps`: Sample WASM applications (e.g., `hello-app`).
- `src/common`: Shared ABI types (Capabilities, NetError) used by both kernel and userspace.
- `src/hal`: Hardware Abstraction Layer for platform independence.

## Getting Started

### Prerequisites
- Rust Nightly
- QEMU (for x86_64 emulation)
- WASM target: `rustup target add wasm32-unknown-unknown`

### Build & Run
```bash
# Build the kernel
cargo build -p sovelma-kernel --target x86_64-unknown-none

# Run in QEMU
cd src/kernel && cargo run

# Build Userspace App
cargo build -p hello-app --target wasm32-unknown-unknown
```

### Testing
```bash
# Run unit tests
cargo test -p sovelma-common

# Run kernel integration tests
cd src/kernel && cargo test --target x86_64-unknown-none
```

## Documentation

- [Design Specification](docs/DESIGN.md)


## License
MIT License.