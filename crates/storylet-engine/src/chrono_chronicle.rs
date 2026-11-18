
use crate::chrono_participants::{FactIx, PartIx};
use serde::{Deserialize, Serialize};

pub type ChronIx = u16;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChronicleEntry {
    pub key: String,
    pub story_time: i64,
    pub label: String,
    pub location: Option<u16>,
    pub facts: Vec<FactIx>,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChronicleTable {
    pub entries: Vec<ChronicleEntry>,
}

impl ChronicleTable {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TellingRow {
    pub holder: PartIx,
    pub entry: ChronIx,
    pub source_event: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TellingTable {
    pub rows: Vec<TellingRow>,
}

impl TellingTable {
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

    pub fn order_for(&self, holder: PartIx) -> Vec<ChronIx> {
        self.rows.iter().filter(|r| r.holder == holder).map(|r| r.entry).collect()
    }

    pub fn told_number(&self, holder: PartIx, entry: ChronIx) -> Option<u32> {
        self.rows
            .iter()
            .filter(|r| r.holder == holder)
            .position(|r| r.entry == entry)
            .map(|i| i as u32 + 1)
    }

    pub fn forget(&mut self, holder: PartIx, entry: ChronIx) -> Option<TellingRow> {
        let at = self.rows.iter().rposition(|r| r.holder == holder && r.entry == entry)?;
        Some(self.rows.remove(at))
    }
}
