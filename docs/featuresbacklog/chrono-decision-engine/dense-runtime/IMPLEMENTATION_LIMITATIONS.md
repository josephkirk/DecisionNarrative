# Dense Runtime Implementation Limits

**Status:** Implementation companion to REQ-DO-01 … REQ-DO-12. The contracts below are implemented as engine modules with tests. This file records which paths are wired into the current runtime and which are deliberately deferred because they require the v6 scheduler or service persistence migration.

## Wired now

| Contract | Implemented | Integrated path |
|---|---|---|
| REQ-DO-02 | `chrono_catalog::StoryletSpace` / `StoryletCatalog` | `compile_chrono_dispatch` builds a frozen catalog; adapter key/index access delegates to it |
| REQ-DO-03 | `chrono_completion::StoryletBitset` | `FlowGraphInstance.completed_storylets` is now a bitset |
| REQ-DO-10 | dense ID lookup + partial catalog | `UnifiedDispatcher::storylet` uses a rebuilt dense index; catalog replaces old storylet-id copies; working-set metadata is partial |
| REQ-DO-12 | graph rules | service/studio validators and compiler already use the authored graph |

## Contract-only / deferred

These are not claimed as end-to-end runtime behavior. They compile and have focused tests, but the v6 scheduler, service save/load migration, or UI exposure is out of scope for this change.

| Contract | Implemented module | Deferred integration |
|---|---|---|
| REQ-DO-01 | `chrono_tables::{StoryletInstanceTable, ScopeTable}` | scheduler does not yet allocate from these tables; per-node `completed_storylets` remains a `BTreeSet`; arena columns deferred |
| REQ-DO-04 | `PersistedCallEntry`; runtime `CallHistoryEntry` + `PersistentHistoryEngine` rebuild; playtest binds catalog keys | SQLite persistence/compaction/rebase migration remains deferred; trigger fired state uses subscription indices |
| REQ-DO-05 | `chrono_triggers::{TriggerTable, EventQueue, TriggerFireTable}` | CSR/stable keys/process_tick implemented; runtime subscriptions not replaced and fired state not rebuilt at load |
| REQ-DO-07 | `chrono_transactions::{EffectTransaction, WriteSet}` | existing `MutationOp` path still commits directly; conflict policy not enabled in service |
| REQ-DO-06/08/09 | `chrono_selection::{SelectionProfile, SelectionGate, FallbackPolicy, weighted_pick_one}`; `StoryletDef.is_ending` and default deck exclusion | ending profile is not yet wired into playtest selection; service still uses its existing deck path; selection record/replay and distribution tests incomplete |
| REQ-DO-11 | `chrono_features::{FeatureIndex, ActorTraitIndex, ItemIndex, ChoiceHistoryIndex, WorldStageIndex}` | state/call-history rebuild implemented; predicate and transactional wiring deferred |

## Deliberate non-goals

- Replacing the v5 `FlowGraphInstance` scheduler in one change.
- Migrating existing SQLite saves to stable-key call history in one change.
- Exposing new selection-profile/gate fields in the Vue UI.
- Adding a second tag/metric/passion store.

These deferrals are recorded rather than masked: each deferred path has a named module and a migration seam.

## Remaining contract gaps (explicit)

These are known gaps in the current implementation. They are recorded so no review or report treats the contract as end-to-end complete.

- **REQ-DO-01:** `StoryletInstanceTable` does not yet store variable/cast arenas or interleave policy. `ScopeTable` release/reuse is implemented, but the scheduler still uses `FlowGraphInstance`.
- **REQ-DO-03:** `FlowGraphInstance.completed_storylets` is a bitset, but per-node completion sets remain `BTreeSet`s. Rollback still clears bits from node-set membership; it does not yet rebuild solely from the remaining commit path. QBN prerequisite chains still consult node sets.
- **REQ-DO-04:** `CallHistoryEntry` is the canonical runtime record. The playtest loader binds dense storylet indices to stable keys, and completion/trigger rebuilds fold the commit path. `PersistedCallEntry` remains a persistence contract; SQLite still stores the old shape and trigger fired state uses dense subscription indices.
- **REQ-DO-05:** `TriggerTable` builds CSR buckets, supports stable subscription keys, and has deterministic `process_tick` with once/scope/active/cap handling and queued-event retention at cap. Event crossing predicates, full event payload production, and scheduler wiring remain deferred.
- **REQ-DO-07:** `WriteSet` conflict bitsets and single-op commit/revert exist, but there is no open-transaction table, conflict-policy resolution, write-set closure from indirect effects, or multi-op transaction body.
- **REQ-DO-08/09:** Selection profile, fallback, floor, gate validation, and weighted pick exist. Runtime selection still uses the v5 deck path; selection records/replay and distribution tests are incomplete. Gate failure returns a typed rejection rather than applying fallback; fallback is reserved for empty eligible sets.
- **REQ-DO-10:** Catalog/working-set metadata is partial (`is_ending`, payload ref, duration); location/role/gate masks and lazy payload loading are not wired. Runtime still compiles all storylet definitions. Dense lookup is sized to the maximum ID and rebuilt on deserialize.
- **REQ-DO-11:** Feature indexes have a `rebuild_from_state` projection over item edges, macro passions, choice pins, and global-stage tags. Predicate compilation and transactional updates remain deferred. Runtime call-history entries currently carry `scope_index: 0`, and choice pins use per-node ordinals, so cross-scope choice keys remain a deferred schema issue.
- **REQ-DO-06:** `StoryletDef.is_ending` is wired. Default deck exclusion is deferred with the ending-profile wiring; playtest currently keeps ending rows selectable. A terminal End with a jump target is treated as a jump (jump wins), and the studio clears terminal when a jump is chosen.

- **REQ-DO-12:** the legacy `ending_without_choice` rule was removed, content reads normalize storylet `ending`, and bundle import/validation now validates graphs and derives `ending`.

## Final integration notes

- `FlowGraphInstance.completed_storylets` is a `StoryletBitset`; `rebuild_completed_bitset` and `verify_completed_bitset` exist. Per-node completion sets and QBN prerequisite chains are still `BTreeSet`-based, so non-head rollback and cross-node prerequisite checks are not fully converged.
- `CallHistoryEntry` now carries `completed`, stable key hooks, trigger, transaction, revision, and tick fields. Runtime still emits no stable storylet key, and `effect_transaction` is not yet populated.
- Ending marker propagation is wired. Normal-deck exclusion is **not** enabled in the current playtest path: endings remain selectable so existing projects are not stranded. The ending profile/gate path is a contract module and is not yet wired; this is an intentional compatibility bridge, not a claim that default exclusion is active.
- `StoryletCatalog` working-set metadata is partial; payload/lazy-load and location/role/gate masks are not wired.
- Feature indexes have dense storage and rebuild parity via tuple inputs, but no `WorldStateTable`/call-history derivation yet.
- Bundle validation/import still bypasses per-storylet graph validation; legacy project validation no longer rejects choice-less endings.

## Persistence / migration notes

- `StoryletBitset` now serializes as a sorted list of set indices, matching the old `BTreeSet<u16>` JSON shape; legacy arrays remain readable.
- The dense dispatcher index falls back to a scan when `storylets.len()` changes. In-place replacement of an existing `storylet_id` with the same row count is not detected; callers must call `rebuild_index()` after mutating IDs.
- Bundle import/validation now parses each storylet graph, runs graph validation, and derives `ending`. Content list/get normalizes `ending` in place without typed re-serialization.

- The unfiltered `query_qbn_deck` / `try_query_qbn_deck` / `try_query_qbn_deck_bitset` APIs still include `is_ending` rows. The filtered `try_query_qbn_deck_bitset_with_ending(..., false)` variant exists, but no runtime/service caller currently uses it; ending exclusion is deferred with the ending-profile wiring.
