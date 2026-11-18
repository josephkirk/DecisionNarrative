
use serde::{Deserialize, Serialize};

pub type PartIx = u16;
pub type FactIx = u16;

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
    pub fn is_confirmed(self) -> bool {
        !matches!(self, KnowKind::Rumor | KnowKind::Testimony)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Controller {
    Human { seat: u8 },
    Rule,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ParticipantTable {
    pub keys: Vec<String>,
    pub entity: Vec<u16>,
    pub controller: Vec<Controller>,
    pub location: Vec<Option<u16>>,
}

impl ParticipantTable {
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

    pub fn colocated(&self, p: PartIx) -> impl Iterator<Item = PartIx> + '_ {
        let here = self.location.get(p as usize).copied().flatten();
        self.location
            .iter()
            .enumerate()
            .filter(move |(i, loc)| here.is_some() && *i != p as usize && **loc == here)
            .map(|(i, _)| i as PartIx)
    }

    pub fn set_location(&mut self, p: PartIx, to: Option<u16>) -> Option<u16> {
        std::mem::replace(&mut self.location[p as usize], to)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeRow {
    pub holder: PartIx,
    pub fact: FactIx,
    pub kind: KnowKind,
    pub source_event: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeTable {
    pub rows: Vec<KnowledgeRow>,
}

impl KnowledgeTable {
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

    pub fn forget(&mut self, holder: PartIx, fact: FactIx) -> Option<KnowledgeRow> {
        self.search(holder, fact).ok().map(|i| self.rows.remove(i))
    }

    pub fn rows_for(&self, holder: PartIx) -> &[KnowledgeRow] {
        let start = self.rows.partition_point(|r| r.holder < holder);
        let end = self.rows.partition_point(|r| r.holder <= holder);
        &self.rows[start..end]
    }

    fn search(&self, holder: PartIx, fact: FactIx) -> Result<usize, usize> {
        self.rows.binary_search_by_key(&(holder, fact), |r| (r.holder, r.fact))
    }
}
