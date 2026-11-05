//! Mapping between Java wire handles and arena handles.
//!
//! Wire handles start at `0x7E0000` and are reset by `TC_RESET`; arena handles
//! stay monotonic so that references decoded after a reset cannot alias older
//! objects.

use crate::value::Handle;
use std::collections::HashMap;

/// First Java wire handle.
pub const BASE_WIRE_HANDLE: u32 = 0x7E_0000;

/// Wire handle table for one stream segment.
#[derive(Debug)]
pub struct WireHandles {
    by_wire: HashMap<u32, Handle>,
    next: u32,
}

impl WireHandles {
    /// Empty table starting at `BASE_WIRE_HANDLE`.
    pub fn new() -> Self {
        Self {
            by_wire: HashMap::new(),
            next: BASE_WIRE_HANDLE,
        }
    }

    /// Assign the next wire handle to an arena value and return it.
    pub fn assign(&mut self, handle: Handle) -> u32 {
        let wire = self.next;
        self.next += 1;
        self.by_wire.insert(wire, handle);
        wire
    }

    /// Look up a `TC_REFERENCE` target.
    pub fn get(&self, wire: u32) -> Option<Handle> {
        self.by_wire.get(&wire).copied()
    }

    /// Drop all mappings; `TC_RESET` keeps arena values alive.
    pub fn reset(&mut self) {
        self.by_wire.clear();
        self.next = BASE_WIRE_HANDLE;
    }
}

impl Default for WireHandles {
    fn default() -> Self {
        Self::new()
    }
}
