# REQ-DO-01 — Dense Runtime Tables

**Status:** Proposed  
**Target:** ChronoFlow v6  
**DOD relationship:** Extends `REQ-DOD-01` … `REQ-DOD-05`; reuses the existing registries and does not create a second tag/metric store.  
**Depends on:** `REQ-DOD-01` … `REQ-DOD-05`, `REQ-DO-02`  
**Related:** `REQ-DO-03`, `REQ-DO-05`, `REQ-DO-06`, `REQ-DO-07`, `REQ-DO-08`, `REQ-DO-10`

## 1. Problem

The v5 runtime describes storylets, triggers, and profile pools as conceptual objects. A direct implementation tends toward:

```text
StoryletInstance { id, scope, state, local_vars: Vec<...>, cast: Vec<...>, ... }
TriggerSubscription { storylet, event, predicate, once, ... }
ProfilePool { storylet_index, weight, gate, ... }
```

That layout pays for a heap allocation and pointer chase per instance, trigger, and ending. It also makes rollback and deterministic iteration depend on container order.

The project mandate is data-oriented. The existing engine already uses:

- `TagBitset` as inline `[u64; 4]` (`crates/storylet-engine/src/chrono_state.rs:36`)
- `WorldStateTable` as a flat state table (`chrono_state.rs:931`)
- `SignalToken` as a packed fixed-size token (`crates/storylet-engine/src/chrono_runtime.rs:120`)
- `MutationOp`/`BijectiveDelta` as the effect/reversal contract (`chrono_state.rs:1575`, `1609`)

The new runtime layer must follow the same pattern.

## 2. Requirement

**REQ-DO-01:** Runtime storylet, scope, trigger, profile-pool, and transaction structures SHALL use dense indices and flat arrays.

Specific rules:

1. Each runtime row SHALL be addressable by a dense `u32` index.
2. Hot-path fields SHALL be stored in parallel `Vec<T>` arrays, not in per-row structs containing `Vec`, `Box`, `HashMap`, `BTreeMap`, or `String`. Row schemas owned by another REQ-DO are referenced, not duplicated here.
3. Variable-length data SHALL live in separate flat arenas referenced by `(base, len)` offsets.
4. Free slots SHALL be reused through a free-list; a generation counter SHALL detect stale handles.
5. Iteration order SHALL be explicit index order or a precomputed sorted index array. It SHALL NOT depend on hash-map or pointer order.
6. All capacities SHALL be validated at load. Overflow SHALL be a typed load error.

## 3. Tables

The names are normative; field order may change if the public contract and tests remain compatible.

### 3.1 Storylet instance table

```text
StoryletInstanceTable
  storylet_index:      Vec<u16>       // dense storylet index
  scope_index:         Vec<u16>       // dense scope index
  state:               Vec<u8>        // Pending|Active|Suspended|Completed|Aborted
  priority:            Vec<i16>
  interleave_policy:   Vec<u8>        // Serialize|Parallel|Preempt
  started_tick:        Vec<u32>
  finished_tick:       Vec<u32>
  local_var_base:      Vec<u32>       // into VariableSlotArena
  local_var_len:       Vec<u16>
  cast_base:           Vec<u32>       // into CastReservationArena
  cast_len:            Vec<u16>
  transaction_index:   Vec<u32>
  generation:          Vec<u16>
  free_head:           u32
  live_count:          u32
```

- `InstanceIndex` is the row index.
- A handle is `(index: u32, generation: u16)`. A stale generation is a typed error.
- `free_head` forms a singly linked free list through an `next_free: Vec<u32>` array.

### 3.2 Scope table

```text
ScopeTable
  parent:              Vec<u16>       // u16::MAX for root
  scope_kind:          Vec<u8>        // World|Mission|Storylet|Conversation
  flags:               Vec<u8>        // InheritsCompletion | Isolated
  completed:           Vec<StoryletBitset>
  history_start:       Vec<u32>       // into CallHistoryTable
  history_len:         Vec<u32>
  active_instance_base:Vec<u32>       // optional CSR view
  active_instance_len: Vec<u32>
  generation:          Vec<u16>
  free_head:           u32
```

`StoryletBitset` is defined in `REQ-DO-03`; it is not `TagBitset` because storylet count and tag count are different capacities.

### 3.3 Trigger table

Canonical schema: `REQ-DO-05`. The table is dense, bucket-keyed, and stores no per-subscription containers. `REQ-DO-01` owns only its allocation and capacity.

### 3.4 Profile pool view

A profile pool is a view over `StoryletIndex`; it is not a second storylet store. The pool schema, membership rules, gates, weights, and terminal behavior are owned by `REQ-DO-08` and `REQ-DO-09`. `REQ-DO-06` defines only the `ending` marker.

### 3.5 Effect transaction table

Canonical row/write-set schema: `REQ-DO-07`. `REQ-DO-01` owns only dense allocation, arenas, and free-list reuse.

## 4. Allocation and lifecycle

### 4.1 Acquire

```text
fn acquire_instance(table) -> Handle:
    if free_head != NONE:
        index = free_head
        free_head = next_free[index]
    else:
        index = rows.len()
        push_default_row()
    generation[index] += 1
    state[index] = Pending
    live_count += 1
    return Handle(index, generation[index])
```

### 4.2 Release

```text
fn release_instance(table, handle):
    require valid_generation(handle)
    state[handle.index] = Aborted
    next_free[handle.index] = free_head
    free_head = handle.index
    live_count -= 1
```

Release SHALL NOT clear the row immediately. Debug builds SHALL poison released fields; release builds MAY leave them until reuse. This avoids a full-row clear on every completion.

### 4.3 Iteration

- Active instances SHALL be iterated in ascending `InstanceIndex` unless the scheduler uses an explicit priority queue whose comparator includes `InstanceIndex`.
- Triggers in a bucket SHALL be sorted by `(priority, storylet_index, subscription_index)`.
- Profile pools SHALL be iterated through their profile order, not raw table order.

## 5. Integration

- `FlowGraphInstance.completed_storylets: BTreeSet<u16>` (`chrono_runtime.rs:1737`) SHALL be replaced by the per-scope bitset from `REQ-DO-03`.
- `WorldStateTable` remains the effect target. Tables do not duplicate tag/metric state.
- `MutationOp`/`BijectiveDelta` remain the effect/reversal primitive. `EffectTransactionTable` references them; it does not replace them.
- `CallHistoryEntry` (`chrono_runtime.rs:360`) gains `instance_index`, `scope_index`, and stable storylet keys per `REQ-DO-04`.
- Existing `SignalToken` stays fixed-size and token-driven. Instances are not tokens; a token may reference an instance through a side table, but the token itself SHALL NOT grow.

## 6. Invariants

1. Every live row index is within its table capacity.
2. A handle with the wrong generation is rejected.
3. `live_count` equals the number of rows not on the free list.
4. No hot-path operation allocates on the heap.
5. Every profile pool is a permutation of its eligible rows.
6. `bucket_entries` is sorted within each bucket.
7. Table growth is amortized and does not invalidate indices.
8. A transaction row is never reused while a call-history entry references its delta.

## 7. Acceptance tests

- `dense_instance_alloc_free_reuse`: allocate/free 10,000 instances; no stale handle accepted; index reuse preserves generation checks.
- `dense_table_capacity_errors`: capacities + 1 produce typed errors, never truncation.
- `dense_iteration_is_index_order`: shuffled insertion yields index-order iteration.
- `dense_trigger_bucket_sorted`: bucket order equals `(priority, storylet, subscription)`.
- `dense_profile_pool_order`: all enabled rows appear exactly once.
- `dense_hot_path_no_allocation`: a counting allocator observes zero allocations after warm-up for 100,000 eligibility/trigger/profile reads.
- `dense_transaction_arena_offsets`: overlapping arena ranges are rejected at load.

## 8. Migration

1. Introduce the tables behind a feature flag; keep existing `BTreeSet` paths until parity tests pass.
2. Replace completion checks first; this has the widest call graph.
3. Replace instance allocation after the scheduler exists.
4. Keep JSON authoring unchanged. Add a binary/runtime representation only after the table contracts are stable.
5. Do not change the C ABI in the same step. The C ABI can expose handles and stable keys.

## 9. Risks

- `MAX_STORYLETS` is one capacity for the whole storylet space, not an ending capacity. Measure scope count before freezing the bitset size.
- `u16` scope indices cap concurrent scopes at 65,536. Validate before content scale grows.
- Generation counters can wrap after 65,536 reuses of one slot. Use `u32` generation if long sessions are expected.
- Parallel arrays are harder to inspect. Provide a debug-only row formatter rather than storing debug strings in the hot table.
- Dense tables must not become a second source of truth beside call history and `WorldStateTable`.

## 10. References

- `crates/storylet-engine/src/chrono_state.rs:16-22` — capacities.
- `crates/storylet-engine/src/chrono_state.rs:36` — `TagBitset`.
- `crates/storylet-engine/src/chrono_state.rs:931` — `WorldStateTable`.
- `crates/storylet-engine/src/chrono_state.rs:1575` — `MutationOp`.
- `crates/storylet-engine/src/chrono_state.rs:1609` — `BijectiveDelta`.
- `crates/storylet-engine/src/chrono_runtime.rs:120` — packed `SignalToken`.
- `crates/storylet-engine/src/chrono_runtime.rs:360` — `CallHistoryEntry`.
- `crates/storylet-engine/src/chrono_runtime.rs:1737` — current `BTreeSet` completion state.
- `chronoflow_narrative_engine_technical_specification_v4.md` §3 — DOD baseline.
