# REQ-DO-05 — Indexed Trigger Events

**Status:** Proposed  
**Target:** ChronoFlow v6  
**DOD relationship:** New runtime layer; consumes dense tag, metric, item, storylet, and world-stage indices from the existing registries.  
**Depends on:** `REQ-DO-01`, `REQ-DO-02`, `REQ-DO-03`  
**Related:** `REQ-DO-04`, `REQ-DO-07`

## 1. Problem

A storylet must be able to start because another storylet changed:

- a quality or tag;
- an item (acquired, lost, shown, hidden);
- a world stage/arc phase;
- another storylet’s completion.

Checking every storylet subscription after every mutation is O(subscriptions × mutations). That is the wrong shape for a real-time game.

The engine needs an event index:

```text
mutation -> affected event buckets -> candidate subscriptions -> deterministic start/stop
```

Not:

```text
mutation -> scan all subscriptions
```

## 2. Requirement

**REQ-DO-05:** Trigger lookup SHALL use an indexed event table, not a full subscription scan.

Specific rules:

1. Every trigger subscription is a dense row.
2. Lookup is keyed by `(EventKind, EventKeyIndex)`.
3. The key index is a dense quality/tag/item/storylet/world-stage index; no strings.
4. Matching subscriptions are stored in CSR buckets sorted by `(priority, storylet_index, subscription_index)`.
5. Event processing is deterministic and bounded per tick.
6. Duplicate triggers, already-active storylets, and disabled subscriptions are skipped.
7. Reentrant mutations enqueue events; they do not recursively start unbounded chains.
8. The scheduler starts/stops storylet instances; the trigger table does not own instance state.

## 3. Event kinds

```rust
#[repr(u8)]
pub enum EventKind {
    Quality = 0,          // numeric quality crossing or changing
    Tag = 1,              // tag added/removed
    Item = 2,             // acquired, lost, shown, hidden
    StoryletCompleted = 3,
    StoryletUncompleted = 4,
    WorldStage = 5,       // arc phase, act, apocalypse stage
    NodeCompleted = 6,
    Custom = 7,
}
```

Each event has a dense key index:

- Quality/Tag: `TagIndex` or `MetricIndex` from the existing registries.
- Item: `ItemIndex`.
- Storylet: `StoryletIndex`.
- WorldStage: a dense stage key (for example `ArcPhaseIndex`, `ApocalypseStageIndex`).
- Custom: a load-time custom event registry.

## 4. Subscription row

```text
TriggerRow
  event_kind:          u8
  event_key:           u16
  storylet_index:      u16
  predicate_index:     u16       // optional extra predicate table
  once:               bool
  enabled:            bool
  priority:           i16
  scope_filter:       u16        // INVALID = any scope
  generation:         u32
```

No `Vec` inside a row. A subscription has one event target. A storylet with multiple triggers has multiple rows.

`once` is a content flag, not per-session state. Fired state lives in a per-session `TriggerFireTable`, not in the immutable trigger row:

```text
TriggerFireTable
  scope_index:        u16
  subscription_index: u32
  fired:              Bitset
```

This keeps the compiled trigger table shareable across sessions. `TriggerFireTable` is a derived index, not independent saved state: a trigger-caused start records `trigger_subscription` in the call entry, and the table is rebuilt from call history on load/reconcile. Rollback removes the corresponding call entry and fires/rebuilds the index.

## 5. CSR buckets

```text
TriggerTable
  rows:             Vec<TriggerRow>
  bucket_offsets:   Vec<u32>    // length event_key_count + 1
  bucket_entries:   Vec<u32>    // subscription indices, sorted
```

For a dense event-key space `K`:

- `bucket_offsets[k]..bucket_offsets[k + 1]` is the bucket for key `k`.
- The bucket contains every subscription whose `event_key == k`.
- Sort each bucket by `(priority, storylet_index, subscription_index)`.
- If event kinds have separate key spaces, use one CSR per event kind or a combined `(kind, key)` index.

### Lookup

```text
fn candidates(kind, key):
    base = bucket_offsets[key]
    len  = bucket_offsets[key + 1] - base
    return &bucket_entries[base .. base + len]
```

This is O(log n) or O(1) plus the number of actual candidates. It never scans unrelated subscriptions.

## 6. Event queue

Mutations enqueue events rather than starting storylets immediately inside the mutation.

```text
EventQueue
  events: Vec<Event>
  head: u32
  generation: u32

Event
  kind: u8
  key: u16
  old_value: f32
  new_value: f32
  source_scope: u16
  source_instance: u32
  tick: u32
  sequence: u32
```

### Tick processing

```text
process_tick:
    sort/merge pending events by (tick, sequence)
    for event in queue:
        candidates = TriggerTable.candidates(event.kind, event.key)
        for sub in candidates:
            if sub.disabled: continue
            if sub.once and fired(event.source_scope, sub): continue
            if sub.scope_filter != INVALID and sub.scope_filter != event.source_scope: continue
            if not predicate_matches(sub, event, state): continue
            if active_instance_for(sub.storylet, sub.scope): continue
            scheduler.start(sub.storylet, sub.scope, priority)
            if sub.once: mark_fired(event.source_scope, sub)
    cap starts per tick
```

`sequence` is a monotonic counter assigned when the event is enqueued. It is the final tie-break.

## 7. Ordering rules

1. Events are processed by `(tick, sequence)`.
2. Candidates are processed by `(priority, storylet_index, subscription_index)`.
3. Instance start order is `(priority, instance_index)`.
4. If two subscriptions target the same storylet/scope, only the first starts an instance.
5. A start triggered by an event inside the same tick is processed before the next tick.
6. Cascades are bounded by `MAX_TRIGGER_STARTS_PER_TICK` and `MAX_TRIGGER_DEPTH`.
7. On cap, remaining events stay queued and are reported as a diagnostics counter.

## 8. Trigger semantics

### 8.1 Quality/tag change

```text
if old < threshold && new >= threshold: Up
if old >= threshold && new < threshold: Down
```

Do not infer crossing from a shared tag bit. Store the previous value or use a crossing predicate.

### 8.2 Item events

```text
ItemAcquired(owner, item, count)
ItemLost(owner, item, count)
ItemShown(instance, item)
ItemHidden(instance, item)
```

Item triggers need both item identity and owner/scope. A shown item may be visible without being carried.

### 8.3 Storylet completion

On `completed.set(storylet)`, enqueue:

```text
StoryletCompleted { storylet_index, scope_index, instance_index }
```

Dependent storylets use a precompiled `required_storylets` bitset; the trigger index is optional acceleration. The completion bitset remains the authoritative eligibility gate.

### 8.4 World stage

World stage is an explicit dense quality or stage index. Do not use string comparisons such as `world_stage == "zombie_apocalypse"` in a trigger.

## 9. Avoiding loops and duplication

- A subscription with `once` records a per-session fired bit after first fire.
- A storylet already active in the same scope is skipped.
- A storylet completed in the same scope is skipped unless `repeatable`.
- An event that does not change a value or membership does not enqueue.
- Trigger generation counters invalidate subscriptions removed or replaced during content reload.
- `StoryletCompleted` and `StoryletUncompleted` must be paired; a rollback enqueues the inverse.

## 10. Invariants

1. Every enabled subscription appears in exactly one bucket.
2. Bucket boundaries are monotonic and cover all rows.
3. Event processing never scans the full subscription table.
4. Candidate order is deterministic.
5. No trigger starts an instance from an inline recursive mutation; all starts go through the queue.
6. The per-tick start cap is enforced.
7. A stale generation is ignored, not started.
8. Events are not dropped silently; overflow is counted and reported.
9. `once` state is per session/scope, derived from call history, and reverted with that scope; the compiled row is immutable.

## 11. Integration

- `StoryletScheduler` consumes candidates and owns `StoryletInstanceTable`.
- `EffectTransaction` emits events after commit, not before.
- `CallHistory` records the start and completion that the trigger caused.
- `ScopeTable` supplies scope filters and active-instance checks.
- `completed_storylets` bitset remains the authoritative dependency check.
- Trigger predicates MAY use the derived feature indexes in `REQ-DO-11`; they do not add a second state store.

## 12. Acceptance tests

- `trigger_bucket_matches_scan`: for 10,000 random events, bucket candidates equal a full filtered scan.
- `trigger_order_deterministic`: shuffled insertion produces identical start order.
- `trigger_once`: a once subscription fires once; later events do not restart it.
- `trigger_active_storylet_skipped`: an active same-scope storylet is not started twice.
- `trigger_cascade_cap`: a self-amplifying chain reaches the cap, reports overflow, and does not hang.
- `trigger_item_events`: acquired/shown/hidden/lost trigger the expected storylets.
- `trigger_completion_events`: completing storylet A starts B in the same tick.
- `trigger_no_full_scan`: instrumentation counts visited rows; it equals bucket length, not total subscriptions.
- `trigger_rollback`: rolling back the source event removes the caused instance/effect, removes/recomputes the fired bit, and enqueues the inverse event.

## 13. Migration

1. Add `TriggerTable` and a compatibility adapter that scans existing trigger-like predicates.
2. Populate buckets from authored storylet conditions.
3. Switch one event kind at a time, starting with `StoryletCompleted`.
4. Add item and world-stage events after inventory and stage keys are dense.
5. Keep the full-scan path behind a debug flag until parity is proven.

## 14. Risks

- The dense event-key space can be sparse if custom events have high IDs. Use a compact custom event registry.
- A trigger that starts a storylet that immediately emits the same event can loop. Require `once`, a scope guard, or an explicit depth cap.
- Event queues can grow without bound if processing is slower than mutation. Cap and report.
- Sorting every tick is wasteful. Use stable insertion order plus a monotonic sequence.
- Trigger rows are content, not state. Do not store per-tick fired flags in the immutable table if the table is shared; use a per-session bitset/generation array.

## 15. References

- `crates/storylet-engine/src/chrono_dispatch.rs:680` — eligibility.
- `crates/storylet-engine/src/chrono_runtime.rs:1737` — current completion set.
- `crates/storylet-engine/src/chrono_runtime.rs:175` — passion threshold event precedent.
- `REQ-DO-03` — completion bitset.
- `REQ-DO-07` — transactional event emission.
