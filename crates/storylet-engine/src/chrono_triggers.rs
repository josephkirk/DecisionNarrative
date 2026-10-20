//! Indexed trigger events and derived fired state (REQ-DO-05).
//!
//! The table is an index over content; the event queue is runtime state. Fired
//! state is derived from call history, not independent saved state.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::chrono_catalog::StoryletIndex;
use crate::chrono_runtime::CallHistoryEntry;

pub const MAX_TRIGGER_SUBSCRIPTIONS: usize = 8192;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TriggerError {
    CapacityExceeded,
}

impl std::fmt::Display for TriggerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CapacityExceeded => write!(f, "trigger subscription capacity exceeded"),
        }
    }
}

impl std::error::Error for TriggerError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum EventKind {
    Quality = 0,
    Tag = 1,
    Item = 2,
    StoryletCompleted = 3,
    StoryletUncompleted = 4,
    WorldStage = 5,
    NodeCompleted = 6,
    Custom = 7,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TriggerRow {
    pub event_kind: EventKind,
    pub event_key: u16,
    pub storylet_index: StoryletIndex,
    pub once: bool,
    pub priority: i16,
    pub predicate_index: Option<u16>,
    pub enabled: bool,
    pub scope_filter: Option<u16>,
    pub generation: u32,
    pub subscription_index: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TriggerTable {
    rows: Vec<TriggerRow>,
    buckets: BTreeMap<(u8, u16), Vec<u32>>,
}

impl TriggerTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn insert(
        &mut self,
        event_kind: EventKind,
        event_key: u16,
        storylet_index: StoryletIndex,
        once: bool,
        priority: i16,
        scope_filter: Option<u16>,
    ) -> Result<u32, TriggerError> {
        if self.rows.len() >= MAX_TRIGGER_SUBSCRIPTIONS {
            return Err(TriggerError::CapacityExceeded);
        }
        let subscription_index = self.rows.len() as u32;
        self.rows.push(TriggerRow {
            event_kind,
            event_key,
            storylet_index,
            once,
            priority,
            predicate_index: None,
            enabled: true,
            scope_filter,
            generation: 0,
            subscription_index,
        });
        let bucket = self
            .buckets
            .entry((event_kind as u8, event_key))
            .or_default();
        bucket.push(subscription_index);
        bucket.sort_by_key(|index| {
            let row = &self.rows[*index as usize];
            (
                std::cmp::Reverse(row.priority),
                row.storylet_index,
                row.subscription_index,
            )
        });
        Ok(subscription_index)
    }

    pub fn candidates(&self, event_kind: EventKind, event_key: u16) -> &[u32] {
        self.buckets
            .get(&(event_kind as u8, event_key))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn row(&self, subscription_index: u32) -> Option<&TriggerRow> {
        self.rows.get(subscription_index as usize)
    }

    pub fn set_enabled(&mut self, subscription_index: u32, enabled: bool) -> bool {
        self.rows
            .get_mut(subscription_index as usize)
            .is_some_and(|row| {
                row.enabled = enabled;
                true
            })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TriggerEvent {
    pub kind: EventKind,
    pub key: u16,
    pub source_scope: u16,
    pub source_instance: u32,
    pub old_value: f32,
    pub new_value: f32,
    pub tick: u32,
    pub sequence: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EventQueue {
    events: VecDeque<TriggerEvent>,
    next_sequence: u32,
}

impl EventQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, kind: EventKind, key: u16, source_scope: u16) -> u32 {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        self.events.push_back(TriggerEvent {
            kind,
            key,
            source_scope,
            source_instance: u32::MAX,
            old_value: 0.0,
            new_value: 0.0,
            tick: 0,
            sequence,
        });
        sequence
    }

    pub fn pop(&mut self) -> Option<TriggerEvent> {
        self.events.pop_front()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

/// Per-session fired bits, derived from call history. A trigger-caused start
/// records its subscription index in the call entry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TriggerFireTable {
    fired: BTreeSet<(u16, u32)>,
}

impl TriggerFireTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mark(&mut self, scope: u16, subscription_index: u32) {
        self.fired.insert((scope, subscription_index));
    }

    pub fn is_fired(&self, scope: u16, subscription_index: u32) -> bool {
        self.fired.contains(&(scope, subscription_index))
    }

    pub fn clear_scope(&mut self, scope: u16) {
        self.fired.retain(|(entry_scope, _)| *entry_scope != scope);
    }

    pub fn len(&self) -> usize {
        self.fired.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fired.is_empty()
    }

    /// Runtime convenience: rebuild directly from canonical commit entries.
    pub fn rebuild_history(entries: &[CallHistoryEntry], scope: u16) -> Self {
        let mut table = Self::new();
        for entry in entries {
            if entry.scope_index == scope {
                if let Some(subscription) = entry.trigger_subscription {
                    table.mark(scope, subscription);
                }
            }
        }
        table
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_are_ordered_and_lookup_is_indexed() {
        let mut table = TriggerTable::new();
        let a = table.insert(EventKind::Tag, 3, 9, false, 1, None).unwrap();
        let b = table.insert(EventKind::Tag, 3, 4, false, 1, None).unwrap();
        let c = table.insert(EventKind::Tag, 3, 4, false, 2, None).unwrap();
        assert_eq!(table.candidates(EventKind::Tag, 3), &[c, b, a]);
        assert!(table.candidates(EventKind::Tag, 4).is_empty());
        assert_eq!(table.row(a).unwrap().storylet_index, 9);
    }

    #[test]
    fn queue_is_fifo_and_sequence_is_monotonic() {
        let mut queue = EventQueue::new();
        let first = queue.push(EventKind::Item, 1, 0);
        let second = queue.push(EventKind::Item, 2, 0);
        assert_eq!(first, 0);
        assert_eq!(second, 1);
        assert_eq!(queue.pop().unwrap().key, 1);
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn fired_state_rebuilds_from_history_and_clears_with_scope() {
        let entries = vec![
            CallHistoryEntry {
                trigger_subscription: Some(7),
                completed: true,
                ..CallHistoryEntry::default()
            },
            CallHistoryEntry {
                ..CallHistoryEntry::default()
            },
        ];
        let table = TriggerFireTable::rebuild_history(&entries, 0);
        assert!(table.is_fired(0, 7));
        assert!(!table.is_fired(1, 7));
        let mut table = table;
        table.clear_scope(0);
        assert!(table.is_empty());
    }

}
