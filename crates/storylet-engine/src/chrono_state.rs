//! Data-oriented gameplay tag bitsets, the layered world graph, and bijective
//! (reversible) state deltas for the ChronoFlow narrative engine.
//!
//! This module owns the state layer only: fixed-size inline tag bitsets, the
//! immutable dense tag registry, the structure-of-arrays world state table, and
//! the reversible delta operators. It contains no dispatch, clock, or
//! call-history logic, and it depends only on `std` and `serde`.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Number of 64-bit words in a `TagBitset` (REQ-DOD-02).
pub const WORD_COUNT: usize = 4;
/// Maximum number of compilable gameplay tags: `WORD_COUNT * 64`.
pub const MAX_TAGS: usize = WORD_COUNT * 64;
/// Maximum number of continuous metric slots (REQ-DOD-04).
pub const MAX_METRICS: usize = 512;
/// Maximum number of per-entity tag containers.
pub const MAX_ENTITIES: usize = 256;
/// Longest accepted gameplay tag string.
pub const MAX_TAG_LENGTH: usize = 128;
/// Maximum candidate assignments explored while matching a graph pattern.
pub const PATTERN_SEARCH_BUDGET: usize = 4096;

/// Gameplay tag that marks an actor row as present at the player's current
/// location.
pub const LOCATION_CURRENT_TAG: &str = "location.current";
/// WorldGraph relation that places a character row at a location; movement
/// recomputes the marker from these edges.
pub const LOCATED_AT_RELATION: &str = "located_at";

/// Fixed-size inline gameplay tag container. Adding, removing, and querying
/// never allocate: the storage is a contiguous inline array of `WORD_COUNT`
/// 64-bit words (REQ-DOD-02).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagBitset {
    words: [u64; WORD_COUNT],
}

impl TagBitset {
    pub const fn new() -> Self {
        TagBitset {
            words: [0; WORD_COUNT],
        }
    }

    pub const fn from_words(words: [u64; WORD_COUNT]) -> Self {
        TagBitset { words }
    }

    /// The raw words, in word order.
    pub const fn words(&self) -> &[u64; WORD_COUNT] {
        &self.words
    }

    /// Bit `index` within the fixed container. Indices outside the container
    /// read as `false` because no bit can exist beyond `MAX_TAGS`.
    pub const fn bit(&self, index: u16) -> bool {
        if index as usize >= MAX_TAGS {
            return false;
        }
        (self.words[index as usize / 64] >> (index as usize % 64)) & 1 == 1
    }

    pub fn set_bit(&mut self, index: u16) -> Result<(), ChronoStateError> {
        if index as usize >= MAX_TAGS {
            return Err(ChronoStateError::TagIndexOutOfRange { index });
        }
        self.words[index as usize / 64] |= 1u64 << (index as usize % 64);
        Ok(())
    }

    pub fn clear_bit(&mut self, index: u16) -> Result<(), ChronoStateError> {
        if index as usize >= MAX_TAGS {
            return Err(ChronoStateError::TagIndexOutOfRange { index });
        }
        self.words[index as usize / 64] &= !(1u64 << (index as usize % 64));
        Ok(())
    }

    /// Self-inverting toggle: applying the same mask twice restores the exact
    /// original bits (REQ-REV-01).
    pub fn apply_xor(&mut self, mask: &TagBitset) {
        for word in 0..WORD_COUNT {
            self.words[word] ^= mask.words[word];
        }
    }

    /// Union into this container; used to fold effect masks into a
    /// `MutationOp`.
    pub fn merge(&mut self, other: &TagBitset) {
        for word in 0..WORD_COUNT {
            self.words[word] |= other.words[word];
        }
    }

    pub fn popcount(&self) -> u32 {
        let mut count = 0u32;
        for word in 0..WORD_COUNT {
            count += self.words[word].count_ones();
        }
        count
    }

    /// Population count of `self & other`, used for salience specificity.
    pub fn popcount_intersection(&self, other: &TagBitset) -> u32 {
        let mut count = 0u32;
        for word in 0..WORD_COUNT {
            count += (self.words[word] & other.words[word]).count_ones();
        }
        count
    }

    pub fn intersects(&self, other: &TagBitset) -> bool {
        for word in 0..WORD_COUNT {
            if self.words[word] & other.words[word] != 0 {
                return true;
            }
        }
        false
    }

    pub fn contains_all(&self, required: &TagBitset) -> bool {
        for word in 0..WORD_COUNT {
            if self.words[word] & required.words[word] != required.words[word] {
                return false;
            }
        }
        true
    }

    /// Branchless precondition test: all required bits present, no forbidden
    /// bits present.
    pub fn matches_query(&self, required_all: &TagBitset, forbidden_any: &TagBitset) -> bool {
        for word in 0..WORD_COUNT {
            if self.words[word] & required_all.words[word] != required_all.words[word] {
                return false;
            }
            if self.words[word] & forbidden_any.words[word] != 0 {
                return false;
            }
        }
        true
    }

    pub fn is_empty(&self) -> bool {
        for word in 0..WORD_COUNT {
            if self.words[word] != 0 {
                return false;
            }
        }
        true
    }

    /// XOR mask between two containers, used to build a `BijectiveDelta`.
    fn xor_with(&self, other: &TagBitset) -> TagBitset {
        let mut mask = TagBitset::new();
        for word in 0..WORD_COUNT {
            mask.words[word] = self.words[word] ^ other.words[word];
        }
        mask
    }
}

/// Semantic layer of a world graph vertex (REQ-WB-01).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphLayer {
    #[default]
    Location,
    Character,
    Item,
    Fact,
}

/// Typed attribute value stored on a vertex.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum AttributeValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Tag(u16),
}

impl AttributeValue {
    /// Numeric view used by ordered attribute predicates.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            AttributeValue::Int(value) => Some(*value as f64),
            AttributeValue::Float(value) if value.is_finite() => Some(*value),
            _ => None,
        }
    }

    fn is_finite(&self) -> bool {
        match self {
            AttributeValue::Float(value) => value.is_finite(),
            _ => true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeOp {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttributePredicate {
    pub key: String,
    pub op: AttributeOp,
    pub value: AttributeValue,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldVertex {
    #[serde(default)]
    pub vertex_id: String,
    #[serde(default)]
    pub layer: GraphLayer,
    /// Entity row this vertex represents, when the vertex stands for an
    /// authored entity. Movement uses this to place the current-location tag on
    /// actor rows through WorldGraph relations.
    #[serde(default)]
    pub entity_index: Option<u16>,
    #[serde(default)]
    pub attributes: BTreeMap<String, AttributeValue>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldEdge {
    #[serde(default)]
    pub edge_id: String,
    #[serde(default)]
    pub source_id: String,
    #[serde(default)]
    pub target_id: String,
    #[serde(default)]
    pub relation_type: String,
    #[serde(default)]
    pub weight: f32,
}

/// Reference to a vertex: a literal id, or the vertex bound by a left-hand
/// side vertex pattern at the given index.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum GraphBinding {
    Literal(String),
    VertexPattern(usize),
    /// Matched LHS edge, used by `remove_edges` so a pattern can delete the
    /// relation it matched.
    EdgePattern(usize),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VertexPattern {
    #[serde(default)]
    pub layer: GraphLayer,
    #[serde(default)]
    pub vertex_id: Option<String>,
    #[serde(default)]
    pub attributes: Vec<AttributePredicate>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EdgePattern {
    #[serde(default)]
    pub edge_id: Option<String>,
    #[serde(default)]
    pub relation_type: Option<String>,
    #[serde(default)]
    pub source: Option<GraphBinding>,
    #[serde(default)]
    pub target: Option<GraphBinding>,
}

/// Left-hand side of a graph transformation rule (REQ-WB-02).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GraphPattern {
    #[serde(default)]
    pub vertices: Vec<VertexPattern>,
    #[serde(default)]
    pub edges: Vec<EdgePattern>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VertexTemplate {
    pub id: GraphBinding,
    #[serde(default)]
    pub layer: GraphLayer,
    #[serde(default)]
    pub attributes: BTreeMap<String, AttributeValue>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EdgeTemplate {
    pub id: GraphBinding,
    pub source: GraphBinding,
    pub target: GraphBinding,
    #[serde(default)]
    pub relation_type: String,
    #[serde(default)]
    pub weight: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttributeAssignment {
    pub vertex: GraphBinding,
    pub key: String,
    pub value: AttributeValue,
}

/// Right-hand side mutation of a graph transformation rule (REQ-WB-02).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GraphMutation {
    #[serde(default)]
    pub add_vertices: Vec<VertexTemplate>,
    #[serde(default)]
    pub remove_vertices: Vec<GraphBinding>,
    #[serde(default)]
    pub add_edges: Vec<EdgeTemplate>,
    #[serde(default)]
    pub remove_edges: Vec<GraphBinding>,
    #[serde(default)]
    pub set_attributes: Vec<AttributeAssignment>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GraphTransformRule {
    pub lhs: GraphPattern,
    pub rhs: GraphMutation,
}

/// Deterministic result of matching a left-hand side pattern: matched ids in
/// pattern order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GraphMatch {
    #[serde(default)]
    pub vertices: Vec<String>,
    #[serde(default)]
    pub edges: Vec<String>,
}

/// Local cache that makes a graph transformation exactly reversible
/// (REQ-WB-03).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GraphDiffRecord {
    #[serde(default)]
    pub added_vertex_ids: Vec<String>,
    #[serde(default)]
    pub removed_vertices: Vec<WorldVertex>,
    #[serde(default)]
    pub added_edge_ids: Vec<String>,
    #[serde(default)]
    pub removed_edges: Vec<WorldEdge>,
    /// vertex_id -> (attribute_key -> prior value)
    #[serde(default)]
    pub previous_attributes: BTreeMap<String, BTreeMap<String, Option<AttributeValue>>>,
}

/// One entry of the compressed sparse row coupling matrix (REQ-DOD-05).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ZeroSumLink {
    pub source_metric_id: u16,
    pub target_metric_id: u16,
    pub weight: f32,
}

/// One metric threshold rule that writes a global tag bit. Used by MENACE
/// quality thresholds; macro-passion rules live on `PassionRegistry`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetricThresholdRule {
    pub metric_id: u16,
    pub threshold_tag_bit: u16,
    pub trigger_value: f32,
    pub hysteresis: f32,
}

/// Immutable, dense, deterministic gameplay tag registry (REQ-DOD-03).
///
/// `compile` assigns tag ids in sorted order, so the same input set always
/// produces the same indices and the same closure masks. The registry stores no
/// parent arrays per container; it stores two precomputed lookup tables plus the
/// CSR coupling matrix and threshold rules.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GameplayTagRegistry {
    tag_to_id: BTreeMap<String, u16>,
    ancestors: Vec<TagBitset>,
    descendants: Vec<TagBitset>,
    /// CSR row offsets over `MAX_METRICS` rows; `entries` is sorted by
    /// `(source_metric_id, target_metric_id)`.
    row_offsets: Vec<u32>,
    entries: Vec<ZeroSumLink>,
    threshold_rules: Vec<MetricThresholdRule>,
}

impl GameplayTagRegistry {
    /// Empty registry: no tags, no coupling links, no threshold rules.
    pub fn empty() -> Self {
        GameplayTagRegistry {
            tag_to_id: BTreeMap::new(),
            ancestors: Vec::new(),
            descendants: Vec::new(),
            row_offsets: vec![0; MAX_METRICS + 1],
            entries: Vec::new(),
            threshold_rules: Vec::new(),
        }
    }

    /// Compiles hierarchical tags into dense indices in deterministic sorted
    /// order and precomputes the ancestor and descendant closure masks.
    pub fn compile(tags: impl IntoIterator<Item = String>) -> Result<Self, ChronoStateError> {
        let mut unique = BTreeSet::new();
        for tag in tags {
            validate_tag(&tag)?;
            if !unique.insert(tag.clone()) {
                return Err(ChronoStateError::DuplicateTag { tag });
            }
        }
        if unique.len() > MAX_TAGS {
            return Err(ChronoStateError::TagLimitExceeded {
                count: unique.len(),
            });
        }

        let mut registry = GameplayTagRegistry::empty();
        for (index, tag) in unique.iter().enumerate() {
            registry.tag_to_id.insert(tag.clone(), index as u16);
        }

        for index in 0..unique.len() {
            let tag = unique.iter().nth(index).expect("compiled tag");
            let id = index as u16;
            let mut ancestor_mask = TagBitset::new();
            ancestor_mask.set_bit(id)?;
            for end in 1..tag.len() {
                if tag.as_bytes()[end] != b'.' {
                    continue;
                }
                let parent = &tag[..end];
                if let Some(parent_id) = registry.tag_to_id.get(parent) {
                    ancestor_mask.set_bit(*parent_id)?;
                }
            }
            registry.ancestors.push(ancestor_mask);

            let mut descendant_mask = TagBitset::new();
            descendant_mask.set_bit(id)?;
            let prefix = format!("{tag}.");
            for other in unique.iter() {
                if other.starts_with(&prefix) {
                    let other_id = registry.tag_to_id[other];
                    descendant_mask.set_bit(other_id)?;
                }
            }
            registry.descendants.push(descendant_mask);
        }

        Ok(registry)
    }

    /// Returns a registry with the given coupling links, sorted deterministically.
    pub fn with_coupling_links(
        mut self,
        links: impl IntoIterator<Item = ZeroSumLink>,
    ) -> Result<Self, ChronoStateError> {
        let mut sorted: BTreeSet<(u16, u16, u32)> = BTreeSet::new();
        for link in links {
            if link.source_metric_id as usize >= MAX_METRICS
                || link.target_metric_id as usize >= MAX_METRICS
            {
                return Err(ChronoStateError::MetricIndexOutOfRange {
                    index: link.source_metric_id,
                });
            }
            if !link.weight.is_finite() {
                return Err(ChronoStateError::NonFiniteMetric {
                    metric_id: link.source_metric_id,
                    value: link.weight,
                });
            }
            if !sorted.insert((
                link.source_metric_id,
                link.target_metric_id,
                link.weight.to_bits(),
            )) {
                return Err(ChronoStateError::DuplicateCouplingLink {
                    source_metric_id: link.source_metric_id,
                    target_metric_id: link.target_metric_id,
                });
            }
        }

        let mut entries = Vec::with_capacity(sorted.len());
        for (source, target, weight_bits) in sorted {
            entries.push(ZeroSumLink {
                source_metric_id: source,
                target_metric_id: target,
                weight: f32::from_bits(weight_bits),
            });
        }

        let mut row_offsets = vec![0u32; MAX_METRICS + 1];
        for (index, entry) in entries.iter().enumerate() {
            let row = entry.source_metric_id as usize;
            for offset in row + 1..=MAX_METRICS {
                row_offsets[offset] = index as u32 + 1;
            }
        }

        self.row_offsets = row_offsets;
        self.entries = entries;
        Ok(self)
    }

    /// Returns a registry with the given threshold rules, sorted deterministically.
    pub fn with_threshold_rules(
        mut self,
        rules: impl IntoIterator<Item = MetricThresholdRule>,
    ) -> Result<Self, ChronoStateError> {
        let mut sorted: BTreeSet<(u16, u16, u32, u32)> = BTreeSet::new();
        for rule in rules {
            if rule.metric_id as usize >= MAX_METRICS {
                return Err(ChronoStateError::MetricIndexOutOfRange {
                    index: rule.metric_id,
                });
            }
            if rule.threshold_tag_bit as usize >= MAX_TAGS {
                return Err(ChronoStateError::TagIndexOutOfRange {
                    index: rule.threshold_tag_bit,
                });
            }
            if !rule.trigger_value.is_finite() || !rule.hysteresis.is_finite() {
                return Err(ChronoStateError::NonFiniteMetric {
                    metric_id: rule.metric_id,
                    value: rule.trigger_value,
                });
            }
            if rule.hysteresis < 0.0 {
                return Err(ChronoStateError::InvalidHysteresis {
                    metric_id: rule.metric_id,
                    value: rule.hysteresis,
                });
            }
            if !sorted.insert((
                rule.metric_id,
                rule.threshold_tag_bit,
                rule.trigger_value.to_bits(),
                rule.hysteresis.to_bits(),
            )) {
                return Err(ChronoStateError::DuplicateThresholdRule {
                    metric_id: rule.metric_id,
                    threshold_tag_bit: rule.threshold_tag_bit,
                });
            }
        }

        self.threshold_rules = sorted
            .into_iter()
            .map(
                |(metric_id, threshold_tag_bit, trigger_bits, hysteresis_bits)| {
                    MetricThresholdRule {
                        metric_id,
                        threshold_tag_bit,
                        trigger_value: f32::from_bits(trigger_bits),
                        hysteresis: f32::from_bits(hysteresis_bits),
                    }
                },
            )
            .collect();
        Ok(self)
    }

    pub fn tag_count(&self) -> usize {
        self.tag_to_id.len()
    }

    pub fn id(&self, tag: &str) -> Option<u16> {
        self.tag_to_id.get(tag).copied()
    }

    pub fn tag(&self, index: u16) -> Option<&str> {
        self.tag_to_id
            .iter()
            .find(|(_, id)| **id == index)
            .map(|(tag, _)| tag.as_str())
    }

    pub fn ancestors_mask(&self, index: u16) -> Option<&TagBitset> {
        self.ancestors.get(index as usize)
    }

    pub fn descendants_mask(&self, index: u16) -> Option<&TagBitset> {
        self.descendants.get(index as usize)
    }

    /// Adds a tag together with its ancestor closure (REQ-DOD-03).
    pub fn add_hierarchical(
        &self,
        container: &mut TagBitset,
        index: u16,
    ) -> Result<(), ChronoStateError> {
        let mask = self
            .ancestors_mask(index)
            .ok_or(ChronoStateError::TagIndexOutOfRange { index })?;
        for word in 0..WORD_COUNT {
            container.words[word] |= mask.words[word];
        }
        Ok(())
    }

    /// Hierarchical query in one bitwise AND (REQ-DOD-03).
    pub fn has_hierarchical(&self, container: &TagBitset, index: u16) -> bool {
        match self.descendants_mask(index) {
            Some(mask) => container.intersects(mask),
            None => false,
        }
    }

    /// Direct coupling links out of one metric row, in deterministic order.
    pub fn links_for(&self, metric_id: u16) -> &[ZeroSumLink] {
        if metric_id as usize >= MAX_METRICS {
            return &[];
        }
        let start = self.row_offsets[metric_id as usize] as usize;
        let end = self.row_offsets[metric_id as usize + 1] as usize;
        &self.entries[start..end]
    }

    pub fn threshold_rules(&self) -> &[MetricThresholdRule] {
        &self.threshold_rules
    }
}

/// Deterministic metric-name registry. Metric indices are a separate index
/// space from gameplay tag ids: quality, passion, and waypoint-weight
/// predicates and effects resolve here, while tag membership resolves through
/// `GameplayTagRegistry`. `ZeroSumLink` and `MetricThresholdRule` use metric
/// indices from this space.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MetricRegistry {
    name_to_index: BTreeMap<String, u16>,
}

impl MetricRegistry {
    pub fn empty() -> Self {
        MetricRegistry {
            name_to_index: BTreeMap::new(),
        }
    }

    /// Compiles metric names into dense indices in deterministic sorted order.
    pub fn compile(names: impl IntoIterator<Item = String>) -> Result<Self, ChronoStateError> {
        let mut unique = BTreeSet::new();
        for name in names {
            if name.is_empty() {
                return Err(ChronoStateError::EmptyMetricName);
            }
            if name.len() > MAX_TAG_LENGTH || !is_valid_hierarchical_name(&name) {
                return Err(ChronoStateError::InvalidMetricName { name });
            }
            if !unique.insert(name.clone()) {
                return Err(ChronoStateError::DuplicateMetricName { name });
            }
        }
        if unique.len() > MAX_METRICS {
            return Err(ChronoStateError::MetricLimitExceeded {
                count: unique.len(),
            });
        }
        let mut registry = MetricRegistry::empty();
        for (index, name) in unique.iter().enumerate() {
            registry.name_to_index.insert(name.clone(), index as u16);
        }
        Ok(registry)
    }

    pub fn id(&self, name: &str) -> Option<u16> {
        self.name_to_index.get(name).copied()
    }

    pub fn name(&self, index: u16) -> Option<&str> {
        self.name_to_index
            .iter()
            .find(|(_, position)| **position == index)
            .map(|(name, _)| name.as_str())
    }

    pub fn count(&self) -> usize {
        self.name_to_index.len()
    }
}

/// Membership of a set of entities plus the role tags accumulated from their
/// entity rows. Both halves are recorded as XOR masks in `BijectiveDelta` so
/// that rollback restores them exactly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MembershipMask {
    /// Per-entity membership bits (entity index space).
    pub entity_mask: TagBitset,
    /// Role/location tags accumulated from the members' entity rows.
    pub tag_mask: TagBitset,
}

impl MembershipMask {
    pub fn new() -> Self {
        MembershipMask {
            entity_mask: TagBitset::new(),
            tag_mask: TagBitset::new(),
        }
    }

    /// Reverses the two halves of a membership record.
    pub fn apply_membership_xor(&mut self, entity_xor: &TagBitset, tag_xor: &TagBitset) {
        self.entity_mask.apply_xor(entity_xor);
        self.tag_mask.apply_xor(tag_xor);
    }
}

/// Clamping bounds for one metric slot. `unbounded()` spans every finite f32
/// value, avoiding non-finite sentinels in JSON snapshots.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetricBound {
    pub min: f32,
    pub max: f32,
}

impl MetricBound {
    pub const fn unbounded() -> Self {
        MetricBound {
            min: f32::MIN,
            max: f32::MAX,
        }
    }
}

impl Default for MetricBound {
    fn default() -> Self {
        Self::unbounded()
    }
}

/// One micro-passion slot for a Star NPC: its weight toward the macro passion
/// and whether it is hidden from the player (`passion_pool` entry).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MicroPassionDef {
    pub npc_id: u16,
    pub passion_id: u16,
    pub weight: f32,
    #[serde(default)]
    pub is_hidden: bool,
}

/// Zero-sum passion link between two Star NPCs: a gain on `source_passion_id`
/// is an equal and opposite change on `target_passion_id`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RivalPassionLink {
    pub target_character_id: u16,
    pub source_passion_id: u16,
    pub target_passion_id: u16,
    pub weight: f32,
}

/// Direction a passion threshold is evaluated in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PassionDirection {
    /// Active while the passion value is at or above the trigger.
    #[default]
    Up,
    /// Active while the passion value is at or below the trigger.
    Down,
}

/// Crossing and reversion rule for one Star NPC macro passion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PassionThresholdRule {
    pub npc_id: u16,
    pub threshold_tag_bit: u16,
    pub trigger_value: f32,
    #[serde(default)]
    pub direction: PassionDirection,
    #[serde(default)]
    pub linked_graph_asset_id: String,
}

/// A Star NPC: micro-passions, trust inertia, clamped macro bounds, and
/// zero-sum rival links.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StarNpcDef {
    pub npc_id: u16,
    pub trust_inertia: f32,
    pub micro_passions: Vec<MicroPassionDef>,
    pub macro_bounds: MetricBound,
    pub zero_sum_links: Vec<RivalPassionLink>,
}

/// Star NPC definitions in deterministic order: NPCs by id, passion slots by
/// id, links and thresholds sorted by their key fields.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PassionRegistry {
    npcs: BTreeMap<u16, StarNpcDef>,
    passion_owner: BTreeMap<u16, u16>,
    thresholds: Vec<PassionThresholdRule>,
}

impl PassionRegistry {
    pub fn empty() -> Self {
        PassionRegistry {
            npcs: BTreeMap::new(),
            passion_owner: BTreeMap::new(),
            thresholds: Vec::new(),
        }
    }

    pub fn compile(
        npcs: impl IntoIterator<Item = StarNpcDef>,
        thresholds: impl IntoIterator<Item = PassionThresholdRule>,
    ) -> Result<Self, ChronoStateError> {
        let mut registry = PassionRegistry::empty();
        for npc in npcs {
            validate_star_npc(&npc)?;
            let npc_id = npc.npc_id;
            if registry.npcs.insert(npc_id, npc).is_some() {
                return Err(ChronoStateError::DuplicateStarNpc { npc_id });
            }
        }
        let mut threshold_bits = BTreeSet::new();
        for rule in thresholds {
            validate_passion_threshold(&rule)?;
            let npc_id = rule.npc_id;
            if !registry.npcs.contains_key(&npc_id) {
                return Err(ChronoStateError::UnknownStarNpc { npc_id });
            }
            // Two rules owning one bit would fight over set/clear; reject the
            // ambiguous authoring instead of emitting contradictory events.
            if !threshold_bits.insert((npc_id, rule.threshold_tag_bit)) {
                return Err(ChronoStateError::DuplicateThresholdRule {
                    metric_id: npc_id,
                    threshold_tag_bit: rule.threshold_tag_bit,
                });
            }
            registry.thresholds.push(rule);
        }
        for npc in registry.npcs.values() {
            for micro in &npc.micro_passions {
                if registry
                    .passion_owner
                    .insert(micro.passion_id, npc.npc_id)
                    .is_some()
                {
                    return Err(ChronoStateError::DuplicateMicroPassion {
                        passion_id: micro.passion_id,
                    });
                }
            }
        }
        for npc in registry.npcs.values() {
            for link in &npc.zero_sum_links {
                if !registry.npcs.contains_key(&link.target_character_id) {
                    return Err(ChronoStateError::UnknownStarNpc {
                        npc_id: link.target_character_id,
                    });
                }
                if registry.passion_owner.get(&link.source_passion_id) != Some(&npc.npc_id) {
                    return Err(ChronoStateError::UnknownMicroPassion {
                        passion_id: link.source_passion_id,
                    });
                }
                if registry.passion_owner.get(&link.target_passion_id)
                    != Some(&link.target_character_id)
                {
                    return Err(ChronoStateError::UnknownMicroPassion {
                        passion_id: link.target_passion_id,
                    });
                }
            }
        }
        for npc in registry.npcs.values_mut() {
            npc.micro_passions.sort_by_key(|micro| micro.passion_id);
            npc.zero_sum_links.sort_by_key(|link| {
                (
                    link.source_passion_id,
                    link.target_passion_id,
                    link.weight.to_bits(),
                )
            });
        }
        registry
            .thresholds
            .sort_by_key(|rule| (rule.npc_id, rule.threshold_tag_bit));
        Ok(registry)
    }

    pub fn npc(&self, npc_id: u16) -> Option<&StarNpcDef> {
        self.npcs.get(&npc_id)
    }

    /// The Star NPC that owns a passion slot.
    pub fn owner_of(&self, passion_id: u16) -> Option<u16> {
        self.passion_owner.get(&passion_id).copied()
    }

    pub fn npc_count(&self) -> usize {
        self.npcs.len()
    }

    pub fn threshold_rules(&self) -> &[PassionThresholdRule] {
        &self.thresholds
    }
}

/// Data-oriented world state table (REQ-DOD-04).
///
/// Entity tag containers and metrics are indexed by dense tag index. Vertex and
/// edge tables are kept sorted by id so that a graph transformation and its
/// reversal restore byte-identical tables.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldStateTable {
    /// QBN, arc, clue, tool, and threshold flags.
    pub global_tags: TagBitset,
    /// Per-entity tag containers (locations, characters, items).
    #[serde(default)]
    pub entity_tags: Vec<TagBitset>,
    /// Continuous qualities and waypoint weights.
    #[serde(default)]
    pub metrics: Vec<f32>,
    /// Per-metric clamping bounds, parallel to `metrics`.
    #[serde(default)]
    pub metric_bounds: Vec<MetricBound>,
    /// Dense Star-NPC passion pool, active passions, and derived macro values.
    #[serde(default)]
    pub passions: PassionState,
    #[serde(default)]
    pub vertices: Vec<WorldVertex>,
    #[serde(default)]
    pub edges: Vec<WorldEdge>,
    #[serde(default)]
    pub current_location_entity: Option<u16>,
    /// Location tags contributed by the current location. Movement removes
    /// exactly these bits before adding the new location's row, so unrelated
    /// arc/quality flags are never clobbered.
    #[serde(default)]
    pub current_location_tag_mask: TagBitset,
    /// Visited locations: per-entity membership bits plus the location role
    /// tags accumulated from those entity rows.
    #[serde(default)]
    pub visited_location_tags: MembershipMask,
    /// Met characters: per-entity membership bits plus the character role tags
    /// accumulated from those entity rows.
    #[serde(default)]
    pub met_character_tags: MembershipMask,
}

impl WorldStateTable {
    /// Canonical table: `MAX_ENTITIES` entity containers and `MAX_METRICS`
    /// metric slots, all zeroed.
    pub fn new() -> Self {
        WorldStateTable {
            global_tags: TagBitset::new(),
            entity_tags: vec![TagBitset::new(); MAX_ENTITIES],
            metrics: vec![0.0; MAX_METRICS],
            metric_bounds: vec![MetricBound::unbounded(); MAX_METRICS],
            passions: PassionState::with_capacity(MAX_ENTITIES, MAX_METRICS),
            vertices: Vec::new(),
            edges: Vec::new(),
            current_location_entity: None,
            current_location_tag_mask: TagBitset::new(),
            visited_location_tags: MembershipMask::new(),
            met_character_tags: MembershipMask::new(),
        }
    }

    /// Smaller table for a bounded scenario; sizes are capped at the maxima.
    pub fn with_capacity(entity_count: usize, metric_count: usize) -> Self {
        let entities = entity_count.min(MAX_ENTITIES);
        let metrics = metric_count.min(MAX_METRICS);
        WorldStateTable {
            global_tags: TagBitset::new(),
            entity_tags: vec![TagBitset::new(); entities],
            metrics: vec![0.0; metrics],
            metric_bounds: vec![MetricBound::unbounded(); metrics],
            vertices: Vec::new(),
            edges: Vec::new(),
            current_location_entity: None,
            current_location_tag_mask: TagBitset::new(),
            visited_location_tags: MembershipMask::new(),
            passions: PassionState::with_capacity(entities, metrics),
            met_character_tags: MembershipMask::new(),
        }
    }

    /// Grows the entity container array up to `MAX_ENTITIES`.
    pub fn ensure_entity(&mut self, entity_id: u16) -> Result<(), ChronoStateError> {
        let needed = entity_id as usize + 1;
        if needed > MAX_ENTITIES {
            return Err(ChronoStateError::EntityIndexOutOfRange { index: entity_id });
        }
        if needed > self.entity_tags.len() {
            self.entity_tags.resize(needed, TagBitset::new());
        }
        Ok(())
    }
    /// Sets the clamping bounds for one metric slot. The bound applies to every
    /// planned write, including coupled writes, while the delta still records
    /// the exact prior value for undo.
    pub fn set_metric_bound(
        &mut self,
        metric_id: u16,
        min: f32,
        max: f32,
    ) -> Result<(), ChronoStateError> {
        if metric_id as usize >= self.metric_bounds.len() {
            return Err(ChronoStateError::MetricIndexOutOfRange { index: metric_id });
        }
        if min.is_nan() || max.is_nan() || min > max {
            return Err(ChronoStateError::InvalidMetricBound { index: metric_id });
        }
        self.metric_bounds[metric_id as usize] = MetricBound { min, max };
        Ok(())
    }

    /// Seeds the current location and the tags it contributes to the global
    /// mask. Call this at scenario start so a later move removes exactly the
    /// contributed bits.
    pub fn seed_current_location(&mut self, entity_id: u16) -> Result<(), ChronoStateError> {
        if entity_id as usize >= self.entity_tags.len() {
            return Err(ChronoStateError::EntityIndexOutOfRange { index: entity_id });
        }
        let row = self.entity_tags[entity_id as usize];
        for word in 0..WORD_COUNT {
            self.current_location_tag_mask.words[word] |= row.words[word];
            self.global_tags.words[word] |= row.words[word];
        }
        self.current_location_entity = Some(entity_id);
        Ok(())
    }

    /// Vertex that represents an authored entity row, if one exists.
    pub fn vertex_for_entity(&self, entity_index: u16) -> Option<&WorldVertex> {
        self.vertices
            .iter()
            .find(|vertex| vertex.entity_index == Some(entity_index))
    }

    /// Places `location_current_bit` on every actor row whose vertex is linked
    /// to `location_vertex_id` by a relation of `relation_type`, and clears the
    /// bit from every other actor row. Vertices are scanned in table order
    /// (sorted by id) and every flip is recorded in `delta`, so rollback
    /// restores each row exactly.
    pub fn sync_actor_presence(
        &mut self,
        delta: &mut BijectiveDelta,
        location_vertex_id: &str,
        location_current_bit: u16,
        relation_type: &str,
    ) -> Result<(), ChronoStateError> {
        if location_current_bit as usize >= MAX_TAGS {
            return Err(ChronoStateError::TagIndexOutOfRange {
                index: location_current_bit,
            });
        }
        let actors: Vec<(String, u16)> = self
            .vertices
            .iter()
            .filter(|vertex| vertex.layer == GraphLayer::Character)
            .filter_map(|vertex| {
                vertex
                    .entity_index
                    .map(|index| (vertex.vertex_id.clone(), index))
            })
            .collect();
        for (vertex_id, entity_index) in actors {
            if entity_index as usize >= self.entity_tags.len() {
                return Err(ChronoStateError::EntityIndexOutOfRange {
                    index: entity_index,
                });
            }
            let present = self.edges.iter().any(|edge| {
                edge.relation_type == relation_type
                    && ((edge.source_id == location_vertex_id && edge.target_id == vertex_id)
                        || (edge.target_id == location_vertex_id && edge.source_id == vertex_id))
            });
            let before = self.entity_tags[entity_index as usize];
            let row = &mut self.entity_tags[entity_index as usize];
            if present {
                row.set_bit(location_current_bit)?;
            } else {
                row.clear_bit(location_current_bit)?;
            }
            let mask = before.xor_with(row);
            if !mask.is_empty() {
                delta.entity_xor_masks.push(EntityMaskPair {
                    entity_id: entity_index,
                    xor_mask: mask,
                });
            }
        }
        Ok(())
    }

    pub fn vertex(&self, vertex_id: &str) -> Option<&WorldVertex> {
        let index = self
            .vertices
            .partition_point(|v| v.vertex_id.as_str() < vertex_id);
        self.vertices
            .get(index)
            .filter(|v| v.vertex_id == vertex_id)
    }

    pub fn edge(&self, edge_id: &str) -> Option<&WorldEdge> {
        let index = self.edges.partition_point(|e| e.edge_id.as_str() < edge_id);
        self.edges.get(index).filter(|e| e.edge_id == edge_id)
    }

    /// Matches a left-hand side pattern deterministically. Patterns are tried
    /// in declaration order and candidates in sorted id order; vertices and
    /// edges backtrack when a later element fails. The search is bounded.
    pub fn match_graph_pattern(
        &self,
        pattern: &GraphPattern,
    ) -> Result<GraphMatch, ChronoStateError> {
        let mut chosen = Vec::with_capacity(pattern.vertices.len());
        let mut used_vertices = BTreeSet::new();
        let mut edges = Vec::with_capacity(pattern.edges.len());
        let mut used_edges = BTreeSet::new();
        let mut budget = PATTERN_SEARCH_BUDGET;
        if !self.match_pattern_vertices(
            pattern,
            0,
            &mut used_vertices,
            &mut chosen,
            &mut used_edges,
            &mut edges,
            &mut budget,
        )? {
            return Err(ChronoStateError::PatternNotMatched { pattern_index: 0 });
        }
        Ok(GraphMatch {
            vertices: chosen,
            edges,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn match_pattern_vertices(
        &self,
        pattern: &GraphPattern,
        index: usize,
        used: &mut BTreeSet<String>,
        chosen: &mut Vec<String>,
        used_edges: &mut BTreeSet<String>,
        matched_edges: &mut Vec<String>,
        budget: &mut usize,
    ) -> Result<bool, ChronoStateError> {
        if index == pattern.vertices.len() {
            // Edges constrain the vertex assignment, so a failed edge match
            // must send the caller back to reconsider its last vertex.
            return self.match_pattern_edges(
                pattern,
                0,
                chosen,
                used_edges,
                matched_edges,
                budget,
            );
        }
        let vertex_pattern = &pattern.vertices[index];
        for vertex in &self.vertices {
            if vertex.layer != vertex_pattern.layer {
                continue;
            }
            if let Some(id) = &vertex_pattern.vertex_id {
                if &vertex.vertex_id != id {
                    continue;
                }
            }
            if !matches_attributes(vertex, &vertex_pattern.attributes) {
                continue;
            }
            if used.contains(&vertex.vertex_id) {
                continue;
            }
            // Only viable candidates consume the backtracking budget.
            if *budget == 0 {
                return Err(ChronoStateError::PatternSearchExhausted);
            }
            *budget -= 1;
            used.insert(vertex.vertex_id.clone());
            chosen.push(vertex.vertex_id.clone());
            if self.match_pattern_vertices(
                pattern,
                index + 1,
                used,
                chosen,
                used_edges,
                matched_edges,
                budget,
            )? {
                return Ok(true);
            }
            chosen.pop();
            used.remove(&vertex.vertex_id);
        }
        Ok(false)
    }

    fn match_pattern_edges(
        &self,
        pattern: &GraphPattern,
        index: usize,
        chosen: &[String],
        used: &mut BTreeSet<String>,
        matched: &mut Vec<String>,
        budget: &mut usize,
    ) -> Result<bool, ChronoStateError> {
        if index == pattern.edges.len() {
            return Ok(true);
        }
        let edge_pattern = &pattern.edges[index];
        for edge in &self.edges {
            if used.contains(&edge.edge_id) {
                continue;
            }
            if let Some(edge_id) = &edge_pattern.edge_id {
                if &edge.edge_id != edge_id {
                    continue;
                }
            }
            if let Some(relation) = &edge_pattern.relation_type {
                if &edge.relation_type != relation {
                    continue;
                }
            }
            if let Some(source) = &edge_pattern.source {
                if resolve_binding(source, chosen)? != edge.source_id {
                    continue;
                }
            }
            if let Some(target) = &edge_pattern.target {
                if resolve_binding(target, chosen)? != edge.target_id {
                    continue;
                }
            }
            if *budget == 0 {
                return Err(ChronoStateError::PatternSearchExhausted);
            }
            *budget -= 1;
            used.insert(edge.edge_id.clone());
            matched.push(edge.edge_id.clone());
            if self.match_pattern_edges(pattern, index + 1, chosen, used, matched, budget)? {
                return Ok(true);
            }
            matched.pop();
            used.remove(&edge.edge_id);
        }
        Ok(false)
    }

    /// Applies a graph transformation rule and returns the cache needed to
    /// reverse it exactly (REQ-WB-03).
    pub fn apply_graph_transform(
        &mut self,
        rule: &GraphTransformRule,
    ) -> Result<GraphDiffRecord, ChronoStateError> {
        let match_ = self.match_graph_pattern(&rule.lhs)?;

        // Resolve and validate the right-hand side before anything changes.
        let removed_vertex_ids = resolve_ids(&rule.rhs.remove_vertices, &match_.vertices)?;
        for id in &removed_vertex_ids {
            if id.is_empty() {
                return Err(ChronoStateError::EmptyVertexId);
            }
            if self.vertex(id).is_none() {
                return Err(ChronoStateError::VertexNotFound {
                    vertex_id: id.clone(),
                });
            }
        }
        let mut removed_edge_ids = resolve_edge_ids(&rule.rhs.remove_edges, &match_)?;
        for edge in &self.edges {
            if removed_vertex_ids.contains(&edge.source_id)
                || removed_vertex_ids.contains(&edge.target_id)
            {
                removed_edge_ids.push(edge.edge_id.clone());
            }
        }
        removed_edge_ids.sort();
        removed_edge_ids.dedup();
        for id in &removed_edge_ids {
            if id.is_empty() {
                return Err(ChronoStateError::EmptyEdgeId);
            }
            if self.edge(id).is_none() {
                return Err(ChronoStateError::EdgeNotFound {
                    edge_id: id.clone(),
                });
            }
        }

        let mut added_vertices: Vec<WorldVertex> = Vec::new();
        for template in &rule.rhs.add_vertices {
            let id = resolve_binding(&template.id, &match_.vertices)?;
            if id.is_empty() {
                return Err(ChronoStateError::EmptyVertexId);
            }
            // Removing and re-adding the same id in one rule moves or replaces
            // the vertex instead of duplicating it.
            if (self.vertex(&id).is_some() && !removed_vertex_ids.contains(&id))
                || added_vertices.iter().any(|v| v.vertex_id == id)
            {
                return Err(ChronoStateError::DuplicateVertexId { vertex_id: id });
            }
            for (key, value) in &template.attributes {
                if key.is_empty() {
                    return Err(ChronoStateError::EmptyAttributeKey);
                }
                if !value.is_finite() {
                    return Err(ChronoStateError::NonFiniteAttributeValue {
                        vertex_id: id.clone(),
                        key: key.clone(),
                        value: value.as_f64().unwrap_or_default(),
                    });
                }
            }
            added_vertices.push(WorldVertex {
                vertex_id: id,
                layer: template.layer,
                entity_index: None,
                attributes: template.attributes.clone(),
            });
        }

        // Spawned vertices count as present, so a rule can create a subgraph
        // (or retarget onto a new vertex) in one transform.
        let present: BTreeSet<String> = self
            .vertices
            .iter()
            .map(|v| v.vertex_id.clone())
            .filter(|id| !removed_vertex_ids.contains(id))
            .chain(added_vertices.iter().map(|v| v.vertex_id.clone()))
            .collect();

        let mut added_edges: Vec<WorldEdge> = Vec::new();
        for template in &rule.rhs.add_edges {
            let id = resolve_binding(&template.id, &match_.vertices)?;
            if id.is_empty() {
                return Err(ChronoStateError::EmptyEdgeId);
            }
            // A removed edge may be re-added under the same id.
            if (self.edge(&id).is_some() && !removed_edge_ids.contains(&id))
                || added_edges.iter().any(|e| e.edge_id == id)
            {
                return Err(ChronoStateError::DuplicateEdgeId { edge_id: id });
            }
            if !template.weight.is_finite() {
                return Err(ChronoStateError::NonFiniteEdgeWeight {
                    edge_id: id.clone(),
                    value: template.weight,
                });
            }
            let source = resolve_binding(&template.source, &match_.vertices)?;
            let target = resolve_binding(&template.target, &match_.vertices)?;
            if !present.contains(&source) || !present.contains(&target) {
                return Err(ChronoStateError::VertexNotFound {
                    vertex_id: if !present.contains(&source) {
                        source
                    } else {
                        target
                    },
                });
            }
            added_edges.push(WorldEdge {
                edge_id: id,
                source_id: source,
                target_id: target,
                relation_type: template.relation_type.clone(),
                weight: template.weight,
            });
        }

        let mut assignments = Vec::new();
        for assignment in &rule.rhs.set_attributes {
            let vertex_id = resolve_binding(&assignment.vertex, &match_.vertices)?;
            if vertex_id.is_empty() {
                return Err(ChronoStateError::EmptyVertexId);
            }
            if assignment.key.is_empty() {
                return Err(ChronoStateError::EmptyAttributeKey);
            }
            if !assignment.value.is_finite() {
                return Err(ChronoStateError::NonFiniteAttributeValue {
                    vertex_id: vertex_id.clone(),
                    key: assignment.key.clone(),
                    value: assignment.value.as_f64().unwrap_or_default(),
                });
            }
            if !present.contains(&vertex_id) {
                return Err(ChronoStateError::VertexNotFound { vertex_id });
            }
            assignments.push((vertex_id, assignment.key.clone(), assignment.value.clone()));
        }

        // Everything is validated; now mutate and cache for exact reversal.
        let mut diff = GraphDiffRecord::default();

        for id in &removed_vertex_ids {
            if let Some(vertex) = self.take_vertex(id) {
                diff.removed_vertices.push(vertex);
            }
        }
        for id in &removed_edge_ids {
            if let Some(edge) = self.take_edge(id) {
                diff.removed_edges.push(edge);
            }
        }
        for vertex in added_vertices {
            diff.added_vertex_ids.push(vertex.vertex_id.clone());
            self.insert_vertex_sorted(vertex);
        }
        for edge in added_edges {
            diff.added_edge_ids.push(edge.edge_id.clone());
            self.insert_edge_sorted(edge);
        }
        for (vertex_id, key, value) in assignments {
            let vertex = self
                .vertices
                .iter_mut()
                .find(|v| v.vertex_id == vertex_id)
                .expect("validated vertex");
            // Only the first write in a diff records the original value;
            // overwriting it would make the reverse restore an intermediate
            // value.
            diff.previous_attributes
                .entry(vertex_id)
                .or_default()
                .entry(key.clone())
                .or_insert_with(|| vertex.attributes.get(&key).cloned());
            vertex.attributes.insert(key, value);
        }

        Ok(diff)
    }

    /// Reverses a graph transformation, restoring the exact prior tables.
    pub fn revert_graph_transform(
        &mut self,
        diff: &GraphDiffRecord,
    ) -> Result<(), ChronoStateError> {
        for id in &diff.added_vertex_ids {
            if self.vertex(id).is_none() {
                return Err(ChronoStateError::VertexNotFound {
                    vertex_id: id.clone(),
                });
            }
        }
        for id in &diff.added_edge_ids {
            if self.edge(id).is_none() {
                return Err(ChronoStateError::EdgeNotFound {
                    edge_id: id.clone(),
                });
            }
        }
        for vertex in &diff.removed_vertices {
            // Remove-and-re-add in one rule is legal; the added copy is removed
            // before the original is restored.
            if self.vertex(&vertex.vertex_id).is_some()
                && !diff.added_vertex_ids.contains(&vertex.vertex_id)
            {
                return Err(ChronoStateError::DuplicateVertexId {
                    vertex_id: vertex.vertex_id.clone(),
                });
            }
        }
        for edge in &diff.removed_edges {
            if self.edge(&edge.edge_id).is_some()
                && !diff.added_edge_ids.contains(&edge.edge_id)
            {
                return Err(ChronoStateError::DuplicateEdgeId {
                    edge_id: edge.edge_id.clone(),
                });
            }
        }

        for id in &diff.added_vertex_ids {
            self.take_vertex(id);
        }
        for id in &diff.added_edge_ids {
            self.take_edge(id);
        }
        for vertex in &diff.removed_vertices {
            self.insert_vertex_sorted(vertex.clone());
        }
        for edge in &diff.removed_edges {
            self.insert_edge_sorted(edge.clone());
        }
        for (vertex_id, attributes) in &diff.previous_attributes {
            let vertex = self
                .vertices
                .iter_mut()
                .find(|v| v.vertex_id == *vertex_id)
                .ok_or_else(|| ChronoStateError::VertexNotFound {
                    vertex_id: vertex_id.clone(),
                })?;
            for (key, value) in attributes {
                match value {
                    Some(value) => {
                        vertex.attributes.insert(key.clone(), value.clone());
                    }
                    None => {
                        vertex.attributes.remove(key);
                    }
                }
            }
        }

        Ok(())
    }

    fn take_vertex(&mut self, vertex_id: &str) -> Option<WorldVertex> {
        let index = self
            .vertices
            .partition_point(|v| v.vertex_id.as_str() < vertex_id);
        if index < self.vertices.len() && self.vertices[index].vertex_id == vertex_id {
            Some(self.vertices.remove(index))
        } else {
            None
        }
    }

    fn take_edge(&mut self, edge_id: &str) -> Option<WorldEdge> {
        let index = self.edges.partition_point(|e| e.edge_id.as_str() < edge_id);
        if index < self.edges.len() && self.edges[index].edge_id == edge_id {
            Some(self.edges.remove(index))
        } else {
            None
        }
    }

    fn insert_vertex_sorted(&mut self, vertex: WorldVertex) {
        let index = self
            .vertices
            .partition_point(|v| v.vertex_id < vertex.vertex_id);
        self.vertices.insert(index, vertex);
    }

    fn insert_edge_sorted(&mut self, edge: WorldEdge) {
        let index = self.edges.partition_point(|e| e.edge_id < edge.edge_id);
        self.edges.insert(index, edge);
    }
}

/// One per-entity tag mutation.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EntityTagMutation {
    pub entity_id: u16,
    pub add_tags: TagBitset,
    pub remove_tags: TagBitset,
}

/// Atomic mutation request (REQ-REV-01).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MutationOp {
    pub add_global_tags: TagBitset,
    pub remove_global_tags: TagBitset,
    #[serde(default)]
    pub metric_deltas: Vec<(u16, f32)>,
    #[serde(default)]
    pub entity_tag_ops: Vec<EntityTagMutation>,
    /// Reversible player movement: sets the current location entity and marks
    /// the destination visited. `None` leaves the location unchanged.
    #[serde(default)]
    pub move_player: Option<u16>,
    /// Records a visited-location bit for the given entity.
    #[serde(default)]
    pub visit_location: Option<u16>,
    /// Records a met-character bit for the given entity.
    #[serde(default)]
    pub meet_character: Option<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetricUndoPair {
    pub metric_id: u16,
    pub old_value: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityMaskPair {
    pub entity_id: u16,
    pub xor_mask: TagBitset,
}

/// Universal reversible record: XOR masks for tags plus the first prior value
/// per metric (REQ-REV-02).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BijectiveDelta {
    pub global_xor_mask: TagBitset,
    #[serde(default)]
    pub entity_xor_masks: Vec<EntityMaskPair>,
    #[serde(default)]
    pub metric_undos: Vec<MetricUndoPair>,
    #[serde(default)]
    pub visited_location_entity_xor: TagBitset,
    #[serde(default)]
    pub visited_location_xor: TagBitset,
    #[serde(default)]
    pub met_character_entity_xor: TagBitset,
    #[serde(default)]
    pub met_character_xor: TagBitset,
    #[serde(default)]
    pub location_tag_mask_xor: TagBitset,
    #[serde(default)]
    pub previous_current_location: Option<u16>,
    /// Inverse cache for Star-NPC passion mutations and threshold flags.
    #[serde(default)]
    pub passion_delta: Option<PassionDelta>,
    /// Reversible layered-world graph transformations in application order.
    #[serde(default)]
    pub graph_diffs: Vec<GraphDiffRecord>,
}

impl BijectiveDelta {
    pub fn has_recorded_metric(&self, metric_id: u16) -> bool {
        self.metric_undos
            .iter()
            .any(|undo| undo.metric_id == metric_id)
    }

    pub fn is_empty(&self) -> bool {
        self.global_xor_mask.is_empty()
            && self.location_tag_mask_xor.is_empty()
            && self.visited_location_entity_xor.is_empty()
            && self.visited_location_xor.is_empty()
            && self.met_character_entity_xor.is_empty()
            && self.met_character_xor.is_empty()
            && self.entity_xor_masks.is_empty()
            && self.metric_undos.is_empty()
            && self.passion_delta.is_none()
            && self.graph_diffs.is_empty()
    }
}

/// Applies a mutation without a registry: no coupling propagation and no
/// threshold evaluation.
pub fn apply_mutation(
    state: &mut WorldStateTable,
    op: &MutationOp,
) -> Result<BijectiveDelta, ChronoStateError> {
    apply_mutation_with_registry(state, op, &GameplayTagRegistry::empty())
}

/// Applies a mutation through the registry: metric deltas, single-pass CSR
/// zero-sum propagation, deterministic threshold evaluation, entity tag masks,
/// and the visited/met/location bookkeeping. Every input is validated before
/// anything changes, so a failed call leaves the state untouched.
pub fn apply_mutation_with_registry(
    state: &mut WorldStateTable,
    op: &MutationOp,
    registry: &GameplayTagRegistry,
) -> Result<BijectiveDelta, ChronoStateError> {
    let plan = plan_metric_changes(state, op, registry)?;
    validate_threshold_rules(state, registry)?;
    validate_entity_bounds(state, op)?;

    let mut delta = BijectiveDelta::default();
    delta.previous_current_location = state.current_location_entity;

    // Actor presence markers are recomputed from `located_at` edges before any
    // other state changes, so explicit entity op masks still win afterwards.
    if let Some(entity_id) = op.move_player {
        if let Some(bit) = registry.id(LOCATION_CURRENT_TAG) {
            let location_vertex_id = state
                .vertex_for_entity(entity_id)
                .map(|vertex| vertex.vertex_id.clone())
                .ok_or_else(|| ChronoStateError::EntityVertexMissing { index: entity_id })?;
            state.sync_actor_presence(&mut delta, &location_vertex_id, bit, LOCATED_AT_RELATION)?;
        }
    }

    // Metrics: record the first prior value per metric, then write the planned
    // value. Iterating the plan in sorted order keeps the record deterministic.
    for (metric_id, new_value) in &plan {
        delta.metric_undos.push(MetricUndoPair {
            metric_id: *metric_id,
            old_value: state.metrics[*metric_id as usize],
        });
        state.metrics[*metric_id as usize] = *new_value;
    }

    // Global tags. The location contribution is swapped first so that explicit
    // op masks and threshold bits are never clobbered by movement.
    let mut new_global = state.global_tags;
    let mut new_location_mask = state.current_location_tag_mask;
    if let Some(entity_id) = op.move_player {
        for word in 0..WORD_COUNT {
            new_global.words[word] &= !new_location_mask.words[word];
        }
        new_location_mask = state.entity_tags[entity_id as usize];
        for word in 0..WORD_COUNT {
            new_global.words[word] |= new_location_mask.words[word];
        }
    }
    for word in 0..WORD_COUNT {
        new_global.words[word] = (new_global.words[word] | op.add_global_tags.words[word])
            & !op.remove_global_tags.words[word];
    }

    // Threshold evaluation with hysteresis, in deterministic rule order.
    for rule in registry.threshold_rules() {
        let value = state.metrics[rule.metric_id as usize];
        let active = new_global.bit(rule.threshold_tag_bit);
        if !active {
            if value >= rule.trigger_value {
                new_global.set_bit(rule.threshold_tag_bit)?;
            }
        } else if value < rule.trigger_value - rule.hysteresis {
            new_global.clear_bit(rule.threshold_tag_bit)?;
        }
    }
    delta.global_xor_mask = state.global_tags.xor_with(&new_global);
    delta.location_tag_mask_xor = state.current_location_tag_mask.xor_with(&new_location_mask);
    state.global_tags = new_global;
    state.current_location_tag_mask = new_location_mask;

    // Per-entity tag containers.
    for item in &op.entity_tag_ops {
        let before = state.entity_tags[item.entity_id as usize];
        let container = &mut state.entity_tags[item.entity_id as usize];
        for word in 0..WORD_COUNT {
            container.words[word] =
                (container.words[word] | item.add_tags.words[word]) & !item.remove_tags.words[word];
        }
        let mask = before.xor_with(container);
        if !mask.is_empty() {
            delta.entity_xor_masks.push(EntityMaskPair {
                entity_id: item.entity_id,
                xor_mask: mask,
            });
        }
    }

    // Player movement, visited locations, and met characters. Each records the
    // prior pointer plus the exact membership and tag-mask bits it flipped.
    if let Some(entity_id) = op.move_player {
        mark_membership(state, &mut delta, entity_id, true)?;
        state.current_location_entity = Some(entity_id);
    }
    if let Some(entity_id) = op.visit_location {
        mark_membership(state, &mut delta, entity_id, true)?;
    }
    if let Some(entity_id) = op.meet_character {
        mark_membership(state, &mut delta, entity_id, false)?;
    }

    Ok(delta)
}

/// Reverses a delta, restoring tag bitsets and float bit patterns exactly.
pub fn revert_delta(
    state: &mut WorldStateTable,
    delta: &BijectiveDelta,
) -> Result<(), ChronoStateError> {
    for pair in &delta.entity_xor_masks {
        if pair.entity_id as usize >= state.entity_tags.len() {
            return Err(ChronoStateError::EntityIndexOutOfRange {
                index: pair.entity_id,
            });
        }
    }
    for undo in &delta.metric_undos {
        if undo.metric_id as usize >= state.metrics.len() {
            return Err(ChronoStateError::MetricIndexOutOfRange {
                index: undo.metric_id,
            });
        }
    }
    for graph_diff in delta.graph_diffs.iter().rev() {
        state.revert_graph_transform(graph_diff)?;
    }
    if let Some(passion_delta) = &delta.passion_delta {
        revert_passion_delta(&mut state.passions, passion_delta, &mut state.global_tags)?;
    }

    state.global_tags.apply_xor(&delta.global_xor_mask);
    state
        .current_location_tag_mask
        .apply_xor(&delta.location_tag_mask_xor);
    state.visited_location_tags.apply_membership_xor(
        &delta.visited_location_entity_xor,
        &delta.visited_location_xor,
    );
    state
        .met_character_tags
        .apply_membership_xor(&delta.met_character_entity_xor, &delta.met_character_xor);
    for pair in &delta.entity_xor_masks {
        state.entity_tags[pair.entity_id as usize].apply_xor(&pair.xor_mask);
    }
    for undo in &delta.metric_undos {
        state.metrics[undo.metric_id as usize] = undo.old_value;
    }
    state.current_location_entity = delta.previous_current_location;

    Ok(())
}

/// Dense Star-NPC passion values, macro values, and active-passion flags.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PassionState {
    pub micro: Vec<f32>,
    pub macro_values: Vec<f32>,
    pub active: Vec<bool>,
}

impl PassionState {
    pub fn new() -> Self {
        Self::with_capacity(0, 0)
    }

    pub fn with_capacity(npc_count: usize, passion_count: usize) -> Self {
        let npcs = npc_count.min(MAX_ENTITIES);
        let passions = passion_count.min(MAX_METRICS);
        PassionState {
            micro: vec![0.0; passions],
            macro_values: vec![0.0; npcs],
            active: vec![false; passions],
        }
    }
}

/// Passion changes and active-pool selection for one authored action.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PassionMutation {
    pub micro_deltas: Vec<(u16, f32)>,
    pub active_ops: Vec<(u16, bool)>,
}

/// Runtime notification for a threshold-linked narrative graph.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PassionThresholdEvent {
    pub npc_id: u16,
    pub threshold_tag_bit: u16,
    pub linked_graph_asset_id: String,
    pub activated: bool,
}

/// The exact inverse data for one passion mutation, plus its forward events.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PassionDelta {
    pub micro_undos: Vec<(u16, f32)>,
    pub macro_undos: Vec<(u16, f32)>,
    pub active_undos: Vec<(u16, bool)>,
    pub tag_xor: TagBitset,
    pub threshold_events: Vec<PassionThresholdEvent>,
}

impl PassionDelta {
    pub fn is_empty(&self) -> bool {
        self.micro_undos.is_empty()
            && self.macro_undos.is_empty()
            && self.active_undos.is_empty()
            && self.tag_xor.is_empty()
            && self.threshold_events.is_empty()
    }
}

/// Applies a passion change, propagates signed zero-sum links, recalculates
/// weighted-average macro values from active passions, and records crossings.
pub fn apply_passion_mutation(
    state: &mut PassionState,
    op: &PassionMutation,
    registry: &PassionRegistry,
    flags: &mut TagBitset,
) -> Result<PassionDelta, ChronoStateError> {
    let mut active_plan = BTreeMap::new();
    for (passion_id, wanted) in &op.active_ops {
        if *passion_id as usize >= state.active.len() {
            return Err(ChronoStateError::MetricIndexOutOfRange { index: *passion_id });
        }
        if registry.owner_of(*passion_id).is_none() {
            return Err(ChronoStateError::UnknownMicroPassion {
                passion_id: *passion_id,
            });
        }
        active_plan.insert(*passion_id, *wanted);
    }

    let mut micro_plan = BTreeMap::new();
    let mut changed_passions = BTreeSet::new();
    for (passion_id, change) in &op.micro_deltas {
        let passion_id = *passion_id;
        if passion_id as usize >= state.micro.len() {
            return Err(ChronoStateError::MetricIndexOutOfRange { index: passion_id });
        }
        if registry.owner_of(passion_id).is_none() {
            return Err(ChronoStateError::UnknownMicroPassion { passion_id });
        }
        if !active_plan
            .get(&passion_id)
            .copied()
            .unwrap_or(state.active[passion_id as usize])
        {
            continue;
        }
        let base = micro_plan
            .get(&passion_id)
            .copied()
            .unwrap_or(state.micro[passion_id as usize]);
        add_planned_passion_value(&mut micro_plan, base, *change, passion_id)?;
        changed_passions.insert(passion_id);
    }

    let mut rival_plan = BTreeMap::new();
    for (passion_id, change) in &op.micro_deltas {
        if !changed_passions.contains(passion_id) {
            continue;
        }
        let owner =
            registry
                .owner_of(*passion_id)
                .ok_or(ChronoStateError::UnknownMicroPassion {
                    passion_id: *passion_id,
                })?;
        let npc = registry
            .npc(owner)
            .ok_or(ChronoStateError::UnknownStarNpc { npc_id: owner })?;
        for link in &npc.zero_sum_links {
            if link.source_passion_id != *passion_id
                || !active_plan
                    .get(&link.target_passion_id)
                    .copied()
                    .unwrap_or_else(|| {
                        state
                            .active
                            .get(link.target_passion_id as usize)
                            .copied()
                            .unwrap_or(false)
                    })
            {
                continue;
            }
            let contribution = *change * link.weight;
            let target_id = link.target_passion_id;
            let base = rival_plan
                .get(&target_id)
                .copied()
                .or_else(|| micro_plan.get(&target_id).copied())
                .unwrap_or_else(|| state.micro.get(target_id as usize).copied().unwrap_or(0.0));
            add_planned_passion_value(&mut rival_plan, base, contribution, target_id)?;
            changed_passions.insert(target_id);
        }
    }
    for (passion_id, value) in rival_plan {
        micro_plan.insert(passion_id, value);
    }

    let mut macro_plan = BTreeMap::new();
    for npc in registry.npcs.values() {
        let touched = npc.micro_passions.iter().any(|micro| {
            changed_passions.contains(&micro.passion_id)
                || active_plan.contains_key(&micro.passion_id)
        });
        if !touched {
            continue;
        }
        if npc.npc_id as usize >= state.macro_values.len() {
            return Err(ChronoStateError::EntityIndexOutOfRange { index: npc.npc_id });
        }
        // A registry compiled against a larger capacity would otherwise index
        // these vectors out of bounds.
        if let Some(micro) = npc
            .micro_passions
            .iter()
            .find(|micro| {
                micro.passion_id as usize >= state.micro.len()
                    || micro.passion_id as usize >= state.active.len()
            })
        {
            return Err(ChronoStateError::MetricIndexOutOfRange {
                index: micro.passion_id,
            });
        }
        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;
        for micro in &npc.micro_passions {
            if !active_plan
                .get(&micro.passion_id)
                .copied()
                .unwrap_or(state.active[micro.passion_id as usize])
            {
                continue;
            }
            let value = micro_plan
                .get(&micro.passion_id)
                .copied()
                .unwrap_or(state.micro[micro.passion_id as usize]);
            weighted_sum += value * micro.weight;
            total_weight += micro.weight;
        }
        if !weighted_sum.is_finite() || !total_weight.is_finite() {
            return Err(ChronoStateError::MetricOverflow {
                metric_id: npc.npc_id,
                value: weighted_sum,
            });
        }
        if total_weight > 0.0 {
            let raw = weighted_sum / total_weight + npc.trust_inertia;
            if !raw.is_finite() {
                return Err(ChronoStateError::MetricOverflow {
                    metric_id: npc.npc_id,
                    value: raw,
                });
            }
            // Spec macro bounds are [-100, 100]; intersect with the authored
            // bound so an unbounded default still obeys the spec range.
            let min = npc.macro_bounds.min.max(-100.0);
            let max = npc.macro_bounds.max.min(100.0);
            if min > max {
                return Err(ChronoStateError::InvalidMetricBound { index: npc.npc_id });
            }
            macro_plan.insert(npc.npc_id, raw.clamp(min, max));
        }
    }

    let mut new_flags = *flags;
    let mut threshold_events = Vec::new();
    for rule in &registry.thresholds {
        let old = state
            .macro_values
            .get(rule.npc_id as usize)
            .copied()
            .ok_or(ChronoStateError::EntityIndexOutOfRange { index: rule.npc_id })?;
        let new = macro_plan.get(&rule.npc_id).copied().unwrap_or(old);
        // The bit is owned by this rule. Inferring the prior state from it
        // would let an external write suppress or duplicate the crossing, so
        // crossings come from the value transition only.
        let crossing = match rule.direction {
            PassionDirection::Up if old < rule.trigger_value && new >= rule.trigger_value => {
                Some(true)
            }
            PassionDirection::Up if old >= rule.trigger_value && new < rule.trigger_value => {
                Some(false)
            }
            PassionDirection::Down if old > rule.trigger_value && new <= rule.trigger_value => {
                Some(true)
            }
            PassionDirection::Down if old <= rule.trigger_value && new > rule.trigger_value => {
                Some(false)
            }
            _ => None,
        };
        if let Some(activated) = crossing {
            if activated {
                new_flags.set_bit(rule.threshold_tag_bit)?;
            } else {
                new_flags.clear_bit(rule.threshold_tag_bit)?;
            }
            threshold_events.push(PassionThresholdEvent {
                npc_id: rule.npc_id,
                threshold_tag_bit: rule.threshold_tag_bit,
                linked_graph_asset_id: rule.linked_graph_asset_id.clone(),
                activated,
            });
        }
    }

    let mut delta = PassionDelta {
        threshold_events,
        ..PassionDelta::default()
    };
    for (passion_id, value) in &micro_plan {
        delta
            .micro_undos
            .push((*passion_id, state.micro[*passion_id as usize]));
        state.micro[*passion_id as usize] = *value;
    }
    for (npc_id, value) in &macro_plan {
        delta
            .macro_undos
            .push((*npc_id, state.macro_values[*npc_id as usize]));
        state.macro_values[*npc_id as usize] = *value;
    }
    for (passion_id, wanted) in &active_plan {
        if state.active[*passion_id as usize] != *wanted {
            delta
                .active_undos
                .push((*passion_id, state.active[*passion_id as usize]));
            state.active[*passion_id as usize] = *wanted;
        }
    }
    delta.tag_xor = flags.xor_with(&new_flags);
    *flags = new_flags;
    Ok(delta)
}

/// Restores all passion values, active flags, macro values, and threshold tags.
pub fn revert_passion_delta(
    state: &mut PassionState,
    delta: &PassionDelta,
    flags: &mut TagBitset,
) -> Result<(), ChronoStateError> {
    for (passion_id, _) in &delta.micro_undos {
        if *passion_id as usize >= state.micro.len() {
            return Err(ChronoStateError::MetricIndexOutOfRange { index: *passion_id });
        }
    }
    for (passion_id, _) in &delta.active_undos {
        if *passion_id as usize >= state.active.len() {
            return Err(ChronoStateError::MetricIndexOutOfRange { index: *passion_id });
        }
    }
    for (npc_id, _) in &delta.macro_undos {
        if *npc_id as usize >= state.macro_values.len() {
            return Err(ChronoStateError::EntityIndexOutOfRange { index: *npc_id });
        }
    }
    flags.apply_xor(&delta.tag_xor);
    for (passion_id, old_value) in &delta.micro_undos {
        state.micro[*passion_id as usize] = *old_value;
    }
    for (npc_id, old_value) in &delta.macro_undos {
        state.macro_values[*npc_id as usize] = *old_value;
    }
    for (passion_id, old_value) in delta.active_undos.iter().rev() {
        state.active[*passion_id as usize] = *old_value;
    }
    Ok(())
}

fn add_planned_passion_value(
    plan: &mut BTreeMap<u16, f32>,
    base: f32,
    change: f32,
    index: u16,
) -> Result<(), ChronoStateError> {
    if !change.is_finite() {
        return Err(ChronoStateError::NonFiniteMetric {
            metric_id: index,
            value: change,
        });
    }
    let next = base + change;
    if !next.is_finite() {
        return Err(ChronoStateError::MetricOverflow {
            metric_id: index,
            value: next,
        });
    }
    plan.insert(index, next.clamp(-100.0, 100.0));
    Ok(())
}

fn validate_star_npc(npc: &StarNpcDef) -> Result<(), ChronoStateError> {
    if npc.npc_id as usize >= MAX_ENTITIES {
        return Err(ChronoStateError::EntityIndexOutOfRange { index: npc.npc_id });
    }
    if npc.micro_passions.is_empty() {
        return Err(ChronoStateError::EmptyPassionPool { npc_id: npc.npc_id });
    }
    if !npc.trust_inertia.is_finite() {
        return Err(ChronoStateError::InvalidTrustInertia {
            npc_id: npc.npc_id,
            value: npc.trust_inertia,
        });
    }
    if npc.macro_bounds.min.is_nan()
        || npc.macro_bounds.max.is_nan()
        || npc.macro_bounds.min > npc.macro_bounds.max
    {
        return Err(ChronoStateError::InvalidMetricBound { index: npc.npc_id });
    }
    for micro in &npc.micro_passions {
        if micro.passion_id as usize >= MAX_METRICS {
            return Err(ChronoStateError::MetricIndexOutOfRange {
                index: micro.passion_id,
            });
        }
        if !micro.weight.is_finite() {
            return Err(ChronoStateError::NonFiniteMetric {
                metric_id: micro.passion_id,
                value: micro.weight,
            });
        }
    }
    for link in &npc.zero_sum_links {
        if link.target_character_id as usize >= MAX_ENTITIES {
            return Err(ChronoStateError::EntityIndexOutOfRange {
                index: link.target_character_id,
            });
        }
        if !link.weight.is_finite() {
            return Err(ChronoStateError::NonFiniteMetric {
                metric_id: link.source_passion_id,
                value: link.weight,
            });
        }
    }
    Ok(())
}

fn validate_passion_threshold(rule: &PassionThresholdRule) -> Result<(), ChronoStateError> {
    if rule.threshold_tag_bit as usize >= MAX_TAGS {
        return Err(ChronoStateError::TagIndexOutOfRange {
            index: rule.threshold_tag_bit,
        });
    }
    if !rule.trigger_value.is_finite() {
        return Err(ChronoStateError::NonFiniteMetric {
            metric_id: rule.npc_id,
            value: rule.trigger_value,
        });
    }
    Ok(())
}

fn mark_membership(
    state: &mut WorldStateTable,
    delta: &mut BijectiveDelta,
    entity_id: u16,
    visited: bool,
) -> Result<(), ChronoStateError> {
    if entity_id as usize >= state.entity_tags.len() {
        return Err(ChronoStateError::EntityIndexOutOfRange { index: entity_id });
    }
    let row = state.entity_tags[entity_id as usize];
    let membership = if visited {
        &mut state.visited_location_tags
    } else {
        &mut state.met_character_tags
    };
    let before_entity = membership.entity_mask;
    let before_tags = membership.tag_mask;
    membership.entity_mask.set_bit(entity_id)?;
    membership.tag_mask.merge(&row);
    let entity_xor = before_entity.xor_with(&membership.entity_mask);
    let tag_xor = before_tags.xor_with(&membership.tag_mask);
    if visited {
        delta.visited_location_entity_xor = entity_xor;
        delta.visited_location_xor = tag_xor;
    } else {
        delta.met_character_entity_xor = entity_xor;
        delta.met_character_xor = tag_xor;
    }
    Ok(())
}

fn plan_metric_changes(
    state: &WorldStateTable,
    op: &MutationOp,
    registry: &GameplayTagRegistry,
) -> Result<BTreeMap<u16, f32>, ChronoStateError> {
    let mut plan: BTreeMap<u16, f32> = BTreeMap::new();
    for (metric_id, change) in &op.metric_deltas {
        add_planned_metric(&mut plan, state, *metric_id, *change)?;
        for link in registry.links_for(*metric_id) {
            let contribution = change * link.weight;
            if !contribution.is_finite() {
                return Err(ChronoStateError::NonFiniteMetric {
                    metric_id: link.target_metric_id,
                    value: contribution,
                });
            }
            add_planned_metric(&mut plan, state, link.target_metric_id, contribution)?;
        }
    }
    Ok(plan)
}

fn add_planned_metric(
    plan: &mut BTreeMap<u16, f32>,
    state: &WorldStateTable,
    metric_id: u16,
    change: f32,
) -> Result<(), ChronoStateError> {
    if change.is_nan() || change.is_infinite() {
        return Err(ChronoStateError::NonFiniteMetric {
            metric_id,
            value: change,
        });
    }
    if metric_id as usize >= state.metrics.len() {
        return Err(ChronoStateError::MetricIndexOutOfRange { index: metric_id });
    }
    let base = plan
        .get(&metric_id)
        .copied()
        .unwrap_or(state.metrics[metric_id as usize]);
    let next = base + change;
    if !next.is_finite() {
        return Err(ChronoStateError::MetricOverflow {
            metric_id,
            value: next,
        });
    }
    let bound = state.metric_bounds[metric_id as usize];
    if bound.min.is_nan() || bound.max.is_nan() || bound.min > bound.max {
        return Err(ChronoStateError::InvalidMetricBound { index: metric_id });
    }
    plan.insert(metric_id, next.clamp(bound.min, bound.max));
    Ok(())
}

fn validate_threshold_rules(
    state: &WorldStateTable,
    registry: &GameplayTagRegistry,
) -> Result<(), ChronoStateError> {
    for rule in registry.threshold_rules() {
        if rule.metric_id as usize >= state.metrics.len() {
            return Err(ChronoStateError::MetricIndexOutOfRange {
                index: rule.metric_id,
            });
        }
        if rule.threshold_tag_bit as usize >= MAX_TAGS {
            return Err(ChronoStateError::TagIndexOutOfRange {
                index: rule.threshold_tag_bit,
            });
        }
    }
    Ok(())
}

fn validate_entity_bounds(
    state: &WorldStateTable,
    op: &MutationOp,
) -> Result<(), ChronoStateError> {
    for item in &op.entity_tag_ops {
        if item.entity_id as usize >= state.entity_tags.len() {
            return Err(ChronoStateError::EntityIndexOutOfRange {
                index: item.entity_id,
            });
        }
    }
    for entity_id in [op.move_player, op.visit_location, op.meet_character]
        .into_iter()
        .flatten()
    {
        if entity_id as usize >= state.entity_tags.len() {
            return Err(ChronoStateError::EntityIndexOutOfRange { index: entity_id });
        }
    }
    // A move target must be a location vertex; a missing or non-location vertex
    // is rejected before anything mutates.
    if let Some(entity_id) = op.move_player {
        let vertex = state
            .vertex_for_entity(entity_id)
            .ok_or_else(|| ChronoStateError::EntityVertexMissing { index: entity_id })?;
        if vertex.layer != GraphLayer::Location {
            return Err(ChronoStateError::VertexLayerMismatch {
                vertex_id: vertex.vertex_id.clone(),
                layer: vertex.layer,
            });
        }
    }
    Ok(())
}

fn matches_attributes(vertex: &WorldVertex, predicates: &[AttributePredicate]) -> bool {
    predicates.iter().all(|predicate| {
        let Some(actual) = vertex.attributes.get(&predicate.key) else {
            return false;
        };
        evaluate_predicate(actual, &predicate.value, predicate.op)
    })
}

fn evaluate_predicate(actual: &AttributeValue, wanted: &AttributeValue, op: AttributeOp) -> bool {
    match op {
        AttributeOp::Eq => return actual == wanted,
        AttributeOp::Neq => return actual != wanted,
        _ => {}
    }
    let (Some(left), Some(right)) = (actual.as_f64(), wanted.as_f64()) else {
        return false;
    };
    match op {
        AttributeOp::Gt => left > right,
        AttributeOp::Gte => left >= right,
        AttributeOp::Lt => left < right,
        _ => left <= right,
    }
}

fn resolve_binding(binding: &GraphBinding, matched: &[String]) -> Result<String, ChronoStateError> {
    match binding {
        GraphBinding::Literal(id) => Ok(id.clone()),
        GraphBinding::VertexPattern(index) => matched
            .get(*index)
            .cloned()
            .ok_or(ChronoStateError::PatternIndexOutOfRange { index: *index }),
        GraphBinding::EdgePattern(index) => Err(ChronoStateError::PatternIndexOutOfRange {
            index: *index,
        }),
    }
}

fn resolve_edge_ids(
    bindings: &[GraphBinding],
    matched: &GraphMatch,
) -> Result<Vec<String>, ChronoStateError> {
    let mut ids = Vec::with_capacity(bindings.len());
    for binding in bindings {
        let id = match binding {
            GraphBinding::Literal(id) => id.clone(),
            GraphBinding::EdgePattern(index) => matched
                .edges
                .get(*index)
                .cloned()
                .ok_or(ChronoStateError::PatternIndexOutOfRange { index: *index })?,
            GraphBinding::VertexPattern(index) => {
                return Err(ChronoStateError::PatternIndexOutOfRange { index: *index })
            }
        };
        ids.push(id);
    }
    ids.sort();
    Ok(ids)
}

fn resolve_ids(
    bindings: &[GraphBinding],
    matched: &[String],
) -> Result<Vec<String>, ChronoStateError> {
    let mut ids = Vec::with_capacity(bindings.len());
    for binding in bindings {
        ids.push(resolve_binding(binding, matched)?);
    }
    ids.sort();
    Ok(ids)
}

fn validate_tag(tag: &str) -> Result<(), ChronoStateError> {
    if tag.is_empty() {
        return Err(ChronoStateError::EmptyTag);
    }
    if tag.len() > MAX_TAG_LENGTH || !is_valid_hierarchical_name(tag) {
        return Err(ChronoStateError::InvalidTag {
            tag: tag.to_string(),
        });
    }
    Ok(())
}

fn is_valid_hierarchical_name(name: &str) -> bool {
    !name.is_empty()
        && name.split('.').all(|segment| {
            !segment.is_empty()
                && segment.chars().all(|character| {
                    character.is_ascii_alphanumeric() || character == '_' || character == '-'
                })
        })
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChronoStateError {
    EmptyTag,
    InvalidTag {
        tag: String,
    },
    DuplicateTag {
        tag: String,
    },
    TagLimitExceeded {
        count: usize,
    },
    EmptyMetricName,
    InvalidMetricName {
        name: String,
    },
    DuplicateMetricName {
        name: String,
    },
    MetricLimitExceeded {
        count: usize,
    },
    InvalidMetricBound {
        index: u16,
    },
    EntityVertexMissing {
        index: u16,
    },
    VertexLayerMismatch {
        vertex_id: String,
        layer: GraphLayer,
    },
    TagIndexOutOfRange {
        index: u16,
    },
    MetricIndexOutOfRange {
        index: u16,
    },
    EntityIndexOutOfRange {
        index: u16,
    },
    NonFiniteMetric {
        metric_id: u16,
        value: f32,
    },
    MetricOverflow {
        metric_id: u16,
        value: f32,
    },
    InvalidHysteresis {
        metric_id: u16,
        value: f32,
    },
    DuplicateCouplingLink {
        source_metric_id: u16,
        target_metric_id: u16,
    },
    DuplicateThresholdRule {
        metric_id: u16,
        threshold_tag_bit: u16,
    },
    EmptyVertexId,
    EmptyEdgeId,
    EmptyAttributeKey,
    VertexNotFound {
        vertex_id: String,
    },
    EdgeNotFound {
        edge_id: String,
    },
    DuplicateVertexId {
        vertex_id: String,
    },
    DuplicateEdgeId {
        edge_id: String,
    },
    NonFiniteEdgeWeight {
        edge_id: String,
        value: f32,
    },
    NonFiniteAttributeValue {
        vertex_id: String,
        key: String,
        value: f64,
    },
    PatternIndexOutOfRange {
        index: usize,
    },
    PatternNotMatched {
        pattern_index: usize,
    },
    PatternSearchExhausted,
    UnknownStarNpc {
        npc_id: u16,
    },
    DuplicateStarNpc {
        npc_id: u16,
    },
    UnknownMicroPassion {
        passion_id: u16,
    },
    DuplicateMicroPassion {
        passion_id: u16,
    },
    InvalidTrustInertia {
        npc_id: u16,
        value: f32,
    },
    EmptyPassionPool {
        npc_id: u16,
    },
}

impl fmt::Display for ChronoStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTag => write!(formatter, "gameplay tag must not be empty"),
            Self::InvalidTag { tag } => {
                write!(
                    formatter,
                    "gameplay tag '{tag}' is not a valid hierarchical tag"
                )
            }
            Self::DuplicateTag { tag } => write!(formatter, "gameplay tag '{tag}' is duplicated"),
            Self::TagLimitExceeded { count } => write!(
                formatter,
                "{count} tags exceed the {MAX_TAGS} tag bitset capacity"
            ),
            Self::EmptyMetricName => write!(formatter, "metric name must not be empty"),
            Self::InvalidMetricName { name } => {
                write!(
                    formatter,
                    "metric name '{name}' is not a valid hierarchical name"
                )
            }
            Self::DuplicateMetricName { name } => {
                write!(formatter, "metric name '{name}' is duplicated")
            }
            Self::MetricLimitExceeded { count } => write!(
                formatter,
                "{count} metric names exceed the {MAX_METRICS} metric array capacity"
            ),
            Self::InvalidMetricBound { index } => {
                write!(formatter, "metric {index} has an invalid clamping bound")
            }
            Self::EntityVertexMissing { index } => {
                write!(formatter, "entity {index} has no world graph vertex")
            }
            Self::VertexLayerMismatch { vertex_id, layer } => {
                write!(
                    formatter,
                    "vertex '{vertex_id}' is on layer {layer:?}, expected location"
                )
            }
            Self::TagIndexOutOfRange { index } => {
                write!(formatter, "tag index {index} is outside the bitset")
            }
            Self::MetricIndexOutOfRange { index } => {
                write!(
                    formatter,
                    "metric index {index} is outside the metric array"
                )
            }
            Self::EntityIndexOutOfRange { index } => {
                write!(
                    formatter,
                    "entity index {index} is outside the entity table"
                )
            }
            Self::NonFiniteMetric { metric_id, value } => {
                write!(
                    formatter,
                    "metric {metric_id} change must be finite, got {value}"
                )
            }
            Self::MetricOverflow { metric_id, value } => {
                write!(formatter, "metric {metric_id} would overflow to {value}")
            }
            Self::InvalidHysteresis {
                metric_id,
                value,
            } => {
                write!(
                    formatter,
                    "hysteresis for metric {metric_id} must be a non-negative finite value, got {value}"
                )
            }
            Self::DuplicateCouplingLink {
                source_metric_id,
                target_metric_id,
            } => write!(
                formatter,
                "coupling link {source_metric_id} -> {target_metric_id} is duplicated"
            ),
            Self::DuplicateThresholdRule {
                metric_id,
                threshold_tag_bit,
            } => write!(
                formatter,
                "threshold rule for metric {metric_id} on tag bit {threshold_tag_bit} is duplicated"
            ),
            Self::EmptyVertexId => write!(formatter, "vertex id must not be empty"),
            Self::EmptyEdgeId => write!(formatter, "edge id must not be empty"),
            Self::EmptyAttributeKey => write!(formatter, "attribute key must not be empty"),
            Self::VertexNotFound { vertex_id } => {
                write!(formatter, "vertex '{vertex_id}' does not exist")
            }
            Self::EdgeNotFound { edge_id } => write!(formatter, "edge '{edge_id}' does not exist"),
            Self::DuplicateVertexId { vertex_id } => {
                write!(formatter, "vertex '{vertex_id}' already exists")
            }
            Self::DuplicateEdgeId { edge_id } => {
                write!(formatter, "edge '{edge_id}' already exists")
            }
            Self::NonFiniteEdgeWeight { edge_id, value } => {
                write!(
                    formatter,
                    "edge '{edge_id}' weight must be finite, got {value}"
                )
            }
            Self::NonFiniteAttributeValue {
                vertex_id,
                key,
                value,
            } => {
                write!(
                    formatter,
                    "attribute '{key}' on vertex '{vertex_id}' must be finite, got {value}"
                )
            }
            Self::PatternIndexOutOfRange { index } => {
                write!(formatter, "graph pattern index {index} is out of range")
            }
            Self::PatternNotMatched { pattern_index } => {
                write!(
                    formatter,
                    "graph pattern element {pattern_index} matched nothing"
                )
            }
            Self::PatternSearchExhausted => {
                write!(formatter, "graph pattern search exhausted its budget")
            }
            Self::UnknownStarNpc { npc_id } => {
                write!(
                    formatter,
                    "star npc {npc_id} is not in the passion registry"
                )
            }
            Self::DuplicateStarNpc { npc_id } => {
                write!(formatter, "star npc {npc_id} is declared more than once")
            }
            Self::UnknownMicroPassion { passion_id } => {
                write!(
                    formatter,
                    "micro passion {passion_id} is not owned by any star npc"
                )
            }
            Self::DuplicateMicroPassion { passion_id } => {
                write!(
                    formatter,
                    "micro passion {passion_id} is declared more than once"
                )
            }
            Self::InvalidTrustInertia { npc_id, value } => {
                write!(
                    formatter,
                    "trust inertia for star npc {npc_id} must be a finite value in [0, 1], got {value}"
                )
            }
            Self::EmptyPassionPool { npc_id } => {
                write!(formatter, "star npc {npc_id} must declare at least one micro passion")
            }
        }
    }
}

impl Error for ChronoStateError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn tag_bit(registry: &GameplayTagRegistry, name: &str) -> u16 {
        registry.id(name).expect("tag must be compiled")
    }

    #[test]
    fn registries_are_deterministic_and_separate() {
        let tags = GameplayTagRegistry::compile(names(&["A", "B", "Quality.Courage"])).unwrap();
        let shuffled = GameplayTagRegistry::compile(names(&["Quality.Courage", "B", "A"])).unwrap();
        assert_eq!(tags, shuffled);
        assert_eq!(tags.id("A"), Some(0));
        assert_eq!(tags.id("Quality.Courage"), Some(2));

        let metrics = MetricRegistry::compile(names(&["Quality.Courage"])).unwrap();
        assert_eq!(
            metrics,
            MetricRegistry::compile(names(&["Quality.Courage"])).unwrap()
        );
        assert_eq!(metrics.id("Quality.Courage"), Some(0));
        assert_eq!(metrics.count(), 1);
        assert_eq!(metrics.name(0), Some("Quality.Courage"));

        // The two index spaces never cross: a name registered in one is absent
        // from the other.
        assert_eq!(metrics.id("A"), None);
        assert_eq!(tags.id("Passion.Anxiety"), None);
        assert_eq!(tags.tag(2), Some("Quality.Courage"));

        // Writing metric index 0 does not touch tag bit 2.
        let mut state = WorldStateTable::with_capacity(1, 4);
        let delta = apply_mutation(
            &mut state,
            &MutationOp {
                metric_deltas: vec![(0, 5.0)],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(state.metrics[0], 5.0);
        assert!(!state.global_tags.bit(2));
        assert_eq!(delta.global_xor_mask, TagBitset::new());
    }

    #[test]
    fn move_player_updates_actor_presence_and_reverts_exactly() {
        let registry = GameplayTagRegistry::compile(names(&[
            "location.cave",
            "location.current",
            "location.start",
            "role.trickster",
        ]))
        .unwrap();

        let mut state = WorldStateTable::with_capacity(3, 8);
        state.entity_tags[0]
            .set_bit(tag_bit(&registry, "location.start"))
            .unwrap();
        state.entity_tags[1]
            .set_bit(tag_bit(&registry, "location.cave"))
            .unwrap();
        state.entity_tags[2]
            .set_bit(tag_bit(&registry, "role.trickster"))
            .unwrap();
        state.vertices.push(WorldVertex {
            vertex_id: "char.rogue".to_string(),
            layer: GraphLayer::Character,
            entity_index: Some(2),
            attributes: BTreeMap::new(),
        });
        state.vertices.push(WorldVertex {
            vertex_id: "loc.cave".to_string(),
            layer: GraphLayer::Location,
            entity_index: Some(1),
            attributes: BTreeMap::new(),
        });
        state.vertices.push(WorldVertex {
            vertex_id: "loc.start".to_string(),
            layer: GraphLayer::Location,
            entity_index: Some(0),
            attributes: BTreeMap::new(),
        });
        state.edges.push(WorldEdge {
            edge_id: "located".to_string(),
            source_id: "char.rogue".to_string(),
            target_id: "loc.cave".to_string(),
            relation_type: LOCATED_AT_RELATION.to_string(),
            weight: 0.0,
        });

        state.seed_current_location(0).unwrap();
        // An unrelated global flag that movement must not clobber.
        state
            .global_tags
            .set_bit(tag_bit(&registry, "role.trickster"))
            .unwrap();

        let before = state.clone();
        let before_json = serde_json::to_string(&state).unwrap();

        let op = MutationOp {
            move_player: Some(1),
            ..Default::default()
        };
        let delta = apply_mutation_with_registry(&mut state, &op, &registry).unwrap();

        assert_eq!(state.current_location_entity, Some(1));
        assert!(state.entity_tags[2].bit(tag_bit(&registry, "location.current")));
        assert!(state.visited_location_tags.entity_mask.bit(1));
        assert!(
            state
                .visited_location_tags
                .tag_mask
                .bit(tag_bit(&registry, "location.cave"))
        );
        assert!(!state.global_tags.bit(tag_bit(&registry, "location.start")));
        assert!(state.global_tags.bit(tag_bit(&registry, "role.trickster")));
        assert!(
            state
                .current_location_tag_mask
                .bit(tag_bit(&registry, "location.cave"))
        );

        revert_delta(&mut state, &delta).unwrap();
        assert_eq!(state, before);
        assert_eq!(serde_json::to_string(&state).unwrap(), before_json);

        // A non-location move target is rejected before anything mutates.
        let rejected = apply_mutation_with_registry(
            &mut state,
            &MutationOp {
                move_player: Some(2),
                ..Default::default()
            },
            &registry,
        );
        assert_eq!(
            rejected.unwrap_err(),
            ChronoStateError::VertexLayerMismatch {
                vertex_id: "char.rogue".to_string(),
                layer: GraphLayer::Character,
            }
        );
        assert_eq!(state, before);
    }

    #[test]
    fn met_character_entity_and_tag_masks_revert_exactly() {
        let registry =
            GameplayTagRegistry::compile(names(&["qbn.opening", "role.trickster"])).unwrap();
        let mut state = WorldStateTable::with_capacity(2, 4);
        state.entity_tags[1]
            .set_bit(tag_bit(&registry, "role.trickster"))
            .unwrap();
        let before = state.clone();

        let op = MutationOp {
            add_global_tags: TagBitset::from_words([1, 0, 0, 0]),
            meet_character: Some(1),
            ..Default::default()
        };
        let delta = apply_mutation_with_registry(&mut state, &op, &registry).unwrap();

        assert!(state.met_character_tags.entity_mask.bit(1));
        assert!(
            state
                .met_character_tags
                .tag_mask
                .bit(tag_bit(&registry, "role.trickster"))
        );
        assert!(delta.met_character_entity_xor.bit(1));
        assert!(delta.met_character_xor.bit(1));

        revert_delta(&mut state, &delta).unwrap();
        assert_eq!(state, before);
        assert_eq!(
            serde_json::to_string(&state).unwrap(),
            serde_json::to_string(&before).unwrap()
        );
    }

    #[test]
    fn metric_bounds_clamp_coupled_writes_and_undo_exactly() {
        let registry = GameplayTagRegistry::empty()
            .with_coupling_links([ZeroSumLink {
                source_metric_id: 0,
                target_metric_id: 1,
                weight: 0.5,
            }])
            .unwrap();

        let mut state = WorldStateTable::with_capacity(1, 2);
        state.set_metric_bound(0, 0.0, 10.0).unwrap();
        state.set_metric_bound(1, -5.0, 5.0).unwrap();
        state.metrics[0] = 8.0;
        let before = state.clone();

        let delta = apply_mutation_with_registry(
            &mut state,
            &MutationOp {
                metric_deltas: vec![(0, 20.0)],
                ..Default::default()
            },
            &registry,
        )
        .unwrap();
        assert_eq!(state.metrics[0], 10.0);
        assert_eq!(state.metrics[1], 5.0);
        assert_eq!(
            delta.metric_undos,
            vec![
                MetricUndoPair {
                    metric_id: 0,
                    old_value: 8.0
                },
                MetricUndoPair {
                    metric_id: 1,
                    old_value: 0.0
                }
            ]
        );

        revert_delta(&mut state, &delta).unwrap();
        assert_eq!(state, before);
        assert_eq!(
            state.set_metric_bound(0, 10.0, 0.0),
            Err(ChronoStateError::InvalidMetricBound { index: 0 })
        );
    }

    #[test]
    fn deterministic_roundtrip_restores_exact_state() {
        let registry = GameplayTagRegistry::compile(names(&[
            "arc.opening",
            "loc.1",
            "loc.2",
            "loc.3",
            "loc.4",
            "location.current",
        ]))
        .unwrap()
        .with_coupling_links([ZeroSumLink {
            source_metric_id: 0,
            target_metric_id: 1,
            weight: 0.5,
        }])
        .unwrap()
        .with_threshold_rules([MetricThresholdRule {
            metric_id: 1,
            threshold_tag_bit: 0,
            trigger_value: 3.0,
            hysteresis: 1.0,
        }])
        .unwrap();

        let mut state = WorldStateTable::with_capacity(5, 8);
        state.vertices.push(WorldVertex {
            vertex_id: "actor".to_string(),
            layer: GraphLayer::Character,
            entity_index: Some(0),
            attributes: BTreeMap::new(),
        });
        for index in 1..5u16 {
            state.vertices.push(WorldVertex {
                vertex_id: format!("loc.{index}"),
                layer: GraphLayer::Location,
                entity_index: Some(index),
                attributes: BTreeMap::new(),
            });
        }
        for target in ["loc.1", "loc.2"] {
            state.edges.push(WorldEdge {
                edge_id: format!("located.{target}"),
                source_id: "actor".to_string(),
                target_id: target.to_string(),
                relation_type: LOCATED_AT_RELATION.to_string(),
                weight: 0.0,
            });
        }
        state.set_metric_bound(0, -10.0, 10.0).unwrap();
        let original = state.clone();

        let ops: Vec<MutationOp> = (0..500)
            .map(|step| {
                let mut mask = TagBitset::new();
                mask.set_bit((step % 6) as u16).unwrap();
                let entity = (step % 5) as u16;
                MutationOp {
                    add_global_tags: mask,
                    metric_deltas: vec![(0, 0.25), (1, -0.5)],
                    entity_tag_ops: vec![EntityTagMutation {
                        entity_id: entity,
                        add_tags: mask,
                        remove_tags: TagBitset::new(),
                    }],
                    move_player: Some((step % 4) as u16 + 1),
                    ..Default::default()
                }
            })
            .collect();

        let mut left = state.clone();
        let mut right = state.clone();
        let left_deltas: Vec<BijectiveDelta> = ops
            .iter()
            .map(|op| apply_mutation_with_registry(&mut left, op, &registry).unwrap())
            .collect();
        let right_deltas: Vec<BijectiveDelta> = ops
            .iter()
            .map(|op| apply_mutation_with_registry(&mut right, op, &registry).unwrap())
            .collect();

        assert_eq!(left_deltas.len(), 500);
        assert_eq!(left, right);
        assert_eq!(left_deltas, right_deltas);
        assert!(left_deltas.iter().any(|delta| !delta.is_empty()));
        assert_ne!(left, original);

        for delta in left_deltas.iter().rev() {
            revert_delta(&mut left, delta).unwrap();
        }
        assert_eq!(left, original);
        assert_eq!(
            serde_json::to_string(&left).unwrap(),
            serde_json::to_string(&original).unwrap()
        );
    }
    #[test]
    fn passion_thresholds_rivals_and_trust_inertia_revert_exactly() {
        let threshold_bit = 7;
        let registry = PassionRegistry::compile(
            [
                StarNpcDef {
                    npc_id: 1,
                    trust_inertia: 0.5,
                    micro_passions: vec![
                        MicroPassionDef {
                            npc_id: 1,
                            passion_id: 0,
                            weight: 0.5,
                            is_hidden: false,
                        },
                        MicroPassionDef {
                            npc_id: 1,
                            passion_id: 1,
                            weight: 1.0,
                            is_hidden: true,
                        },
                    ],
                    macro_bounds: MetricBound {
                        min: -100.0,
                        max: 100.0,
                    },
                    zero_sum_links: vec![RivalPassionLink {
                        target_character_id: 2,
                        source_passion_id: 0,
                        target_passion_id: 2,
                        weight: -0.5,
                    }],
                },
                StarNpcDef {
                    npc_id: 2,
                    trust_inertia: 0.0,
                    micro_passions: vec![MicroPassionDef {
                        npc_id: 2,
                        passion_id: 2,
                        weight: 1.0,
                        is_hidden: false,
                    }],
                    macro_bounds: MetricBound {
                        min: -100.0,
                        max: 100.0,
                    },
                    zero_sum_links: Vec::new(),
                },
            ],
            [PassionThresholdRule {
                npc_id: 1,
                threshold_tag_bit: threshold_bit,
                trigger_value: 1.0,
                direction: PassionDirection::Up,
                linked_graph_asset_id: "crisis".into(),
            }],
        )
        .unwrap();
        let mut state = PassionState::with_capacity(3, 3);
        state.micro = vec![4.0, -3.0, 2.0];
        state.active = vec![true; 3];
        state.macro_values[1] = -1.0 / 6.0;
        state.macro_values[2] = 2.0;
        let original_state = state.clone();
        let mut flags = TagBitset::new();
        let original_flags = flags;

        let rise = apply_passion_mutation(
            &mut state,
            &PassionMutation {
                micro_deltas: vec![(0, 4.0)],
                ..PassionMutation::default()
            },
            &registry,
            &mut flags,
        )
        .unwrap();
        assert_eq!(state.micro, vec![8.0, -3.0, 0.0]);
        assert!((state.macro_values[1] - 7.0 / 6.0).abs() < 0.0001);
        assert_eq!(state.macro_values[2], 0.0);
        assert!(flags.bit(threshold_bit));
        assert_eq!(rise.threshold_events.len(), 1);
        assert!(rise.threshold_events[0].activated);

        let inactive_pool = apply_passion_mutation(
            &mut state,
            &PassionMutation {
                active_ops: vec![(1, false)],
                ..PassionMutation::default()
            },
            &registry,
            &mut flags,
        )
        .unwrap();
        assert_eq!(state.macro_values[1], 8.5);
        revert_passion_delta(&mut state, &inactive_pool, &mut flags).unwrap();

        let fall = apply_passion_mutation(
            &mut state,
            &PassionMutation {
                micro_deltas: vec![(0, -4.0)],
                ..PassionMutation::default()
            },
            &registry,
            &mut flags,
        )
        .unwrap();
        assert_eq!(state.micro, vec![4.0, -3.0, 2.0]);
        assert!((state.macro_values[1] + 1.0 / 6.0).abs() < 0.0001);
        assert_eq!(state.macro_values[2], 2.0);
        assert!(!flags.bit(threshold_bit));
        assert_eq!(fall.threshold_events.len(), 1);
        assert!(!fall.threshold_events[0].activated);

        revert_passion_delta(&mut state, &fall, &mut flags).unwrap();
        revert_passion_delta(&mut state, &rise, &mut flags).unwrap();
        assert_eq!(state, original_state);
        assert_eq!(flags, original_flags);
    }
}
