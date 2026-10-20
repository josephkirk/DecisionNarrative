# REQ-DO-04 — Call History Is Canonical; Bitsets Are Rebuildable

**Status:** Proposed  
**Target:** ChronoFlow v6  
**DOD relationship:** Keeps v5 `REQ-CH-01` … `REQ-CH-06`; adds stable keys, dense runtime indices, and bitset rebuild verification.  
**Depends on:** `REQ-DO-02`, `REQ-DO-03`  
**Related:** `REQ-DO-01`, `REQ-DO-05`, `REQ-DO-07`

## 1. Problem

The v5 spec says call history defines the current story state and is the save/replay record. The runtime also stores derived state:

- completed storylets
- edge-use counts
- visited locations
- threshold-active flags
- item/visibility state

If derived state is persisted as authoritative, rollback and content patches drift. If call history is queried for every eligibility check, the hot path becomes O(history).

The correct split is one `CommitRecord`:

```text
CommitRecord = effect transaction delta + CallEntry + post-commit events
```



- **Call history is canonical.**
- **Dense bitsets and tables are materialized indexes.**
- **Every materialized index is rebuildable and verifiable.**

## 2. Requirement

**REQ-DO-04:** Call history SHALL remain the canonical replay record; the completed-storylet bitset SHALL be rebuildable and verifiable.

Specific rules:

1. Every committed storylet completion, node completion, choice, and effect transaction produces a history entry.
2. Persisted history stores stable keys, not dense indices.
3. Runtime history entries carry dense indices for the current content revision.
4. Per-scope completion bitsets are rebuilt from history on load/reconcile.
5. Rebuild MUST be deterministic and idempotent.
6. A debug verification pass MUST compare rebuild output with live derived state.
7. Rollback updates both history and derived state atomically at a commit boundary.
8. No eligibility check scans history. History scans occur only at load, reconcile, debug verification, and save compaction.

## 3. History table layout

Use one flat arena and per-scope index ranges where possible.

```text
CallHistoryTable
  entries: Vec<CallEntry>            // append-only within a branch
  commit_head: Vec<u32>              // per scope
  commit_parent: Vec<u32>            // tree/stack parent
  commit_scope: Vec<u16>
  commit_generation: Vec<u32>
  // Per-scope range metadata lives in ScopeTable.history_start/history_len (REQ-DO-01).
```

`CallEntry` extends the existing `CallHistoryEntry` with scope, instance, stable key, completion, and transaction references:

```text
CallEntry
  scope_index: u16
  instance_index: u32
  node_guid: u32
  executed_input_pin: u16
  chosen_output_pin: u16
  stable_storylet_key: Option<StableStoryletKey>   // persisted
  storylet_index: Option<u16>                     // runtime, resolved
  completed: bool
  trigger_subscription: Option<u32>   // set when this start was trigger-caused
  effect_transaction: Option<u32>
  tick: u32
  generation: u32
```

The persisted form omits `storylet_index` and any other runtime index:

```text
PersistedCallEntry
  scope_key: StableScopeKey
  instance_key: StableInstanceKey
  node_guid: u32
  executed_input_pin: string
  chosen_output_pin: string
  chosen_storylet_key: Option<StableStoryletKey>
  trigger_key: Option<StableTriggerKey>
  content_revision: u64
  tick: u32
```

## 4. Write path

`REQ-DO-07` owns transaction planning, conflict detection, apply, and revert. This requirement owns the history/derived-state append after a successful commit:

```text
after_commit(scope, instance, storylet_index, tx_index):
    entry = CallEntry {
        scope, instance, storylet_index,
        completed: true,
        effect_transaction: Some(tx_index),
        ...
    }
    parent = commit_head[scope]
    index = history.entries.push(entry)
    history.commit_parent.push(parent)
    history.commit_scope.push(scope)
    commit_head[scope] = index
    if entry.completed:
        scope.completed[scope].set(storylet_index)
    emit StoryletCompleted(scope, storylet_index)
```

The world mutation, history entry, completion bit, and trigger event share one logical commit. If the world mutation fails, none of the other changes commit. Do not duplicate a second commit protocol here.

## 5. Rebuild

```text
fn rebuild_derived(scope):
    bits.clear()
    history = entries_on_path(commit_head[scope])
    for entry in history:
        if entry.completed and entry.storylet_index is Some(index):
            bits.set(index)
    return bits
```

Rebuild is not a general replay of effects. Effects are already in `WorldStateTable` when loading a snapshot; rebuild only reconstructs indexes.

For load from call history alone:

1. Resolve stable keys to runtime indices.
2. Replay entries in forward order through `ForceComplete`/effect application.
3. Rebuild bitsets as each completion commits.
4. Verify the final world hash against the saved hash if present.
5. On mismatch, fail load; do not silently continue.

## 6. Verification

Debug and CI builds run:

```text
fn verify_derived(scope):
    rebuilt = rebuild_derived(scope)
    assert rebuilt == scope.completed
    assert all bits map to a live storylet
    assert every entry.storylet_index resolves back to its stable key
```

Production builds MAY sample verification at load and at explicit checkpoints. They MUST run full verification after content patch rebase and after timeline reconciliation.

## 7. Reconciliation

When reconciling from current history to a target history:

1. Find the LCA commit index.
2. Cancel entries above the LCA in reverse order, including:
   - world deltas
   - completion bits
   - item/visibility changes
   - trigger generation counters
3. Force-complete entries from the LCA to the target head.
4. Rebuild per-scope bitsets from the target path.
5. Verify.

Bitset rollback can be incremental (`clear` on cancel, `set` on force-complete) or a full rebuild. Incremental is preferred in production; full rebuild is required in verification.

## 8. Content patch rebase

Call history uses stable keys. On load against a new content revision:

- Removed storylet key: apply the existing `REQ-GP` rebase policy; remove or splice the entry.
- Renamed key: explicit migration mapping; do not guess.
- Reordered dense rows: no history change; indices are re-resolved.
- Inserted prerequisite storylet: splice a synthetic entry only if the content patch declares the mapping.
- Ambiguous key: fail load.

After rebase, rebuild all scope bitsets and run verification.

## 9. Save compaction

History grows monotonically. Add an explicit compaction boundary:

- Snapshot the world hash and all derived bitsets.
- Keep the commit path required for active rollback branches.
- Drop entries that are ancestors of a declared checkpoint only if the checkpoint stores enough state to rebuild or resume.
- Persist the compaction watermark and content revision.
- Never drop the only record of a completion that a live scope bitset depends on without storing the bitset as a checkpoint.

## 10. Invariants

1. `scope.completed` equals `rebuild_derived(scope)` at every checkpoint.
2. History stores stable keys at rest; runtime indices are per-load.
3. A call entry references an existing transaction or explicitly has none.
4. Commit parent pointers form a tree with no cycles.
5. LCA is computed over commit indices, not just array positions, when branches exist.
6. A rollback never leaves a completion bit set without a corresponding history path, or vice versa.
7. Verification is deterministic and does not depend on hash-map order.

## 11. Acceptance tests

- `history_rebuild_bitset_exact`: 50,000 completions across 8 scopes; rebuilt bitsets equal live bitsets.
- `history_stable_keys_roundtrip`: save/load with reordered dense rows preserves completion identity.
- `history_patch_removed_storylet`: removed key follows rebase policy; no stale bit remains.
- `history_branch_reconcile`: current and target branches share LCA; bitsets after reconcile match target path.
- `history_rollback_atomic`: failed effect leaves no history entry, no bit, no trigger.
- `history_duplicate_commit_rejected`: replaying the same commit key twice is idempotent or rejected.
- `history_no_runtime_string_lookup`: after load, no stable-key lookup occurs during ticks.
- `history_world_hash_after_replay`: call-history replay reaches the saved world hash.

## 12. Migration

1. Keep the existing `HistoryCommit` structure (`chrono_runtime.rs` path) until the flat table is implemented.
2. Add stable keys to persisted entries behind a schema version.
3. Add a rebuild function that accepts both current and target histories.
4. Switch verification on in CI.
5. Switch completion bitsets to the rebuilt result.
6. Remove the old `BTreeSet` completion state after parity.

## 13. Risks

- A flat arena with per-scope ranges is efficient only if scope histories remain contiguous. Interleaved commits may require per-scope index lists. Measure before fixing the layout; a `Vec<u32>` per scope is acceptable only if scope count is bounded and allocation is outside the hot tick.
- Call-history replay can be expensive. Do not replay on every load if a verified snapshot exists; use the snapshot and verify sampled indexes.
- Compaction can break rollback branches. Keep branch heads explicit.
- Stable keys add persistence size. This is the correct tradeoff for patch safety.
- Reconciliation and completion bitsets must share a commit boundary; otherwise a crash can leave an inconsistent save.

## 14. References

- `crates/storylet-engine/src/chrono_runtime.rs:360` — `CallHistoryEntry`.
- `crates/storylet-engine/src/chrono_runtime.rs:1712` — `FlowGraphInstance`.
- `crates/storylet-engine/src/chrono_runtime.rs:1737` — current completion `BTreeSet`.
- `chronoflow_narrative_engine_technical_specification_v5.md` `REQ-CH-01` … `REQ-CH-06`.
- `REQ-DO-02` — stable keys and dense indices.
- `REQ-DO-03` — per-scope bitset.
