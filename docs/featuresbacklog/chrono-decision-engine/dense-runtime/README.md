# Dense Runtime Backlog — Storylet Instances, Triggers, Completions, Endings, Transactions

**Status:** Proposed  
**Target:** ChronoFlow-Narrative Engine v6 / post-v5 hardening  
**Base spec:** `../chronoflow_narrative_engine_technical_specification_v5.md`  
**Related DOD baseline:** `../chronoflow_narrative_engine_technical_specification_v4.md` §3 (`REQ-DOD-01` … `REQ-DOD-05`)

## Why this backlog exists

The v5 spec is strong on deterministic scene execution, QBN/salience/waypoint dispatch, and call-history rollback. The gas-station and realtime stress tests exposed a missing runtime layer:

- multiple storylets must be active at once;
- a storylet must start because another storylet changed a quality, item, or world stage;
- storylets must depend on earlier storylet completions;
- endings must be selectable from authored storylets and profile pools, not a separate ending object;
- items must be shown, hidden, given, taken, and reacted to;
- concurrent effects need deterministic conflict rules.

Adding object graphs with nested `Vec`s for those concerns would violate the project’s data-oriented mandate. This backlog defines the runtime layer as dense tables, dense bitsets, and indexed event buckets.

The twelve requirements are:

| File | Requirement |
|---|---|
| [REQ-DO-01](REQ-DO-01-dense-runtime-tables.md) | Runtime storylet, scope, trigger, profile-pool, and transaction structures use dense indices and flat arrays. |
| [REQ-DO-02](REQ-DO-02-authoring-to-dense-ids.md) | Authoring IDs map to dense indices at load; runtime eligibility never uses string/UUID lookup. |
| [REQ-DO-03](REQ-DO-03-scope-completed-bitset.md) | Completed-storylet state is a per-scope bitset derived from call history. |
| [REQ-DO-04](REQ-DO-04-call-history-canonical.md) | Call history remains the canonical replay record; the bitset is rebuildable and verifiable. |
| [REQ-DO-05](REQ-DO-05-indexed-trigger-events.md) | Trigger lookup uses an indexed event table, not a full subscription scan. |
| [REQ-DO-06](REQ-DO-06-ending-profile.md) | An ending is a storylet marker plus an opt-in selection profile; zero-ending projects are valid. |
| [REQ-DO-07](REQ-DO-07-effect-write-sets.md) | Effect transactions declare a write set for concurrent conflict detection. |
| [REQ-DO-08](REQ-DO-08-selection-profiles.md) | One deterministic selection mechanism for normal, ending, QBN, salience, and custom profiles. |
| [REQ-DO-09](REQ-DO-09-selection-gates.md) | Reusable selection gates; pacing is an opt-in gate, not an ending rule. |
| [REQ-DO-10](REQ-DO-10-catalog-and-working-set.md) | Catalog/working-set split, O(1) IDs, bounded candidates, configurable profile pools. |
| [REQ-DO-11](REQ-DO-11-derived-feature-indexes.md) | Rebuildable derived indexes for tags, metrics, actor traits, items, choices, and stages. |
| [REQ-DO-12](REQ-DO-12-authoring-neutral-graph-contract.md) | Authored graph is canonical; one Start, one-or-more Ends, optional endings. |

## Design pillar

**Narrative designer flexibility is the primary constraint.** Dense tables, selection profiles, gates, and indexes are implementation details; they must not decide the game's genre, plot shape, ending count, candidate count, or required metadata. Every mechanism below is opt-in or project-configured, with a documented default only where a safe default exists.

## Shared concepts

The twelve files are not twelve independent mechanisms. Five concepts recur, and each has one owner:

| Concept | Owner | Reused by |
|---|---|---|
| **StoryletSpace** — stable key -> dense index, one capacity, one completion bitset | `REQ-DO-02`, `REQ-DO-03` | 01, 04, 06, 10 |
| **SelectionProfile** — pool + eligibility + gates + policy + terminal behavior + fallback | `REQ-DO-08` | 06, 09, 10 |
| **SelectionGate** — reusable counters/thresholds evaluated before a profile draws | `REQ-DO-09` | 06, 08, 10 |
| **DerivedIndex** — rebuildable view over authoritative state | `REQ-DO-11` | 03, 04, 05, 07 |
| **CommitRecord** — world delta + history entry + post-commit events | `REQ-DO-04`, `REQ-DO-07` | 03, 05, 11 |

Normal traversal, ending selection, QBN hubs, salience pools, waypoint choices, and any future mode are **profile configurations**, not separate algorithms. Endings are one profile with a terminal policy; a project may have zero such profiles.

### Consolidation decisions

1. **One storylet space.** `REQ-DO-02` owns the stable-key registry, dense index, and one 65,536-row capacity. `REQ-DO-03` owns the one completion bitset. No ending registry, ending index, or second capacity; overflow fails load.
2. **One selection mechanism.** `REQ-DO-08` owns `SelectionProfile`; `REQ-DO-06` defines the ending marker and default profile, not a second selector. Normal, QBN, salience, waypoint, and ending differ by configuration.
3. **One gate mechanism.** `REQ-DO-09` owns reusable gates. Pacing is a gate configuration, not an ending branch.
4. **One derived-index mechanism.** `REQ-DO-11` owns rebuildable feature indexes. They serve profiles and triggers, not only endings.
5. **One commit path.** `REQ-DO-07` owns transaction/write-set/apply/revert; `REQ-DO-04` only appends history, derived bits, and events after that commit.
6. **One catalog.** `REQ-DO-10` extends the `REQ-DO-02` storylet registry with payload/profile metadata and working-set limits; it does not define another registry.

Conflicts removed by this consolidation:

- no separate ending registry/index/capacity beside the storylet marker;
- no duplicate `ScopeTable`, `TriggerTable`, `EffectTransactionTable`, or `StoryletCatalog` schemas;
- no per-row `once_fired` flag; fired state is per-session and rollbackable;
- no duplicate commit protocol between history and effect transactions;
- no engine-wide 150/300 pacing rule; gates are opt-in profile data.

## Cross-cutting conventions

### Stable IDs are for persistence; dense indices are for runtime

| Boundary | Representation |
|---|---|
| Authoring/project JSON | Stable string or UUID keys |
| Call history on disk | Stable `StoryletKey`, scope key, and content revision |
| Load-time registries | `BTreeMap<StableKey, DenseIndex>` |
| Runtime hot path | `u16`/`u32` indices only |
| Save/replay | Stable keys resolved through the load-time registry |

Dense indices are not stable across content patches. Call history MUST NOT depend on a dense index surviving a patch.

### Existing dense primitives to reuse

- `TagBitset` is a fixed inline `[u64; 4]` with no heap allocation per query (`crates/storylet-engine/src/chrono_state.rs:36`).
- `MAX_TAGS = 256`, `MAX_ENTITIES = 256`, `MAX_METRICS = 512` (`chrono_state.rs:16-22`).
- `MetricRegistry` and `GameplayTagRegistry` already compile stable names to dense `u16` indices.
- `SignalToken` is already a packed fixed-size token (`crates/storylet-engine/src/chrono_runtime.rs:120`).
- `WorldStateTable` is a flat table of tags, metrics, passions, vertices, and edges (`chrono_state.rs:931`).
- `MutationOp` and `BijectiveDelta` are the existing effect/reversal contract (`chrono_state.rs:1575`, `1609`).

The new tables must follow the same style: capacity bounds checked at load, fixed-size rows, free lists, and no per-instance container allocation in the hot path.

### Separate capacities

`MAX_TAGS` is not the storylet capacity. Storylet completion needs its own bound:

| Constant | Suggested default | Reason |
|---|---:|---|
| `MAX_STORYLETS` | 65,536 | 1024 words = 8 KB per scope bitset at 65,536 storylets |
| `MAX_SCOPES` | 256 | Per-scope completed bitsets and history heads |
| `MAX_INSTANCES` | 1024 | Active storylet instances |
| `MAX_TRIGGER_SUBSCRIPTIONS` | 8192 | Event-indexed rows |
| `MAX_PROFILE_POOL` | 65,536 | Profile pool candidates; metadata-only scan |
| `MAX_PROFILE_CANDIDATES` | 256 | Hard per-profile ceiling; exact limit is a profile field (`REQ-DO-08`) |
| `MAX_ACTIVE_TRANSACTIONS` | 256 | Concurrent effect transactions |

Capacities are compile-time defaults, validated at load. Exceeding a capacity is a load error, not a silent truncation.

### Hot-path rules

1. No `String`, `&str`, UUID, or `BTreeMap` lookup in eligibility, trigger lookup, profile selection, or completion checks.
2. No heap allocation per tick for instance, trigger, profile, or transaction rows.
3. Iterate dense arrays in index order; use explicit tie-break keys.
4. Use `TagBitset`/`StoryletBitset` for masks; use CSR or bucketed arrays for sparse relations.
5. Persist stable keys and rebuild dense indices on load.

## Relationship to REQ-DOD and v5

This backlog is an **extension** of the v4 DOD baseline, not a replacement:

- `REQ-DOD-01` … `REQ-DOD-05` remain authoritative for tag indexing, inline bitsets, unified metric arrays, and CSR passion propagation.
- `REQ-DO-01` … `REQ-DO-12` add the runtime tables, ending, selection, pacing, catalog, feature, and authoring-neutrality contracts that v4 did not define.
- Do not create a second tag/metric registry. Reuse `GameplayTagRegistry`, `MetricRegistry`, `TagBitset`, and `MetricBound`.

**Supersede note:** v4 §5.1 proposed `Storylet.Completed.<Name>` tag bits for non-repeatable storylets. v5’s implementation instead keeps `FlowGraphInstance.completed_storylets: BTreeSet<u16>` (`chrono_runtime.rs:1737`) and per-node completion sets cloned into plans (`chrono_runtime.rs:2038`, `2506`). `REQ-DO-03` supersedes both with a per-scope `StoryletBitset`; `done.*` tags, if retained, are projections for quality/salience use, not the eligibility source.

The v5 spec header links back to this directory. When a new REQ-DO changes a v4 REQ-DOD rule, record the supersede note in the individual file, not only here.

**Ending decision:** an ending is a storylet marked `ending`, not a runtime ending-component table. Any ending composition is authoring-time generation; when a project opts into terminal endings, the runtime uses one ending `SelectionProfile` (`REQ-DO-06`). Projects with no terminal ending have no ending profile.

## Validation of the approach

A standalone Rust prototype validates the shape of the proposed layouts without changing the engine:

```sh
cargo run --release --manifest-path crates/storylet-engine/Cargo.toml --example dense_runtime_prototype
```

See [VALIDATION.md](VALIDATION.md) for the full log. It modelled the proposed layouts in release mode on this workstation:

- 4,096 storylets, 100,000 history entries:
  - dense bitset build: **48.1 µs**
  - `BTreeSet<u16>` build: **3.3131 ms**
  - 10,000,000 membership queries: **5.8878 ms** dense vs **95.5017 ms** `BTreeSet`
- 4,096 trigger subscriptions over 64 event keys, 100,000 events:
  - CSR bucket enumeration: **1.1638 ms** for 6,400,000 matched rows
  - full subscription scan: **64.2989 ms** for the same matches
- 128 legacy ending-component rows, 100,000 resolutions:
  - pre-sorted resolution: **3.8146 ms** (superseded by profile pools)
- 10,000 write sets, 32 neighbour pairs each:
  - tag-mask conflict scan: **2.9969 ms**, including one deliberately overlapping pair

These numbers validate the shape, not the production implementation. They show that dense bitsets and indexed buckets remove the asymptotic problems. The prototype did not exercise serialization, cache pressure from real rows, or content-patch rebasing.

A second engine-type probe validates the current seam:

```sh
cargo run --release --manifest-path crates/storylet-engine/Cargo.toml --example dense_runtime_engine_probe
```


- `FlowGraphInstance` QBN completions `[1,2,3]` produced **3 call-history entries**, and rebuilding a bitset from `global_call_history` produced `0b1110`, matching the live `BTreeSet`.
- A zero-sum passion mutation that explicitly named micro-passion `1` produced macro writes for **both NPC 0 and NPC 1**, proving that a write set derived from the explicit op alone is incomplete.

The workspace database currently contains **502 storylets**, so a `TagBitset`-sized completion set (256 bits) is insufficient. The target 20,000 ending storylets plus a normal library requires `MAX_STORYLETS = 65,536` by default (8 KB per scope); `StoryletBitset` must not reuse `TagBitset` capacity.

The next sections define the contracts, invariants, tests, and migration path for each requirement.

## Recommended implementation order

1. `REQ-DO-02` load-time registries and capacity validation.
2. `REQ-DO-03` per-scope completed bitset, initially rebuilt from the existing `BTreeSet` callers.
3. `REQ-DO-04` call-history canonicalization and rebuild verification.
4. `REQ-DO-01` dense instance/scope/transaction tables, replacing `BTreeSet` completion state.
5. `REQ-DO-05` trigger index and event queue.
6. `REQ-DO-06` ending marker and opt-in terminal profile.
7. `REQ-DO-07` write-set conflict detection and commit policy.
8. `REQ-DO-08` selection profiles and one-shot weighted policy.
9. `REQ-DO-09` reusable selection gates and optional pacing.
10. `REQ-DO-10` catalog/working-set split and profile capacity.
11. `REQ-DO-11` derived feature indexes.
12. `REQ-DO-12` authoring-neutral graph contract.

Do not start with the trigger index. It depends on stable dense IDs and per-scope completion state.

## Non-goals

- Replacing QBN, salience, waypoint, or scene-composer algorithms.
- Requiring every authoring field to be dense on disk.
- Changing the public C ABI or persisted JSON format without a versioned migration.
- Adding a separate narrative engine or object graph beside the dense runtime.
- Implementing the full v6 scheduler in this backlog; this backlog defines the data contracts the scheduler needs.
