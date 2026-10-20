# Chrono Decision Engine

**Status:** Current implementation design and delivery record  
**Scope:** The deterministic narrative runtime behind DecisionNarrative's storylet studio  
**Supersedes:** `docs/featuresbacklog/chrono-decision-engine/` (v3/v4/v5 specifications and the dense-runtime backlog)

## 1. Summary

Chrono is the deterministic decision core for storylet-driven narrative games. It evaluates authored eligibility, selects content, executes typed effects, records reversible history, and exposes diagnostics to the studio.

The design is host-independent Rust with no HTTP, SQLite, browser, or model dependency. The local service adds persistence and MCP; the Vue studio adds authoring and playtest surfaces.

The central rule is:

> The authored graph is the narrative contract. The backend may optimize, index, and validate it, but it must not decide the game's genre, plot shape, required ending, or mandatory metadata.

## 2. Architectural pillars

1. **Authored graph, not a genre template.** Nodes, edges, endings, and optional metadata are project data.
2. **Deterministic execution.** Same state, seed, cursor, content revision, and command produce the same result.
3. **Data-oriented runtime.** Dense indices, flat arrays, fixed bitsets, CSR indexes, and explicit iteration order.
4. **Stable identity at rest, dense identity in memory.** Persistence uses stable keys; runtime uses dense indices.
5. **Call history is canonical.** Completion bitsets, trigger fire state, and indexes are rebuildable projections.
6. **Transactional effects.** World mutation, history entry, completion bit, and post-commit events share a commit boundary.
7. **Designer flexibility first.** Normal traversal, endings, QBN hubs, salience, waypoints, and future modes are profile configurations, not separate engines.

## 3. Runtime layers

```text
authored project JSON
  -> project/scenario validation
  -> stable-key catalog + dense indices
  -> compiled storylet graph and dispatcher rows
  -> runtime scope/instance state
  -> eligibility + selection profile + gates
  -> reversible effect transaction
  -> call history + completion/index projections
  -> event queue + next storylet
```

### 3.1 Project and graph

- `StoryletRecord` keeps authored data: prose, choices, effects, parameters, preconditions, graph, and metadata.
- `NarrativeGraph` is canonical for storylet flow.
- One Start is required.
- One or more End nodes are allowed.
- Every node must be reachable from Start and able to reach an End.
- A jump target takes precedence over a leftover terminal flag.
- A dedicated ending requires every End to be terminal and to have no jump.
- Unknown or non-executable node types fail validation; supported types execute deterministically.

### 3.2 Storylet space and catalog

`StoryletSpace` is the single identity/capacity contract:

```text
stable key -> dense StoryletIndex (u16)
MAX_STORYLETS = 65,536
overflow -> typed load error
```

`StoryletCatalog` adds `is_ending`, payload reference, and duration metadata. It is frozen after load. The dispatcher's dense lookup is sized to the maximum loaded ID and rebuilt on deserialize.

Authoring and persistence use stable keys. Runtime eligibility uses dense indices only. Call history records stable keys for persistence and dense indices for the current content revision.

### 3.3 Dense runtime tables

Two table families exist:

```text
StoryletInstanceTable
  storylet_index, scope_index, state, priority,
  started_tick, finished_tick, transaction_index,
  generation, free list, live count

ScopeTable
  parent, scope_kind, flags, completed bitset,
  history range, active-instance range,
  generation, free list
```

Rows are parallel vectors. Variable-length data belongs in flat arenas keyed by `(base, len)`. Released rows are generation-checked and reused through a free list.

`StoryletBitset` is 65,536 bits = 1,024 words = 8 KiB per scope. It supersedes global `done.*` tags as the eligibility source. `done.*` tags, if retained, are projections.

### 3.4 Scope completion

Each scope owns a completion bitset. Inheritance is opt-in and directional. A child may see a parent's bits without copying them. Completion changes are recorded in call history; rollback removes the corresponding bit at the commit boundary.

`FlowGraphInstance.completed_storylets` is a bitset. Per-node completion sets remain sparse sets for node-local bookkeeping, but instance eligibility reads the bitset.

### 3.5 Call history

`CallHistoryEntry` is the canonical replay record. It carries:

- node GUID and input/output pins;
- chosen storylet ID and stable storylet key;
- scope index;
- completion flag;
- trigger key/subscription;
- effect transaction reference;
- content revision, tick, and canonical choice key.

`PersistentHistoryEngine` stores commits as a parent-linked DAG. `path_entries(head)` returns the active branch. Replay, checkout, reconciliation, and patch rebasing use that path.

A completion bitset is rebuilt by folding the commit path. `StoryletBitset` serializes as a sorted index list, matching the old `BTreeSet<u16>` JSON shape and keeping saved sessions compact.

### 3.6 Triggers

`TriggerTable` is compiled content, not per-session state:

```text
rows: event_kind, event_key, storylet_index,
      priority, once, enabled, scope_filter,
      stable key, generation

CSR:
  bucket_offsets[kind * key_space + key]
  bucket_entries[subscription_index]
```

Buckets are sorted by priority, storylet index, and subscription index. `EventQueue` holds runtime events; `TriggerFireTable` holds per-session fired bits derived from call history.

`process_tick` uses a persistent `TriggerCursor`. It:

- processes candidates in deterministic order;
- skips disabled, scope-mismatched, already-fired, and already-started rows;
- advances exactly once per consumed candidate;
- resumes a capped event on the next tick;
- keeps an event queued until its candidates are drained.

Scopes are `u64` across events, rows, active keys, and fired state.

### 3.7 Effects and transactions

`MutationOp` and `BijectiveDelta` remain the effect/reversal primitive. `EffectTransaction` adds:

```text
state, scope, instance, write set,
operation arenas, delta, sequence
```

`WriteSet` covers tags, metrics, entities, items, scopes, reservations, actor traits, and choices. Two transactions conflict when their write sets intersect. The intended policies are `RejectLater`, `PriorityWins`, `MergeAdditive`, and `Serialize`.

Indirect writes must be included in the closure: zero-sum passions, macro rows, threshold tags, location/movement masks, graph transforms, item/cast changes, and feature rows.

### 3.8 Selection profiles and gates

Selection is one mechanism for many modes:

```text
SelectionProfile
  key, pool, include_ending, candidate_limit, floor,
  eligibility, gates, policy, terminal policy,
  terminal target, fallback policy
```

```text
SelectionPolicy = Priority | SalienceMax | Weighted | SeededOrder
TerminalPolicy  = Continue | EndScope | Jump
FallbackPolicy  = Storylet(key) | Continue | Error
```

Normal traversal, ending selection, QBN hubs, salience pools, waypoint choices, and future modes are profile configurations. Weighted selection is one-shot O(n); the sampler has an explicit seed/cursor and does not mutate world state.

`SelectionGate` is reusable: count, time, flag, metric, tag, or custom. Pacing is an opt-in gate, not an ending rule. Gate failure returns a typed rejection; fallback applies only after gates pass and the eligible set is empty.

### 3.9 Endings

An ending is a storylet marker plus a selection profile. There is no runtime ending-component table and no separate ending index.

- `is_ending` is a storylet bit.
- A dedicated ending graph has all Ends terminal with no jump.
- The ending profile is a `SelectionProfile`; it is not a second selector.
- A project may have zero ending storylets.
- Terminal behavior is profile-owned; `EndScope` is the default, with `Continue`/`Jump` available for epilogues, chapter endings, and NG+.
- When a project has no ending profile, endings remain outside the normal deck in the target design. The current playtest path keeps them selectable until that profile is wired, to avoid stranding existing projects.

### 3.10 Derived feature indexes

Feature indexes are materialized views over authoritative state:

- `ActorTraitIndex`: authored actor/trait rows.
- `ItemIndex`: item ownership/quantity.
- `ChoiceHistoryIndex`: canonical `(node_guid << 16) | choice_index` keys.
- `WorldStageIndex`: global stage tags.
- `ChoiceHistoryIndex` keys are `u64`; scope keys are `u64`.

`rebuild_from_state`-style rebuild projects item edges and world stages. Actor traits and choices require explicit authored bindings; the engine must not infer them from macro passions or output-pin ordinals.

Indexes are rebuildable from `WorldStateTable` and the resolved call-history path. Feature predicate compilation and transactional updates remain the next integration step.

## 4. Implementation status

| Contract | Status | Evidence |
|---|---|---|
| Authoring graph | Wired | Rust/TS validators, adapter compiler, service save/read |
| Catalog / dense IDs | Wired | `chrono_catalog`, adapter, `UnifiedDispatcher` index |
| Completion bitset | Wired | `FlowGraphInstance.completed_storylets` |
| Call-history rebuild | Wired | `PersistentHistoryEngine::call_history`, bitset rebuild |
| Trigger table | Contract + tests | CSR, stable keys, cursor `process_tick` |
| Transactions | Contract + tests | write sets, conflict detection, single-op commit/revert |
| Selection profile | Contract + tests | profile, policy, fallback, floor |
| Selection gates | Contract + tests | typed gates and boundary tests |
| Ending marker | Wired | derived `ending`, deck filters, jump precedence |
| Catalog working set | Partial | dense lookup wired; lazy payload and masks deferred |
| Feature indexes | Partial | rebuild + explicit bindings; predicates/transactions deferred |
| Bundle validation | Wired | graph validation and ending derivation on import/validate |

## 5. Known limitations and deferrals

These are implementation limits, not hidden behavior.

### 5.1 Runtime adoption

- The v6 scheduler does not yet allocate from `StoryletInstanceTable`/`ScopeTable`.
- Per-node completion sets remain `BTreeSet`s; instance eligibility uses the bitset.
- Rollback still updates bits from node-set membership and does not always rebuild solely from the remaining commit path.
- QBN prerequisite chains still consult node-local completion sets.

### 5.2 Persistence

- SQLite still stores the v5 history shape; stable-key save/load migration, compaction, and rebase are not complete.
- Trigger fired state uses dense subscription indices; stable trigger-key resolution is not wired into load.
- `PersistedCallEntry` is a contract; service persistence migration remains.

### 5.3 Triggers

- Event crossing predicates and full typed event payload production are not wired.
- Runtime subscriptions are not yet replaced by `TriggerTable`.
- `process_tick` is tested as a contract, not used by the live scheduler.

### 5.4 Transactions

- No open-transaction table or conflict-policy execution in the service.
- No transitive write-set closure from all indirect effects.
- Multi-operation effect bodies are not yet one transaction.

### 5.5 Selection and endings

- Runtime selection still uses the v5 deck path.
- Ending profile is not wired into playtest; endings remain selectable there.
- Selection records/replay and distribution tests are incomplete.
- Gate failure returns typed rejection; fallback is only for empty eligible sets.

### 5.6 Catalog and features

- Catalog metadata is partial; lazy payload loading and location/role/gate masks are not wired.
- Runtime still compiles all storylet definitions.
- Feature indexes are not yet used by predicate compilation or transactional updates.
- Choice identity is canonical `u64`, but choice/trait rows require authored bindings.

### 5.7 Authoring/import

- No-choice ending storylets are accepted; the old project rule was removed.
- Bundle import/validation now validates graphs and derives `ending`.
- Content list/get preserve unknown fields and normalize only `ending`.

## 6. Validation

Current checks:

```sh
cargo test --manifest-path crates/storylet-engine/Cargo.toml --all-targets
cargo test --manifest-path crates/storylet-engine-service/Cargo.toml
bun test
bun run typecheck
bun run build
```

Latest recorded result:

- engine: 144 passed
- service: 68 passed
- Bun: 49 passed
- typecheck/build: pass

Known flake: full `bun test` can race parallel service startup; the isolated playtest HTTP test passes.

## 7. Code map

| Area | Files |
|---|---|
| World state / deltas | `crates/storylet-engine/src/chrono_state.rs` |
| Dispatcher / eligibility | `crates/storylet-engine/src/chrono_dispatch.rs` |
| Flow graph / history | `crates/storylet-engine/src/chrono_runtime.rs` |
| Catalog / IDs | `crates/storylet-engine/src/chrono_catalog.rs` |
| Completion bitset | `crates/storylet-engine/src/chrono_completion.rs` |
| Dense tables | `crates/storylet-engine/src/chrono_tables.rs` |
| Triggers | `crates/storylet-engine/src/chrono_triggers.rs` |
| Transactions | `crates/storylet-engine/src/chrono_transactions.rs` |
| Selection/endings/gates | `crates/storylet-engine/src/chrono_selection.rs` |
| Feature indexes | `crates/storylet-engine/src/chrono_features.rs` |
| Service adapter | `crates/storylet-engine-service/src/chrono_adapter.rs` |
| Graph compiler | `crates/storylet-engine-service/src/chrono_graph_adapter.rs` |
| Graph model | `crates/storylet-engine-service/src/narrative_graph.rs` |
| Studio graph | `src/web/studio/narrativeGraphTypes.ts` |

## 8. Migration note

The former `docs/featuresbacklog/chrono-decision-engine/` tree contained v3/v4/v5 specifications, the dense-runtime REQ-DO backlog, validation log, and implementation limitations. This document is the condensed durable record. Historical detail is intentionally not duplicated; the code and tests are the executable source of truth.
