# REQ-DO-10 — Storylet Catalog and Working Set

**Status:** Proposed  
**Target:** ChronoFlow v6  
**DOD relationship:** Extends dense runtime tables with a catalog/runtime split; reuses `GameplayTagRegistry`/`MetricRegistry`.  
**Depends on:** `REQ-DO-01`, `REQ-DO-02`, `REQ-DO-03`  
**Related:** `REQ-DO-06`, `REQ-DO-08`, `REQ-DO-09`, `REQ-DO-11`

## 1. Problem

Stress target, not a mandatory content count:

```text
20,000 profile-pool storylets (ending is one example)
+ normal storylet library
+ configurable candidates per beat
+ configurable path length
```

Current architecture:

- `StoryletDef.storylet_id: u16` — current dense ID;
- the proposed `StoryletSpace` (`REQ-DO-02`) owns capacity and overflow;
- `UnifiedDispatcher.storylet(id)` scans `Vec<StoryletDef>` linearly (`chrono_dispatch.rs:540`);
- `completed_storylets: BTreeSet<u16>` (`chrono_runtime.rs:1737`);
- no catalog/runtime distinction.

At the 20k stress target, dense IDs remain viable; capacity and overflow are owned by `REQ-DO-02`. The selection path still degrades linearly.

## 2. Requirement

**REQ-DO-10:** The runtime SHALL support a configurable storylet catalog, load only a bounded working set per `SelectionProfile`, and resolve storylet IDs in O(1).

Rules:

1. Persisted catalog may exceed runtime candidate capacity.
2. Runtime tables hold only the active candidate set and profile pools.
3. `storylet_index_by_id` is O(1), not a `Vec::iter().find`.
4. Capacity and bitset width are owned by `StoryletSpace` (`REQ-DO-02`) and `StoryletBitset` (`REQ-DO-03`); this requirement does not restate them.
5. Each profile pool is separate from the normal candidate set; a stress target of 20,000 ending storylets is supported, not required.
6. Only profile metadata is needed for pool eligibility/weighting; payload graphs may be lazy-loaded.
7. Catalog validation rejects duplicate stable keys and over-capacity runtime sets.

## 3. Catalog structure

```text
StoryletCatalog = StoryletRegistry (REQ-DO-02) + metadata columns
  revision:          Vec<u64>
  is_ending:         StoryletBitset
  location_mask:     Vec<TagBitset>
  role_mask:         Vec<TagBitset>
  metric_gate_base:  Vec<u32>
  metric_gate_len:   Vec<u16>
  payload_ref:       Vec<u32>          // asset/graph location, lazy
  duration_minutes:  Vec<f32>
```

Runtime rows for the selected candidate set:

```text
StoryletRuntimeTable
  storylet_index:    Vec<u16>          // dense ID
  definition:        Vec<StoryletDef>  // candidate rows only
  index_by_id:       IndexById         // direct Vec<u16> if dense, sorted Vec<u32> if sparse
```

`index_by_id` must be a direct array lookup when IDs are dense, or a sorted `u32` index with binary search if sparse. It must not scan.

## 4. Candidate pipeline

```text
1. exact retrieval from catalog by location/role/tag/arc
2. cap to the profile candidate limit
3. load those StoryletDefs into runtime
4. exact QBN/salience
5. select one storylet through the profile policy

At a profile draw:
1. select the profile pool (ending is one example)
2. filter exact profile eligibility and gates
3. load profile metadata/weights (payload lazy)
4. apply the profile policy
5. load the selected storylet payload
```

The catalog remains authoritative for stored content; runtime tables are a bounded working set.

## 5. Capacity ownership

- `StoryletSpace` (`REQ-DO-02`): catalog/index capacity and overflow.
- `StoryletBitset` (`REQ-DO-03`): completion bitset width.
- `SelectionProfile` (`REQ-DO-08`): per-profile candidate limit.
- Runtime table capacities: README.

This requirement only defines the catalog/working-set split and lazy loading. It does not repeat capacity constants.

## 6. Profile pools

A profile pool is a catalog projection:

```text
profile_pool: Vec<StoryletIndex>  // sorted by profile priority/tie-break
```

`SelectionProfile` (`REQ-DO-08`) decides membership, candidate limit, gates, and policy. The ending profile uses `is_ending` rows; other profiles may use decks, tags, QBN sets, salience pools, or custom queries. Pools are disjoint where the profile requires it. Payload loading is deferred until the selected storylet is chosen.

## 7. Current linear lookup

Replace:

```rust
self.storylets.iter().find(|def| def.storylet_id == storylet_id)
```

with an indexed lookup. At 502 storylets the scan is acceptable; at 20,000 candidates it is not. This is required before the 20,000-ending target.

## 8. Invariants

1. Catalog and runtime tables agree on stable keys and revisions.
2. Runtime candidate count never exceeds the active profile `candidate_limit`.
3. `index_by_id` resolves every loaded storylet exactly once.
4. A profile pool never leaks into another profile unless its configuration explicitly permits it.
5. Payloads are loaded only for candidates/selected storylet.
6. Completion bitset covers the configured storylet space.
7. `u16` overflow fails load; no silent wrap.
8. Candidate retrieval is deterministic for the same catalog revision and state.

## 9. Acceptance tests

- `catalog_20k_stress_load`: 20,000 profile-pool rows + normal library load without ID overflow.
- `catalog_index_lookup`: 1,000,000 ID lookups, no linear scans.
- `catalog_candidate_cap`: each profile stays at its configured candidate limit.
- `catalog_profile_pool_disjoint`: profile pools do not leak into normal candidates unless `include_ending` is authored.
- `catalog_lazy_payload`: profile payload not loaded until selected.
- `catalog_completion_bitset`: configured bitset covers all rows; no TagBitset truncation.
- `catalog_capacity_error`: exceeding `StoryletSpace` capacity fails with a typed load error.
- `catalog_retrieval_determinism`: same state/revision returns same candidates.

## 10. Migration

1. Add the catalog and lazy payload refs beside current `Vec<StoryletDef>`.
2. Build `index_by_id`.
3. Move normal QBN to the runtime candidate set.
4. Add profile-pool projection.
5. Apply the `StoryletSpace` capacity and `StoryletBitset` width.
6. Keep `u32` as a future schema migration; do not widen individual arrays.
7. Remove linear lookup after parity.

## 11. Risks

- Large profile payloads may be large; lazy loading is mandatory if all graphs do not fit.
- Catalog/runtime duplication can drift; use one revision/fingerprint.
- Capacity is owned by `StoryletSpace`; do not approximate a future `u32` migration by widening one table.
- Candidate retrieval quality becomes a narrative concern; retrieval must preserve exact eligibility.
- Do not put a large profile pool in the normal candidate set just because IDs fit.

## 12. References

- `crates/storylet-engine/src/chrono_dispatch.rs:540` — linear lookup.
- `crates/storylet-engine/src/chrono_state.rs:16-22` — capacities.
- `crates/storylet-engine/src/chrono_runtime.rs:1737` — completion set.
- `REQ-DO-02` — stable/dense IDs.
- `REQ-DO-03` — completion bitset.
- `REQ-DO-06` — ending storylets.
- `REQ-DO-09` — path gate.
