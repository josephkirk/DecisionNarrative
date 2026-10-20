# REQ-DO-02 — Authoring IDs to Dense Runtime Indices

**Status:** Proposed  
**Target:** ChronoFlow v6  
**DOD relationship:** Extends `REQ-DOD-01`; adds storylet/item registries and the catalog boundary beside the existing tag and metric registries.  
**Depends on:** `REQ-DO-01`, `REQ-DOD-01`  
**Related:** `REQ-DO-03`, `REQ-DO-04`, `REQ-DO-05`, `REQ-DO-06`, `REQ-DO-10`

## 1. Problem

Authoring data uses stable identities: storylet IDs, quality names, tag paths, item keys, profile keys, and content revisions. Runtime hot paths must not resolve strings, UUIDs, or `BTreeMap` keys.

The engine already compiles tags and metrics to dense `u16` indices:

- `GameplayTagRegistry` maps tag names to `u16`.
- `MetricRegistry` maps metric names to `u16`.
- `StoryletDef.storylet_id` is already `u16` (`crates/storylet-engine/src/chrono_dispatch.rs:226`).
- `FlowGraphInstance.completed_storylets` is a `BTreeSet<u16>` (`chrono_runtime.rs:1737`).

What is missing is a single load-time contract for every runtime table and a persisted stable key that survives content patches.

## 2. Requirement

**REQ-DO-02:** Authoring IDs SHALL be mapped to dense indices at load; runtime eligibility SHALL NOT use string/UUID lookup.

Specific rules:

1. Every runtime table row has a stable authoring key.
2. A load-time registry resolves stable keys to dense indices exactly once.
3. After load, runtime queries use indices and bitsets only.
4. Duplicate, missing, over-capacity, or unresolved keys are typed load errors.
5. Call history stores stable keys and content revision, not runtime indices.
6. Dense indices are scoped to a loaded content revision and never persisted as the only identity.

## 3. Stable keys and runtime indices

```text
StableStoryletKey   = string or UUID     // authoring identity
StableQualityKey    = string             // quality/tag/metric name
StableItemKey       = string
StableProfileKey    = string             // selection profile identity
StableTriggerKey    = string             // trigger subscription identity
StableScopeKey      = string             // runtime-created, persisted
StoryletIndex       = u16                // runtime dense index
ScopeIndex          = u16
ItemIndex           = u16
ProfileIndex        = u16
```

The runtime type alias is deliberately narrow:

```rust
pub type StoryletIndex = u16;
pub type ScopeIndex = u16;
pub type ItemIndex = u16;
pub type ProfileIndex = u16;
```

`StoryletSpace` is the one capacity contract for the catalog, completion bitset, profile pools, and call-history resolution:

```text
StoryletSpace
  max_storylets: 65,536       // u16 index space
  bitset_words:  1,024        // REQ-DO-03
  overflow:      typed load error; u32 is a future schema migration
```

If content exceeds the `u16` space, fail load with a typed capacity error. A `u32` space is a future schema migration, not a per-array widening. Do not introduce a separate ending capacity.

## 4. Load pipeline

```text
1. Parse project bundle
2. Validate stable key uniqueness and graph references
3. Compile tag registry       -> TagIndex
4. Compile metric registry    -> MetricIndex
5. Compile storylet registry  -> StoryletIndex
6. Compile item registry      -> ItemIndex
7. Compile profile registry   -> ProfileIndex
8. Compile trigger rows       -> TriggerIndex, CSR buckets
9. Compile profile memberships -> ProfileIndex views
10. Validate all index references in:
    - preconditions
    - effects
    - trigger keys
    - profile membership/gates/weights
    - required storylet dependencies
11. Build scopes and empty completed bitsets
12. Freeze the registries; reject further mutation
```

### Registry row

```text
DenseRegistry<T>
  keys:      Vec<StableKey>            // index -> stable key
  lookup:    BTreeMap<StableKey, u16>  // load-time only
  revision:  u64
  frozen:    bool
```

`lookup` exists only during load and authoring tools. Runtime code must not receive it.

### Mapping functions

```text
fn resolve_storylet(key: &StableStoryletKey) -> Result<StoryletIndex, LoadError>
fn storylet_key(index: StoryletIndex) -> &StableStoryletKey
```

`resolve_storylet` is a load/editor function. It SHALL NOT appear in the eligibility, trigger, ending, or scheduler hot paths.

## 5. Validation rules

| Check | Failure |
|---|---|
| Duplicate stable key | `LoadError::DuplicateKey { kind, key }` |
| Missing stable key | `LoadError::MissingKey { kind, key }` |
| More rows than capacity | `LoadError::CapacityExceeded { kind, capacity }` |
| Unknown tag/metric/item in a compiled row | `LoadError::UnresolvedReference { row, field }` |
| Storylet dependency references unknown storylet | `LoadError::UnresolvedStoryletDependency` |
| Trigger references unknown event key | `LoadError::UnresolvedTriggerKey` |
| Profile references unknown storylet/gate/predicate | `LoadError::UnresolvedProfileReference` |
| Duplicate trigger row `(kind, key, storylet, predicate)` | `LoadError::DuplicateTrigger` |
| Duplicate profile key | `LoadError::DuplicateKey` |
| `MAX_STORYLETS` exceeded | `LoadError::StoryletCapacityExceeded` |

All checks run before the runtime starts. A malformed content bundle never produces a partially initialized runtime.

## 6. Runtime pass

After load:

- `StoryletDef.required_storylets` is a `StoryletBitset` (see `REQ-DO-03`).
- `StoryletDef.required_tags` and `forbidden_tags` are `TagBitset`.
- `TriggerTable` keys are `u16`.
- Ending is a storylet marker (`StoryletCatalog.is_ending`) plus a `SelectionProfile`; there is no ending registry or ending index.
- `ScopeTable` indices are `u16`.
- `EffectTransactionTable` stores indices.
- `CallHistoryEntry` stores stable keys for persistence and dense indices for the current session.

## 7. Call history and content patches

Call history is persisted with stable keys. The canonical `PersistedCallEntry` schema is owned by `REQ-DO-04`; this requirement only fixes the identity rule:

```text
persist stable keys + content_revision; never persist a dense index as identity
```

At load:

1. Resolve stable keys to dense indices.
2. If a key no longer exists, apply the existing RebaseCallHistoryOnPatch policy.
3. If a key is ambiguous after a patch, fail load; do not pick the first.
4. Rebuild per-scope completed bitsets from the resolved history.
5. Never trust a dense index read from disk.

## 8. Hot-path prohibition

The following are forbidden after load:

- `BTreeMap<String, _>` lookup for storylet/quality/item/ending identity.
- `String` allocation in eligibility or selection.
- Hashing a stable key per candidate.
- Comparing stable keys in a sort comparator.
- Re-resolving a storylet key during rollback.

Debug builds MAY assert that a resolved index maps back to the expected stable key. Release builds skip that check.

## 9. Integration

- `GameplayTagRegistry` and `MetricRegistry` remain the existing compile path for tags and metrics.
- Add `StoryletRegistry` and `ItemRegistry` beside them. Profiles reference storylet indices; they are not a second storylet registry.
- Extend `StoryletDef` with `required_storylet_mask` and stable-key metadata only at the authoring boundary.
- The storylet catalog (`REQ-DO-10`) is the storylet registry plus payload/profile metadata; do not add a parallel catalog map.
- Extend `CallHistoryEntry` per `REQ-DO-04`.
- The service projection/import path resolves authoring IDs before constructing runtime tables.

## 10. Acceptance tests

- `dense_ids_duplicate_key_rejected`: two storylets with the same key fail load.
- `dense_ids_unknown_reference_rejected`: a trigger or profile referencing a missing key fails load.
- `dense_ids_capacity_rejected`: `MAX_STORYLETS + 1` fails with a typed error.
- `dense_ids_runtime_no_lookup`: a counting instrumentation hook shows zero stable-key lookups after load for 100,000 QBN/trigger/profile operations.
- `dense_ids_roundtrip`: every dense index maps back to its stable key.
- `dense_ids_patch_rebase`: a removed storylet key is handled by the rebase policy; an ambiguous key fails load.
- `dense_ids_history_indices_never_loaded`: a save containing a dense index but no stable key is rejected.

## 11. Migration

1. Add storylet/item/profile registries beside the existing tag/metric registries.
2. Convert storylet IDs from ad hoc `u16` literals to compiled indices.
3. Add stable keys to call history with a schema version.
4. Keep a compatibility reader for old saves: resolve old storylet IDs against the last known content revision or mark them unrecoverable.
5. Remove string resolution from runtime paths only after all call sites are converted.

## 12. Risks

- Stable keys and dense indices can drift if content patches reorder rows. Always persist stable keys.
- `BTreeMap` lookup at load is acceptable; make its lifetime explicit so it cannot leak into runtime code.
- Duplicate stable keys may exist in legacy content. Fail load rather than choosing one.
- A content patch that renames a key is a migration, not a rebase. Record it explicitly.
- Capacity defaults must be measured against real content, not guessed from the stress fixture.

## 13. References

- `crates/storylet-engine/src/chrono_state.rs:399` — tag registry.
- `crates/storylet-engine/src/chrono_state.rs:653` — metric registry.
- `crates/storylet-engine/src/chrono_dispatch.rs:226` — `StoryletDef` with `u16` ID.
- `crates/storylet-engine/src/chrono_runtime.rs:360` — `CallHistoryEntry`.
- `chronoflow_narrative_engine_technical_specification_v4.md` `REQ-DOD-01` — dense tag indexing.
- `REQ-DO-04` — call-history canonicalization.
