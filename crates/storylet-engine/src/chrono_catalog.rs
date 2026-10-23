//! Stable authoring keys and the dense storylet catalog.
//!
//! Authoring and persistence use stable keys. Runtime selection uses dense
//! `StoryletIndex` values resolved once at load. The catalog is the load-time
//! and working-set boundary, not a second source of truth.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

/// One `u16` storylet index space. Capacity is a single contract shared by the
/// catalog, completion bitset, and profile pools.
pub const MAX_STORYLETS: usize = 65_536;
pub type StoryletIndex = u16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogError {
    EmptyKey,
    DuplicateKey { kind: &'static str, key: String },
    CapacityExceeded { kind: &'static str, capacity: usize },
    UnknownKey { kind: &'static str, key: String },
    Frozen,
    IndexOutOfRange { index: StoryletIndex },
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyKey => write!(f, "storylet key is empty"),
            Self::DuplicateKey { kind, key } => write!(f, "duplicate {kind} key '{key}'"),
            Self::CapacityExceeded { kind, capacity } => {
                write!(f, "{kind} capacity exceeded ({capacity})")
            }
            Self::UnknownKey { kind, key } => write!(f, "unknown {kind} key '{key}'"),
            Self::Frozen => write!(f, "storylet catalog is frozen"),
            Self::IndexOutOfRange { index } => write!(f, "storylet index {index} is out of range"),
        }
    }
}

impl Error for CatalogError {}

/// Stable-key -> dense-index registry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StoryletSpace {
    keys: Vec<String>,
    by_key: BTreeMap<String, StoryletIndex>,
    revision: u64,
    frozen: bool,
}

impl StoryletSpace {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    pub fn remaining_capacity(&self) -> usize {
        MAX_STORYLETS.saturating_sub(self.keys.len())
    }

    /// Insert a stable key. The dense index is the insertion order.
    pub fn insert(&mut self, key: impl Into<String>) -> Result<StoryletIndex, CatalogError> {
        if self.frozen {
            return Err(CatalogError::Frozen);
        }
        let key = key.into();
        if key.trim().is_empty() {
            return Err(CatalogError::EmptyKey);
        }
        if self.by_key.contains_key(&key) {
            return Err(CatalogError::DuplicateKey {
                kind: "storylet",
                key,
            });
        }
        if self.keys.len() >= MAX_STORYLETS {
            return Err(CatalogError::CapacityExceeded {
                kind: "storylet",
                capacity: MAX_STORYLETS,
            });
        }
        let index = self.keys.len() as StoryletIndex;
        self.keys.push(key.clone());
        self.by_key.insert(key, index);
        Ok(index)
    }

    /// Resolve a stable key at load/authoring time. Runtime code must use
    /// `key(index)` and dense indices instead.
    pub fn resolve(&self, key: &str) -> Option<StoryletIndex> {
        self.by_key.get(key).copied()
    }

    pub fn require(&self, key: &str) -> Result<StoryletIndex, CatalogError> {
        self.resolve(key).ok_or_else(|| CatalogError::UnknownKey {
            kind: "storylet",
            key: key.to_owned(),
        })
    }

    pub fn key(&self, index: StoryletIndex) -> Option<&str> {
        self.keys.get(index as usize).map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = (StoryletIndex, &str)> {
        self.keys
            .iter()
            .enumerate()
            .map(|(index, key)| (index as StoryletIndex, key.as_str()))
    }

    /// Freeze the registry at a content revision. Load-time lookup remains
    /// available, but no new identity may be inserted. Freezing twice is an
    /// error, so a post-load holder cannot rewrite the revision.
    pub fn freeze(&mut self, revision: u64) -> Result<(), CatalogError> {
        if self.frozen {
            return Err(CatalogError::Frozen);
        }
        self.revision = revision;
        self.frozen = true;
        Ok(())
    }
}

/// Catalog metadata needed by selection and working-set loading. The catalog is
/// the storylet registry plus payload/profile metadata; it is not a
/// parallel registry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StoryletCatalog {
    space: StoryletSpace,
    is_ending: Vec<bool>,
    payload_ref: Vec<u32>,
    duration_minutes: Vec<f32>,
    frozen: bool,
}

impl StoryletCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.space.len()
    }

    pub fn is_empty(&self) -> bool {
        self.space.is_empty()
    }

    pub fn revision(&self) -> u64 {
        self.space.revision()
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    pub fn space(&self) -> &StoryletSpace {
        &self.space
    }

    /// Insert one storylet with its catalog metadata.
    pub fn insert(
        &mut self,
        key: impl Into<String>,
        is_ending: bool,
        payload_ref: u32,
        duration_minutes: f32,
    ) -> Result<StoryletIndex, CatalogError> {
        if self.frozen {
            return Err(CatalogError::Frozen);
        }
        let index = self.space.insert(key)?;
        self.is_ending.push(is_ending);
        self.payload_ref.push(payload_ref);
        self.duration_minutes.push(duration_minutes.max(0.0));
        debug_assert_eq!(self.is_ending.len(), self.space.len());
        debug_assert_eq!(self.payload_ref.len(), self.space.len());
        debug_assert_eq!(self.duration_minutes.len(), self.space.len());
        Ok(index)
    }

    pub fn resolve(&self, key: &str) -> Option<StoryletIndex> {
        self.space.resolve(key)
    }

    pub fn require(&self, key: &str) -> Result<StoryletIndex, CatalogError> {
        self.space.require(key)
    }

    pub fn key(&self, index: StoryletIndex) -> Option<&str> {
        self.space.key(index)
    }

    pub fn is_ending(&self, index: StoryletIndex) -> Option<bool> {
        self.is_ending.get(index as usize).copied()
    }

    pub fn set_ending(&mut self, index: StoryletIndex, ending: bool) -> Result<(), CatalogError> {
        if self.frozen {
            return Err(CatalogError::Frozen);
        }
        let slot = self
            .is_ending
            .get_mut(index as usize)
            .ok_or(CatalogError::IndexOutOfRange { index })?;
        *slot = ending;
        Ok(())
    }

    pub fn payload_ref(&self, index: StoryletIndex) -> Option<u32> {
        self.payload_ref.get(index as usize).copied()
    }

    pub fn duration_minutes(&self, index: StoryletIndex) -> Option<f32> {
        self.duration_minutes.get(index as usize).copied()
    }

    /// Dense ending pool. `include_ending=false` is the normal deck's default.
    pub fn ending_pool(&self) -> Vec<StoryletIndex> {
        (0..self.len())
            .filter(|index| self.is_ending(*index as StoryletIndex) == Some(true))
            .map(|index| index as StoryletIndex)
            .collect()
    }

    /// Bounded working set. The caller supplies the profile predicate; the
    /// catalog only enforces the candidate limit and index range.
    pub fn working_set(
        &self,
        predicate: impl Fn(StoryletIndex) -> bool,
        limit: usize,
    ) -> Vec<StoryletIndex> {
        (0..self.len())
            .map(|index| index as StoryletIndex)
            .filter(|index| predicate(*index))
            .take(limit)
            .collect()
    }

    pub fn freeze(&mut self, revision: u64) -> Result<(), CatalogError> {
        self.space.freeze(revision)?;
        self.frozen = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_keys_round_trip_duplicate_and_freeze() {
        let mut space = StoryletSpace::new();
        let a = space.insert("stl.a").unwrap();
        let b = space.insert("stl.b").unwrap();
        assert_ne!(a, b);
        assert_eq!(space.resolve("stl.a"), Some(a));
        assert_eq!(space.key(b), Some("stl.b"));
        assert_eq!(
            space.insert("stl.a"),
            Err(CatalogError::DuplicateKey {
                kind: "storylet",
                key: "stl.a".into()
            })
        );
        assert_eq!(space.insert(" "), Err(CatalogError::EmptyKey));
        space.freeze(7).unwrap();
        assert_eq!(space.revision(), 7);
        assert!(space.is_frozen());
        assert_eq!(space.insert("stl.c"), Err(CatalogError::Frozen));
        assert_eq!(space.freeze(8), Err(CatalogError::Frozen));
    }

    #[test]
    fn catalog_metadata_and_ending_pool_are_dense() {
        let mut catalog = StoryletCatalog::new();
        let a = catalog.insert("stl.a", false, 1, 2.0).unwrap();
        let b = catalog.insert("stl.b", true, 2, 0.0).unwrap();
        catalog.insert("stl.c", true, 3, 1.5).unwrap();
        assert_eq!(catalog.resolve("stl.b"), Some(b));
        assert_eq!(catalog.ending_pool(), vec![b, 2]);
        assert_eq!(catalog.duration_minutes(a), Some(2.0));
        assert_eq!(catalog.working_set(|index| index != b, 2), vec![a, 2]);
        assert_eq!(catalog.working_set(|_| true, 1), vec![a]);
        catalog.freeze(9).unwrap();
        assert_eq!(catalog.set_ending(a, true), Err(CatalogError::Frozen));
        assert_eq!(catalog.freeze(10), Err(CatalogError::Frozen));
    }

    #[test]
    fn capacity_constant_covers_u16_space_and_is_enforced() {
        assert_eq!(MAX_STORYLETS, 65_536);
        assert_eq!(u16::MAX as usize + 1, MAX_STORYLETS);
        let mut space = StoryletSpace::new();
        for index in 0..MAX_STORYLETS {
            space.insert(format!("stl.{index}")).unwrap();
        }
        assert_eq!(
            space.insert("overflow"),
            Err(CatalogError::CapacityExceeded {
                kind: "storylet",
                capacity: MAX_STORYLETS
            })
        );
    }
}
