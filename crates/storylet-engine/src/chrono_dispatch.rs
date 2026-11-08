//! First-class beyond-branching dispatchers for the v5 runtime (CFN-SPEC-001 Module 2):
//! the QBN deck query, the salience matcher, and the waypoint topic navigator.
//!
//! `UnifiedDispatcher` is the single eligibility and scoring policy shared by the QBN
//! deck and the salience match. Authoring data uses gameplay tags and role
//! requirements; runtime effects remain `MutationOp`, so the bijective delta and
//! rollback path owned by `chrono_state` is unchanged.
//!
//! Determinism: every query walks the deck in the caller's order, ties break by
//! storylet ID or edge ID, and the salience tie pick comes from the caller's seed.
//! Nothing here reads a clock or a random source. Out-of-range IDs and non-finite
//! values are rejected as ineligible or as typed errors; nothing indexes panics.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::chrono_completion::StoryletBitset;
use crate::chrono_state::{GameplayTagRegistry, MutationOp, TagBitset, WorldStateTable};

/// Salience distance penalty coefficient, `alpha` in REQ-BB-04.
pub const SALIENCE_DISTANCE_ALPHA: f64 = 0.05;
/// Scores inside this window are treated as ties and resolved from the seed.
pub const SALIENCE_TIE_EPSILON: f64 = 0.0001;

fn default_base_cost() -> f64 {
    1.0
}

fn default_used_penalty_cost() -> f64 {
    100.0
}

fn default_urgency_tag() -> String {
    "Standard".to_owned()
}

// ---------------------------------------------------------------------------
// Authoring vocabulary (v5 section 5.3)
// ---------------------------------------------------------------------------

/// Pacing cluster of a storylet. Authoring data; the dispatcher does not branch on it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoryletClusterType {
    /// Constrained high-tension chain.
    LinearProgression,
    /// Open hub for investigation or gearing up.
    #[default]
    PlayerPassage,
    /// Randomized or weighted opportunity card deck.
    WheelDeck,
    /// Rapid consequence sequence.
    AftermathAccelerator,
}


/// What one completion effect does. Effects mutate gameplay tags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    #[default]
    AddTag,
    RemoveTag,
}

/// Authoring form of a tag completion effect.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoryletEffect {
    pub target_id: String,
    #[serde(default)]
    pub kind: EffectKind,
    #[serde(default)]
    pub tag_mask: TagBitset,
}

/// Role selector bound to an available actor entity, the v5 form of the legacy
/// `StoryletDefinition.required_roles` behavior.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RoleRequirement {
    /// Role gameplay tag, for example `Role.Detective`.
    pub role_tag: String,
    /// Optional location tag the actor must be at. The adapter writes the
    /// current-location bit on the entity row; this module does not read vertex
    /// adjacency because `WorldVertex` fields are not part of the shared contract.
    #[serde(default)]
    pub location_tag: Option<String>,
    #[serde(default)]
    pub role_index: Option<u16>,
    #[serde(default)]
    pub location_index: Option<u16>,
    /// Compiled ancestor closure masks written by `UnifiedDispatcher::resolve`.
    #[serde(default)]
    pub role_mask: TagBitset,
    #[serde(default)]
    pub location_mask: TagBitset,
}

/// One role selector bound to one entity row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleBinding {
    pub role_index: u16,
    pub entity_index: usize,
}

/// A dispatchable storylet.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StoryletDef {
    pub storylet_id: u16,
    /// Authoring marker: this storylet is a dedicated ending candidate.
    #[serde(default)]
    pub is_ending: bool,
    /// Filler content used only when no progress storylet is available.
    #[serde(default)]
    pub is_fallback: bool,
    /// Ranks storylets in a large QBN deck; higher first.
    #[serde(default)]
    pub display_priority: i32,
    #[serde(default = "default_urgency_tag")]
    pub urgency_tag: String,
    #[serde(default)]
    pub cluster_type: StoryletClusterType,
    /// Tag closure masks, already compiled by the registry.
    #[serde(default)]
    pub required_tags: TagBitset,
    /// Any-of mask: at least one bit must be present. Kept separate from
    /// `required_tags`, which is an all-of mask.
    #[serde(default)]
    pub required_any_tags: TagBitset,
    #[serde(default)]
    pub forbidden_tags: TagBitset,
    /// Tags that raise the salience specificity term when present.
    #[serde(default)]
    pub salience_bonus_tags: TagBitset,
    /// Dramatic arc phases this storylet may fire in (REQ-BB-05). An empty mask
    /// accepts any phase.
    #[serde(default)]
    pub required_arc_phases: TagBitset,
    /// Role selectors. A non-empty list requires at least one bound entity.
    #[serde(default)]
    pub required_roles: Vec<RoleRequirement>,
    /// Participants who may play this storylet. Empty = any human. A rule
    /// participant plays its storylets on its own at the start of a slot.
    #[serde(default)]
    pub played_by: Vec<crate::chrono_participants::PartIx>,
    /// Eligible clock slots; empty means any time.
    #[serde(default)]
    pub window: Vec<u32>,
    /// Optional pure candidate gate compiled from the authored precondition graph.
    #[serde(default)]
    pub precondition_graph: Option<crate::chrono_runtime::FlowGraphAsset>,

    /// Authoring effects; compiled into `on_complete_mutation`.
    #[serde(default)]
    pub effects_on_complete: Vec<StoryletEffect>,
    /// Runtime effect applied on completion.
    #[serde(default)]
    pub on_complete_mutation: MutationOp,
    #[serde(default)]
    pub payload_node_guid: u32,
    #[serde(default)]
    pub is_repeatable: bool,
    /// Cooldown in integer milliseconds after a completion. This module has no clock;
    /// the caller's fixed-point clock supplies `now_ms` to `cooldown_remaining_ms`.
    #[serde(default)]
    pub cooldown_ms: i64,
    /// Turn-based cooldown, kept separate from `cooldown_ms`: there is no conversion
    /// contract between action turns and fixed-point milliseconds. A storylet that
    /// declares both is reported by `cooldown_unit`.
    #[serde(default)]
    pub cooldown_turns: u64,
}

// ---------------------------------------------------------------------------
// Waypoint topic graph (v5 section 5.3)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WaypointTopicNode {
    pub topic_id: String,
    #[serde(default)]
    pub is_player_only_trigger: bool,
    #[serde(default)]
    pub triggers_scene_exit: bool,
    #[serde(default)]
    pub exit_output_pin: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WaypointTopicEdge {
    pub edge_id: String,
    pub from_topic_id: String,
    pub to_topic_id: String,
    #[serde(default)]
    pub dialogue_transition_asset_id: String,
    #[serde(default = "default_base_cost")]
    pub base_cost: f64,
    /// Added once per traversal so the NPC routes around used edges.
    #[serde(default = "default_used_penalty_cost")]
    pub used_penalty_cost: f64,
    #[serde(default)]
    pub is_fallback_default: bool,
}

/// Directed topic graph. Node and edge order is the authoring order, and that order
/// is part of the deterministic result.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WaypointTopicGraph {
    #[serde(default)]
    pub nodes: Vec<WaypointTopicNode>,
    #[serde(default)]
    pub edges: Vec<WaypointTopicEdge>,
}

/// The transition the NPC takes this turn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaypointStep {
    pub edge_index: usize,
    pub edge_id: String,
    pub to_topic_id: String,
    /// Dynamic cost of this edge: base plus penalty per recorded traversal.
    pub cost: f64,
    /// True when the normal path was exhausted and a fallback edge was taken.
    pub via_fallback: bool,
    /// Optional authored line/asset to speak while taking this edge.
    #[serde(default)]
    pub dialogue_transition_asset_id: String,
}

/// Which cooldown unit an authored storylet uses. Turns and milliseconds are not
/// interchangeable; the engine keeps both paths.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CooldownUnit {
    #[default]
    Milliseconds,
    Turns,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum DispatchError {
    /// A topic or gameplay tag name is not registered.
    UnresolvedTarget { target_id: String },
    /// The weighted distance term is not finite.
    NonFiniteDistance { storylet_id: u16 },
    /// A storylet declares both a millisecond and a turn cooldown.
    CooldownUnitConflict {
        storylet_id: u16,
        cooldown_ms: i64,
        cooldown_turns: u64,
    },
    /// A role selector has no distinct available actor.
    NoDistinctActorForRole { role_tag: String },
    /// An entity row is outside the entity tag table.
    EntityIndexOutOfRange { entity_index: usize, len: usize },
    /// An edge cost or traversal penalty is negative or not finite.
    InvalidEdgeCost { edge_id: String, cost: f64 },
    /// A traversal count makes the dynamic edge cost non-finite.
    InvalidTraversalCount { edge_id: String, count: u64 },
    /// A topic name is not in the topic graph.
    TopicNotFound { topic_id: String },
    /// No path exists from the current topic to the goal topic.
    NoPathToGoal {
        from_topic_id: String,
        goal_topic_id: String,
    },
    /// No path exists and the current topic has no fallback edge.
    NoFallbackEdge { from_topic_id: String },
}

impl fmt::Display for DispatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Error for DispatchError {}

// ---------------------------------------------------------------------------
// Dispatcher
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize)]
pub struct UnifiedDispatcher {
    #[serde(default)]
    pub storylets: Vec<StoryletDef>,
    /// Dense ID -> row lookup. Skipped in JSON; rebuilt by `new`/`rebuild_index`.
    #[serde(skip, default)]
    index_by_id: Vec<Option<u32>>,
    #[serde(skip, default)]
    indexed_count: usize,
}

impl PartialEq for UnifiedDispatcher {
    fn eq(&self, other: &Self) -> bool {
        self.storylets == other.storylets
    }
}

impl<'de> Deserialize<'de> for UnifiedDispatcher {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            #[serde(default)]
            storylets: Vec<StoryletDef>,
        }
        let wire = Wire::deserialize(deserializer)?;
        let mut dispatcher = Self {
            storylets: wire.storylets,
            index_by_id: Vec::new(),
            indexed_count: 0,
        };
        dispatcher.rebuild_index();
        Ok(dispatcher)
    }
}

impl UnifiedDispatcher {
    pub fn new(storylets: Vec<StoryletDef>) -> Self {
        let mut dispatcher = Self {
            storylets,
            index_by_id: Vec::new(),
            indexed_count: 0,
        };
        dispatcher.rebuild_index();
        dispatcher
    }

    /// Rebuild the dense ID index after direct mutation of `storylets`.
    pub fn rebuild_index(&mut self) {
        let max_id = self
            .storylets
            .iter()
            .map(|def| def.storylet_id as usize)
            .max()
            .map(|id| id + 1)
            .unwrap_or(0);
        self.index_by_id = vec![None; max_id];
        self.indexed_count = self.storylets.len();
        for (row, def) in self.storylets.iter().enumerate() {
            let slot = def.storylet_id as usize;
            if slot < self.index_by_id.len() && self.index_by_id[slot].is_none() {
                self.index_by_id[slot] = Some(row as u32);
            }
        }
    }

    /// Look up one storylet by ID. Declaration order defines the first match.
    /// A built index is authoritative for hits and misses; the linear fallback
    /// exists only for an index that was never built or became stale.
    pub fn storylet(&self, storylet_id: u16) -> Option<&StoryletDef> {
        if !self.index_by_id.is_empty() && self.indexed_count == self.storylets.len() {
            if let Some(Some(row)) = self.index_by_id.get(storylet_id as usize) {
                if let Some(def) = self.storylets.get(*row as usize) {
                    if def.storylet_id == storylet_id {
                        return Some(def);
                    }
                }
            } else {
                return None;
            }
        }
        self.storylets
            .iter()
            .find(|def| def.storylet_id == storylet_id)
    }

    /// Compile gameplay tag and role references into dense registry indices.
    pub fn resolve(&mut self, tags: &GameplayTagRegistry) -> Result<usize, DispatchError> {
        let mut resolved = 0;
        for def in &mut self.storylets {
            for role in &mut def.required_roles {
                let index = tags.id(&role.role_tag).ok_or_else(|| DispatchError::UnresolvedTarget { target_id: role.role_tag.clone() })?;
                role.role_index = Some(index);
                role.role_mask = *tags.ancestors_mask(index).ok_or_else(|| DispatchError::UnresolvedTarget { target_id: role.role_tag.clone() })?;
                resolved += 1;
                if let Some(location_tag) = &role.location_tag {
                    let index = tags.id(location_tag).ok_or_else(|| DispatchError::UnresolvedTarget { target_id: location_tag.clone() })?;
                    role.location_index = Some(index);
                    role.location_mask = *tags.ancestors_mask(index).ok_or_else(|| DispatchError::UnresolvedTarget { target_id: location_tag.clone() })?;
                    resolved += 1;
                }
            }
            for effect in &mut def.effects_on_complete {
                let index = tags.id(&effect.target_id).ok_or_else(|| DispatchError::UnresolvedTarget { target_id: effect.target_id.clone() })?;
                effect.tag_mask = *tags.ancestors_mask(index).ok_or_else(|| DispatchError::UnresolvedTarget { target_id: effect.target_id.clone() })?;
                resolved += 1;
            }
        }
        Ok(resolved)
    }

    /// Hard eligibility: all-of and any-of tag masks, the dramatic arc phase gate,
    /// and a distinct available actor for every role selector. ChronoRuntime evaluates
    /// the authored precondition graph before adding a storylet to a candidate pool.
    /// Completion locks live in the caller's mask; use `is_playable` for the deck view.
    pub fn is_eligible(&self, def: &StoryletDef, table: &WorldStateTable) -> bool {
        self.try_is_eligible(def, table).unwrap_or(false)
    }

    /// Describe the first missing hard requirement in reader-facing language.
    pub fn explain_ineligible(&self, def: &StoryletDef, table: &WorldStateTable, registry: &GameplayTagRegistry) -> Option<String> {
        if !def.window.is_empty() && table.clock.as_ref().is_none_or(|clock| !def.window.contains(&clock.slot)) { return Some("outside its time window".into()); }
        for i in (0..registry.tag_count()).rev() {
            let i = i as u16;
            if def.required_tags.bit(i) && !table.global_tags.bit(i) {
                let tag = registry.tag(i).unwrap_or("requirement");
                let label = tag.strip_prefix("fact.").unwrap_or(tag).replace(['_', '.'], " ");
                let reason = if tag.starts_with("fact.") { format!("needs fact \"{label}\"") } else { format!("needs {label}") };
                return Some(reason.chars().take(120).collect());
            }
        }
        if !def.required_any_tags.is_empty() && !table.global_tags.intersects(&def.required_any_tags) { return Some("needs an alternative requirement".into()); }
        if !def.forbidden_tags.is_empty() && table.global_tags.intersects(&def.forbidden_tags) { return Some("a forbidden condition is present".into()); }
        if !def.required_arc_phases.is_empty() && !table.global_tags.intersects(&def.required_arc_phases) { return Some("needs a different story phase".into()); }
        if !def.required_roles.is_empty() && self.bind_roles(def, table).is_err() { return Some("needs a participant present".into()); }
        None
    }
    /// Fallible form of `is_eligible`: malformed content returns the typed
    /// dispatch error instead of silently deleting the storylet.
    pub fn try_is_eligible(
        &self,
        def: &StoryletDef,
        table: &WorldStateTable,
    ) -> Result<bool, DispatchError> {
        if !def.window.is_empty() && table.clock.as_ref().is_none_or(|clock| !def.window.contains(&clock.slot)) { return Ok(false); }
        if !table.global_tags.contains_all(&def.required_tags) {
            return Ok(false);
        }
        if !def.required_any_tags.is_empty()
            && !table.global_tags.intersects(&def.required_any_tags)
        {
            return Ok(false);
        }
        if !def.forbidden_tags.is_empty() && table.global_tags.intersects(&def.forbidden_tags) {
            return Ok(false);
        }
        if !def.required_arc_phases.is_empty()
            && !table.global_tags.intersects(&def.required_arc_phases)
        {
            return Ok(false);
        }
        if !def.required_roles.is_empty() && self.bind_roles(def, table).is_err() {
            return Ok(false);
        }
        Ok(true)
    }

    /// Bind every role selector to a distinct entity row: the legacy requirement that
    /// each required role has its own available actor at the player's current
    /// location. A row must carry the role mask and, when the selector declares a
    /// location, the current-location bit the adapter writes on the actor row.
    pub fn bind_roles(
        &self,
        def: &StoryletDef,
        table: &WorldStateTable,
    ) -> Result<Vec<RoleBinding>, DispatchError> {
        self.bind_roles_excluding(def, table, &BTreeSet::new())
    }

    /// Same binding with busy or reserved rows excluded. Pass an empty set when the
    /// runtime has no reservation state; the engine never infers it. Assignment is a
    /// maximum bipartite matching (Kuhn) over role selectors in declaration order and
    /// entity rows in index order, so a replay reproduces the same bindings. A role
    /// that cannot get its own actor is a typed error, never a shared actor.
    pub fn bind_roles_excluding(
        &self,
        def: &StoryletDef,
        table: &WorldStateTable,
        busy: &BTreeSet<usize>,
    ) -> Result<Vec<RoleBinding>, DispatchError> {
        let mut candidates = Vec::with_capacity(def.required_roles.len());
        for role in &def.required_roles {
            let role_index = role
                .role_index
                .ok_or_else(|| DispatchError::UnresolvedTarget {
                    target_id: role.role_tag.clone(),
                })?;
            let mut rows = Vec::new();
            for (entity_index, row) in table.entity_tags.iter().enumerate() {
                if busy.contains(&entity_index) {
                    continue;
                }
                if !row.contains_all(&role.role_mask) {
                    continue;
                }
                if !role.location_mask.is_empty() && !row.intersects(&role.location_mask) {
                    continue;
                }
                rows.push(entity_index);
            }
            if rows.is_empty() {
                return Err(DispatchError::NoDistinctActorForRole {
                    role_tag: role.role_tag.clone(),
                });
            }
            candidates.push((role_index, rows));
        }
        let mut owner = vec![usize::MAX; table.entity_tags.len()];
        for role in 0..candidates.len() {
            let mut seen = vec![false; table.entity_tags.len()];
            if !augment_role(role, &candidates, &mut owner, &mut seen) {
                return Err(DispatchError::NoDistinctActorForRole {
                    role_tag: def.required_roles[role].role_tag.clone(),
                });
            }
        }
        let mut bindings = Vec::with_capacity(candidates.len());
        for role in 0..candidates.len() {
            for (row, assigned_role) in owner.iter().enumerate() {
                if *assigned_role == role {
                    bindings.push(RoleBinding {
                        role_index: candidates[role].0,
                        entity_index: row,
                    });
                    break;
                }
            }
        }
        Ok(bindings)
    }

    /// Eligibility plus the completion lock for non-repeatable storylets. `completed`
    /// is the caller's mask of storylet IDs that have already run.
    pub fn is_playable(
        &self,
        def: &StoryletDef,
        table: &WorldStateTable,
        completed: &BTreeSet<u16>,
    ) -> bool {
        self.try_is_playable(def, table, completed).unwrap_or(false)
    }

    /// Fallible form of `is_playable`.
    pub fn try_is_playable(
        &self,
        def: &StoryletDef,
        table: &WorldStateTable,
        completed: &BTreeSet<u16>,
    ) -> Result<bool, DispatchError> {
        if !def.is_repeatable && completed.contains(&def.storylet_id) {
            return Ok(false);
        }
        self.try_is_eligible(def, table)
    }

    /// Bitset form used by the per-scope completion contract.
    pub fn try_is_playable_bitset(
        &self,
        def: &StoryletDef,
        table: &WorldStateTable,
        completed: &StoryletBitset,
    ) -> Result<bool, DispatchError> {
        if !def.is_repeatable && completed.contains(def.storylet_id) {
            return Ok(false);
        }
        self.try_is_eligible(def, table)
    }

    /// QBN deck query: filter the deck IDs to playable storylets and order them by
    /// `display_priority` descending, then by storylet ID ascending. Unknown deck IDs
    /// are skipped, and a repeated ID appears once.
    pub fn query_qbn_deck(
        &self,
        deck: &[u16],
        table: &WorldStateTable,
        completed: &BTreeSet<u16>,
    ) -> Vec<u16> {
        self.try_query_qbn_deck(deck, table, completed)
            .unwrap_or_default()
    }

    /// Fallible form of `query_qbn_deck`; malformed content fails the query.
    pub fn try_query_qbn_deck(
        &self,
        deck: &[u16],
        table: &WorldStateTable,
        completed: &BTreeSet<u16>,
    ) -> Result<Vec<u16>, DispatchError> {
        let mut seen = BTreeSet::new();
        let mut eligible = Vec::new();
        for &id in deck {
            if !seen.insert(id) {
                continue;
            }
            let Some(def) = self.storylet(id) else {
                continue;
            };
            if self.try_is_playable(def, table, completed)? {
                eligible.push(def);
            }
        }
        eligible.sort_by(|a, b| {
            b.display_priority
                .cmp(&a.display_priority)
                .then(a.storylet_id.cmp(&b.storylet_id))
        });
        Ok(eligible.into_iter().map(|def| def.storylet_id).collect())
    }

    /// Bitset deck query with an explicit ending opt-in. The default normal
    /// profile passes `false`; an ending profile is fetched from its own pool.
    pub fn try_query_qbn_deck_bitset_with_ending(
        &self,
        deck: &[u16],
        table: &WorldStateTable,
        completed: &StoryletBitset,
        include_ending: bool,
    ) -> Result<Vec<u16>, DispatchError> {
        let mut seen = BTreeSet::new();
        let mut eligible = Vec::new();
        for &id in deck {
            if !seen.insert(id) {
                continue;
            }
            let Some(def) = self.storylet(id) else {
                continue;
            };
            if def.is_ending && !include_ending {
                continue;
            }
            if self.try_is_playable_bitset(def, table, completed)? {
                eligible.push(def);
            }
        }
        eligible.sort_by(|a, b| {
            b.display_priority
                .cmp(&a.display_priority)
                .then(a.storylet_id.cmp(&b.storylet_id))
        });
        Ok(eligible.into_iter().map(|def| def.storylet_id).collect())
    }

    /// Bitset form of `try_query_qbn_deck`.
    pub fn try_query_qbn_deck_bitset(
        &self,
        deck: &[u16],
        table: &WorldStateTable,
        completed: &StoryletBitset,
    ) -> Result<Vec<u16>, DispatchError> {
        let mut seen = BTreeSet::new();
        let mut eligible = Vec::new();
        for &id in deck {
            if !seen.insert(id) {
                continue;
            }
            let Some(def) = self.storylet(id) else {
                continue;
            };
            if self.try_is_playable_bitset(def, table, completed)? {
                eligible.push(def);
            }
        }
        eligible.sort_by(|a, b| {
            b.display_priority
                .cmp(&a.display_priority)
                .then(a.storylet_id.cmp(&b.storylet_id))
        });
        Ok(eligible.into_iter().map(|def| def.storylet_id).collect())
    }

    pub fn salience_score(&self, def: &StoryletDef, table: &WorldStateTable) -> Result<f64, DispatchError> {
        Ok(f64::from(table.global_tags.popcount_intersection(&def.salience_bonus_tags)))
    }

    /// Salience match: score only playable candidates and return the best. Scores
    /// inside `SALIENCE_TIE_EPSILON` are ties, resolved from `seed` so a replay
    /// reproduces the same pick. Returns `None` when nothing is eligible.
    pub fn match_most_salient(
        &self,
        pool: &[u16],
        table: &WorldStateTable,
        completed: &BTreeSet<u16>,
        seed: u64,
    ) -> Option<u16> {
        self.try_match_most_salient(pool, table, completed, seed)
            .unwrap_or(None)
    }

    /// Fallible form of `match_most_salient`; a malformed candidate fails the
    /// dispatch instead of silently disappearing.
    pub fn try_match_most_salient(
        &self,
        pool: &[u16],
        table: &WorldStateTable,
        completed: &BTreeSet<u16>,
        seed: u64,
    ) -> Result<Option<u16>, DispatchError> {
        let mut seen = BTreeSet::new();
        let mut best: Vec<u16> = Vec::new();
        let mut highest = f64::NEG_INFINITY;
        for &id in pool {
            if !seen.insert(id) {
                continue;
            }
            let Some(def) = self.storylet(id) else {
                continue;
            };
            // REQ-BB-05: every salience candidate SHALL declare an arc-phase
            // mask. An empty mask is the premature-climax case, so fail closed.
            if def.required_arc_phases.is_empty() {
                continue;
            }
            if !self.try_is_playable(def, table, completed)? {
                continue;
            }
            let score = self.salience_score(def, table)?;
            if (score - highest).abs() < SALIENCE_TIE_EPSILON {
                best.push(id);
            } else if score > highest {
                highest = score;
                best.clear();
                best.push(id);
            }
        }
        if best.is_empty() {
            return Ok(None);
        }
        if best.len() == 1 {
            return Ok(Some(best[0]));
        }
        Ok(Some(best[deterministic_index(seed, best.len())]))
    }


    /// Bitset form of `try_match_most_salient`.
    pub fn try_match_most_salient_bitset(
        &self,
        pool: &[u16],
        table: &WorldStateTable,
        completed: &StoryletBitset,
        seed: u64,
    ) -> Result<Option<u16>, DispatchError> {
        let mut seen = BTreeSet::new();
        let mut best: Vec<u16> = Vec::new();
        let mut highest = f64::NEG_INFINITY;
        for &id in pool {
            if !seen.insert(id) {
                continue;
            }
            let Some(def) = self.storylet(id) else {
                continue;
            };
            if def.required_arc_phases.is_empty() {
                continue;
            }
            if !self.try_is_playable_bitset(def, table, completed)? {
                continue;
            }
            let score = self.salience_score(def, table)?;
            if (score - highest).abs() < SALIENCE_TIE_EPSILON {
                best.push(id);
            } else if score > highest {
                highest = score;
                best.clear();
                best.push(id);
            }
        }
        if best.is_empty() {
            return Ok(None);
        }
        if best.len() == 1 {
            return Ok(Some(best[0]));
        }
        Ok(Some(best[deterministic_index(seed, best.len())]))
    }

    /// Report the cooldown unit of an authored storylet. `Ok(None)` means the
    /// storylet has no cooldown. A storylet that declares both units is a typed
    /// incompatibility, so the service can keep the old turn-based path for it.
    pub fn cooldown_unit(&self, def: &StoryletDef) -> Result<Option<CooldownUnit>, DispatchError> {
        if def.cooldown_ms > 0 && def.cooldown_turns > 0 {
            return Err(DispatchError::CooldownUnitConflict {
                storylet_id: def.storylet_id,
                cooldown_ms: def.cooldown_ms,
                cooldown_turns: def.cooldown_turns,
            });
        }
        if def.cooldown_turns > 0 {
            return Ok(Some(CooldownUnit::Turns));
        }
        if def.cooldown_ms > 0 {
            return Ok(Some(CooldownUnit::Milliseconds));
        }
        Ok(None)
    }

    /// Turn-based cooldown remaining, given the turn of the last completion. Returns
    /// 0 when the storylet is ready. This module owns no turn counter, so the caller
    /// supplies both turns.
    pub fn cooldown_remaining_turns(
        def: &StoryletDef,
        last_completed_turn: u64,
        current_turn: u64,
    ) -> u64 {
        if def.cooldown_turns == 0 {
            return 0;
        }
        def.cooldown_turns
            .saturating_sub(current_turn.saturating_sub(last_completed_turn))
    }

    /// Millisecond cooldown remaining, given the millisecond of the last completion.
    /// Returns 0 when the storylet is ready. This module owns no clock, so the caller
    /// supplies both.
    pub fn cooldown_remaining_ms(def: &StoryletDef, last_completed_ms: i64, now_ms: i64) -> i64 {
        if def.cooldown_ms <= 0 {
            return 0;
        }
        def.cooldown_ms
            .saturating_sub(now_ms.saturating_sub(last_completed_ms))
            .max(0)
    }

    /// Fold the authoring effects into a `MutationOp` for the current state. An
    /// increment becomes a delta; a set becomes the delta that reaches the assigned
    /// value from the current one; a tag kind writes the compiled mask into the global
    /// add/remove masks. The result is the storylet's `on_complete_mutation` plus
    /// these terms, so the runtime effect policy stays a single `MutationOp` and
    /// `revert_delta` keeps the exact inverse. Entity-scoped tag effects stay in
    /// `on_complete_mutation.entity_tag_ops`; this module does not build them.
    pub fn compile_effects(&self, def: &StoryletDef) -> Result<MutationOp, DispatchError> {
        let mut mutation = def.on_complete_mutation.clone();
        for effect in &def.effects_on_complete {
            match effect.kind {
                EffectKind::AddTag => mutation.add_global_tags.merge(&effect.tag_mask),
                EffectKind::RemoveTag => mutation.remove_global_tags.merge(&effect.tag_mask),
            }
        }
        Ok(mutation)
    }

    /// Next conversational transition for the NPC.
    ///
    /// Edge weight is `base_cost + traversal_count * used_penalty_cost`. Player-only
    /// topic nodes are excluded from the path, except when they are the explicit start
    /// or goal. Ties between equal-cost paths are broken by the smaller edge ID. When
    /// no path exists, the deterministic fallback edge from the current topic is
    /// returned with `via_fallback` set; when that is also missing the call returns
    /// `DispatchError::NoFallbackEdge`. Negative or non-finite costs are rejected with
    /// `DispatchError::InvalidEdgeCost`.
    pub fn compute_next_npc_topic_step(
        &self,
        graph: &WaypointTopicGraph,
        current_topic: &str,
        goal_topic: &str,
        traversal_counts: &BTreeMap<String, u64>,
    ) -> Result<WaypointStep, DispatchError> {
        let start = topic_index(graph, current_topic)?;
        let goal = topic_index(graph, goal_topic)?;

        let costs = graph
            .edges
            .iter()
            .map(|edge| {
                for cost in [edge.base_cost, edge.used_penalty_cost] {
                    if !cost.is_finite() || cost < 0.0 {
                        return Err(DispatchError::InvalidEdgeCost {
                            edge_id: edge.edge_id.clone(),
                            cost,
                        });
                    }
                }
                let count = traversal_counts.get(&edge.edge_id).copied().unwrap_or(0);
                let penalty = count as f64 * edge.used_penalty_cost;
                if !penalty.is_finite() {
                    return Err(DispatchError::InvalidTraversalCount {
                        edge_id: edge.edge_id.clone(),
                        count,
                    });
                }
                Ok(edge.base_cost + penalty)
            })
            .collect::<Result<Vec<f64>, DispatchError>>()?;

        // Adjacency in authoring order, then by edge ID, so relaxation is stable.
        let mut adjacency: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (index, edge) in graph.edges.iter().enumerate() {
            let from = topic_index(graph, &edge.from_topic_id)?;
            let to = topic_index(graph, &edge.to_topic_id)?;
            if graph.nodes[to].is_player_only_trigger && to != goal {
                continue;
            }
            adjacency.entry(from).or_default().push(index);
        }
        for neighbours in adjacency.values_mut() {
            neighbours.sort_by_key(|&index| graph.edges[index].edge_id.clone());
        }

        let allowed = |index: usize| {
            !graph.nodes[index].is_player_only_trigger || index == start || index == goal
        };

        let mut best_cost = vec![f64::INFINITY; graph.nodes.len()];
        let mut best_edge: Vec<Option<usize>> = vec![None; graph.nodes.len()];
        let mut visited = vec![false; graph.nodes.len()];
        let mut queue: BinaryHeap<SearchItem> = BinaryHeap::new();

        best_cost[start] = 0.0;
        queue.push(SearchItem {
            cost: 0.0,
            edge_id: String::new(),
            node: start,
        });

        while let Some(item) = queue.pop() {
            if visited[item.node] {
                continue;
            }
            visited[item.node] = true;
            for &edge_index in adjacency.get(&item.node).map(Vec::as_slice).unwrap_or(&[]) {
                let to = topic_index(graph, &graph.edges[edge_index].to_topic_id)?;
                if !allowed(to) || visited[to] {
                    continue;
                }
                let cost = best_cost[item.node] + costs[edge_index];
                if !cost.is_finite() {
                    return Err(DispatchError::InvalidTraversalCount {
                        edge_id: graph.edges[edge_index].edge_id.clone(),
                        count: traversal_counts
                            .get(&graph.edges[edge_index].edge_id)
                            .copied()
                            .unwrap_or(0),
                    });
                }
                let better = cost < best_cost[to]
                    || (cost == best_cost[to]
                        && graph.edges[edge_index].edge_id < best_edge_id(graph, best_edge[to]));
                if better {
                    best_cost[to] = cost;
                    best_edge[to] = Some(edge_index);
                    queue.push(SearchItem {
                        cost,
                        edge_id: graph.edges[edge_index].edge_id.clone(),
                        node: to,
                    });
                }
            }
        }

        if best_cost[goal] < f64::INFINITY && goal != start {
            // Walk back from the goal, then take the first edge of that path.
            let mut path = Vec::new();
            let mut node = goal;
            // Bound the walk: a valid predecessor chain is shorter than the
            // node count, so a longer walk means the pointers disagree.
            for _ in 0..=graph.nodes.len() {
                let Some(edge_index) = best_edge[node] else {
                    break;
                };
                path.push(edge_index);
                node = topic_index(graph, &graph.edges[edge_index].from_topic_id)?;
                if node == start {
                    break;
                }
            }
            if let Some(&edge_index) = path.last() {
                return Ok(WaypointStep {
                    edge_index,
                    edge_id: graph.edges[edge_index].edge_id.clone(),
                    to_topic_id: graph.edges[edge_index].to_topic_id.clone(),
                    cost: costs[edge_index],
                    via_fallback: false,
                    dialogue_transition_asset_id: graph.edges[edge_index]
                        .dialogue_transition_asset_id
                        .clone(),
                });
            }
        }

        // Fallback: the player exhausted the normal routes.
        let mut fallback: Option<usize> = None;
        for (index, edge) in graph.edges.iter().enumerate() {
            if !edge.is_fallback_default || topic_index(graph, &edge.from_topic_id)? != start {
                continue;
            }
            if fallback.is_none() || edge.edge_id < graph.edges[fallback.unwrap()].edge_id {
                fallback = Some(index);
            }
        }
        let Some(edge_index) = fallback else {
            return Err(DispatchError::NoFallbackEdge {
                from_topic_id: current_topic.to_owned(),
            });
        };
        Ok(WaypointStep {
            edge_index,
            edge_id: graph.edges[edge_index].edge_id.clone(),
            to_topic_id: graph.edges[edge_index].to_topic_id.clone(),
            cost: costs[edge_index],
            via_fallback: true,
            dialogue_transition_asset_id: graph.edges[edge_index]
                .dialogue_transition_asset_id
                .clone(),
        })
    }
}

/// One Kuhn augmenting-path step: find a free row for `role`, moving an already
/// matched row's owner elsewhere if that owner can move. Roles and rows are visited in
/// a fixed order, so the matching is reproducible.
fn augment_role(
    role: usize,
    candidates: &[(u16, Vec<usize>)],
    owner: &mut [usize],
    seen: &mut [bool],
) -> bool {
    for row in &candidates[role].1 {
        if seen[*row] {
            continue;
        }
        seen[*row] = true;
        let current = owner[*row];
        if current == usize::MAX || augment_role(current, candidates, owner, seen) {
            owner[*row] = role;
            return true;
        }
    }
    false
}


fn topic_index(graph: &WaypointTopicGraph, topic_id: &str) -> Result<usize, DispatchError> {
    graph
        .nodes
        .iter()
        .position(|node| node.topic_id == topic_id)
        .ok_or_else(|| DispatchError::TopicNotFound {
            topic_id: topic_id.to_owned(),
        })
}

fn best_edge_id(graph: &WaypointTopicGraph, edge_index: Option<usize>) -> String {
    match edge_index {
        Some(index) => graph.edges[index].edge_id.clone(),
        None => String::new(),
    }
}

/// Min-heap item ordered by cost, then edge ID, then node index. Costs are checked
/// finite and non-negative before use, so their bit patterns order like the values.
struct SearchItem {
    cost: f64,
    edge_id: String,
    node: usize,
}

impl PartialOrd for SearchItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for SearchItem {
    fn eq(&self, other: &Self) -> bool {
        self.cost.to_bits() == other.cost.to_bits()
            && self.edge_id == other.edge_id
            && self.node == other.node
    }
}

impl Eq for SearchItem {}

impl Ord for SearchItem {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .to_bits()
            .cmp(&self.cost.to_bits())
            .then(other.edge_id.cmp(&self.edge_id))
            .then(other.node.cmp(&self.node))
    }
}

/// Deterministic pick among tied candidates from the caller's seed.
fn deterministic_index(seed: u64, count: usize) -> usize {
    let mut z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf03_8538_4dc2_d6fc);
    z = (z ^ (z >> 27)).wrapping_mul(0x85eb_ca6b_1b31_10d2);
    (z ^ (z >> 31)) as usize % count
}


#[cfg(test)]
mod dense_lookup_tests {
    use super::*;

    #[test]
    fn dense_id_lookup_and_rebuild() {
        let mut dispatcher = UnifiedDispatcher::new(vec![
            StoryletDef { storylet_id: 7, ..Default::default() },
            StoryletDef { storylet_id: 2, ..Default::default() },
            StoryletDef { storylet_id: 7, ..Default::default() },
        ]);
        assert_eq!(dispatcher.storylet(2).unwrap().storylet_id, 2);
        assert_eq!(dispatcher.storylet(7).unwrap().storylet_id, 7);
        assert!(dispatcher.storylet(9).is_none());

        // Deserialization skips the dense index; lookup must still work.
        let encoded = serde_json::to_string(&dispatcher).unwrap();
        let decoded: UnifiedDispatcher = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.storylet(7).unwrap().storylet_id, 7);
        assert_eq!(decoded, dispatcher);

        dispatcher.storylets.push(StoryletDef { storylet_id: 9, ..Default::default() });
        dispatcher.rebuild_index();
        assert_eq!(dispatcher.storylet(9).unwrap().storylet_id, 9);
        assert!(dispatcher.storylet(8).is_none());

        // A stale cache falls back to a scan rather than reporting a false miss.
        let mut stale = dispatcher.clone();
        stale.storylets.reverse();
        assert!(stale.storylet(2).is_some());
        assert!(stale.storylet(7).is_some());
    }
}
