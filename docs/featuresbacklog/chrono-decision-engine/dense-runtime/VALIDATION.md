# Dense Runtime Validation Log

**Status:** Prototype validation  
**Prototype:** `crates/storylet-engine/examples/dense_runtime_prototype.rs`  
**Run:**

```sh
cargo run --release --manifest-path crates/storylet-engine/Cargo.toml --example dense_runtime_prototype
```

## 1. Purpose

Validate that the proposed dense layouts are viable before changing the engine:

- a storylet-completion bitset replaces `BTreeSet<u16>` for eligibility;
- CSR trigger buckets avoid full subscription scans;
- a pre-sorted 128-row ending-component table resolves combinations deterministically (superseded model; profile pools not prototyped);
- write-set bitset intersections detect conflicts without per-transaction hash sets.

The prototype is standalone. It does not import or modify engine runtime tables. It measures the layout and algorithmic shape, not production integration.

## 2. Hypotheses

| Hypothesis | Measurement |
|---|---|
| Bitset completion is faster than a tree for membership and rebuild | build 100,000 history entries; 10,000,000 membership checks |
| Trigger lookup scales with bucket size, not total subscriptions | 4,096 subscriptions, 100,000 events, 64 event keys |
| Ending-component resolution is a sorted linear pass (superseded model) | 128 components, 100,000 resolutions |
| Write-set conflicts are cheap tag-mask intersections | 10,000 transactions, 32 neighbour pairs each |

## 3. Results

Release build, current workstation:

| Operation | Dense layout | Comparison |
|---|---:|---:|
| Build from 100,000 completions | **48.1 µs** bitset | **3.3131 ms** `BTreeSet<u16>` |
| 10,000,000 membership queries | **5.8878 ms** bitset | **95.5017 ms** `BTreeSet` |
| 100,000 events, 4,096 subscriptions | **1.1638 ms** CSR buckets, 6,400,000 matched rows | **64.2989 ms** full scan |
| 100,000 ending resolutions, 128 components | **3.8146 ms** sorted table | — |
| 10,000 write sets, 32 neighbour pairs | **2.9969 ms**, 1 known conflict | — |

Both trigger paths perform the same per-match checksum work. The bucket path visits only matching rows; the scan path visits every subscription for every event. The earlier length-only bucket loop was rejected because it did not perform per-hit work.

These are synthetic microbenchmarks. They exclude:

- real cache pressure from full engine rows;
- serialization/deserialization;
- content-patch rebasing;
- concurrent access or lock contention;
- correctness of a production scheduler.

They are sufficient to reject the naive object/tree layout and proceed to contract tests.

## 4. Engine-type validation

Engine probe:

```sh
cargo run --release --manifest-path crates/storylet-engine/Cargo.toml --example dense_runtime_engine_probe
```

It uses the real engine types, not the standalone bit model:

- `FlowGraphInstance` QBN hub completed storylets `[1,2,3]`.
- `global_call_history` contained 3 entries.
- A bitset rebuilt from `chosen_storylet_id` was `0b1110`, matching the live `completed_storylets` `BTreeSet<u16>`.
- A zero-sum passion mutation explicitly naming micro-passion `1` produced macro writes for NPC `0` and NPC `1`.

This confirms two critical spec claims:

1. Call history is sufficient to rebuild completion state.
2. A write set derived only from the explicit `MutationOp`/passion delta under-reports. Zero-sum propagation writes targets the op never names.

The probe does not perform save/load or patch rebase. Those remain engine tests.

## 5. Capacity check

The workspace database has **502 storylets**. `TagBitset` has 256 bits (`MAX_TAGS`), so it cannot serve as the completion set. `StoryletBitset` needs its own capacity. The target 20,000-ending space requires a 65,536-bit default (1024 words, 8 KB per scope); 502 storylets fit easily. Measure scope count before freezing the capacity.

## 6. What each result validates

### REQ-DO-01 / REQ-DO-03 — dense tables and bitset

The bitset build and query results confirm that completion is a fixed-size word operation. The prototype tested 4,096 storylets: 64 `u64` words (512 bytes) per scope. The 20,000-ending target raises the default to 65,536 bits (1024 words, 8 KB per scope); that larger size has not been benchmarked here. Rebuild is linear in history, but runtime eligibility is not.

The result does **not** prove memory usage across thousands of scopes. Measure scope count before freezing `MAX_STORYLETS`.

### REQ-DO-02 — dense IDs

The prototype uses integer indices throughout. It does not test load-time string mapping. The mapping contract is validated by the registry tests listed in `REQ-DO-02`.

### REQ-DO-04 — call history canonical

The bitset is built from a synthetic completion history. This validates the rebuild direction. It does not validate stable-key persistence or patch rebasing; those require an engine-level test.

### REQ-DO-05 — trigger index

The bucket result validates the asymptotic claim: lookup is proportional to the number of subscriptions in the bucket. The prototype does not test event ordering, once semantics, active-instance suppression, or cascades. Add the acceptance tests in `REQ-DO-05`.

### REQ-DO-06 — ending storylets

Endings are storylets marked `ending` and selected through a profile pool. The prototype’s 128 ending-component rows are a legacy shape check; the current model is a `SelectionProfile`. No runtime ending table is validated.

### REQ-DO-07 — write sets

The conflict scan validates that tag-mask intersections are cheap. It does not test item ownership, reservations, merge policies, or rollback. Those are in `REQ-DO-07`.

## 7. Acceptance mapping

| Requirement | Prototype evidence | Required contract tests |
|---|---|---|
| `REQ-DO-01` | dense index build/query | allocation, capacity, generation, iteration |
| `REQ-DO-02` | integer-only loops | duplicate/missing/capacity/roundtrip |
| `REQ-DO-03` | bitset rebuild/membership | isolation, inheritance, rollback, no history scan |
| `REQ-DO-04` | bitset from history | stable-key save/load, rebase, reconcile, hash |
| `REQ-DO-05` | CSR bucket vs scan | order, once, active skip, cascade cap, rollback |
| `REQ-DO-06` | none (not prototyped) | marker mapping, terminal graph, profile pool, fallback |
| `REQ-DO-07` | tag-mask conflict | items, reservations, policies, exact revert |
| `REQ-DO-08` | none (not prototyped) | deterministic weighted pick, zero total, floor, replay, large-pool one-shot |
| `REQ-DO-09` | none (not prototyped) | empty/min/max gate, shortcut, unknown reference, zero-ending profile |
| `REQ-DO-10` | none (not prototyped) | 20k stress load, O(1) IDs, profile candidate cap, lazy payload, capacity error |
| `REQ-DO-11` | none (not prototyped) | item/trait/choice index parity, rebuild, rollback, write-set conflict, profile filter |
| `REQ-DO-12` | none (not prototyped); service/studio/adapter tests added | graph round-trip, zero-ending project, unknown node rejection, no-location project |

## 8. Limitations and follow-up

1. **Do not treat these numbers as a performance guarantee.** They are a shape check.
2. **Do not ship the prototype as engine code.** It is a standalone example; production must use the engine’s registries, state tables, and rollback contract.
3. **Measure real scope counts.** Per-scope bitsets can dominate memory if scopes are unbounded.
4. **Measure real content.** `MAX_STORYLETS = 65,536` and trigger counts are defaults for the 20k-ending target, not measured production limits.
5. **Add allocation instrumentation.** The prototype does not assert zero allocations in production hot paths.
6. **Validate save/load with stable keys.** The prototype has no persistence.
7. **Validate concurrency policy.** The prototype has no threads or deterministic scheduler.
