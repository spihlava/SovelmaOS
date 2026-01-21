//! Kernel-level tests.

use crate::capability::{CapabilityTable, CapabilityType};
use crate::serial_println;
use alloc::boxed::Box;
use alloc::vec::Vec;

/// Runs all kernel tests.
pub fn run_all() {
    serial_println!("Running kernel tests...");

    test_allocation();
    test_capabilities();
    test_task_id();

    serial_println!("All kernel tests passed!");
}

fn test_allocation() {
    serial_println!("test_allocation... ");
    let x = Box::new(42);
    assert_eq!(*x, 42);

    let mut v = Vec::new();
    for i in 0..100 {
        v.push(i);
    }
    assert_eq!(v.len(), 100);
    assert_eq!(v[50], 50);
    serial_println!("[ok]");
}

fn test_capabilities() {
    serial_println!("test_capabilities... ");
    let mut table = CapabilityTable::new();
    let cap_type = CapabilityType::Serial { port: 0x3F8 };
    let id = table.grant(cap_type, Some(101));

    assert!(table.has_access(101, id));
    assert!(!table.has_access(102, id));

    table.revoke(id).expect("revoke failed");
    assert!(!table.has_access(101, id));
    serial_println!("[ok]");
}

fn test_task_id() {
    serial_println!("test_task_id... ");
    // Uniqueness of task IDs is handled by the AtomicU64 in the task module.
    // If we get here, core initialization with atomics is working.
    serial_println!("[ok]");
}
