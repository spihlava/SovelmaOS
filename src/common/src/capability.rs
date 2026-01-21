use bitflags::bitflags;
use core::sync::atomic::{AtomicU32, Ordering};

/// A unique identifier for a capability, including a generation for revocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CapId {
    index: u32,
    generation: u32,
}

impl CapId {
    /// Create from components.
    pub fn new(index: u32, generation: u32) -> Self {
        CapId { index, generation }
    }

    /// Create from raw u64.
    pub fn from_u64(val: u64) -> Self {
        CapId {
            index: (val & 0xFFFFFFFF) as u32,
            generation: (val >> 32) as u32,
        }
    }

    /// Get raw u64.
    pub fn as_u64(&self) -> u64 {
        ((self.generation as u64) << 32) | (self.index as u64)
    }

    /// Get index.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Get generation.
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Generate the next unique CapId with generation 0.
    pub fn next() -> Self {
        static NEXT_INDEX: AtomicU32 = AtomicU32::new(100); // Start high to avoid collision with small manual ones
        CapId::new(NEXT_INDEX.fetch_add(1, Ordering::Relaxed), 0)
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
        static NEXT_INDEX: AtomicU32 = AtomicU32::new(1);
        Self {
            id: CapId::new(NEXT_INDEX.fetch_add(1, Ordering::Relaxed), 0),
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
    /// Filesystem Directory (handle)
    Directory(u64),
    /// Open File (handle)
    File(u64),
}
