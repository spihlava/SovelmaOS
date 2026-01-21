//! Capability-based security system.

use alloc::collections::BTreeMap;

pub use sovelma_common::capability::{CapId, CapabilityType};

/// A capability token that grants access to a resource.
#[derive(Debug, Clone)]
pub struct Capability {
    /// Unique identifier for this capability.
    pub id: CapId,
    /// The resource this capability grants access to.
    pub resource: CapabilityType,
    /// The ID of the task that owns this capability.
    pub owner_task_id: Option<u64>, // TaskId from task module
}

/// A table that stores and manages capabilities for the system.
pub struct CapabilityTable {
    caps: BTreeMap<CapId, Capability>,
}

impl Default for CapabilityTable {
    fn default() -> Self {
        Self::new()
    }
}

impl CapabilityTable {
    /// Create a new, empty capability table.
    pub fn new() -> Self {
        CapabilityTable {
            caps: BTreeMap::new(),
        }
    }

    /// Grant a new capability.
    pub fn grant(&mut self, resource: CapabilityType, owner_task_id: Option<u64>) -> CapId {
        let id = CapId::next();
        let cap = Capability {
            id,
            resource,
            owner_task_id,
        };
        self.caps.insert(id, cap);
        id
    }

    /// Revoke a capability.
    pub fn revoke(&mut self, id: CapId) -> Result<(), CapError> {
        if self.caps.remove(&id).is_some() {
            Ok(())
        } else {
            Err(CapError::NotFound)
        }
    }

    /// Check if a task has access to a capability.
    pub fn has_access(&self, task_id: u64, cap_id: CapId) -> bool {
        if let Some(cap) = self.caps.get(&cap_id) {
            cap.owner_task_id == Some(task_id)
        } else {
            false
        }
    }
}

/// Errors related to capability management.
#[derive(Debug)]
pub enum CapError {
    /// The specified capability was not found.
    NotFound,
    /// Permission was denied for the requested operation.
    PermissionDenied,
}
