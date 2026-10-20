//! Derived feature indexes (REQ-DO-11).
//!
//! These are dense projections of authoritative state. Rebuild is the source of
//! truth for verification; incremental updates are an optimization.

use crate::chrono_runtime::CallHistoryEntry;
use crate::chrono_state::{AttributeValue, TagBitset, WorldStateTable};

pub const MAX_ACTORS: usize = 256;
pub const MAX_TRAITS: usize = 256;
pub const MAX_ITEMS: usize = 256;
pub const MAX_CHOICES: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActorTraitIndex {
    values: Vec<i8>,
}

impl Default for ActorTraitIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl ActorTraitIndex {
    pub fn new() -> Self {
        Self {
            values: vec![0; MAX_ACTORS * MAX_TRAITS],
        }
    }

    pub fn set(&mut self, actor: u16, trait_id: u16, value: i8) -> bool {
        let index = actor as usize * MAX_TRAITS + trait_id as usize;
        if actor as usize >= MAX_ACTORS || trait_id as usize >= MAX_TRAITS {
            return false;
        }
        self.values[index] = value;
        true
    }

    pub fn get(&self, actor: u16, trait_id: u16) -> i8 {
        if actor as usize >= MAX_ACTORS || trait_id as usize >= MAX_TRAITS {
            return 0;
        }
        self.values[actor as usize * MAX_TRAITS + trait_id as usize]
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemIndex {
    quantities: Vec<u32>,
}

impl Default for ItemIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl ItemIndex {
    pub fn new() -> Self {
        Self {
            quantities: vec![0; MAX_ITEMS * MAX_ITEMS],
        }
    }

    pub fn set_quantity(&mut self, owner: u16, item: u16, quantity: u32) -> bool {
        if owner as usize >= MAX_ITEMS || item as usize >= MAX_ITEMS {
            return false;
        }
        self.quantities[owner as usize * MAX_ITEMS + item as usize] = quantity;
        true
    }

    pub fn quantity(&self, owner: u16, item: u16) -> u32 {
        if owner as usize >= MAX_ITEMS || item as usize >= MAX_ITEMS {
            return 0;
        }
        self.quantities[owner as usize * MAX_ITEMS + item as usize]
    }

    pub fn rebuild(rows: &[(u16, u16, u32)]) -> Self {
        let mut index = Self::new();
        for (owner, item, quantity) in rows {
            index.set_quantity(*owner, *item, *quantity);
        }
        index
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChoiceHistoryIndex {
    counts: Vec<u16>,
}

impl Default for ChoiceHistoryIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl ChoiceHistoryIndex {
    pub fn new() -> Self {
        Self {
            counts: vec![0; MAX_CHOICES],
        }
    }

    pub fn record(&mut self, choice_key: u16) -> bool {
        if choice_key as usize >= MAX_CHOICES {
            return false;
        }
        self.counts[choice_key as usize] = self.counts[choice_key as usize].saturating_add(1);
        true
    }

    pub fn count(&self, choice_key: u16) -> u16 {
        self.counts.get(choice_key as usize).copied().unwrap_or(0)
    }

    pub fn rebuild(rows: &[u16]) -> Self {
        let mut index = Self::new();
        for key in rows {
            index.record(*key);
        }
        index
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorldStageIndex {
    pub active: TagBitset,
}

impl WorldStageIndex {
    pub fn set(&mut self, stage: u16) -> bool {
        self.active.set_bit(stage).is_ok()
    }

    pub fn clear(&mut self, stage: u16) -> bool {
        self.active.clear_bit(stage).is_ok()
    }

    pub fn contains(&self, stage: u16) -> bool {
        self.active.bit(stage)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeatureIndex {
    pub actor_traits: ActorTraitIndex,
    pub items: ItemIndex,
    pub choices: ChoiceHistoryIndex,
    pub world_stage: WorldStageIndex,
}

impl Default for FeatureIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl FeatureIndex {
    pub fn new() -> Self {
        Self {
            actor_traits: ActorTraitIndex::new(),
            items: ItemIndex::new(),
            choices: ChoiceHistoryIndex::new(),
            world_stage: WorldStageIndex::default(),
        }
    }

    pub fn rebuild(
        traits: &[(u16, u16, i8)],
        item_rows: &[(u16, u16, u32)],
        choices: &[u16],
        stages: &[u16],
    ) -> Self {
        let mut index = Self::new();
        for (actor, trait_id, value) in traits {
            index.actor_traits.set(*actor, *trait_id, *value);
        }
        index.items = ItemIndex::rebuild(item_rows);
        index.choices = ChoiceHistoryIndex::rebuild(choices);
        for stage in stages {
            index.world_stage.set(*stage);
        }
        index
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FeatureError {
    OutOfRange,
}

impl std::fmt::Display for FeatureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfRange => write!(f, "feature row is out of range"),
        }
    }
}

impl std::error::Error for FeatureError {}

impl FeatureIndex {
    /// Derive the index from authoritative world state plus the resolved
    /// call-history path for one scope. Unknown relation types are ignored.
    pub fn rebuild_from_state(
        state: &WorldStateTable,
        history: &[CallHistoryEntry],
        scope: u16,
    ) -> Result<Self, FeatureError> {
        let mut index = Self::new();
        for edge in &state.edges {
            if !edge.relation_type.eq_ignore_ascii_case("owns") {
                continue;
            }
            let Some(owner_vertex) = state.vertices.iter().find(|v| v.vertex_id == edge.source_id) else {
                continue;
            };
            let Some(item_vertex) = state.vertices.iter().find(|v| v.vertex_id == edge.target_id) else {
                continue;
            };
            let (Some(owner), Some(item)) = (owner_vertex.entity_index, item_vertex.entity_index) else {
                continue;
            };
            let quantity = match item_vertex.attributes.get("quantity") {
                Some(AttributeValue::Int(value)) if *value >= 0 => *value as u32,
                Some(AttributeValue::Float(value)) if *value >= 0.0 => *value as u32,
                _ => 1,
            };
            if !index.items.set_quantity(owner, item, quantity) {
                return Err(FeatureError::OutOfRange);
            }
        }

        for (actor, value) in state.passions.macro_values.iter().enumerate() {
            if actor >= MAX_ACTORS {
                return Err(FeatureError::OutOfRange);
            }
            if !index.actor_traits.set(actor as u16, 1, value.clamp(-127.0, 127.0) as i8) {
                return Err(FeatureError::OutOfRange);
            }
        }

        for entry in history {
            if entry.scope_index != scope {
                continue;
            }
            if let Some(key) = entry
                .chosen_output_pin
                .strip_prefix("choice_")
                .and_then(|value| value.parse::<u16>().ok())
                && !index.choices.record(key)
            {
                return Err(FeatureError::OutOfRange);
            }
        }

        for bit in 0..crate::chrono_state::MAX_TAGS as u16 {
            if state.global_tags.bit(bit) && !index.world_stage.set(bit) {
                return Err(FeatureError::OutOfRange);
            }
        }

        Ok(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_indexes_are_dense_and_bounded() {
        let mut index = FeatureIndex::new();
        assert!(index.actor_traits.set(3, 7, 42));
        assert_eq!(index.actor_traits.get(3, 7), 42);
        assert!(!index.actor_traits.set(MAX_ACTORS as u16, 0, 1));
        assert!(index.items.set_quantity(2, 9, 5));
        assert_eq!(index.items.quantity(2, 9), 5);
        assert!(index.choices.record(11));
        assert_eq!(index.choices.count(11), 1);
        assert!(index.world_stage.set(4));
        assert!(index.world_stage.contains(4));
        index.world_stage.clear(4);
        assert!(!index.world_stage.contains(4));
    }

    #[test]
    fn rebuild_from_state_and_history() {
        use crate::chrono_state::{GraphLayer, WorldEdge, WorldVertex};
        let mut state = WorldStateTable::new();
        state.vertices.push(WorldVertex {
            vertex_id: "actor".into(),
            layer: GraphLayer::Character,
            entity_index: Some(1),
            attributes: Default::default(),
        });
        state.vertices.push(WorldVertex {
            vertex_id: "key".into(),
            layer: GraphLayer::Item,
            entity_index: Some(2),
            attributes: [("quantity".into(), AttributeValue::Int(3))].into(),
        });
        state.edges.push(WorldEdge {
            edge_id: "owns".into(),
            source_id: "actor".into(),
            target_id: "key".into(),
            relation_type: "owns".into(),
            weight: 1.0,
        });
        state.global_tags.set_bit(4).unwrap();
        let history = vec![CallHistoryEntry {
            scope_index: 0,
            chosen_output_pin: "choice_9".into(),
            ..CallHistoryEntry::default()
        }];
        let index = FeatureIndex::rebuild_from_state(&state, &history, 0).unwrap();
        assert_eq!(index.items.quantity(1, 2), 3);
        assert_eq!(index.choices.count(9), 1);
        assert!(index.world_stage.contains(4));
    }

    #[test]
    fn rebuild_matches_incremental() {
        let traits = [(1, 2, 7), (3, 4, -2)];
        let items = [(0, 5, 2), (6, 7, 9)];
        let choices = [1, 1, 3];
        let stages = [8];
        let rebuilt = FeatureIndex::rebuild(&traits, &items, &choices, &stages);
        let mut incremental = FeatureIndex::new();
        for (actor, trait_id, value) in traits {
            incremental.actor_traits.set(actor, trait_id, value);
        }
        for (owner, item, quantity) in items {
            incremental.items.set_quantity(owner, item, quantity);
        }
        for choice in choices {
            incremental.choices.record(choice);
        }
        for stage in stages {
            incremental.world_stage.set(stage);
        }
        assert_eq!(rebuilt, incremental);
    }
}
