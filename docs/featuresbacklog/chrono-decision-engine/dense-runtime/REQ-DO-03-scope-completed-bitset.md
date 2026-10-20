# REQ-DO-03 — Per-Scope Completed-Storylet Bitset

**Status:** Proposed  
**Target:** ChronoFlow v6  
**DOD relationship:** **Supersedes** the v4 §5.1 completion-tag rule. The v5 `BTreeSet<u16>` completion state is implementation drift; see the supersede note below.  
**Depends on:** `REQ-DO-01`, `REQ-DO-02`, `REQ-DO-04`  
**Related:** `REQ-DO-05`, `REQ-DO-07`

## 1. Problem

QBN eligibility currently clones or reads a `BTreeSet<u16>` of completed storylets:

- `FlowGraphInstance.completed_storylets: BTreeSet<u16>` (`crates/storylet-engine/src/chrono_runtime.rs:1737`)
- per-node completion sets (`chrono_runtime.rs:464`)
- `try_query_qbn_deck(..., &completed)` uses set membership.

A tree lookup per candidate per tick is unnecessary. Completion is a boolean set. It should be a bitset.

A single global bitset is also insufficient. Concurrent mission, conversation, and storylet scopes must not share completion state. The stress scenario needed a first-class way to say “this storylet is complete in this scope, not everywhere.”

## 2. Requirement

**REQ-DO-03:** Completed-storylet state SHALL be a per-scope bitset derived from call history.

Specific rules:

1. Each dense scope owns one fixed-size `StoryletBitset`.
2. The bitset is indexed by dense `StoryletIndex`.
3. Completion checks use `ContainsAll`/`Contains` on the bitset.
4. The bitset is materialized state. Call history remains canonical (see `REQ-DO-04`).
5. A bitset is rebuilt from history on load, replay, and reconciliation.
6. Completion SHALL NOT be represented only by global tags such as `done.<id>`.
7. Scope inheritance SHALL be explicit and directional.

## 2.1 Relationship to v4 completion tags

This requirement **supersedes** the v4 §5.1 design that represented non-repeatable storylet completion as `Storylet.Completed.<Name>` tag bits.

Current implementation drift:

- `FlowGraphInstance.completed_storylets` is a `BTreeSet<u16>` (`crates/storylet-engine/src/chrono_runtime.rs:1737`).
- Per-node `completed_storylets` sets are cloned into plans (`chrono_runtime.rs:2038`, `2506`).

`REQ-DO-03` replaces both with a per-scope `StoryletBitset`. If `done.*` tags remain for salience or UI projection, they MUST be derived from the bitset and SHALL NOT be the authoritative prerequisite check.

## 3. Bitset layout

`TagBitset` has 256 bits because it covers tags. Storylet count is a separate capacity.

```rust
pub const MAX_STORYLETS: usize = 65536;
pub const STORYLET_WORDS: usize = MAX_STORYLETS / 64;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct StoryletBitset {
    pub words: [u64; STORYLET_WORDS],
}

impl StoryletBitset {
    pub fn set(&mut self, index: StoryletIndex) -> Result<(), CapacityError> { ... }
    pub fn clear(&mut self, index: StoryletIndex) -> Result<(), CapacityError> { ... }
    pub fn contains(&self, index: StoryletIndex) -> bool { ... }
    pub fn contains_all(&self, mask: &StoryletBitset) -> bool { ... }
    pub fn is_empty(&self) -> bool { ... }
}
```

At the full 65,536-storylet space the bitset is 1024 words = **8 KB per scope**. Do not use `Vec<u64>` per scope; that is a heap allocation per scope.

The current workspace database contains **502 storylets**. `TagBitset` is 256 bits, so it cannot serve as the completion set. The shared `StoryletSpace` (`REQ-DO-02`) uses one capacity up to 65,536; 502 and the 20k stress target both fit. Load validation MUST compare the content storylet count with that same capacity.

`MAX_STORYLETS` is the single storylet-space capacity from `REQ-DO-02`; do not define a second ending capacity. Recompute `STORYLET_WORDS` from it and validate total scope-table memory.

## 4. Scope table

`ScopeTable` is owned by `REQ-DO-01`. Completion-specific fields are:

```text
ScopeTable (completion view)
  completed:  Vec<StoryletBitset>
  flags:      Vec<u8>   // InheritsCompletion | Isolated
```

Each scope has exactly one bitset row. There is no `BTreeMap<Scope, Bitset>`.

### Inheritance

Default: **no inheritance**. A mission or conversation scope has its own completion history.

If a scope row declares `InheritsCompletion`, lookup is:

```text
visible_completed(scope, storylet) =
    own.contains(storylet) || inherited.contains(storylet)
```

The inheritance chain SHALL be precomputed as a parent mask or resolved through a bounded parent array. It SHALL NOT walk a tree per eligibility check.

A world-scope bitset MAY be the common inherited root. Do not copy parent bits into every child; copy-on-read or a two-level check is cheaper and preserves rollback.

## 5. Completion update

On a completed storylet:

```text
fn mark_completed(scope, storylet_index):
    require storylet is bounded
    scope.completed[scope].set(storylet_index)
    emit StoryletCompleted(scope, storylet_index)
```

The event is consumed by `REQ-DO-05` to start dependent storylets.

On rollback:

```text
fn unmark_completed(scope, storylet_index):
    scope.completed[scope].clear(storylet_index)
    emit StoryletUncompleted(scope, storylet_index)
```

Rollback MUST remove the bit even if call history later rebuilds it. The bit and history must agree at every commit boundary.

## 6. Rebuild from call history

```text
fn rebuild_scope_bitset(scope, history):
    bits.clear()
    for entry in history entries for scope:
        if entry.completed and entry.storylet_index is valid:
            bits.set(entry.storylet_index)
    return bits
```

Rebuild rules:

1. Use the runtime index resolved from the stable key.
2. Skip entries removed by content-patch rebase.
3. Do not infer completion from tags, flags, or node state.
4. Rebuild both the scope bitset and any inherited view cache.
5. Verify against the previous bitset in debug builds; a mismatch is a rollback/replay defect.

## 7. Eligibility

A storylet prerequisite check is:

```text
eligible = scope.completed.contains_all(storylet.required_storylets)
        && global_tags.contains_all(storylet.required_tags)
        && !global_tags.intersects(storylet.forbidden_tags)
```

This is a fixed number of word operations:

- `MAX_STORYLETS = 65,536` → 1024 `u64` comparisons for a full `contains_all`.
- Typical required sets are sparse. A compiled `required_storylet_mask` can be checked with the same `contains_all`.

Do not loop over a list of prerequisite storylet IDs in the hot path.

## 8. Scope creation and destruction

- Creating a scope allocates a dense `ScopeIndex`, clears its bitset, and initializes history range.
- Releasing a scope returns the index to a free list; generation prevents stale handles.
- A scope with `InheritsCompletion` stores the parent index. Changing the parent requires a rebuild or explicit migration.
- Scopes are not garbage collected during a tick. Release occurs at a deterministic boundary.

## 9. Invariants

1. `completed[scope]` is a pure function of the resolved call history for that scope, plus live commits.
2. A storylet completion appears in exactly one owning scope unless inheritance is explicitly requested.
3. A bitset bit for an out-of-range storylet is a capacity error, not a silent ignore.
4. Rollback of a completion clears exactly one bit.
5. Rebuild is idempotent.
6. No hot path scans call history to answer a completion query.
7. A child scope with inheritance never mutates the parent bitset.

## 10. Acceptance tests

- `scope_bitset_rebuild_matches_history`: 10,000 mixed completions; rebuilt bitset equals live bitset.
- `scope_bitset_isolation`: two scopes complete different storylets; neither sees the other’s bits.
- `scope_bitset_inheritance`: child with `InheritsCompletion` sees parent bits without copying.
- `scope_bitset_rollback`: complete three storylets, roll back the middle commit, assert the exact bit clears and history/bitset agree.
- `scope_bitset_capacity`: an index `>= MAX_STORYLETS` fails with a typed error before indexing.
- `scope_bitset_no_history_scan`: eligibility instrumentation shows no call-history access for 1,000,000 checks.
- `scope_bitset_rebuild_after_reconcile`: reconcile to an older branch, rebuild, assert bitset equals branch history.
- `scope_bitset_tag_projection`: if `done.*` tags are retained for salience, they are projections of the bitset and do not gate prerequisites.

## 11. Migration from `BTreeSet`

1. Add `StoryletBitset` and `ScopeTable` beside `FlowGraphInstance.completed_storylets`.
2. Populate both from current completions.
3. Switch QBN and prerequisite checks to bitset.
4. Keep a debug assertion that `BTreeSet` and bitset agree for one release cycle.
5. Remove the `BTreeSet` fields after parity.
6. Keep `done.*` tags only if salience or UI needs them as qualities; otherwise remove them.

## 12. Risks

- 8 KB per scope at the full 65,536-storylet space can be significant with thousands of scopes. Measure scope counts; consider a paged/sparse completion structure if scopes are numerous.
- Inheritance can create surprising eligibility. Default to isolated scopes; opt in explicitly.
- A rollback that clears a bit but leaves the call-history commit is a bug. Commit and bitset update must be atomic at the save boundary.
- Do not use `TagBitset` for storylets. Reusing the tag capacity couples unrelated limits and can silently cap storylets at 256.

## 13. References

- `crates/storylet-engine/src/chrono_state.rs:36` — `TagBitset` precedent.
- `crates/storylet-engine/src/chrono_runtime.rs:464` — per-node completed set.
- `crates/storylet-engine/src/chrono_runtime.rs:1737` — instance completed set.
- `crates/storylet-engine/src/chrono_dispatch.rs:680` — eligibility.
- `REQ-DO-04` — call-history canonicalization.
- `REQ-DO-05` — completion-triggered storylets.
