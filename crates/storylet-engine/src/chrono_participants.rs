//! Participants (who acts) and per-participant knowledge (who knows which fact)
//! for the ChronoFlow runtime.
//!
//! Both tables are structure-of-arrays or sorted flat rows, depend only on
//! `std` and `serde`, and perform no dispatch or I/O.

use serde::{Deserialize, Serialize};

/// Index of a participant in a `ParticipantTable`.
pub type PartIx = u16;
/// Index of a fact tag in the `GameplayTagRegistry` (tags under root `fact`).
pub type FactIx = u16;

/// How a participant came to hold a fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowKind {
    Evidence,
    Witnessed,
    Told,
    Rumor,
    Recollection,
    Testimony,
    Inference,
}

impl KnowKind {
    /// Rumors and testimony may be wrong; every other kind is confirmed.
    pub fn is_confirmed(self) -> bool {
        !matches!(self, KnowKind::Rumor | KnowKind::Testimony)
    }
}

/// Who drives a participant's actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Controller {
    Human { seat: u8 },
    Rule,
}

/// Structure-of-arrays participant table; all columns share one length.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ParticipantTable {
    pub keys: Vec<String>,
    pub entity: Vec<u16>,
    pub controller: Vec<Controller>,
    pub location: Vec<Option<u16>>,
}

impl ParticipantTable {
    /// The single human participant of a solo run. It has no backing entity,
    /// so `u16::MAX` marks "no entity".
    pub fn implicit_player(location: Option<u16>) -> Self {
        let mut table = Self::default();
        table.push("player", u16::MAX, Controller::Human { seat: 0 }, location);
        table
    }

    pub fn push(
        &mut self,
        key: &str,
        entity: u16,
        controller: Controller,
        location: Option<u16>,
    ) -> PartIx {
        let ix = self.keys.len() as PartIx;
        self.keys.push(key.to_string());
        self.entity.push(entity);
        self.controller.push(controller);
        self.location.push(location);
        ix
    }

    pub fn index_of(&self, key: &str) -> Option<PartIx> {
        self.keys.iter().position(|k| k == key).map(|i| i as PartIx)
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Human participants ordered by seat, ties broken by index.
    pub fn humans(&self) -> impl Iterator<Item = PartIx> + '_ {
        let mut humans: Vec<(u8, PartIx)> = self
            .controller
            .iter()
            .enumerate()
            .filter_map(|(i, c)| match c {
                Controller::Human { seat } => Some((*seat, i as PartIx)),
                Controller::Rule => None,
            })
            .collect();
        humans.sort_unstable();
        humans.into_iter().map(|(_, ix)| ix)
    }

    /// Participants sharing `p`'s `Some` location, excluding `p` itself.
    pub fn colocated(&self, p: PartIx) -> impl Iterator<Item = PartIx> + '_ {
        let here = self.location.get(p as usize).copied().flatten();
        self.location
            .iter()
            .enumerate()
            .filter(move |(i, loc)| here.is_some() && *i != p as usize && **loc == here)
            .map(|(i, _)| i as PartIx)
    }

    /// Moves `p` and returns its previous location.
    pub fn set_location(&mut self, p: PartIx, to: Option<u16>) -> Option<u16> {
        std::mem::replace(&mut self.location[p as usize], to)
    }
}

/// One fact held by one participant, with the in-world event that taught it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeRow {
    pub holder: PartIx,
    pub fact: FactIx,
    pub kind: KnowKind,
    pub source_event: u32,
}

/// Knowledge rows kept sorted by `(holder, fact)` for binary search.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeTable {
    pub rows: Vec<KnowledgeRow>,
}

impl KnowledgeTable {
    /// Records `row`. Returns `false` and changes nothing if the holder already
    /// knows the fact: the first source wins.
    pub fn learn(&mut self, row: KnowledgeRow) -> bool {
        match self.search(row.holder, row.fact) {
            Ok(_) => false,
            Err(at) => {
                self.rows.insert(at, row);
                true
            }
        }
    }

    pub fn knows(&self, holder: PartIx, fact: FactIx) -> Option<&KnowledgeRow> {
        self.search(holder, fact).ok().map(|i| &self.rows[i])
    }

    /// Removes a row. Used only when reverting an event.
    pub fn forget(&mut self, holder: PartIx, fact: FactIx) -> Option<KnowledgeRow> {
        self.search(holder, fact).ok().map(|i| self.rows.remove(i))
    }

    /// The contiguous rows of `holder`; empty if it knows nothing.
    pub fn rows_for(&self, holder: PartIx) -> &[KnowledgeRow] {
        let start = self.rows.partition_point(|r| r.holder < holder);
        let end = self.rows.partition_point(|r| r.holder <= holder);
        &self.rows[start..end]
    }

    fn search(&self, holder: PartIx, fact: FactIx) -> Result<usize, usize> {
        self.rows.binary_search_by_key(&(holder, fact), |r| (r.holder, r.fact))
    }
}
