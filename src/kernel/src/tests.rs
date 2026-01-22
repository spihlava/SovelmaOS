//! Kernel-level tests.
//!
//! These tests run during boot to verify core kernel functionality.

use crate::capability::{CapabilityTable, CapabilityType};
use crate::serial_println;
use alloc::boxed::Box;
use alloc::vec::Vec;

/// Runs all kernel tests.
///
/// Results are logged to serial output for debugging.
pub fn run_all() {
    serial_println!("[test] Running kernel tests...");

    test_allocation();
    test_capabilities();
    test_task_id();
    test_capability_generation_revocation();

    serial_println!("[test] All kernel tests passed!");
}

fn test_allocation() {
    serial_println!("[test] test_allocation... ");
    let x = Box::new(42);
    assert_eq!(*x, 42);

    let mut v = Vec::new();
    for i in 0..100 {
        v.push(i);
    }
    assert_eq!(v.len(), 100);
    assert_eq!(v[50], 50);
    serial_println!("[test] test_allocation... ok");
}

fn test_capabilities() {
    serial_println!("[test] test_capabilities... ");
    let mut table = CapabilityTable::new();
    let cap_type = CapabilityType::Serial { port: 0x3F8 };
    let id = table.grant(cap_type, Some(101));

    assert!(table.has_access(101, id));
    assert!(!table.has_access(102, id));

    table.revoke(id).expect("revoke failed");
    assert!(!table.has_access(101, id));
    serial_println!("[test] test_capabilities... ok");
}

fn test_task_id() {
    serial_println!("[test] test_task_id... ");
    // Uniqueness of task IDs is handled by the AtomicU64 in the task module.
    // If we get here, core initialization with atomics is working.
    serial_println!("[test] test_task_id... ok");
}

/// Test generation-based capability revocation in HostState.
///
/// This tests the core security mechanism: when a capability is revoked,
/// any subsequent access attempts using the old CapId should fail due to
/// generation mismatch.
fn test_capability_generation_revocation() {
    use crate::wasm::HostState;
    use sovelma_common::capability::{CapId, Capability, CapabilityRights};
    // Note: CapabilityType already imported at module level

    serial_println!("[test] test_capability_generation_revocation... ");

    let mut host_state = HostState::new();

    // Create a file capability
    let file_cap = Capability::new(
        CapabilityType::File(42),
        CapabilityRights::READ | CapabilityRights::WRITE,
    );
    let cap_id = file_cap.id;

    // Add capability - should be accessible
    host_state.add_capability(file_cap);
    assert!(
        host_state.get_capability(cap_id).is_some(),
        "Capability should be accessible after grant"
    );

    // Verify the capability has correct rights
    let cap = host_state.get_capability(cap_id).unwrap();
    assert!(cap.rights.contains(CapabilityRights::READ));
    assert!(cap.rights.contains(CapabilityRights::WRITE));

    // Revoke the capability
    host_state.revoke(cap_id);

    // After revocation, the capability should not be accessible
    assert!(
        host_state.get_capability(cap_id).is_none(),
        "Capability should not be accessible after revocation"
    );

    // Test: Create a new capability and verify generation validation works
    let new_cap = Capability::new(CapabilityType::File(100), CapabilityRights::READ);
    let new_cap_id = new_cap.id;
    host_state.add_capability(new_cap);

    // Fabricate a CapId with wrong generation
    let wrong_gen_id = CapId::new(new_cap_id.index(), new_cap_id.generation() + 1);

    // Access with wrong generation should fail
    assert!(
        host_state.get_capability(wrong_gen_id).is_none(),
        "Capability access with wrong generation should fail"
    );

    // Access with correct ID should succeed
    assert!(
        host_state.get_capability(new_cap_id).is_some(),
        "Capability access with correct generation should succeed"
    );

    serial_println!("[test] test_capability_generation_revocation... ok");
}
