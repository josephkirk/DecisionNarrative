# REQ-DO-08 — Selection Profiles and Deterministic Choice

**Status:** Proposed  
**Target:** ChronoFlow v6  
**DOD relationship:** Reuses the existing seeded weighted-selection algorithm from the legacy storylet path; adds a dense Chrono adapter.  
**Depends on:** `REQ-DO-01`, `REQ-DO-02`  
**Related:** `REQ-DO-03`, `REQ-DO-06`, `REQ-DO-09`

## 1. Problem

The Chrono path has two selection modes:

- `query_qbn_deck` returns eligible rows sorted by priority;
- `match_most_salient` returns the argmax candidate with a seeded tie-break (`chrono_dispatch.rs:1117`).

Neither gives a probability distribution over candidates. The repository already has a seeded weighted selector in the legacy path:

```rust
pub fn seeded_weighted_shuffle(
    world: &mut WorldState,
    candidates: &[CandidateEvaluation],
) -> Vec<String>
```

`crates/storylet-engine/src/storylets.rs:232`, tested at `tests/storylet_selection.rs:270`.

The gap is not “build an RNG.” The gap is one reusable selection profile:

1. adapt the weighted selector to dense Chrono rows;
2. provide a **one-shot** weighted pick, not an O(n²) full shuffle;
3. let normal, ending, QBN, salience, and custom modes share the same primitive;
4. keep weighting optional; authored priority/salience remain valid policies.

## 2. Requirement

**REQ-DO-08:** Storylet selection SHALL be a deterministic `SelectionProfile` over an eligible candidate set, with priority, salience, or weighted policy.

Rules:

1. Weights are non-negative finite `f32`.
2. Zero-weight candidates are eligible only if an authored floor/fallback says so.
3. The sampler returns one candidate in O(n) scoring + O(n) or O(log n) draw.
4. The result is deterministic for `(state, seed, random_cursor, content_revision)`.
5. The sampler does not mutate `WorldStateTable`; it advances only a sampler cursor.
6. Normal, ending, QBN, salience, waypoint, and custom selection use the same profile primitive with different pools, gates, and weights.
7. A weighted draw is recorded in call history/save.
8. No full shuffle of a large profile pool.

## 3. Selection profile and policy

```text
SelectionProfile
  key:              ProfileKey
  pool:             Deck | QBN | Salience | Ending | CatalogQuery | Custom
  include_ending:   bool              // default false; explicit opt-in only
  candidate_limit:  u16
  eligibility:      compiled exact predicates
  gates:            Vec<GateKey>          // REQ-DO-09; empty = no extra gate
  policy:           Priority | SalienceMax | Weighted | SeededOrder
  terminal:         Continue | EndScope | Jump
  terminal_target:  Option<StoryletKey>  // required when terminal=Jump
  fallback:         FallbackPolicy
```

```rust
pub enum SelectionPolicy {
    Priority,      // display_priority, then storylet index
    SalienceMax,   // current match_most_salient
    Weighted,      // one-shot weighted pick
    SeededOrder,   // optional no-replacement ordering
}

pub enum FallbackPolicy {
    Storylet(StoryletKey),  // default for an ending profile
    Continue,               // keep the current scope alive
    Error,                  // explicit typed failure
}
```

Generic profiles default to `FallbackPolicy::Error`; an ending profile defaults to `FallbackPolicy::Storylet(authored)`. Normal traversal, the ending profile, QBN hubs, salience pools, and waypoint choices are configurations of the same struct. `Weighted` is the default for an ending profile but is not required by the engine; an author may use `Priority` or `SalienceMax` instead.

## 4. Weight computation

```text
weight(candidate) =
    max(0, base_weight or display_weight)
    * exp(score / temperature)
    * exploration_factor
    * repetition_penalty
```

- `score` is salience/quality distance.
- `temperature` defaults to 1.0 and is authored per selection context.
- `exploration_factor` is optional; if absent, 1.0.
- `repetition_penalty` is optional; if absent, 1.0.
- A probability floor can be applied before normalization:

```text
p_i = max(floor, weight_i)
p_i = p_i / sum(p)
```

The floor is only allowed when the author declares it. It must not make an ineligible candidate selectable.

## 5. One-shot algorithm

```text
fn weighted_pick(candidates, weights, rng) -> index:
    total = 0
    for w in weights: total += w
    if total <= 0: apply FallbackPolicy
    draw = rng_next_f64() * total
    acc = 0
    for (i, w) in weights:
        acc += w
        if draw < acc: return i
    return last
```

Complexity: O(n) to sum and scan. For a large profile pool, that is one pass per selection, not per candidate beat.

Do not use `seeded_weighted_shuffle` as-is for a one-shot profile if it would remove every candidate. It returns a full order and is O(n²). Add a `weighted_pick_one` function and keep the shuffle for legacy/no-replacement callers.

## 6. Randomness and replay

Use a sampler state separate from world state:

```text
SelectionRng
  seed: u64
  cursor: u64
```

Advance `cursor` exactly once per draw. Do not read a clock or OS RNG.

Persist:

```text
selection_seed
selection_cursor
selected_storylet_key
content_revision
```

Replay must use the recorded selected storylet, not a fresh draw. This is especially important for large profile pools.

The existing `WorldState.random_seed/random_cursor` is the legacy pattern; Chrono needs the same explicit cursor in its save contract.

## 7. Profile examples

```text
normal:
  pool=Deck, candidate_limit=100,
  gates=[],
  policy=Priority|SalienceMax|Weighted,
  terminal=Continue,
  fallback=Error

ending:
  pool=Ending, candidate_limit=profile,
  gates=[PacingGate],
  policy=Weighted,
  terminal=EndScope,
  fallback=Storylet(authored)

qbn_hub:
  pool=QBN, candidate_limit=deck size,
  gates=[],
  policy=Priority|Weighted,
  terminal=Continue|Jump,
  fallback=Error

salience:
  pool=Salience, gates=[],
  policy=SalienceMax|Weighted,
  terminal=Continue,
  fallback=Error
```

Weights may use salience, quality distance, and profile-specific overrides. The profile, not the policy, decides whether a result ends the scope.

## 8. Invariants

1. Ineligible candidates never enter the weight vector.
2. A zero total weight returns `FallbackPolicy::Storylet`, `Continue`, or a typed `Error` as configured.
3. Same state/seed/cursor/version produces the same index.
4. A draw advances the cursor exactly once.
5. The sampler does not mutate world state.
6. Ending selection is one draw, not a shuffle.
7. Probability floors do not bypass eligibility.
8. The selected ID is recorded before effects commit or atomically with them.

## 9. Acceptance tests

- `weighted_pick_deterministic`: same seed/cursor/weights returns same candidate.
- `weighted_pick_distribution`: 100,000 seeded draws approximate authored weights.
- `weighted_pick_zero_total`: follows `Storylet`/`Continue`/`Error`, never panics.
- `weighted_pick_floor`: floor changes distribution but never selects an ineligible row.
- `weighted_pick_one_profile`: large profile pool, one draw, O(n) scan, no full shuffle.
- `weighted_pick_replay`: a saved profile selection replays without a new draw.
- `weighted_pick_profile_determinism`: normal and ending profiles replay identically.
- `weighted_pick_no_world_mutation`: world hash unchanged by sampling.

## 10. Migration

1. Add `SelectionRng` to the session/save contract.
2. Add `weighted_pick_one` with dense indices.
3. Use it for the ending profile first; keep normal profiles on authored policies.
4. Keep `Priority`/`SalienceMax` as default normal policies.
5. Add `Weighted` normal selection only where authored.
6. Replace `seeded_weighted_shuffle` in new code; retain it for legacy callers until migration completes.

## 11. Risks

- Salience scores are not calibrated probabilities. The distribution is authored, not statistically calibrated.
- A probability floor on a large profile pool can swamp the intended distribution. Default floor is 0.
- Reusing the world RNG cursor can couple unrelated systems. Keep a selection cursor explicit.
- Full `seeded_weighted_shuffle` is O(n²) and must not be used for large profile pools.
- If content patches change weights, replay must still use the recorded selection.

## 12. References

- `crates/storylet-engine/src/storylets.rs:232` — existing `seeded_weighted_shuffle`.
- `crates/storylet-engine/tests/storylet_selection.rs:270` — reproducibility test.
- `crates/storylet-engine/src/chrono_dispatch.rs:1117` — Chrono argmax selector.
- `docs/design/social-simulation.md:42` — seeded variation must remain replayable.
- `REQ-DO-06` — ending storylet pool.
- `REQ-DO-09` — selection gates.
