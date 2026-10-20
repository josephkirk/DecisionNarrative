# REQ-DO-07 — Effect Transactions and Write Sets

**Status:** Proposed  
**Target:** ChronoFlow v6  
**DOD relationship:** Wraps the existing `MutationOp`/`BijectiveDelta`; it does not replace the effect or revert implementation.  
**Depends on:** `REQ-DO-01`, `REQ-DO-02`  
**Related:** `REQ-DO-04`, `REQ-DO-05`

## 1. Problem

Two concurrent storylets can mutate the same state:

- both change `guilt`;
- both add/remove the same tag;
- both grant or consume the same item;
- one moves an actor while another binds that actor;
- one completes a storylet while another checks a prerequisite.

`SceneComposer` arbitrates animation commands on one actor. It does not arbitrate narrative effects. A committed transaction plus its `CallEntry` is the `CommitRecord` defined in `REQ-DO-04`. Without an explicit conflict contract, the result depends on execution order and rollback can no longer explain which storylet won.

The existing `MutationOp` and `BijectiveDelta` (`crates/storylet-engine/src/chrono_state.rs:1575`, `1609`) already provide atomic apply/revert. They need a declared write set and a conflict policy.

## 2. Requirement

**REQ-DO-07:** Effect transactions SHALL declare a write set for concurrent conflict detection.

Specific rules:

1. An effect transaction declares every tag, metric, entity, item, scope, and reservation it may write.
2. The write set is available before the transaction commits.
3. Two transactions conflict when their write sets intersect.
4. Conflict policy is authored or configured per scope; it is deterministic.
5. Apply is atomic: either all effects and the history entry commit, or none.
6. Revert uses the existing `BijectiveDelta` and restores exact prior values.
7. Event emission happens after commit, never during partial application.
8. No per-transaction `HashSet`/`BTreeMap` allocation in the hot path.

## 3. Write-set layout

```text
WriteSet
  tags:          TagBitset          // global tags
  metrics:       MetricBitset       // dense metric indices
  entities:      EntityBitset       // dense entity indices
  items:         ItemBitset         // dense item indices
  actor_traits:  ActorTraitBitset   // derived actor-trait rows (REQ-DO-11)
  choices:       ChoiceBitset       // choice-history rows (REQ-DO-11)
  scopes:        ScopeBitset        // dense scope indices
  reservations:  ReservationBitset  // cast/role reservations
```

`TagBitset` is the existing inline bitset. `MetricBitset`, `EntityBitset`, `ItemBitset`, `ScopeBitset`, and `ReservationBitset` are fixed inline bitsets sized by their own capacities. Do not use a `Vec` per transaction for common cases.

For rare sparse sets, a bounded `SmallVec`-style arena entry is allowed, but the default row is a fixed bitset.

## 4. Transaction row

```text
EffectTransaction
  state:             u8             // Open|Committed|Aborted
  scope_index:       u16
  instance_index:    u32
  tag_add:           TagBitset
  tag_remove:        TagBitset
  metric_base:       u32
  metric_len:        u16
  entity_base:       u32
  entity_len:        u16
  item_base:         u32
  item_len:          u16
  write_set:         WriteSet
  delta:             Option<BijectiveDelta>
  sequence:          u32
```

`MetricOpArena`, `EntityOpArena`, and `ItemOpArena` hold the actual operations. `write_set` is the conflict index, not a duplicate of the operations.

## 4.1 Write-set completeness: transitive closure

A write set declared only from the explicit `MutationOp` is incomplete. The engine has indirect writes:

- zero-sum passion propagation writes rival micro-passions;
- macro-passion recalculation writes macro rows;
- metric threshold rules can set or clear tag bits;
- passion threshold rules can set or clear tag bits;
- clamping can change a metric even when the requested delta is small;
- `move_player` changes the current-location mask and visited membership;
- `visit_location` and `meet_character` change membership masks;
- graph transforms change vertices, edges, and attributes;
- item/cast operations change ownership/reservation rows.

Therefore:

```text
WriteSet = closure(
  explicit_op,
  zero_sum_links,
  macro_rows,
  metric_threshold_tags,
  passion_threshold_tags,
  location_masks,
  membership_masks,
  graph_transform_targets,
  item_ops,
  cast_ops,
  feature_index_targets
)
```

### Plan-time closure

Compute the closure before apply:

1. Start with explicit tag add/remove, metric deltas, entity ops, item ops, and movement.
2. Expand each changed micro-passion through the CSR zero-sum rows.
3. Add every affected macro-passion row.
4. Add tag bits owned by metric and passion threshold rules for affected rows.
5. Add the current-location mask and visited/met membership masks for movement.
6. Add graph transform vertex/edge/attribute targets.
7. Add cast/role reservations.
8. Add every affected `ActorTraitIndex` and `ChoiceHistoryIndex` row (`REQ-DO-11`).

If the engine cannot compute a target before apply (for example an opaque graph transform), the transaction SHALL NOT claim `MergeAdditive` or `RejectLater`; it SHALL be `Serialize` or exclusive.

### Delta audit

After apply, `BijectiveDelta` is the audit record. In debug builds:

```text
assert every changed field in delta is represented in write_set
```

If the delta contains an unlisted field, the transaction is a write-set defect. Production MAY derive the write set from the delta only for transactions that are already serialized; using that derived set to admit parallel execution is too late.

### Validation

The real engine probe applied a passion mutation that explicitly named micro-passion `1` with a zero-sum link to NPC 1. The resulting `PassionDelta.macro_undos` contained NPC `0` and NPC `1`. A write set of `{micro 1}` would miss NPC 1 and fail to conflict with a concurrent op that writes NPC 1.

## 5. Conflict detection

```text
fn conflicts(a, b) -> bool:
    return a.tags.intersects(&b.tags)
        || a.metrics.intersects(&b.metrics)
        || a.entities.intersects(&b.entities)
        || a.items.intersects(&b.items)
        || a.scopes.intersects(&b.scopes)
        || a.reservations.intersects(&b.reservations)
```

For `MAX_TAGS = 256` and `MAX_METRICS = 512`, a full conflict test is a small fixed number of word operations. For entities/items/scopes, the same.

Do not compare operation lists field by field in the hot path. Build the write sets when the transaction opens.

## 6. Conflict policies

```rust
pub enum ConflictPolicy {
    RejectLater,        // first committed wins; later transaction aborts
    PriorityWins,       // higher storylet priority wins; tie by instance index
    MergeAdditive,      // metric deltas sum; tag add/remove merge
    Serialize,          // queue later transaction until earlier commits
}
```

Policy requirements:

- **RejectLater** is the default for non-repeatable storylets and item transfers.
- **PriorityWins** is for authored cinematic events that must preempt.
- **MergeAdditive** is allowed only when every conflicting metric operation is additive and every tag operation is compatible. It is not allowed for item quantities, ownership, or reservations.
- **Serialize** is the safe default for effects that are not commutative but must both eventually run.

The policy is resolved by `(scope, storylet priority, explicit author setting)`. It must not depend on hash-map iteration.

## 7. Commit protocol

```text
fn commit(tx):
    require tx.state == Open
    for other in open_transactions_in_scope:
        if conflicts(tx.write_set, other.write_set):
            resolve(tx, other, policy)
            if tx aborts: return Aborted
    planned = plan_mutation(tx)
    delta = apply_mutation_with_registry(state, planned, tags)
    history_entry = append_history(tx, delta)
    tx.delta = Some(delta)
    tx.state = Committed
    emit_events(tx)
    return Committed
```

Rules:

1. Plan all mutations before applying any.
2. Apply through `apply_mutation_with_registry` so the delta is exact.
3. Append history before emitting events.
4. Emit events after the commit boundary.
5. On any error, apply nothing and leave the transaction `Open` or `Aborted`.

## 8. Revert protocol

```text
fn revert(tx):
    require tx.state == Committed
    delta = tx.delta.take().ok_or(MissingDelta)?
    revert_delta(state, &delta)
    remove_history_entry(tx)
    emit_inverse_events(tx)
    tx.state = Aborted
```

Order matters:

- Revert world state first.
- Then remove the history entry.
- Then emit inverse events.
- Do not reverse a transaction whose delta is missing.

For a multi-effect storylet, revert only the transaction owned by that storylet instance. This is why `instance_index` must be in the call history.

## 9. Items and reservations

Item operations require stricter conflict rules:

- `Give`, `Take`, `Spawn`, `Despawn` write the item index and owner/scope.
- Two transactions cannot both consume the same item quantity.
- `Show`/`Hide` write presentation state, not ownership.
- A cast reservation writes the actor index and role index.

Write sets for items must include the item index, even if the operation is additive.

## 10. Invariants

1. A committed transaction has a `BijectiveDelta`.
2. Reverting a committed transaction restores the exact prior state hash.
3. A transaction never applies a partial effect.
4. A conflict is detected from write sets before apply.
5. Event emission occurs only after a successful commit.
6. A transaction's write set is a superset of the fields it actually changes.
7. Two transactions with disjoint write sets may commit in either order and produce the same state hash for additive operations.
8. Item ownership and cast reservations are never merged additively.

## 11. Acceptance tests

- `write_set_overlap_tag`: two transactions write the same tag; `RejectLater` aborts the later one.
- `write_set_overlap_metric_additive`: two additive metric deltas merge exactly; final value equals serial application.
- `write_set_overlap_item`: two transactions consume the same item; only one commits.
- `write_set_zero_sum_conflict`: explicit ops target different micro-passions linked zero-sum; the transitive write sets overlap and the conflict is detected.
- `write_set_threshold_tag_conflict`: a metric/passion threshold flips a tag; a concurrent op writing that tag conflicts.
- `write_set_move_player_masks`: player movement includes current-location and visited masks.
- `write_set_disjoint`: disjoint transactions commit and rollback independently.
- `write_set_priority`: higher priority wins deterministically; equal priority uses instance index.
- `write_set_serialize`: queued transaction applies after the first commit without losing effects.
- `write_set_rollback_exact`: commit/revert 10,000 transactions; state hash returns to baseline.
- `write_set_history_pairing`: every committed transaction has one history entry and one delta.
- `write_set_no_allocation`: after warm-up, 100,000 conflict checks allocate zero times.
- `write_set_item_visibility`: Show/Hide does not change ownership; Give/Take does.

## 12. Integration

- `MutationOp` remains the low-level effect payload.
- `BijectiveDelta` remains the exact inverse.
- `EffectTransaction` is the concurrency envelope around `MutationOp`.
- `CallHistoryEntry` references the transaction index and delta.
- `TriggerTable` consumes post-commit events.
- `EffectTransactionTable` is keyed by scope; conflict policy is project/profile configuration, not a `ScopeTable` field.

## 13. Migration

1. Build write sets alongside existing `MutationOp` construction.
2. Add a debug assertion that applying a transaction changes only fields in its write set.
3. Enable conflict detection for item and cast operations first.
4. Enable tag/metric conflicts for concurrent storylets.
5. Add `Serialize` for content that cannot merge.
6. Remove the single-writer assumption only after all tests pass.

## 14. Risks

- A write set that omits an indirect write (for example, a threshold tag derived from a metric) will miss conflicts. Include derived threshold bits and event keys.
- `MergeAdditive` is unsafe for clamping metrics unless the merge is applied in a deterministic order. If clamping is involved, use `Serialize`.
- Write sets can be conservative and cause false conflicts. Measure false-positive rates before tightening.
- Transaction rows must not be reused while a call-history delta references them.
- Item ownership conflicts cannot be solved by last-writer-wins; use reject or serialize.

## 15. References

- `crates/storylet-engine/src/chrono_state.rs:1575` — `MutationOp`.
- `crates/storylet-engine/src/chrono_state.rs:1609` — `BijectiveDelta`.
- `crates/storylet-engine/src/chrono_state.rs:1669` — apply mutation.
- `crates/storylet-engine/src/chrono_state.rs:1772` — revert delta.
- `crates/storylet-engine/src/chrono_runtime.rs:360` — call history.
- `REQ-DO-04` — commit/replay.
- `REQ-DO-05` — post-commit events.
