# Code Review — ChronoFlow v5 conformance (pass 3)

Date: 2026-10-19
Spec: `docs/featuresbacklog/chrono-decision-engine/chronoflow_narrative_engine_technical_specification_v5.md`
Previous: `.codereview/2026-10-19-chrono-v5-conformance-r2.md`

Scope: current state of the untracked ChronoFlow files after the r2 fixes. No git baseline; reviewed on its own.

## Verification run

| Gate | Result |
|---|---|
| `cargo test` (storylet-engine) | pass — 99 tests; acceptance 27 → 33 |
| `cargo test` (storylet-engine-service, `target/review-tests`) | pass — 56 tests (lib 33, local_service 19, twee 3, legacy_import 1) |
| `cargo check --all-targets` (service) | clean — no warnings (previous unused-import warnings gone) |

Independent code checks (no probe file this round; behaviour covered by the new tests):
`beats()` vs its doc, `signal_event` call-data path, `execute_input` instance guard, START/REGULAR/END classification, dispatch error swallowing, salience fallback compile, arc-phase seeding, idempotency/revision enforcement, import revision bump, mission asset map.

## r2 items — resolved

**Mission scoping**
- ✅ `mission_asset_map` walks root + child assets, rejects a GUID claimed by two assets, and scoped histories with no owning asset are dropped instead of rebased against the root (`chrono_playtest.rs:510-560`).
- ✅ `reset()` no longer preserves mirrors; cancel snapshots and restores locally. Tests cover cancel → reconcile and reset → empty mirror.

**Import revision**
- ✅ `replace_project_content` bumps monotonically and returns the revision; `import_bundle`, HTTP, and MCP responses carry `"revision"`. DB read uses `.optional()?` so real errors propagate.

**Scrub / history**
- ✅ Forward → back → forward leaves one completion (history 1 → 0 → 1); crossing the end removes the tail completion commit and moves the head.

**Arc phase**
- ✅ `chrono_graph_adapter.rs:312` rejects a salience candidate without `required_arc_phases`, naming node and storylet; runtime rejection kept as backstop.
- ✅ Phase is now seeded at world init from `scenario.current_arc_phase` (`chrono_adapter.rs:1091-1105`), so arc-gated content is reachable without an unrelated mutation.

**Concurrency / conformance**
- ✅ Persisted `mutation_revision` (column + migration at `store.rs:108,119`), incremented per mutation, surfaced as `mutationRevision`; `expectedMutationRevision` enforced on all 8 v2 request types and declared in the MCP schema.
- ✅ Idempotency key accepted on v2 requests, persisted, and replayed (`chrono_playtest.rs:1395,1448`); unit test `idempotent_retries_return_the_original_payload`.
- ✅ `execute_input` now rejects a token minted by another instance.
- ✅ START/REGULAR/END decomposition implemented (`chrono_runtime.rs:3463-3496`).
- ✅ Salience `fallback_storylet_id` now compiled from the authored node (`chrono_graph_adapter.rs:323`).
- ✅ `SceneComposer.conflicts` asserted (`chrono_v5_acceptance.rs:1408-1438`).
- ✅ MONTE_CARLO now exercises the fallback (99 is the fallback, plus a no-fallback case), and non-repeatable locking is instance-wide.

## Still open

### 🔴 Admitted design-scale

- `chrono_runtime.rs:124` — `SignalToken.target_pin_name` is still a heap `String`; the token vector is not fixed-size or cache-flat. REQ-FG-03.
- `chrono_runtime.rs:1483` — `execute_sorted_action_parts` still arbitrates only the single node's parts, so concurrent scene sections never arbitrate and `conflicts` cannot span nodes. REQ-SC-06.

### 🟡 Behaviour

- `chrono_runtime.rs:75` vs `:1576` — doc says "Lower values win", `beats()` picks the **higher** priority; sort is ascending. Internal contradiction. Fix one convention.
- `chrono_runtime.rs:1998` — `signal_event` force-wakes a latent waiter through `plan(force=true)`, which pushes a second `NodeCallData::Waiter` on top of the arrival record; inverse-cache depth no longer matches history depth. Skip the call-data push for an already-latent waiter.
- `chrono_dispatch.rs:705,715,1109` — `is_eligible` still swallows predicate errors with `unwrap_or(false)`, and `match_most_salient` drops candidates whose score errors. One malformed predicate silently deletes content from both dispatch paths. Surface typed errors.
- `chrono_dispatch.rs:1052` — bonus-tag popcount is still added to specificity although REQ-BB-04 defines specificity as predicate weights only. Now documented as intentional; either amend the spec or drop the term.
- `chrono_dispatch.rs:97` — no `PredicateSubject` reads `state.passions` or `WorldVertex.attributes`; REQ-BB-01 passion/vertex predicates remain inexpressible. (Admitted open.)
- `chrono_adapter.rs:252` — MENACE still only sets a `required_tags` bit; no auto-dispatch/cancel/revert of the crisis storylet. REQ-WB-08. (Admitted open.)
- `chrono_adapter.rs:333` — `ideal_state_vector` weights hardcoded 1.0; spec's `v_j` unexpressible. (Admitted open.)
- `chrono_playtest.rs:110-125` — v2 request structs do not use `deny_unknown_fields`; a misspelled `expectedMutationRevision` silently skips the guard.

### 🟡 `chrono_state.rs` (unchanged since pass 2 — all still present)

- `L1126` — LHS matching greedy, no backtracking; a different valid candidate assignment fails.
- `L2080` — `validate_star_npc` accepts an empty `micro_passions` pool; REQ-WB-04 requires 1..N.
- `L1949` — macro clamp uses `npc.macro_bounds` (default unbounded) instead of `[-100, 100]`; REQ-WB-05.
- `L1979` — threshold events suppressed when the tag bit already matches the crossing direction; external writes desynchronize spawn/cancel bookkeeping.
- `L382` — `MacroThresholdRule`/`star_macro_id` documented as macro-passion but indexes `state.metrics`.

### 🟡 Test procedures still weaker than §11

- `chrono_v5_acceptance.rs:292-405` — INVOKE_EQUIVALENCE graph is still linear (`Mutation → QbnHub → Poi`), the "manual" side is `force_complete`, `declared_routes` is empty, and POI completion is never asserted. REQ-CH-05's ALL fan-in and Choice/Waypoint route branches remain untested.
- `chrono_v5_acceptance.rs:73-274` — ROLLBACK_IDENTITY still drives all 500 mutations through `force_complete`, same-NPC zero-sum only, vertex-only transforms, no threshold rules, empty `MetricRegistry`.
- `chrono_v5_acceptance.rs:414` — `elapsed < 0.002` wall-clock assertion in a parallel debug build; flaky on loaded CI. Gate on release or assert operation counts.
- `chrono_v5_acceptance.rs:613` — waypoint test still starts with `force_complete` rather than the live latent path, and never asserts that diverting into the player-only `Revolution` topic is refused before `PopularUnrest` exists.

### 🔵 Small

- `crates/storylet-engine/src/bin/benchmark.rs:151` — `unused variable: locations_count`; pre-existing warning in a file touched by the cutover.

## Requirement status deltas (pass 2 → pass 3)

- REQ-GP-01/02/03: Partial → **Implemented** (scoped rebase per mission asset, conflict detection, revision bump on import).
- REQ-SC-04/05: Partial → **Implemented** (START/REGULAR/END, backward completion removal).
- REQ-BB-04: Partial → **Partial** (fallback fixed; weights and swallowed errors remain).
- REQ-BB-05: Partial → **Implemented** (compile-time rejection + seeded phase).
- REQ-FG-03, REQ-SC-06, REQ-WB-08, passion/vertex subjects: unchanged (admitted open).

## Recommended next actions

1. `chrono_runtime.rs:1998` — stop pushing `NodeCallData::Waiter` when force-waking an already-latent waiter (one-line guard, prevents rollback drift).
2. `chrono_runtime.rs:1576` — make `beats()` and its doc agree on priority direction; add one ordering test.
3. `chrono_dispatch.rs:705` — replace `unwrap_or(false)` with a propagated `DispatchError` so bad predicates fail authoring instead of deleting content.
4. `chrono_playtest.rs:110` — add `#[serde(deny_unknown_fields)]` to the v2 request types.
5. Then the admitted design items: flat `SignalToken`, cross-section composer pass, passion/vertex predicate subjects.
