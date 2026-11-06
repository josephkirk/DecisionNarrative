//! The canon chronicle (what happened, in story-time order) and the per-holder
//! telling table (the order in which each participant was told it).
//!
//! The chronicle is immutable content; the telling table is world state and is
//! reversible through `BijectiveDelta::tellings_added`. Telling order is free:
//! it is the append order of `TellingTable::rows`, independent of story time.

use crate::chrono_participants::{FactIx, PartIx};
use serde::{Deserialize, Serialize};

/// Index of an entry in a `ChronicleTable`.
pub type ChronIx = u16;

/// One canon event.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChronicleEntry {
    pub key: String,
    pub story_time: i64,
    pub label: String,
    pub location: Option<u16>,
    pub facts: Vec<FactIx>,
    pub text: String,
}

/// Canon entries sorted by `(story_time, key)`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChronicleTable {
    pub entries: Vec<ChronicleEntry>,
}

impl ChronicleTable {
    /// Sorts `entries` into canonical order.
    pub fn new(mut entries: Vec<ChronicleEntry>) -> Self {
        entries.sort_by(|a, b| (a.story_time, &a.key).cmp(&(b.story_time, &b.key)));
        Self { entries }
    }

    pub fn get(&self, entry: ChronIx) -> Option<&ChronicleEntry> {
        self.entries.get(entry as usize)
    }

    pub fn index_of(&self, key: &str) -> Option<ChronIx> {
        self.entries.iter().position(|e| e.key == key).map(|i| i as ChronIx)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// `holder` was told `entry` by the event at `source_event`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TellingRow {
    pub holder: PartIx,
    pub entry: ChronIx,
    pub source_event: u32,
}

/// Telling rows in append order; at most one row per `(holder, entry)`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TellingTable {
    pub rows: Vec<TellingRow>,
}

impl TellingTable {
    /// Records `row`. Returns `false` and changes nothing if the holder was
    /// already told this entry.
    pub fn reveal(&mut self, row: TellingRow) -> bool {
        if self.is_told(row.holder, row.entry) {
            return false;
        }
        self.rows.push(row);
        true
    }

    pub fn is_told(&self, holder: PartIx, entry: ChronIx) -> bool {
        self.rows.iter().any(|r| r.holder == holder && r.entry == entry)
    }

    /// Entries `holder` was told, in the order they were told.
    pub fn order_for(&self, holder: PartIx) -> Vec<ChronIx> {
        self.rows.iter().filter(|r| r.holder == holder).map(|r| r.entry).collect()
    }

    /// 1-based position of `entry` in `holder`'s telling order.
    pub fn told_number(&self, holder: PartIx, entry: ChronIx) -> Option<u32> {
        self.rows
            .iter()
            .filter(|r| r.holder == holder)
            .position(|r| r.entry == entry)
            .map(|i| i as u32 + 1)
    }

    /// Removes a row. Used only when reverting an event.
    pub fn forget(&mut self, holder: PartIx, entry: ChronIx) -> Option<TellingRow> {
        let at = self.rows.iter().rposition(|r| r.holder == holder && r.entry == entry)?;
        Some(self.rows.remove(at))
    }
}
