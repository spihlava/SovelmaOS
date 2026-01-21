
use core::sync::atomic::{AtomicU32, Ordering};
use bitflags::bitflags;

/// A unique identifier for a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CapId(u32);

impl CapId {
    /// Generate the next unique CapId.
    pub fn next() -> Self {
        static NEXT_ID: AtomicU32 = AtomicU32::new(1);
        CapId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

bitflags! {
    /// Permissions granted by a capability.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct CapabilityRights: u32 {
        const READ      = 1 << 0;
        const WRITE     = 1 << 1;
        const EXECUTE   = 1 << 2;
        const GRANT     = 1 << 3; // Ability to share this cap
        const CALL      = 1 << 4; // Ability to invoke (for HostFunctions/IPC)
    }
}

/// A capability guarding a resource with specific rights.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capability {
    /// The ID of this specific capability instance.
    pub id: CapId,
    /// The rights granted by this capability.
    pub rights: CapabilityRights,
    /// The underlying resource type.
    pub object: CapabilityType,
    /// Generation counter for revocation (0 = initial).
    pub generation: u64,
}

impl Capability {
    pub fn new(object: CapabilityType, rights: CapabilityRights) -> Self {
        Self {
            id: CapId::next(),
            rights,
            object,
            generation: 0,
        }
    }
}

/// The type of resource a capability grants access to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityType {
    /// Access to a range of physical memory.
    Memory {
        /// Start address of the memory range.
        start: usize,
        /// Size of the memory range in bytes.
        size: usize,
    },
    /// Access to a specific serial port.
    Serial {
        /// The I/O port address.
        port: u16,
    },
    /// Access to the system timer.
    Timer,
    /// Access to a specific hardware interrupt.
    Interrupt {
        /// The IRQ number.
        irq: u8,
    },
    /// Net socket
    Network(u32),
}

