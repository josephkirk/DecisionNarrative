//! Effect transactions and write-set conflict detection.

use std::error::Error;
use std::fmt;

use crate::chrono_state::{
    BijectiveDelta, GameplayTagRegistry, MutationOp, TagBitset, WorldStateTable,
    apply_mutation_with_registry, revert_delta,
};

pub const MAX_METRICS: usize = 512;
pub const MAX_ENTITIES: usize = 256;
pub const MAX_ITEMS: usize = 256;
pub const MAX_SCOPES: usize = 256;
pub const MAX_RESERVATIONS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DenseBitset<const WORDS: usize> {
    words: [u64; WORDS],
}

impl<const WORDS: usize> Default for DenseBitset<WORDS> {
    fn default() -> Self {
        Self { words: [0; WORDS] }
    }
}

impl<const WORDS: usize> DenseBitset<WORDS> {
    pub fn set(&mut self, index: usize) -> bool {
        let word = index / 64;
        let bit = index % 64;
        if word >= WORDS {
            return false;
        }
        self.words[word] |= 1u64 << bit;
        true
    }

    pub fn intersects(&self, other: &Self) -> bool {
        self.words
            .iter()
            .zip(other.words.iter())
            .any(|(left, right)| left & right != 0)
    }

    pub fn merge(&mut self, other: &Self) {
        for (left, right) in self.words.iter_mut().zip(other.words.iter()) {
            *left |= *right;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|word| *word == 0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WriteSet {
    pub tags: TagBitset,
    pub metrics: DenseBitset<8>,  // 512
    pub entities: DenseBitset<4>, // 256
    pub items: DenseBitset<4>,
    pub scopes: DenseBitset<4>,
    pub reservations: DenseBitset<4>,
    pub actor_traits: DenseBitset<4>,
    pub choices: DenseBitset<4>,
}

impl WriteSet {
    pub fn conflicts(&self, other: &Self) -> bool {
        self.tags.intersects(&other.tags)
            || self.metrics.intersects(&other.metrics)
            || self.entities.intersects(&other.entities)
            || self.items.intersects(&other.items)
            || self.scopes.intersects(&other.scopes)
            || self.reservations.intersects(&other.reservations)
            || self.actor_traits.intersects(&other.actor_traits)
            || self.choices.intersects(&other.choices)
    }

    pub fn merge(&mut self, other: &Self) {
        self.tags.merge(&other.tags);
        self.metrics.merge(&other.metrics);
        self.entities.merge(&other.entities);
        self.items.merge(&other.items);
        self.scopes.merge(&other.scopes);
        self.reservations.merge(&other.reservations);
        self.actor_traits.merge(&other.actor_traits);
        self.choices.merge(&other.choices);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConflictPolicy {
    #[default]
    RejectLater,
    PriorityWins,
    MergeAdditive,
    Serialize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TransactionState {
    #[default]
    Open,
    Committed,
    Aborted,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EffectTransaction {
    pub scope_index: u16,
    pub instance_index: u32,
    pub state: TransactionState,
    pub write_set: WriteSet,
    pub delta: Option<BijectiveDelta>,
    pub sequence: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransactionError {
    AlreadyCommitted,
    MissingDelta,
    State(String),
}

impl fmt::Display for TransactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyCommitted => write!(f, "transaction is already committed"),
            Self::MissingDelta => write!(f, "transaction has no delta"),
            Self::State(error) => write!(f, "transaction state error: {error}"),
        }
    }
}

impl Error for TransactionError {}

impl EffectTransaction {
    pub fn open(scope_index: u16, instance_index: u32, sequence: u32, write_set: WriteSet) -> Self {
        Self {
            scope_index,
            instance_index,
            state: TransactionState::Open,
            write_set,
            delta: None,
            sequence,
        }
    }

    pub fn conflicts_with(&self, other: &Self) -> bool {
        self.write_set.conflicts(&other.write_set)
    }

    pub fn commit(
        &mut self,
        state: &mut WorldStateTable,
        op: &MutationOp,
        registry: &GameplayTagRegistry,
    ) -> Result<(), TransactionError> {
        if self.state != TransactionState::Open {
            return Err(TransactionError::AlreadyCommitted);
        }
        let delta = apply_mutation_with_registry(state, op, registry)
            .map_err(|error| TransactionError::State(error.to_string()))?;
        self.delta = Some(delta);
        self.state = TransactionState::Committed;
        Ok(())
    }

    pub fn revert(&mut self, state: &mut WorldStateTable) -> Result<(), TransactionError> {
        if self.state != TransactionState::Committed {
            return Err(TransactionError::MissingDelta);
        }
        let delta = self.delta.take().ok_or(TransactionError::MissingDelta)?;
        revert_delta(state, &delta).map_err(|error| TransactionError::State(error.to_string()))?;
        self.state = TransactionState::Aborted;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrono_state::GameplayTagRegistry;

    fn write_tag(set: &mut WriteSet, tag: u16) {
        set.tags.set_bit(tag).unwrap();
    }

    #[test]
    fn overlapping_write_sets_conflict_and_disjoint_ones_do_not() {
        let mut a = WriteSet::default();
        let mut b = WriteSet::default();
        write_tag(&mut a, 1);
        write_tag(&mut b, 1);
        assert!(a.conflicts(&b));
        let mut c = WriteSet::default();
        write_tag(&mut c, 2);
        assert!(!a.conflicts(&c));
        let mut merged = a;
        merged.merge(&c);
        assert!(merged.conflicts(&b));
        assert!(merged.conflicts(&c));
    }

    #[test]
    fn commit_and_revert_restore_exact_state() {
        let mut state = WorldStateTable::new();
        let registry = GameplayTagRegistry::compile(["flag.a".to_owned()]).unwrap();
        let bit = registry.id("flag.a").unwrap();
        let mut op = MutationOp::default();
        op.add_global_tags.set_bit(bit).unwrap();
        let mut write = WriteSet::default();
        write_tag(&mut write, bit);
        let mut tx = EffectTransaction::open(0, 0, 1, write);
        tx.commit(&mut state, &op, &registry).unwrap();
        assert!(state.global_tags.bit(bit));
        tx.revert(&mut state).unwrap();
        assert!(!state.global_tags.bit(bit));
    }

    #[test]
    fn item_and_metric_conflicts_are_detected() {
        let mut a = WriteSet::default();
        let mut b = WriteSet::default();
        assert!(a.items.set(3));
        assert!(b.items.set(3));
        assert!(a.conflicts(&b));
        let mut c = WriteSet::default();
        let mut d = WriteSet::default();
        assert!(c.metrics.set(9));
        assert!(d.metrics.set(9));
        assert!(c.conflicts(&d));
        assert!(!a.conflicts(&c));
    }
}
