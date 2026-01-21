# 🔍 Critical Review: SovelmaOS — Conformance to Design & State of the Art Assessment

**Last Updated**: 2026-01-21

## Executive Summary

This review evaluates SovelmaOS against its [Design Specification](docs/DESIGN.md) and state-of-the-art microkernel standards (seL4, Fuchsia, Zephyr). **Significant progress** has been made since the initial prototype, but the project **still does not meet "State of the Art" standards** for a secure microkernel due to two critical gaps.

---

## ✅ What the Project Gets RIGHT

| Area | Status | Notes |
|------|--------|-------|
| **Capability-Based FS API** | ✅ Fixed | `sp_fs_open` now requires a `dir_cap` parameter (→ `open_at` pattern). Ambient authority is **banned** for FS operations. |
| **Generation-Counter Enforcement** | ✅ Fixed | `host.rs` validates `cap.generation` against `id.generation()` before returning capability. |
| **Capability Storage** | ✅ Improved | `HostState.capabilities` uses `BTreeMap<CapId, Capability>` (O(log N)), not linear `Vec`. |
| **Hierarchical Filesystem** | ✅ Fixed | `ramfs.rs` implements a true tree structure: `Node::Directory(BTreeMap<String, Arc<RwLock<Node>>>)`. |
| **Async Executor with Priority** | ✅ Good | `Executor` has 4 priority queues (Idle, Normal, High, Critical). Tasks polled high→low. |
| **Resumable WASM Calls** | ✅ Improved | `WasmCallFuture` and `WasmTask` use `call_resumable` and handle `ResumableCall::Resumable`. |
| **Network Polling as Task** | ✅ Fixed | `main.rs` spawns network stack polling as an async `Task`. |
| **DirCap Model** | ✅ Implemented | `sp_get_root()` returns a capability for `/`, and `sp_fs_open` requires a directory capability. |

---

## ❌ Critical Issues STILL Present

### 1. Root Capability Ambient Acquisition — *Medium Severity*

| Location | Issue |
|----------|-------|
| `src/kernel/src/wasm/host.rs:84-102` (`sp_get_root`) | **Any WASM module can call `sp_get_root()` and get unrestricted READ access to the root directory.** |

**Analysis**: While `sp_fs_open` now requires a DirCap, the `sp_get_root` function grants root access freely. In a true object-capability OS (seL4, Fuchsia), initial capabilities are granted **at spawn time** based on a manifest—not on-demand.

**Fix Priority**: 🔴 High  
**Recommendation**: Remove `sp_get_root`. Inject initial `CapId`s into `HostState` during `spawn_process()` based on a security policy/manifest.

---

### 2. WASM Async Host Call Integration Incomplete — *High Severity*

| Location | Issue |
|----------|-------|
| `src/kernel/src/wasm/host.rs` | All host functions (e.g., `sp_fs_read`, `sp_net_*`) are **synchronous**. They do not return `Poll::Pending`. |

**Analysis**: If a WASM module calls a host function that performs I/O (e.g., network recv waiting for packets), the **entire executor thread blocks**. The `WasmCallFuture` only handles WASM fuel exhaustion/yield traps—not host function blocking.

**Fix Priority**: 🔴 Critical  
**Recommendation**: Host functions that may block must:
1. Register a "pending" operation with the kernel.
2. Return a trap (`HostTrap::Sleep` or similar).
3. The executor resumes the WASM module when the operation completes.

This requires deep integration with `wasmi::ResumableCall` API.

---

### 3. Fuel Trap ≠ Yield — *Medium Severity*

| Location | Issue |
|----------|-------|
| `src/kernel/src/wasm/mod.rs:225-231` | When fuel is exhausted, `wasmi` returns `Err(TrapCode::FuelExhausted)`. The code returns `Poll::Ready(Err(e))` — **killing the task instead of resuming**. |

**Analysis**: True preemption requires that when fuel runs out, the task should **yield and be re-queued**—not terminate.

**Fix Priority**: 🟠 Medium  
**Recommendation**: Catch `TrapCode::FuelExhausted` specifically, refill fuel, and return `Poll::Pending` to resume later.

---

### 4. No Rights Degradation on DirCap → FileCap Transition — *Low Severity*

| Location | Issue |
|----------|-------|
| `src/kernel/src/wasm/host.rs:174-175` | When opening a file via `sp_fs_open`, the new capability always gets `READ | WRITE`. It ignores the parent DirCap's rights. |

**Analysis**: In capability security, derived capabilities should have **equal or fewer** rights than the parent. Currently, a `READ`-only DirCap can open a file and grant `WRITE` access—a **privilege escalation**.

**Fix Priority**: 🟡 Low (Design debt)  
**Recommendation**: Intersect parent rights with the operation type.

---

### 5. Global FS Lock (Acceptable for Now) — *Informational*

| Location | Issue |
|----------|-------|
| `src/kernel/src/fs/ramfs.rs` | Uses `RwLock` per-node (good), but handle table uses `Mutex`. |

**Analysis**: Fine for single-core. Will bottleneck on SMP.

**Fix Priority**: ⚪ Deferred (SMP).

---

## 📊 Conformance Matrix

| Design Spec Requirement | Implementation Status |
|------------------------|----------------------|
| **Object-Capability Model** | 🟡 Partial — `sp_get_root` bypasses policy |
| **Fuel-Based Preemption** | 🟡 Partial — fuel exhaust = kill, not yield |
| **Hierarchical FS** | ✅ Complete |
| **Generation Revocation** | ✅ Complete |
| **O(1) Cap Lookup** | ✅ Fixed (BTreeMap: O(log N), acceptable) |
| **Async Host Functions** | ❌ Not Implemented |
| **Network as Async Task** | ✅ Complete |

---

## 🏆 Is It "State of the Art"?

### Verdict: **No — Prototype/Research Grade**

| Metric | State-of-Art Reference | SovelmaOS |
|--------|----------------------|-----------|
| **Capability Discipline** | seL4, Fuchsia (strict grant at spawn) | 🟡 `sp_get_root` bypass |
| **Async I/O in Kernel** | Zephyr RTOS, Redox, Tock | ❌ Blocking host calls |
| **Preemption** | FreeRTOS, Zephyr (time-sliced) | 🟡 Fuel-based, trap=kill |
| **Formal Verification** | seL4 | ❌ None |

---

## 🛣️ Recommended Roadmap

| Priority | Task | Complexity | Impact |
|----------|------|------------|--------|
| **P0** | Remove `sp_get_root`; inject caps at spawn | Medium | Security |
| **P0** | Implement async host function pattern | High | Core functionality |
| **P1** | Handle `FuelExhausted` as yield, not kill | Low | Scheduler stability |
| **P2** | Rights degradation on cap derivation | Low | Security hardening |
| **P3** | Add integration tests for capability revocation | Medium | Verification |

---

## 🎯 NEXT THREE THINGS TO DO

Based on the analysis above, the **three highest-impact changes** to pursue next are:

### 1. 🔒 Implement Capability Injection at Spawn (Remove `sp_get_root`)

**What**: Modify `WasmEngine::spawn_process()` to accept a `Vec<Capability>` parameter representing the initial capabilities for the process. Remove the `sp_get_root` host function entirely.

**Why**: This eliminates the ambient authority violation and enforces true object-capability discipline where processes can only access resources explicitly granted to them.

**Files to modify**:
- `src/kernel/src/wasm/mod.rs` — Add `initial_caps` parameter to `spawn_process()`
- `src/kernel/src/wasm/host.rs` — Remove `sp_get_root` registration
- `src/kernel/src/main.rs` — Update spawn call sites

---

### 2. ⚡ Fix FuelExhausted to Yield Instead of Kill

**What**: In `WasmCallFuture` and `WasmTask`, catch `TrapCode::FuelExhausted` and return `Poll::Pending` after refilling fuel, instead of `Poll::Ready(Err(e))`.

**Why**: This enables true cooperative preemption. Currently, running out of fuel kills the WASM task, which defeats the purpose of fuel-based scheduling.

**Files to modify**:
- `src/kernel/src/wasm/mod.rs` — Update error handling in `poll()` implementations

---

### 3. 🧪 Add Integration Test for Capability Revocation

**What**: Create a test that:
1. Spawns a WASM process with a file capability
2. Revokes the capability mid-execution
3. Verifies subsequent access attempts fail with "generation mismatch" error

**Why**: The generation-counter revocation code exists but is **untested**. A regression here would be a critical security vulnerability.

**Files to create/modify**:
- `src/kernel/src/tests.rs` — Add `test_capability_revocation()`

---

## Summary

> **SovelmaOS has made substantial progress** since the initial critique. The capability-based file system with DirCap is correctly implemented, the FS is hierarchical, and the executor is async-aware. However, **two critical gaps remain**: the `sp_get_root` ambient authority bypass and the lack of truly async host functions. Until these are addressed, the project remains a **promising prototype**, not a production-grade secure microkernel.

---

*Good night! 🌙*
