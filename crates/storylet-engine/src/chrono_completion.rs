//! Per-scope completion bitsets and canonical call-history entries
//! (REQ-DO-03 / REQ-DO-04).
//!
//! The bitset is derived state. Call history is the canonical replay record;
//! `rebuild_completed` is the only rebuild rule used by this contract.

use std::error::Error;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::chrono_catalog::{MAX_STORYLETS, StoryletIndex};

pub const STORYLET_WORDS: usize = MAX_STORYLETS / 64;
const _: () = assert!(MAX_STORYLETS % 64 == 0);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompletionError {
    IndexOutOfRange { index: u32 },
    InvalidCommitPath { head: u32, scope: u16 },
    InheritanceCycle { scope: u16 },
    MissingParent { scope: u16, parent: u16 },
    UnknownStoryletKey { key: String },
    UnknownScopeKey { key: String },
    DuplicateCommit { scope: u16, commit_index: u32 },
}

impl fmt::Display for CompletionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IndexOutOfRange { index } => write!(f, "storylet index {index} is out of range"),
            Self::InvalidCommitPath { head, scope } => {
                write!(f, "commit path {head} is invalid for scope {scope}")
            }
            Self::InheritanceCycle { scope } => {
                write!(f, "inheritance cycle at scope {scope}")
            }
            Self::MissingParent { scope, parent } => {
                write!(f, "scope {scope} is missing parent {parent}")
            }
            Self::UnknownStoryletKey { key } => write!(f, "unknown persisted storylet key '{key}'"),
            Self::UnknownScopeKey { key } => write!(f, "unknown persisted scope key '{key}'"),
            Self::DuplicateCommit {
                scope,
                commit_index,
            } => {
                write!(f, "duplicate commit {commit_index} in scope {scope}")
            }
        }
    }
}

impl Error for CompletionError {}

/// Fixed-size 65,536-bit storylet mask: 1024 words = 8 KiB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoryletBitset {
    words: [u64; STORYLET_WORDS],
}

impl Default for StoryletBitset {
    fn default() -> Self {
        Self::new()
    }
}

impl Serialize for StoryletBitset {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.iter_ones().collect::<Vec<_>>().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StoryletBitset {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let indices = Vec::<u16>::deserialize(deserializer)?;
        let mut bits = StoryletBitset::new();
        for index in indices {
            bits.set(index).map_err(serde::de::Error::custom)?;
        }
        Ok(bits)
    }
}

impl StoryletBitset {
    pub const fn new() -> Self {
        Self {
            words: [0; STORYLET_WORDS],
        }
    }

    pub fn set(&mut self, index: StoryletIndex) -> Result<(), CompletionError> {
        self.set_index(index as u32)
    }

    pub fn set_index(&mut self, index: u32) -> Result<(), CompletionError> {
        if index as usize >= MAX_STORYLETS {
            return Err(CompletionError::IndexOutOfRange { index });
        }
        let word = index as usize / 64;
        let bit = index as usize % 64;
        self.words[word] |= 1u64 << bit;
        Ok(())
    }

    pub fn clear(&mut self, index: StoryletIndex) -> Result<(), CompletionError> {
        self.clear_index(index as u32)
    }

    pub fn clear_index(&mut self, index: u32) -> Result<(), CompletionError> {
        if index as usize >= MAX_STORYLETS {
            return Err(CompletionError::IndexOutOfRange { index });
        }
        let word = index as usize / 64;
        let bit = index as usize % 64;
        self.words[word] &= !(1u64 << bit);
        Ok(())
    }

    pub fn contains(&self, index: StoryletIndex) -> bool {
        self.contains_index(index as u32)
    }

    pub fn contains_index(&self, index: u32) -> bool {
        if index as usize >= MAX_STORYLETS {
            return false;
        }
        let word = index as usize / 64;
        let bit = index as usize % 64;
        self.words[word] & (1u64 << bit) != 0
    }

    pub fn contains_all(&self, mask: &Self) -> bool {
        self.words
            .iter()
            .zip(mask.words.iter())
            .all(|(left, right)| left & right == *right)
    }

    pub fn intersects(&self, other: &Self) -> bool {
        self.words
            .iter()
            .zip(other.words.iter())
            .any(|(left, right)| left & right != 0)
    }

    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|word| *word == 0)
    }

    pub fn clear_all(&mut self) {
        self.words = [0; STORYLET_WORDS];
    }

    pub fn union_with(&mut self, other: &Self) {
        for (left, right) in self.words.iter_mut().zip(other.words.iter()) {
            *left |= *right;
        }
    }

    pub fn iter_ones(&self) -> impl Iterator<Item = StoryletIndex> + '_ {
        self.words
            .iter()
            .enumerate()
            .flat_map(|(word_index, word)| {
                let word = *word;
                (0..64).filter_map(move |bit| {
                    if word & (1u64 << bit) == 0 {
                        None
                    } else {
                        Some((word_index * 64 + bit) as StoryletIndex)
                    }
                })
            })
    }

    pub fn words(&self) -> &[u64; STORYLET_WORDS] {
        &self.words
    }
}

/// One scope's materialized completion state. Inheritance is opt-in; the
/// parent is checked without copying its bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScopeCompletion {
    pub scope_index: u16,
    pub bits: StoryletBitset,
    pub parent: Option<u16>,
    pub inherits: bool,
}

impl ScopeCompletion {
    /// Resolve the complete inherited view once at rebuild. The parent chain is
    /// bounded by the supplied slice and cycles are a typed error.
    pub fn visible_bits(&self, scopes: &[Self]) -> Result<StoryletBitset, CompletionError> {
        let mut visible = self.bits;
        let mut current = self;
        let mut seen = Vec::new();
        while current.inherits {
            let Some(parent_index) = current.parent else {
                return Err(CompletionError::MissingParent {
                    scope: current.scope_index,
                    parent: u16::MAX,
                });
            };
            if seen.contains(&parent_index) {
                return Err(CompletionError::InheritanceCycle {
                    scope: self.scope_index,
                });
            }
            seen.push(parent_index);
            let parent = scopes
                .iter()
                .find(|scope| scope.scope_index == parent_index)
                .ok_or(CompletionError::MissingParent {
                    scope: current.scope_index,
                    parent: parent_index,
                })?;
            visible.union_with(&parent.bits);
            current = parent;
        }
        Ok(visible)
    }

    pub fn visible_contains(
        &self,
        storylet: StoryletIndex,
        scopes: &[Self],
    ) -> Result<bool, CompletionError> {
        Ok(self.visible_bits(scopes)?.contains(storylet))
    }

    pub fn visible_contains_all(
        &self,
        mask: &StoryletBitset,
        scopes: &[Self],
    ) -> Result<bool, CompletionError> {
        Ok(self.visible_bits(scopes)?.contains_all(mask))
    }
}

/// Persisted history uses stable keys, never a dense index as identity.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PersistedCallEntry {
    pub scope_key: String,
    pub instance_key: String,
    pub node_guid: u32,
    pub executed_input_pin: String,
    pub chosen_output_pin: String,
    pub storylet_key: Option<String>,
    pub chosen_storylet_key: Option<String>,
    pub trigger_key: Option<String>,
    pub content_revision: u64,
    pub commit_index: u32,
    pub commit_parent: Option<u32>,
    pub tick: u32,
    pub completed: bool,
}

/// Runtime history uses dense indices only for the current content revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DenseCallEntry {
    pub commit_index: u32,
    pub commit_parent: Option<u32>,
    pub scope_index: u16,
    pub storylet_index: Option<StoryletIndex>,
    pub completed: bool,
    pub trigger_subscription: Option<u32>,
    pub chosen_output_pin: String,
}

/// Resolve persisted stable keys into dense runtime history. A missing storylet
/// key is a typed load error; dense indices are never trusted from disk.
pub fn resolve_persisted(
    entries: &[PersistedCallEntry],
    storylets: &crate::chrono_catalog::StoryletSpace,
    mut scope_index_for: impl FnMut(&str) -> Option<u16>,
) -> Result<Vec<DenseCallEntry>, CompletionError> {
    let mut seen = std::collections::BTreeSet::new();
    entries
        .iter()
        .map(|entry| {
            let scope_index = scope_index_for(&entry.scope_key).ok_or_else(|| {
                CompletionError::UnknownScopeKey {
                    key: entry.scope_key.clone(),
                }
            })?;
            if !seen.insert((scope_index, entry.commit_index)) {
                return Err(CompletionError::DuplicateCommit {
                    scope: scope_index,
                    commit_index: entry.commit_index,
                });
            }
            let key = entry
                .storylet_key
                .as_deref()
                .or(entry.chosen_storylet_key.as_deref());
            let storylet_index = key
                .map(|key| {
                    storylets
                        .resolve(key)
                        .ok_or_else(|| CompletionError::UnknownStoryletKey {
                            key: key.to_owned(),
                        })
                })
                .transpose()?;
            Ok(DenseCallEntry {
                commit_index: entry.commit_index,
                commit_parent: entry.commit_parent,
                scope_index,
                storylet_index,
                completed: entry.completed,
                trigger_subscription: None,
                chosen_output_pin: entry.chosen_output_pin.clone(),
            })
        })
        .collect()
}

/// Rebuild the completion bitset for one scope by walking the commit path from
/// `head` through `commit_parent`. Effects are not replayed here.
/// Resolve one scope's commit path from `head` to root. This is the only
/// traversal used by derived-history rebuilds.
pub fn commit_path<'a>(
    scope: u16,
    entries: &'a [DenseCallEntry],
    head: Option<u32>,
) -> Result<Vec<&'a DenseCallEntry>, CompletionError> {
    let Some(head) = head else {
        return Ok(Vec::new());
    };
    let by_commit: std::collections::BTreeMap<u32, &DenseCallEntry> = entries
        .iter()
        .filter(|entry| entry.scope_index == scope)
        .map(|entry| (entry.commit_index, entry))
        .collect();
    let mut path = Vec::new();
    let mut current = Some(head);
    let mut guard = 0usize;
    while let Some(index) = current {
        guard += 1;
        if guard > by_commit.len() + 1 {
            return Err(CompletionError::InvalidCommitPath { head, scope });
        }
        let entry = by_commit
            .get(&index)
            .copied()
            .ok_or(CompletionError::InvalidCommitPath { head, scope })?;
        path.push(entry);
        current = entry.commit_parent;
    }
    Ok(path)
}

pub fn rebuild_completed(
    scope: u16,
    entries: &[DenseCallEntry],
    head: Option<u32>,
) -> Result<StoryletBitset, CompletionError> {
    let path = commit_path(scope, entries, head)?;
    let mut bits = StoryletBitset::new();
    for entry in path.into_iter().rev() {
        if entry.completed {
            if let Some(index) = entry.storylet_index {
                bits.set(index)?;
            }
        }
    }
    Ok(bits)
}

/// Total rebuild check used by debug verification.
pub fn verify_completed(
    bits: &StoryletBitset,
    scope: u16,
    entries: &[DenseCallEntry],
    head: u32,
) -> Result<bool, CompletionError> {
    Ok(bits == &rebuild_completed(scope, entries, Some(head))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(commit: u32, parent: Option<u32>, scope: u16, storylet: u16) -> DenseCallEntry {
        DenseCallEntry {
            commit_index: commit,
            commit_parent: parent,
            scope_index: scope,
            storylet_index: Some(storylet),
            completed: true,
            trigger_subscription: None,
            chosen_output_pin: "output".into(),
        }
    }

    #[test]
    fn bitset_set_clear_contains_all_and_roundtrip() {
        let mut bits = StoryletBitset::new();
        bits.set(0).unwrap();
        bits.set(63).unwrap();
        bits.set(64).unwrap();
        bits.set(65_535).unwrap();
        assert!(bits.contains(0));
        assert!(bits.contains(63));
        assert!(bits.contains(64));
        assert!(bits.contains(65_535));
        assert!(!bits.contains(1));
        assert_eq!(
            bits.iter_ones().collect::<Vec<_>>(),
            vec![0, 63, 64, 65_535]
        );
        bits.clear(64).unwrap();
        assert!(!bits.contains(64));
        assert!(bits.contains(65_535));
    }

    #[test]
    fn contains_all_is_mask_containment_and_capacity_is_typed() {
        let mut left = StoryletBitset::new();
        let mut right = StoryletBitset::new();
        left.set(1).unwrap();
        left.set(2).unwrap();
        right.set(2).unwrap();
        assert!(left.contains_all(&right));
        assert!(!right.contains_all(&left));
        assert_eq!(
            left.set_index(MAX_STORYLETS as u32),
            Err(CompletionError::IndexOutOfRange {
                index: MAX_STORYLETS as u32
            })
        );
    }

    #[test]
    fn rebuild_matches_history_and_ignores_non_completions() {
        let entries = vec![
            entry(0, None, 0, 1),
            DenseCallEntry {
                completed: false,
                ..entry(1, Some(0), 0, 2)
            },
            entry(2, Some(1), 0, 3),
        ];
        let bits = rebuild_completed(0, &entries, Some(2)).unwrap();
        assert!(bits.contains(1));
        assert!(!bits.contains(2));
        assert!(bits.contains(3));
        assert!(verify_completed(&bits, 0, &entries, 2).unwrap());

        // The sibling scope is isolated even when its entries are in the same arena.
        assert!(rebuild_completed(1, &entries, Some(2)).is_err());
    }

    #[test]
    fn rollback_path_excludes_abandoned_entries() {
        let entries = vec![entry(0, None, 0, 1), entry(1, Some(0), 0, 2)];
        let bits = rebuild_completed(0, &entries, Some(0)).unwrap();
        assert!(bits.contains(1));
        assert!(!bits.contains(2));
    }

    #[test]
    fn inheritance_is_opt_in_and_does_not_copy_parent_bits() {
        let mut parent = ScopeCompletion {
            scope_index: 0,
            bits: StoryletBitset::new(),
            parent: None,
            inherits: false,
        };
        parent.bits.set(4).unwrap();
        let mut child = ScopeCompletion {
            scope_index: 1,
            bits: StoryletBitset::new(),
            parent: Some(0),
            inherits: true,
        };
        child.bits.set(8).unwrap();
        assert!(child.visible_contains(4, &[parent, child]).unwrap());
        assert!(child.visible_contains(8, &[parent, child]).unwrap());
        assert!(!child.bits.contains(4));
        child.inherits = false;
        assert!(!child.visible_contains(4, &[parent, child]).unwrap());
    }

    #[test]
    fn persisted_keys_resolve_and_unknown_keys_fail() {
        let mut space = crate::chrono_catalog::StoryletSpace::new();
        let a = space.insert("stl.a").unwrap();
        let persisted = vec![PersistedCallEntry {
            scope_key: "scope.0".into(),
            instance_key: "instance.0".into(),
            node_guid: 1,
            executed_input_pin: "In".into(),
            chosen_output_pin: "Out".into(),
            storylet_key: Some("stl.a".into()),
            chosen_storylet_key: None,
            trigger_key: None,
            content_revision: 1,
            commit_index: 0,
            commit_parent: None,
            tick: 0,
            completed: true,
        }];
        let dense = resolve_persisted(&persisted, &space, |_| Some(0)).unwrap();
        assert_eq!(dense[0].storylet_index, Some(a));
        assert!(rebuild_completed(0, &dense, Some(0)).unwrap().contains(a));

        let mut unknown = persisted.clone();
        unknown[0].storylet_key = Some("missing".into());
        assert_eq!(
            resolve_persisted(&unknown, &space, |_| Some(0)),
            Err(CompletionError::UnknownStoryletKey {
                key: "missing".into()
            })
        );
    }

    #[test]
    fn word_count_is_1024() {
        assert_eq!(STORYLET_WORDS, 1024);
    }
}
