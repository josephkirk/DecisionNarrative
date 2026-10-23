# Code Review — ChronoFlow v5 conformance (pass 2)

Date: 2026-10-19
Base spec: `docs/featuresbacklog/chrono-decision-engine/chronoflow_narrative_engine_technical_specification_v5.md`
Previous pass: `.codereview/2026-10-19-chrono-v5-conformance.md`

Scope: current state of the untracked ChronoFlow files (no git baseline; reviewed on its own).

## Verification run

| Gate | Result |
|---|---|
| `cargo test` (storylet-engine, all targets) | pass — acceptance suite 8 → 27 tests, dispatch 8, rest unchanged |
| `cargo test` (storylet-engine-service) | pass — 54 tests (lib 31, local_service 19, twee 3, legacy_import 1) |
| Probe: zero-cost cycle | pass — `new()` rejects with `ZeroCostCycle` |
| Probe: costed start node (5 ms + 2 ms) | pass — runs, history 2, no double-execution |
| Probe: 2-pin `LogicAll` fan-in | pass — gate `Completed`, end `Completed`, history `[1,2,3,4,5]` |
| Probe: `shortest_bridge_path` over an inserted node | pass — `[(2, "In", "Out")]` |
| Probe: `rebase_call_history` with inserted node | pass — splices node 2 with `in=In out=Out` |
| Probe: `replay_history` with `In_0` input pin | pass — no `PinNotFound` |
| Probe: `checkout` to an earlier commit | pass — history 2 → 1, global history 1 (no duplicate) |
| Probe: `reconcile_history` with an equal target | pass — history stays 2 |

Probe file was temporary and has been deleted.

## Previous blockers — resolved

1. ✅ Zero-cost cycle hang — `validate`/`new` now rejects with `ZeroCostCycle`; `tick` also caps at `MAX_TICK_EXECUTIONS`.
2. ✅ Multi-input `LogicAll` deadlock — `arrived_input_pins` is written back in `commit_plan` and cleared on completion.
3. ✅ Costed node from `start()` — `tick` now assigns `token.time_budget_ms = remaining.max(cost)` before execution, and defers exactly one frame.
4. ✅ `shortest_bridge_path` one-hop BFS — walks the full edge list; pins correct.
5. ✅ `enforce_revision_pin` rejected every rebase — now compares a compiled ID-space fingerprint and adopts the rebased history.

Also fixed: LHS/RHS add-before-validate, `previous_attributes` overwrite, `EdgePattern.edge_id`, edge-remove bindings, LHS backtracking (partial — see below), `MacroThresholdRule` per-ancestor bonus scoring, cooldown enforcement on the eligibility path, waypoint transition asset surfacing, non-repeatable lock scope, replay input pin, reconcile LCA sentinel, checkout duplicate push and `plan()` rebuild, scene clock reset on re-entry, negative-duration validation, backward window boundary, `commit_plan` mission routing, `cancel_and_revert` head revert, `last_completed_ms` restore, mission-rollback HTTP+MCP exposure, backward scrubbing exposure, v2 `storyletId` deck/`is_playable`/cooldown enforcement, `choose` request validation, and rebase tests.

Requirement deltas since pass 1: REQ-FG-03/04, REQ-CH-03/06, REQ-GP-01/03 move from Broken to Partial/Implemented.

---

## Remaining findings

### 🔴 Still open

- `crates/storylet-engine-service/src/chrono_playtest.rs:488` — **scoped mission histories are rebased against the root asset.** `rebase_call_history(asset, …)` is called with the root `FlowGraphAsset` for the per-mission `scoped` map, but mission-child entries carry child-asset GUIDs; they are dropped (`node()` miss → `continue`) or silently misread on GUID collision. Rebase each scoped history against its mission node's asset.
- `crates/storylet-engine/src/chrono_runtime.rs:3314` — **mission cancel discards the child history**, so the forward pass of `reconcile_history` force-completes the mission boundary without child effects, and replayed scopes pass an empty mission map (`L3142`) — nested missions can never replay. Snapshot the scoped array before unwinding and thread nested scopes through reconcile.
- `crates/storylet-engine/src/chrono_runtime.rs:1483` — **REQ-SC-06 arbitration is still per node.** `execute_sorted_action_parts` only arbitrates the parts of the single `node_guid` it is handed and stamps every command with that node, so concurrent scene sections never conflict and `conflicts` cannot span nodes. Collect all active sections' parts into one composer pass.
- `crates/storylet-engine/src/chrono_runtime.rs:124` — **REQ-FG-03 violated at the type level.** `SignalToken.target_pin_name` is a heap `String` plus separate status fields; the token vector is neither fixed-size nor cache-flat. Store `u16`/`u32` pin indices and pack the flags.

### 🟡 Behaviour / conformance gaps

- `chrono_dispatch.rs:680` — REQ-BB-05 remains unenforced: `required_arc_phases` is `#[serde(default)]` and an **empty mask bypasses the gate**, which is exactly the premature-ending case the requirement targets. Require a non-empty mask for salience candidates.
- `chrono_adapter.rs:287` — `required_arc_phases` compiles to a tag mask but nothing seeds the active phase at world init, so phase-gated content is unplayable until an unrelated mutation adds the tag. Seed the phase (or add a phase field to the world).
- `chrono_graph_adapter.rs:305` — salience selectors compile `fallback_storylet_id: None`; the authored node has no fallback field, so a fully-gated pool returns `NoSalienceMatch` instead of the spec's fallback (§11.3).
- `chrono_dispatch.rs:699,1092` — `is_eligible` still swallows predicate errors (`unwrap_or(false)`) and `match_most_salient` drops candidates whose score errors (`let Ok(score) = … else continue`); one malformed predicate silently removes content from both dispatch paths. Surface typed errors.
- `chrono_playtest.rs:89` — no chrono v2 request carries `expectedRevision` or `idempotencyKey` and `Deserialize` ignores unknown fields; retries double-apply and a stale `reconcile` can rewind newer state. Add both fields, `deny_unknown_fields`, and enforce them.
- `store.rs:630` — `replace_project_content` writes `project_metadata.revision` verbatim, so a same-revision bundle with an unchanged ID space rebinds live sessions to different effect content. Bump revision monotonically on import.
- `chrono_runtime.rs:3410` — long event fully inside the window is still labelled `START` only; derive activity from window crossings (START / REGULAR / END) per REQ-SC-05.
- `chrono_runtime.rs:1576` — `beats()` picks the highest `action_key_priority` while the doc and the ascending sort say lower wins; winner and dispatch order disagree.
- `chrono_runtime.rs:1952` — `signal_event` force-wakes a latent waiter and pushes a second `NodeCallData::Waiter`, leaving an unmatched inverse-cache entry (the timer-wake path was fixed, this one was not).
- `chrono_runtime.rs:2780` — `execute_input` is public and does not check `graph_instance_id`; only `tick` does.
- `chrono_runtime.rs:3463` — backward scrub below `section_duration_ms` leaves `state = Completed` and the completion entry in history, and `was_completed` stays true, so forward play cannot re-emit completion.
- `chrono_state.rs:1126,2080,1949,1979,382` — LHS matching still greedy without backtracking; `validate_star_npc` accepts an empty passion pool (REQ-WB-04 says 1..N); macro clamp uses unbounded `macro_bounds` instead of `[-100,100]`; threshold events suppressed when the tag bit already matches; `MacroThresholdRule` still indexes `state.metrics` despite the macro-passion name.
- `chrono_adapter.rs:252` — REQ-WB-08 MENACE crisis still only sets a `required_tags` bit on the crisis storylet; nothing auto-dispatches, cancels, or reverts it. Implement the trigger or rescope the requirement.
- `chrono_dispatch.rs:97` — no `PredicateSubject` reads `state.passions` or `WorldVertex.attributes`, so REQ-BB-01's passion and vertex-attribute preconditions remain inexpressible.

### 🟡 Test coverage still short of §11

- `chrono_v5_acceptance.rs:454` — MONTE_CARLO fallback branch still unreachable: 99 is a normal pool member. Remove it from the pool and assert fallback selection.
- `chrono_v5_acceptance.rs:341` — INVOKE_EQUIVALENCE graph is still linear with empty `declaredRoutes`, and POI completion is not asserted.
- `chrono_v5_acceptance.rs:267` — ROLLBACK_IDENTITY still drives all 500 mutations through `force_complete`, same-NPC zero-sum only, vertex-only transforms, no threshold rules, empty `MetricRegistry`.
- `chrono_v5_acceptance.rs:414` — the 2.0 ms wall-clock assertion runs in a parallel debug build; flaky by construction.
- `chrono_v5_acceptance.rs:609` — waypoint test still force-completes before diverting and never asserts the player-only topic is locked.

### 🔵 Small

- `chrono_adapter.rs:333` — `ideal_state_vector` weights hardcoded 1.0; spec's `v_j` unexpressible. Document or add weights to the authoring contract.
- `chrono_dispatch.rs:1040` — bonus-tag popcount is still added to specificity although REQ-BB-04 defines specificity as predicate weights only; either drop it or amend the spec.
- `chrono_v5_acceptance.rs:1249` — `SceneComposer.conflicts` is never asserted and the scrub payload drops conflicts.
- Service crate emits `unused imports` warnings (`StoryletChoiceRecord`, `StoryletContentRecord`, `StoryletRecord`, `storylets_to_twee_story`, `twee_story_to_storylets`) — leftover from the cutover; remove.

## Requirement status (updated)

Implemented: REQ-WB-01/02/03/04/06/07/09, REQ-BB-03, REQ-FG-01/02/04/05, REQ-SC-01/02/03, REQ-CH-01/02/03/04/05.

Partial: REQ-WB-05 (clamp bounds), REQ-BB-01 (no passion/vertex subjects), REQ-BB-02 (progress lock), REQ-BB-04 (weights, fallback, swallowed errors), REQ-BB-05 (empty mask), REQ-BB-06 (dialogue asset unused), REQ-FG-03 (token layout), REQ-SC-04/05 (activity classification, backward completion), REQ-CH-06 (mission tails), REQ-GP-01/03 (scoped rebase).

Missing: REQ-WB-08 (menace crisis auto-dispatch, quality categories), REQ-SC-06 (cross-section arbitration), REQ-GP-02 (no explicit tombstone test).

## Recommended next actions

1. `chrono_playtest.rs:488` — rebase scoped histories against their mission asset (data loss on every patch).
2. `chrono_runtime.rs:3314` — preserve child history across mission cancel so reconcile replays effects.
3. `chrono_dispatch.rs:680` — make the arc gate fail closed (empty mask = reject, not pass).
4. `chrono_runtime.rs:1483` — one composer pass across active sections; then assert `conflicts` in a test.
5. Seed the arc phase in `chrono_adapter.rs:287` and add a salience fallback field so §11's starvation criterion can actually be tested.
