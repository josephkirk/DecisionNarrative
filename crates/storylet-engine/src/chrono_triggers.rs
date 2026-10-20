//! Indexed trigger events and derived fired state (REQ-DO-05).
//!
//! The table is an index over content; the event queue is runtime state. Fired
//! state is derived from call history, not independent saved state.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::chrono_catalog::StoryletIndex;
use crate::chrono_runtime::CallHistoryEntry;

pub const MAX_TRIGGER_SUBSCRIPTIONS: usize = 8192;
pub const EVENT_KIND_COUNT: usize = 8;
pub const EVENT_KEY_COUNT: usize = 65_536;
const BUCKET_COUNT: usize = EVENT_KIND_COUNT * EVENT_KEY_COUNT;

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
    pub stable_key: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TriggerTable {
    rows: Vec<TriggerRow>,
    bucket_offsets: Vec<u32>,
    bucket_entries: Vec<u32>,
    built: bool,
    stable_keys: BTreeMap<String, u32>,
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
        self.insert_inner(
            event_kind,
            event_key,
            storylet_index,
            once,
            priority,
            scope_filter,
            None,
        )
    }

    pub fn insert_keyed(
        &mut self,
        stable_key: impl Into<String>,
        event_kind: EventKind,
        event_key: u16,
        storylet_index: StoryletIndex,
        once: bool,
        priority: i16,
        scope_filter: Option<u16>,
    ) -> Result<u32, TriggerError> {
        self.insert_inner(
            event_kind,
            event_key,
            storylet_index,
            once,
            priority,
            scope_filter,
            Some(stable_key.into()),
        )
    }

    fn insert_inner(
        &mut self,
        event_kind: EventKind,
        event_key: u16,
        storylet_index: StoryletIndex,
        once: bool,
        priority: i16,
        scope_filter: Option<u16>,
        stable_key: Option<String>,
    ) -> Result<u32, TriggerError> {
        if self.rows.len() >= MAX_TRIGGER_SUBSCRIPTIONS {
            return Err(TriggerError::CapacityExceeded);
        }
        if let Some(key) = &stable_key {
            if self.stable_keys.contains_key(key) {
                return Err(TriggerError::CapacityExceeded);
            }
        }
        let subscription_index = self.rows.len() as u32;
        if let Some(key) = &stable_key {
            self.stable_keys.insert(key.clone(), subscription_index);
        }
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
            stable_key,
        });
        self.built = false;
        Ok(subscription_index)
    }

    pub fn resolve_key(&self, stable_key: &str) -> Option<u32> {
        self.stable_keys.get(stable_key).copied()
    }

    /// Build the CSR buckets once after load. Sorting is deterministic.
    pub fn build(&mut self) {
        let mut order: Vec<u32> = (0..self.rows.len() as u32).collect();
        order.sort_by_key(|index| {
            let row = &self.rows[*index as usize];
            (
                row.event_kind as u8,
                row.event_key,
                std::cmp::Reverse(row.priority),
                row.storylet_index,
                row.subscription_index,
            )
        });
        self.bucket_entries = order;
        self.bucket_offsets = vec![0; BUCKET_COUNT + 1];
        for entry in &self.bucket_entries {
            let row = &self.rows[*entry as usize];
            let bucket = row.event_kind as usize * EVENT_KEY_COUNT + row.event_key as usize;
            self.bucket_offsets[bucket + 1] += 1;
        }
        for index in 1..self.bucket_offsets.len() {
            self.bucket_offsets[index] += self.bucket_offsets[index - 1];
        }
        self.built = true;
    }

    pub fn candidates(&self, event_kind: EventKind, event_key: u16) -> &[u32] {
        if !self.built {
            return &[];
        }
        let bucket = event_kind as usize * EVENT_KEY_COUNT + event_key as usize;
        let start = self.bucket_offsets[bucket] as usize;
        let end = self.bucket_offsets[bucket + 1] as usize;
        &self.bucket_entries[start..end]
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

    pub fn peek(&self) -> Option<&TriggerEvent> {
        self.events.front()
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TriggerTickReport {
    pub events_processed: u32,
    pub starts: u32,
    pub capped: bool,
}

impl TriggerTable {
    /// Deterministic event processing. The caller owns instance creation; the
    /// callback returns whether a start actually occurred.
    pub fn process_tick(
        &self,
        queue: &mut EventQueue,
        fired: &mut TriggerFireTable,
        active: &BTreeSet<(u16, StoryletIndex)>,
        max_starts: u32,
        mut start: impl FnMut(u16, StoryletIndex) -> bool,
    ) -> TriggerTickReport {
        let mut report = TriggerTickReport::default();
        let mut started = active.clone();
        while let Some(event) = queue.peek().cloned() {
            let candidates = self.candidates(event.kind, event.key);
            let mut distinct = BTreeSet::new();
            for subscription in candidates {
                let row = &self.rows[*subscription as usize];
                if row.enabled
                    && row.scope_filter.is_none_or(|scope| scope == event.source_scope)
                    && !(row.once && fired.is_fired(event.source_scope, row.subscription_index))
                    && !started.contains(&(event.source_scope, row.storylet_index))
                {
                    distinct.insert((event.source_scope, row.storylet_index));
                }
            }
            if report.starts.saturating_add(distinct.len() as u32) > max_starts {
                report.capped = true;
                return report;
            }
            queue.pop();
            report.events_processed += 1;
            for subscription in candidates {
                let row = &self.rows[*subscription as usize];
                if !row.enabled {
                    continue;
                }
                if let Some(scope) = row.scope_filter
                    && scope != event.source_scope
                {
                    continue;
                }
                if row.once && fired.is_fired(event.source_scope, row.subscription_index) {
                    continue;
                }
                let key = (event.source_scope, row.storylet_index);
                if started.contains(&key) {
                    continue;
                }
                if start(event.source_scope, row.storylet_index) {
                    report.starts += 1;
                    started.insert(key);
                    if row.once {
                        fired.mark(event.source_scope, row.subscription_index);
                    }
                }
            }
        }
        report
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
        table.build();
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
    fn stable_trigger_keys_resolve_to_subscription() {
        let mut table = TriggerTable::new();
        let index = table
            .insert_keyed("trigger.a", EventKind::Tag, 3, 9, false, 1, None)
            .unwrap();
        assert_eq!(table.resolve_key("trigger.a"), Some(index));
        assert_eq!(table.resolve_key("missing"), None);
        table.build();
        assert_eq!(table.candidates(EventKind::Tag, 3), &[index]);
    }

    #[test]
    fn process_tick_keeps_events_when_cap_is_zero() {
        let mut table = TriggerTable::new();
        table.insert(EventKind::Tag, 1, 5, false, 1, None).unwrap();
        table.build();
        let mut queue = EventQueue::new();
        queue.push(EventKind::Tag, 1, 0);
        let mut fired = TriggerFireTable::new();
        let report = table.process_tick(&mut queue, &mut fired, &BTreeSet::new(), 0, |_, _| true);
        assert!(report.capped);
        assert_eq!(queue.len(), 1);
        assert_eq!(report.starts, 0);
    }

    #[test]
    fn process_tick_deduplicates_same_storylet_in_one_event() {
        let mut table = TriggerTable::new();
        table.insert(EventKind::Tag, 1, 5, false, 1, None).unwrap();
        table.insert(EventKind::Tag, 1, 5, false, 0, None).unwrap();
        table.build();
        let mut queue = EventQueue::new();
        queue.push(EventKind::Tag, 1, 0);
        let mut fired = TriggerFireTable::new();
        let mut starts = 0;
        table.process_tick(&mut queue, &mut fired, &BTreeSet::new(), 8, |_, _| {
            starts += 1;
            true
        });
        assert_eq!(starts, 1);
    }

    #[test]
    fn process_tick_does_not_livelock_on_duplicate_storylet_at_cap() {
        let mut table = TriggerTable::new();
        table.insert(EventKind::Tag, 1, 5, false, 1, None).unwrap();
        table.insert(EventKind::Tag, 1, 5, false, 0, None).unwrap();
        table.build();
        let mut queue = EventQueue::new();
        queue.push(EventKind::Tag, 1, 0);
        let mut fired = TriggerFireTable::new();
        let mut starts = 0;
        let report = table.process_tick(&mut queue, &mut fired, &BTreeSet::new(), 1, |_, _| {
            starts += 1;
            true
        });
        assert_eq!(starts, 1);
        assert!(!report.capped);
        assert!(queue.is_empty());
    }

    #[test]
    fn process_tick_respects_once_scope_active_and_cap() {
        let mut table = TriggerTable::new();
        table.insert(EventKind::Tag, 1, 5, true, 1, Some(0)).unwrap();
        table.insert(EventKind::Tag, 1, 6, false, 1, Some(1)).unwrap();
        table.build();
        let mut queue = EventQueue::new();
        queue.push(EventKind::Tag, 1, 0);
        queue.push(EventKind::Tag, 1, 0);
        let mut fired = TriggerFireTable::new();
        let mut starts = Vec::new();
        let report = table.process_tick(&mut queue, &mut fired, &BTreeSet::new(), 8, |scope, id| {
            starts.push((scope, id));
            true
        });
        assert_eq!(starts, vec![(0, 5)]);
        assert!(fired.is_fired(0, 0));
        assert_eq!(report.starts, 1);
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
