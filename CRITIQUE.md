# Ruthless Critique & Audit: SovelmaOS

## 1. Executive Summary
While SovelmaOS has established a functional baseline (WASM runtime, basic scheduler, in-memory FS), it currently **fails** to meet the rigorous standards of a "State of the Art" secure microkernel. The primary deficiencies are in **Capability Discipline** (Ambient Authority violates the security model) and **Runtime Efficiency** (Linear lookups, blocking structures).

## 2. Capability System (Security)
**Rating: Critical Failure**

### 2.1 Ambient Authority Violation
- **Current**: `sp_fs_open(path)` allows potentially any WASM process to open *any* file by string path.
- **Critique**: This violates the fundamental "Object-Capability" principle. A process should only be able to open a file if it possesses a **Directory Capability** for the parent folder.
- **Reference**: Fuchsia, seL4, CloudABI.
- **Fix Estimate**: High. Requires refactoring `sp_fs_open` to `sp_fs_open_at(dir_cap, path)` and implementing Directory Capabilities.

### 2.2 Inefficient Lookup
- **Current**: [HostState](file:///c:/Users/Sakari/Projects/SovelmaOS/src/kernel/src/wasm/host.rs#11-15) stores `capabilities: Vec<CapId>`. Access checks are linear $O(N)$.
- **Critique**: Unacceptable for an OS kernel. As capability sets grow (files, sockets, ports), performance will degrade.
- **Fix Estimate**: Medium. Replace `Vec` with a `SlotMap` or `HashMap` for $O(1)$ lookup.

### 2.3 Weak Revocation
- **Current**: `Generation` counters exist in struct but are unchecked during access.
- **Critique**: Security theater. Revocation primitives must be enforced at the lookup/access point.
- **Fix Estimate**: Medium. Integrate generation checks into the `get_capability` helper.

## 3. WASM Runtime & Preemption
**Rating: Major Issues**

### 3.1 Synchronous Host Calls
- **Current**: [wasm/mod.rs](file:///c:/Users/Sakari/Projects/SovelmaOS/src/kernel/src/wasm/mod.rs) structure implies that [call()](file:///c:/Users/Sakari/Projects/SovelmaOS/src/kernel/src/wasm/mod.rs#88-104) runs until completion or fuel exhaust.
- **Critique**: If a WASM module calls a host function that blocks (e.g., `sp_net_recv` waiting for packet), the **entire kernel thread blocks**. This defeats the purpose of the async executor.
- **Fix Estimate**: High. Host functions must return `Poll::Pending` equivalent, requiring full integration with `wasmi`'s `ResumableCall` API and logic to suspend the [WasmProcess](file:///c:/Users/Sakari/Projects/SovelmaOS/src/kernel/src/wasm/mod.rs#82-86).

### 3.2 Fuel Handling
- **Current**: Fuel is added, but the "Trapping" mechanism forces a hard stop.
- **Critique**: Need a smooth "Yield" interrupt that doesn't just trap-and-fail but trap-and-resume.
- **Fix Estimate**: Medium.

## 4. Scheduling & Concurrency
**Rating: Moderate**

### 4.1 Global Main Loop
- **Current**: `executor.run()` is called, but network polling (`net_stack.poll()`) happens manually in [main.rs](file:///c:/Users/Sakari/Projects/SovelmaOS/src/kernel/src/main.rs) loop.
- **Critique**: The network stack is effectively treated as a background task but coupled to the main loop. It should be an async task or interrupt-driven.
- **Fix Estimate**: Low. Wrap network polling in a [Task](file:///c:/Users/Sakari/Projects/SovelmaOS/src/kernel/src/task/mod.rs#37-42).

### 4.2 Global Lock Contention
- **Current**: `ROOT_FS` is a global `SpinMutex`.
- **Critique**: In a single-core environment (current target), this is acceptable. For SMP, this is a bottleneck. "State of the Art" demands fine-grained locking or lock-free structures.
- **Fix Estimate**: Low (Deferred until SMP).

## 5. Filesystem
**Rating: Minimal**

### 5.1 Lack of Hierarchy
- **Current**: [RamFs](file:///c:/Users/Sakari/Projects/SovelmaOS/src/kernel/src/fs/ramfs.rs#11-15) is a flat map `String -> Vec<u8>`.
- **Critique**: Toy implementation. Real OS needs directory hierarchy to support the Capability model (DirCaps).
- **Fix Estimate**: Medium. Implement Tree structure.

## 6. Recommendations & Roadmap

| Priority | Component | Action | Complexity |
| :--- | :--- | :--- | :--- |
| **1 (Critical)** | **Security** | **Ban Ambient Authority**: Implement `DirCap` and `open_at`. | High |
| **2 (Critical)** | **WASM** | **Async Host Calls**: Implement resumable calls for network/timers. | High |
| **3 (Major)** | **Performance** | **O(1) Capabilities**: Switch `Vec` to `SlotMap`. | Low |
| **4 (Major)** | **Filesystem** | **Hierarchical FS**: Implement Directories. | Medium |
| **5 (Minor)** | **Network** | **Async Polling**: Move net stack to Executor. | Low |

### Verdict
The current implementation is a **prototype**, not yet a "product". To verify "State of the Art" status, we must forcefully implement the **Capability Discipline** (Priority 1) immediately.
