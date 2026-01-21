---
trigger: always_on
---

# SovelmaOS Development Rules

## 1. Code Quality & Standards

### 1.1 Language Standards
- **Rust**: Use current stable. `#[no_std]` for kernel.
  - run `cargo fmt` and `cargo clippy` before every commit.
  - No `unsafe` without a comment explaining WHY it is safe.
- **C**: C11 standard.
- **Assembly**: RISC-V or x86_64, clearly commented.

### 1.2 Documentation
- All public functions/structs must have doc comments (`///`).
- Architecture decisions must be documented in `docs/architecture/`.
- Update `docs/specs/` when changing behavior.

## 2. File Structure

- `src/kernel/`: Core kernel code.
- `src/userspace/`: User-space libraries and default programs.
- `src/drivers/`: Platform-specific drivers.
- `docs/`: All documentation.

## 3. Workflow

- **Refactoring**: Create a plan first if changing >5 files.
- **Testing**: Run `cargo test` (where applicable) before push.
- **Commits**: Conventional Commits style (e.g., `feat: add scheduler`).

## 4. Forbidden Patterns
- No specialized "magic numbers" without named constants.
- No `unwrap()` in kernel code (use `expect` or handle errors).
- No global mutable state outside of strict synchronization primitives.
