# REQ-DO-11 — Derived Feature Indexes

**Status:** Proposed  
**Target:** ChronoFlow v6  
**DOD relationship:** Materialized views over `WorldStateTable`; no duplicate passion or inventory store.  
**Depends on:** `REQ-DO-01`, `REQ-DO-02`, `REQ-DO-03`  
**Related:** `REQ-DO-04`, `REQ-DO-05`, `REQ-DO-06`, `REQ-DO-07`, `REQ-DO-08`, `REQ-DO-10`

## 1. Problem

Selection profiles need to test and weight:

- global qualities and world stage;
- per-character personality and emotion;
- items held;
- choices made.

The engine already has raw material:

- `TagBitset`/`MetricRegistry` for qualities;
- `PassionRegistry`/`PassionState` for per-NPC micro/macro passions;
- `WorldVertex` `CHARACTER`/`ITEM` layers and `WorldEdge` relations;
- call history/event log.

But the Chrono eligibility/weighting path cannot efficiently read all of these:

- `QualityPredicate` supports global metrics, entity tags, location/visited/met, not per-actor numeric traits;
- raw passion macro values are not directly queryable; only threshold tags;
- item ownership/quantity lives in graph vertices/edges/attributes without a canonical index;
- choices live in history, not a feature index.

Evaluating large profile pools over graph scans would be too slow and would duplicate state. The answer is materialized indexes over existing state, shared by normal, ending, QBN, salience, trigger, and custom profiles.

## 2. Requirement

**REQ-DO-11:** Selection profiles, trigger predicates, and weights SHALL read dense feature indexes derived from `WorldStateTable` and call history; indexes SHALL NOT become an independent source of truth.

Rules:

1. `WorldStateTable` remains authoritative for tags, metrics, passions, vertices, and edges.
2. Call history remains authoritative for choices/completions.
3. Feature indexes are rebuilt or updated transactionally with state changes.
4. Selection profiles compile requirements into masks/ranges over feature indices.
5. No index duplicates the full passion pool or item graph.
6. Index updates participate in the effect transaction/write set.
7. Feature lookup is O(1)/O(bitset), not a graph scan per candidate.

## 3. Feature families

### 3.1 Global qualities and world stage

Existing:

- `state.metrics[]` via `MetricRegistry`;
- `state.global_tags` via `GameplayTagRegistry`;
- world-stage tags.

Add only:

```text
WorldStageIndex
  stage_id -> dense index / bucket
  active_mask: TagBitset
```

If world stage is already tag-based, this is a compiled alias, not a new store.

### 3.2 Per-character personality and emotion

Existing:

- `StarNpcDef.micro_passions`, `PassionState.micro/macro/active`;
- threshold rules produce tag bits;
- character vertex attributes.

Add:

```text
ActorTraitIndex
  actor_index: u16
  trait_index: u16
  value_bucket: u8
```

Sources:

- macro/micro passions for emotion/approval;
- authored character attributes for personality;
- role-bound actor traits if authored.

This is a derived view. Do not add a second `PassionRegistry`. If raw numeric values are required for profile weights, expose them through a compiled predicate subject, not a parallel table.

### 3.3 Items held

Existing:

- ITEM-layer vertices;
- `WorldEdge` relations (for example `OWNS`, `LOCATED_IN`);
- `WorldVertex.attributes` (`quantity`, flags).

Add:

```text
ItemIndex
  owner_index: u16
  item_index: u16
  quantity: u32
  location_index: u16
  flags: TagBitset
```

The index is rebuilt from item graph mutations. The graph remains authoritative.

Required item predicates:

- `ItemOwned(owner, item)`;
- `ItemQuantity(owner, item, min, max)`;
- `ItemPresent(location, item)`;
- `ItemShown/Hidden` if presentation affects selection.

### 3.4 Choices made

Existing:

- call history entries;
- choice effects can set tags/counters.

Add:

```text
ChoiceHistoryIndex
  scope_id: u16
  choice_key: u16
  count: u16
  first_tick/last_tick: u32
```

It can be a dense bitset/counter table per scope, or a compact map for sparse choice keys. Do not scan call history per candidate.

### 3.5 World state

Existing tags/metrics plus world-stage index. No new authority.

## 4. Predicate integration

Add compiled feature subjects:

```text
PredicateSubject::ActorTrait
PredicateSubject::ItemOwned
PredicateSubject::ItemQuantity
PredicateSubject::ChoiceMade
PredicateSubject::WorldStage
```

Profiles compile their requirements into:

- tag masks;
- metric gates;
- actor-trait masks/ranges;
- item masks/quantity ranges;
- choice masks;
- world-stage mask.

The exact eligibility check remains a fixed set of bitset/range operations.

## 5. Weight integration

Profile weights may use:

```text
base_weight
+ salience bonus from tag overlap
+ quality distance
+ actor-trait alignment
+ item/choice bonuses
```

Compute this once over the selected profile pool. Do not traverse the item graph or call history per candidate.

## 6. Transactional updates

When a transaction commits:

1. Apply `MutationOp`/`GraphMutation`/`PassionMutation`.
2. Update affected feature indexes.
3. Update the write set (`REQ-DO-07`) to include trait/item/choice rows.
4. Emit feature events (`REQ-DO-05`).

On rollback, restore the index rows from the transaction delta/rebuilt view. Index and authoritative state must agree at every checkpoint.

## 7. Invariants

1. `ItemIndex` equals the projection of the ITEM graph.
2. `ActorTraitIndex` equals the projection of passions/attributes according to the feature schema.
3. `ChoiceHistoryIndex` equals the resolved call-history prefix for the scope.
4. No profile predicate reads the full graph or full history.
5. Feature indexes are rebuildable from authoritative state.
6. Index rows participate in conflict detection.
7. Feature schema changes create a versioned migration.

## 8. Acceptance tests

- `feature_item_index_matches_graph`: add/remove item vertices/edges, index matches.
- `feature_item_quantity`: quantity predicates filter correctly.
- `feature_actor_trait`: macro/micro passion changes update actor trait index.
- `feature_choice_index`: choice effects update counts/flags; no history scan.
- `feature_world_stage`: stage changes update mask.
- `feature_profile_filter`: a large profile pool is filtered using indexes without graph scans.
- `feature_rollback`: commit/revert updates indexes exactly.
- `feature_write_set`: an item update conflicts with a transaction writing the same item.
- `feature_rebuild`: rebuilding all indexes from state/history equals incremental result.

## 9. Migration

1. Add feature schema IDs and resolve them at load.
2. Build indexes from current `WorldStateTable` and call history.
3. Add predicate subjects.
4. Add selection-profile compilation to masks.
5. Switch profile eligibility/weights to indexes.
6. Keep debug rebuild verification.

## 10. Risks

- Indexes can become a second store. Enforce “rebuildable from authoritative state” in tests.
- Item graph `relation_type` is a string; compile to a dense relation kind at load.
- Actor traits must not assume every character is a Star NPC.
- Choice history can grow; index only authored choice keys, not every event.
- Feature schema changes need migration/version pinning for saves.

## 11. References

- `PassionRegistry`/`PassionState` — `crates/storylet-engine/src/chrono_state.rs:802, 1821`.
- `WorldVertex`/`WorldEdge` — `chrono_state.rs:227, 242`.
- `MutationOp`/`GraphMutation` — `chrono_state.rs:1575, 331`.
- `QualityPredicate` — `chrono_dispatch.rs:116`.
- `CallHistoryEntry` — `chrono_runtime.rs:360`.
- `REQ-DO-06` — ending storylets.
- `REQ-DO-07` — write sets.
- `REQ-DO-08` — profile weights/selection.
