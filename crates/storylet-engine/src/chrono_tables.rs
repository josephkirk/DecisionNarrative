//! Dense runtime tables (REQ-DO-01): storylet instances and scopes.
//!
//! The table is deliberately independent of the scheduler. It provides the
//! allocation, generation, free-list, and per-scope completion storage that a
//! scheduler can adopt without changing its execution semantics.

use crate::chrono_catalog::StoryletIndex;
use crate::chrono_completion::{ScopeCompletion, StoryletBitset};

pub const INVALID_INDEX: u32 = u32::MAX;
pub const MAX_INSTANCES: usize = 1024;
pub const MAX_SCOPES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstanceState {
    Pending,
    Active,
    Suspended,
    Completed,
    Aborted,
}

impl Default for InstanceState {
    fn default() -> Self {
        Self::Pending
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InstanceHandle {
    pub index: u32,
    pub generation: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TableError {
    CapacityExceeded { capacity: usize },
    StaleHandle { index: u32, generation: u16 },
    UnknownInstance { index: u32 },
    UnknownScope { index: u16 },
    ScopeCapacityExceeded,
}

impl std::fmt::Display for TableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CapacityExceeded { capacity } => {
                write!(f, "table capacity exceeded ({capacity})")
            }
            Self::StaleHandle { index, generation } => {
                write!(f, "stale instance handle {index} generation {generation}")
            }
            Self::UnknownInstance { index } => write!(f, "unknown instance {index}"),
            Self::UnknownScope { index } => write!(f, "unknown scope {index}"),
            Self::ScopeCapacityExceeded => write!(f, "scope capacity exceeded"),
        }
    }
}

impl std::error::Error for TableError {}

/// Structure-of-arrays instance table with generation-checked free-list reuse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoryletInstanceTable {
    pub storylet_index: Vec<StoryletIndex>,
    pub scope_index: Vec<u16>,
    pub state: Vec<InstanceState>,
    pub priority: Vec<i16>,
    pub started_tick: Vec<u32>,
    pub finished_tick: Vec<u32>,
    pub transaction_index: Vec<u32>,
    pub generation: Vec<u16>,
    next_free: Vec<u32>,
    free_head: u32,
    live_count: u32,
}

impl Default for StoryletInstanceTable {
    fn default() -> Self {
        Self::new()
    }
}

impl StoryletInstanceTable {
    pub fn new() -> Self {
        Self {
            storylet_index: Vec::new(),
            scope_index: Vec::new(),
            state: Vec::new(),
            priority: Vec::new(),
            started_tick: Vec::new(),
            finished_tick: Vec::new(),
            transaction_index: Vec::new(),
            generation: Vec::new(),
            next_free: Vec::new(),
            free_head: INVALID_INDEX,
            live_count: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.storylet_index.len()
    }

    pub fn live_count(&self) -> u32 {
        self.live_count
    }

    pub fn acquire(
        &mut self,
        storylet_index: StoryletIndex,
        scope_index: u16,
        priority: i16,
        tick: u32,
    ) -> Result<InstanceHandle, TableError> {
        let index = if self.free_head != INVALID_INDEX {
            let index = self.free_head;
            self.free_head = self.next_free[index as usize];
            index
        } else {
            if self.len() >= MAX_INSTANCES {
                return Err(TableError::CapacityExceeded {
                    capacity: MAX_INSTANCES,
                });
            }
            let index = self.len() as u32;
            self.storylet_index.push(storylet_index);
            self.scope_index.push(scope_index);
            self.state.push(InstanceState::Pending);
            self.priority.push(priority);
            self.started_tick.push(tick);
            self.finished_tick.push(0);
            self.transaction_index.push(INVALID_INDEX);
            self.generation.push(0);
            self.next_free.push(INVALID_INDEX);
            index
        };
        self.generation[index as usize] = self.generation[index as usize].wrapping_add(1);
        self.storylet_index[index as usize] = storylet_index;
        self.scope_index[index as usize] = scope_index;
        self.state[index as usize] = InstanceState::Pending;
        self.priority[index as usize] = priority;
        self.started_tick[index as usize] = tick;
        self.finished_tick[index as usize] = 0;
        self.transaction_index[index as usize] = INVALID_INDEX;
        self.live_count += 1;
        Ok(InstanceHandle {
            index,
            generation: self.generation[index as usize],
        })
    }

    pub fn is_live(&self, handle: InstanceHandle) -> bool {
        self.state.get(handle.index as usize).is_some_and(|state| {
            *state != InstanceState::Aborted
                && self.generation[handle.index as usize] == handle.generation
        })
    }

    pub fn release(&mut self, handle: InstanceHandle) -> Result<(), TableError> {
        if !self.is_live(handle) {
            return Err(TableError::StaleHandle {
                index: handle.index,
                generation: handle.generation,
            });
        }
        self.state[handle.index as usize] = InstanceState::Aborted;
        self.next_free[handle.index as usize] = self.free_head;
        self.free_head = handle.index;
        self.live_count = self
            .live_count
            .checked_sub(1)
            .expect("live_count underflow");
        Ok(())
    }

    pub fn iter_live(&self) -> impl Iterator<Item = (u32, &StoryletIndex, &InstanceState)> {
        self.storylet_index
            .iter()
            .zip(self.state.iter())
            .enumerate()
            .filter(|(_, (_, state))| **state != InstanceState::Aborted)
            .map(|(index, (storylet, state))| (index as u32, storylet, state))
    }
}

/// Scope rows. Completion bits are owned here; call history remains canonical.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScopeHandle {
    pub index: u16,
    pub generation: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScopeTable {
    pub rows: Vec<ScopeCompletion>,
    pub scope_kind: Vec<u8>,
    pub flags: Vec<u8>,
    pub history_start: Vec<u32>,
    pub history_len: Vec<u32>,
    pub active_instance_base: Vec<u32>,
    pub active_instance_len: Vec<u32>,
    pub generation: Vec<u16>,
    next_free: Vec<u32>,
    free_head: u32,
}

impl Default for ScopeTable {
    fn default() -> Self {
        Self::new()
    }
}

impl ScopeTable {
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            scope_kind: Vec::new(),
            flags: Vec::new(),
            history_start: Vec::new(),
            history_len: Vec::new(),
            active_instance_base: Vec::new(),
            active_instance_len: Vec::new(),
            generation: Vec::new(),
            next_free: Vec::new(),
            free_head: INVALID_INDEX,
        }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn create(
        &mut self,
        parent: Option<u16>,
        inherits: bool,
        scope_kind: u8,
    ) -> Result<ScopeHandle, TableError> {
        if parent.is_some_and(|parent| parent as usize >= self.rows.len()) {
            return Err(TableError::UnknownScope {
                index: parent.unwrap(),
            });
        }
        let index = if self.free_head != INVALID_INDEX {
            let index = self.free_head;
            self.free_head = self.next_free[index as usize];
            index
        } else {
            if self.rows.len() >= MAX_SCOPES {
                return Err(TableError::ScopeCapacityExceeded);
            }
            let index = self.rows.len() as u32;
            self.rows.push(ScopeCompletion::default());
            self.scope_kind.push(0);
            self.flags.push(0);
            self.history_start.push(0);
            self.history_len.push(0);
            self.active_instance_base.push(INVALID_INDEX);
            self.active_instance_len.push(0);
            self.generation.push(0);
            self.next_free.push(INVALID_INDEX);
            index
        };
        let index_u16 = index as u16;
        self.generation[index as usize] = self.generation[index as usize].wrapping_add(1);
        self.rows[index as usize] = ScopeCompletion {
            scope_index: index_u16,
            bits: StoryletBitset::new(),
            parent,
            inherits,
        };
        self.scope_kind[index as usize] = scope_kind;
        self.flags[index as usize] = u8::from(inherits);
        self.history_start[index as usize] = 0;
        self.history_len[index as usize] = 0;
        self.active_instance_base[index as usize] = INVALID_INDEX;
        self.active_instance_len[index as usize] = 0;
        Ok(ScopeHandle {
            index: index_u16,
            generation: self.generation[index as usize],
        })
    }

    pub fn is_live(&self, handle: ScopeHandle) -> bool {
        self.generation
            .get(handle.index as usize)
            .is_some_and(|generation| {
                *generation == handle.generation
                    && self
                        .flags
                        .get(handle.index as usize)
                        .is_some_and(|flag| *flag != u8::MAX)
            })
    }

    pub fn release(&mut self, handle: ScopeHandle) -> Result<(), TableError> {
        if !self.is_live(handle) {
            return Err(TableError::UnknownScope {
                index: handle.index,
            });
        }
        self.rows[handle.index as usize] = ScopeCompletion::default();
        self.flags[handle.index as usize] = u8::MAX; // released marker
        self.generation[handle.index as usize] =
            self.generation[handle.index as usize].wrapping_add(1);
        self.next_free[handle.index as usize] = self.free_head;
        self.free_head = handle.index as u32;
        Ok(())
    }

    pub fn get(&self, index: u16) -> Option<&ScopeCompletion> {
        self.rows.get(index as usize).filter(|_| {
            self.flags
                .get(index as usize)
                .is_some_and(|flag| *flag != u8::MAX)
        })
    }

    pub fn get_mut(&mut self, index: u16) -> Option<&mut ScopeCompletion> {
        self.rows.get_mut(index as usize).filter(|_| {
            self.flags
                .get(index as usize)
                .is_some_and(|flag| *flag != u8::MAX)
        })
    }

    /// Resolve the inherited view with direct parent indexing and a fixed-size
    /// visited buffer. No heap allocation and no linear parent scan.
    pub fn visible_bits(&self, index: u16) -> Result<StoryletBitset, TableError> {
        let scope = self.get(index).ok_or(TableError::UnknownScope { index })?;
        let mut bits = scope.bits;
        let mut current = scope;
        let mut seen = [false; MAX_SCOPES];
        while current.inherits {
            let parent = current.parent.ok_or(TableError::UnknownScope { index })?;
            let parent_usize = parent as usize;
            let parent_scope = self
                .get(parent)
                .ok_or(TableError::UnknownScope { index: parent })?;
            if seen[parent_usize] {
                return Err(TableError::UnknownScope { index: parent });
            }
            seen[parent_usize] = true;
            bits.union_with(&parent_scope.bits);
            current = parent_scope;
        }
        Ok(bits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_alloc_free_reuses_index_with_new_generation() {
        let mut table = StoryletInstanceTable::new();
        let first = table.acquire(1, 0, 0, 10).unwrap();
        let _second = table.acquire(2, 0, 0, 10).unwrap();
        assert_eq!(table.live_count(), 2);
        table.release(first).unwrap();
        assert!(!table.is_live(first));
        let third = table.acquire(3, 0, 0, 11).unwrap();
        assert_eq!(third.index, first.index);
        assert_ne!(third.generation, first.generation);
        assert_eq!(table.len(), 2);
        assert_eq!(table.live_count(), 2);
        assert_eq!(table.state[third.index as usize], InstanceState::Pending);
    }

    #[test]
    fn default_table_is_not_corrupt() {
        let mut table = StoryletInstanceTable::default();
        let handle = table.acquire(1, 0, 0, 0).unwrap();
        assert_eq!(handle.index, 0);
    }

    #[test]
    fn stale_handle_is_rejected() {
        let mut table = StoryletInstanceTable::new();
        let handle = table.acquire(1, 0, 0, 10).unwrap();
        table.release(handle).unwrap();
        assert_eq!(
            table.release(handle),
            Err(TableError::StaleHandle {
                index: handle.index,
                generation: handle.generation
            })
        );
    }

    #[test]
    fn scope_table_owns_completion_and_inheritance() {
        let mut table = ScopeTable::new();
        let parent = table.create(None, false, 0).unwrap();
        let child = table.create(Some(parent.index), true, 1).unwrap();
        table.get_mut(parent.index).unwrap().bits.set(2).unwrap();
        table.get_mut(child.index).unwrap().bits.set(4).unwrap();
        assert!(table.visible_bits(child.index).unwrap().contains(2));
        assert!(table.visible_bits(child.index).unwrap().contains(4));
        assert!(!table.get(child.index).unwrap().bits.contains(2));
        assert_eq!(
            table.create(Some(99), false, 0),
            Err(TableError::UnknownScope { index: 99 })
        );
        table.release(child).unwrap();
        assert!(table.get(child.index).is_none());
        let reused = table.create(None, false, 0).unwrap();
        assert_eq!(reused.index, child.index);
        assert_ne!(reused.generation, child.generation);
    }
}
