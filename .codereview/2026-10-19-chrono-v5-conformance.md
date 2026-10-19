# Code Review — ChronoFlow v5 conformance

Date: 2026-10-19
Base spec: `docs/featuresbacklog/chrono-decision-engine/chronoflow_narrative_engine_technical_specification_v5.md` (CFN-SPEC-001)
Scope: untracked ChronoFlow implementation (`crates/storylet-engine/src/chrono_{state,dispatch,runtime}.rs`, `crates/storylet-engine-service/src/chrono_{adapter,graph_adapter,playtest}.rs`, tests) plus its HTTP/FFI/store wiring.

Verification run:
- `cargo test --lib` → 17/17 pass
- `cargo test --test chrono_v5_acceptance --test chrono_dispatch` → 16/16 pass
- Prototype probes (temporary `tests/zz_probe_tmp.rs`, now deleted) reproduced three blockers below with actual engine behavior.

Legend: `✅ proven by running code` · `[code]` proven by reading the code path · `[review]` reported by static conformance pass, not independently re-run.

---

## Top 5 blockers (fix before any spec-conformance claim)

1. `crates/storylet-engine/src/chrono_runtime.rs:2479` — ✅ **zero-cost cycle hangs `tick` forever.** Engine loop drains active tokens with no per-tick execution cap and `validate()` does not reject zero-cost cycles. Probe `1→2→1` never returned (killed at 180 s). REQ-FG-03/05.
2. `crates/storylet-engine/src/chrono_runtime.rs:1682` — ✅ **multi-input `LogicAll` never fires.** `plan.arrived_input_pins` is built locally and never written back to `FlowNodeInstance.arrived_input_pins` (field only read at L1678/L1781). Probe: 2-pin ALL gate stays `ActiveLatent` after both branches arrive. REQ-FG-04.
3. `crates/storylet-engine/src/chrono_runtime.rs:2400` — ✅ **any node with `cost_ms > 0` cannot execute from `start()`.** Start token is seeded with `time_budget_ms: 0`; `execute_input` (L2515) rejects `cost > 0`. Probe: `InsufficientBudget { cost_ms: 5, budget_ms: 0 }`, zero calls recorded. `tick`'s `frame_budget_ms` is tracked separately and never written into the token. REQ-FG-03/05.
4. `crates/storylet-engine/src/chrono_runtime.rs:721` — [code] **REQ-GP-03 bridge splicing can never work.** `shortest_bridge_path` builds `adjacency = sorted_adjacency(from)`, which filters `source_node_guid == from`, then BFS-filters `edge.0 == current` — every hop after the first is empty. Any patched graph with an inserted node returns `NoBridgePath`.
5. `crates/storylet-engine-service/src/chrono_playtest.rs:432` — [review] **REQ-GP-01..03 are structurally unreachable.** `enforce_revision_pin` rejects a session as soon as rebasing changes the history (`rebased_global != global` → 409), so tombstones and bridge splices can never be adopted; any patch that deletes/inserts a node permanently bricks saved sessions.

---

## Module 1 — Layered world graph, qualities, passions (REQ-WB)

- `chrono_state.rs:L1256-1280` — 🔴 `[review]` `add_edges` validates against a `present` set that excludes RHS `added_vertices`; an edge to a vertex created by the same rule always fails `VertexNotFound`. "Spawn a subgraph" transforms are inexpressible. REQ-WB-02. Extend `present` with `added_vertices` before validating added edges.
- `chrono_state.rs:L1347-1352` — 🔴 `[review]` `previous_attributes` uses `insert`, so a second transform on the same vertex/key overwrites the cached original; `revert_graph_transform` restores the intermediate value. REQ-WB-03. Use `entry().or_insert_with(...)`.
- `chrono_state.rs:L1153-1178` — 🔴 `[code]` `EdgePattern.edge_id` is never compared, so a pattern naming `"e1"` binds the first relation-matching edge (or the first edge at all). `GraphMatch.edges` can report ids that do not exist. REQ-WB-02.
- `chrono_state.rs:L1235` — 🔴 `[review]` `remove_edges` resolves bindings from `match_.vertices`; `GraphBinding` has no edge variant, so a matched LHS edge can never be removed and literal ids are the only workable form. REQ-WB-02/03.
- `chrono_state.rs:L1616-1625` + `chrono_adapter.rs:L231-237` — 🟡 **REQ-WB-08 MENACE crisis is not implemented as specified.** A crossed menace only sets a global tag; the adapter turns that tag into a `required_tags` bit, i.e. the crisis storylet merely becomes QBN-eligible. Nothing auto-dispatches, cancels, or reverts it. Either implement the auto-trigger or change the spec wording.
- `chrono_state.rs:L1199,1269` — 🟡 `[review]` add/remove validation blocks remove+add of the same id in one rule, so "move actor"/"retarget edge" transforms have no expressible RHS. REQ-WB-02.
- `chrono_state.rs:L1123-1153` — 🟡 `[review]` LHS matching is greedy, no backtracking; a two-vertex + one-edge pattern returns `PatternNotMatched` even when another candidate pair satisfies the edge. REQ-WB-02.
- `chrono_state.rs:L2039-2058` — 🟡 `[review]` `validate_star_npc` accepts an empty `micro_passions` pool; REQ-WB-04 says 1..N. Such an NPC compiles and can never cross a threshold.
- `chrono_state.rs:L1899-1908` — 🟡 `[review]` macro clamp uses `npc.macro_bounds`, whose `Default` is unbounded, not the spec's `[-100, 100]`. REQ-WB-05.
- `chrono_state.rs:L1880-1886` — 🟡 `[review]` `state.active`/`state.micro` are indexed unchecked for every passion of a touched NPC; a registry compiled against a larger capacity panics on a smaller `WorldStateTable`. Return a typed error.
- `chrono_state.rs:L1938` — 🟡 `[review]` threshold events are suppressed when the tag bit already matches the crossing direction; because the bit is shared mutable global state, an external write desynchronizes spawn/cancel bookkeeping. REQ-WB-07.
- `chrono_state.rs:L378-384,L1617` — 🟡 `[review]` `MacroThresholdRule` is documented as a macro-passion threshold but indexes `state.metrics`, while macro values live in `state.passions.macro_values`. Misleading name or wrong table. REQ-WB-05/07.

## Module 2 — QBN / salience / waypoint (REQ-BB)

- `chrono_dispatch.rs:L1275-1299` — 🔴 `[review]` Dijkstra rewrites predecessor/cost for already-visited nodes and the goal unwind has no visited guard; with legal zero-cost edges an equal-cost update installs a predecessor cycle and the unwind loops forever. REQ-BB-06. Skip visited nodes on relaxation; bounded unwind.
- `chrono_adapter.rs:L219-227` — 🟡 `[review]` every `parameters` entry becomes a `RoleRequirement{role_tag: param_name}` while the legacy contract derives the role from `parameter.query`; role binding then looks for an entity carrying the param-name bit and storylets with parameters silently vanish from every QBN/salience pool. REQ-BB-01/02.
- `chrono_dispatch.rs:L250,L658-661` — 🟡 `[review]` REQ-BB-05 is unenforced: `required_arc_phases` is `#[serde(default)]`, and an **empty** mask skips the gate — exactly the King-of-Chicago case the requirement exists to prevent. No `current_dramatic_arc_phase` field exists; the phase is whatever external mutation wrote to `global_tags`.
- `chrono_dispatch.rs:L1016-1019` — 🟡 `[review]` the bonus specificity term is compiled with the tag ancestor closure and intersected with closure-holding `global_tags`, so one authored bonus scores per ancestor and sibling tags cross-trigger; the term is not in REQ-BB-04's formula.
- `chrono_dispatch.rs:L774-812,L1050-1088` — 🟡 `[review]` `cooldown_ms` is consulted by nothing on the eligibility path; `cooldown_remaining_ms`/`cooldown_remaining_turns`/`cooldown_unit` have no callers outside tests. A cooldown storylet is immediately replayable. REQ-BB-01/§5.3.
- `chrono_dispatch.rs:L677,687,946-990` — 🟡 `[review]` `resolve_predicate` never validates `comparison_value`, `MetricGate.metric_id`, or `IdealMetricTarget.metric_id` against `MetricRegistry::count()`, and `is_eligible` swallows every error with `unwrap_or(false)`; one bad gate silently deletes a storylet from both dispatch paths. REQ-BB-01.
- `chrono_dispatch.rs:L1070-1072` — 🟡 `[review]` `match_most_salient` drops candidates whose score errors, so an out-of-range ideal metric makes the candidate permanently invisible instead of failing authoring. REQ-BB-04.
- `chrono_dispatch.rs:L334-342,L312` — 🟡 `[review]` `WaypointTopicEdge.dialogue_transition_asset_id` is stored, never read, and absent from `WaypointStep`/`NodeCallData::WaypointStep`; REQ-BB-06's "speaking its transition line" has no output path.
- `chrono_runtime.rs:L408,L2610` — 🟡 `[review]` the non-repeatable completion lock is per node instance, while §5.4's `completed_non_repeatable_storylets` is dispatcher-global; a once-only storylet completed in one hub can complete again in another. REQ-BB-02.
- `chrono_dispatch.rs:L119-162` — 🟡 `[review]` no `PredicateSubject` reads `state.passions` or `WorldVertex.attributes`; REQ-BB-01's passion and vertex-attribute preconditions are not expressible (tags/metrics only).
- `chrono_adapter.rs:L303` + `chrono_graph_adapter.rs:L305-306` — 🟡 `[review]` `ideal_state_vector` weights are hardcoded 1.0 (spec's `v_j` unexpressible), and salience nodes compile with `fallback_storylet_id: None`, so a fully-gated pool errors `NoSalienceMatch` instead of falling back. REQ-BB-04, §11.3.

**Spec inconsistency to resolve:** REQ-BB-04's formula uses per-dimension weights `v_j`; the §5.4 pseudocode uses unweighted distance with `alpha = 0.05`. The implementation must pick one and the spec should be corrected in the same change.

## Modules 3–4 — Token flow graph and scene sections (REQ-FG / REQ-SC)

- `chrono_runtime.rs:L115` — 🔴 `[code]` `SignalToken` holds a `String target_pin_name` plus two status fields, so `Vec<SignalToken>` is neither fixed-size nor cache-flat. REQ-FG-03 is violated at the type level. Use `u16`/`u32` pin indices.
- `chrono_runtime.rs:L1327,L3080` — 🔴 `[review]` every `ComposerCommand` is tagged with the calling node and arbitration runs per-node, so REQ-SC-06 arbitration across concurrent scene instances does not exist; the later section simply overwrites `actor_pose` and `conflicts` never spans nodes.
- `chrono_runtime.rs:L3024` — 🔴 `[review]` `clamp(0, duration_ms)` panics when `section_duration_ms < 0`, and `FlowGraphAsset::validate` (L545) does not reject negative durations or event times. REQ-SC-01.
- `chrono_runtime.rs:L2078` — 🔴 `[review]` re-entering a completed scene section does not reset `section_clock_ms`, so it fires a phantom completion with zero events and never replays the timeline. REQ-SC-01/05.
- `chrono_runtime.rs:L2490` — 🔴 `[review]` deferral sets `deferred_until_ms = clock + cost` and re-arms on every failure; with `cost > frame_budget_ms` the token livelocks, and when it revives with `time_budget_ms < cost` it errors at L2515. Defer exactly one frame and carry unused budget.
- `chrono_runtime.rs:L3057` — 🟡 `[review]` backward window uses `end > t_end`; spec §7.2 states `ev_end >= t_end`. Events ending exactly at the left edge are dropped.
- `chrono_runtime.rs:L3044` — 🟡 `[review]` one `ActionPart` per event: an event fully inside the window is labelled START only, never END, so REQ-SC-05's START/REGULAR/END decomposition is incomplete.
- `chrono_runtime.rs:L3067` — 🟡 `[review]` backward `local_cursor_time_ms` sign is inverted versus the §7.2 algorithm.
- `chrono_runtime.rs:L1401` — 🟡 `[review]` `beats()` picks the highest `action_key_priority` while L70 documents "lower values win" and L1322 applies ascending. Pin one convention.
- `chrono_runtime.rs:L2178,L2751` — 🟡 `[review]` a latent waiter pushes `NodeCallData::Waiter` on arrival and again on wake, leaving an unmatched inverse-cache entry that makes the next `cancel_and_revert` pop the wrong record. REQ-FG-05.
- `chrono_runtime.rs:L2144` vs `L2224` — 🟡 `[review]` `commit_plan` writes `global_call_history` unconditionally and ignores `mission_scope`, while `record_call` routes scoped nodes into the child history; the two record paths disagree. REQ-FG-05/REQ-CH-04.
- `chrono_runtime.rs:L2507` — 🟡 `[review]` `execute_input` never checks `token.graph_instance_id`; a token minted by another instance can drive this graph. REQ-FG-01/03.

## Modules 5–6 — Call history, invoke, reconcile, rebase (REQ-CH / REQ-GP)

- `chrono_runtime.rs:L3614` — 🔴 `[code]` `replay_history` calls `force_complete`, which hardcodes `"In"` (L2771) and discards `entry.executed_input_pin`; the adapter names fan-in pins `In_0..In_N`, so replaying any diamond-merge history fails `PinNotFound`. REQ-CH-03.
- `chrono_runtime.rs:L785` — 🔴 `[code]` `path_steps` returns `(guid, predecessor.source_pin, node.target_pin)` as `(guid, input_pin, output_pin)`; `rebase_call_history` (L4048-4053) writes those into `executed_input_pin`/`chosen_output_pin`, so spliced entries are invalid even after the BFS fix. REQ-GP-03.
- `chrono_runtime.rs:L3668-3677` — 🔴 `[code]` `reconcile_history` compares `target[position]` against `commits[current[position]]`, but `path_to` includes the root sentinel commit 0, so the first comparison is against an empty default entry. `lca` is always 0 → every reconcile reverts the entire history and replays the target instead of diverging at the LCA. REQ-CH-06.
- `chrono_runtime.rs:L3727` — 🔴 `[code]` `checkout` pushes `record.entry` after `commit_plan` already pushed it (`plan.commit=false` only suppresses the history commit, not the `global_call_history` push at L2137). History doubles on every checkout and the duplication is replayed. REQ-CH-01/06.
- `chrono_runtime.rs:L3710` — 🔴 `[review]` `checkout` reconstructs via `plan()` rather than `force_complete`, so scene timeline events and mission child histories are never replayed, and `plan()` mutates `ctx.state` before the delta comparison (a `CallDataMismatch` leaves partial state). REQ-CH-06.
- `chrono_runtime.rs:L2956-2961` — 🔴 `[review]` cancelling a Mission clears `local_call_history`, so the forward pass of reconcile sees an empty scope and silently skips replaying child effects while still recording the boundary as complete; nested missions cannot replay at all (L2807 passes an empty map). REQ-CH-04/06.
- `chrono_playtest.rs:L528-536` — 🔴 `[review]` the rebased path replays the full history into a session already at post-history state → every delta applied twice after any project save. Reset to the seed state (and pending tokens) before replay. REQ-CH-03.
- `chrono_playtest.rs:L659` — 🔴 `[review]` a rebuilt session never restores pending start tokens or latent state; a save taken at a Choice/QBN/Waiter/scene node loses that interaction after restart, and an empty history replays to zero tokens so `tick` does nothing. REQ-CH-02/03.
- `chrono_playtest.rs:L1500` → `chrono_runtime.rs:L2097` — 🔴 `[review]` `choose` with `storyletId` calls `select_storylet`, which applies any compiled storylet without checking deck membership, repeatability, `is_playable`, preconditions, or the arc gate. A client can force once-only or ineligible content. REQ-BB-01/04.
- `chrono_runtime.rs:L2967` — 🟡 `[review]` direct `cancel_and_revert` edits `global_call_history` but leaves `history.head`/commits on the cancelled entry; a later checkout/reconcile re-cancels it (`RevertStackEmpty` or pops an older same-node record).
- `chrono_runtime.rs:L2198` — 🟡 `[review]` `commit_plan` inserts `last_completed_ms` for every storylet and `cancel_and_revert` never restores it, so a rolled-back storylet stays on cooldown. REQ-CH-06 state-exactness.
- `chrono_runtime.rs:L3092` — 🟡 `[review]` backward scrubbing below `section_duration_ms` leaves `state = Completed` and the completion entry in history while the clock disagrees; the section can never re-emit.

## Integration surface

- `http.rs:L145` — 🔴 `[code]` no HTTP route, MCP op, or FFI function exposes `rollback_mission_to_start`; REQ-CH-04 rewind exists only in Rust tests.
- `chrono_playtest.rs:L121` — 🟡 `[review]` backward scrubbing is unreachable: `frameBudgetMs` rejects negatives and `tick` is the only window entry point. REQ-SC-04 has no consumer.
- `chrono_playtest.rs:L111-241` — 🟡 `[review]` no v2 request carries `expectedRevision` or an idempotency key (v1 choice does), and unknown fields are ignored; a stale client can POST `reconcile` with an old history and rewind newer state, and a retried tick/signal/choose double-advances. REQ-CH-06.
- `chrono_playtest.rs:L432` — 🟡 `[review]` the revision gate trusts the mutable `project_metadata.revision` counter; `import_bundle` restores a bundle's revision verbatim, so different content at the same revision silently replays persisted u16 ids against a different compilation. Compare a compiled ID-space fingerprint. REQ-GP-01.
- `chrono_adapter.rs:L265` — 🟡 `[review]` `required_arc_phases` compiles to a tag mask but nothing seeds `current_dramatic_arc_phase` (no scenario field, no runtime API); arc-gated content is permanently ineligible unless an author adds the same tag. REQ-BB-05.
- `crates/storylet-engine/src/ffi.rs` — 🟡 `[code]` the chrono engine is **not** exposed over the C ABI at all; embedders (the spec's Unreal target) must be Rust. State this limitation in the docs or add FFI.

## Spec §11 verification suites — coverage gaps

The 16 passing tests cover happy paths. Against §11's stated procedures:

- `chrono_v5_acceptance.rs:453` — 🔴 `[review]` TEST_SALIENCE_MONTE_CARLO never exercises the fallback: 99 is a normal pool member and every trajectory seeds a matching gated storylet, so the assertion also passes if the node ignores `fallback_storylet_id`. §11.3.
- `chrono_v5_acceptance.rs:405` — 🔴 `[review]` TEST_INVOKE_EQUIVALENCE's graph is linear (`Mutation→QbnHub→Poi`) with an empty `declared_routes`; the ALL fan-in and the Choice/Waypoint route branch of REQ-CH-05 are untested. The POI completion is never asserted either.
- `chrono_v5_acceptance.rs` (whole file) — 🔴 `[review]` no test calls `rebase_call_history`; REQ-GP-01..03 have zero coverage. (Consistent with the fact that the function has no non-test caller.)
- `chrono_v5_acceptance.rs` (whole file) — 🔴 `[review]` no test calls `process_time_window` with a negative delta and no test populates `SceneComposer.conflicts`; REQ-SC-04/05/06 unverified.
- `chrono_v5_acceptance.rs:115-148` — 🟡 `[review]` TEST_ROLLBACK_IDENTITY: zero-sum link targets the same NPC (rival propagation untested), no threshold rules registered (so "no leaked events" is vacuous), no edges added/removed and no attribute overwritten, empty `MetricRegistry` (no numeric quality rollback), and all 500 calls go through `force_complete` so ExecuteNormal inverse caches are never rolled back.
- `chrono_v5_acceptance.rs:413` — 🟡 `[review]` the 2.0 ms criterion is a single wall-clock sample in a parallel debug build; flaky by construction.
- `chrono_v5_acceptance.rs:608,629` — 🟡 `[review]` TEST_WAYPOINT_DIVERSION force-completes the node before diverting (so the live `OnPlayerDivertTopic` re-route is untested) and diverts to the player-only topic only after the clue exists, so "unlocked via PopularUnrest" is not actually asserted.

## Requirement status matrix

| Requirement | Status |
|---|---|
| REQ-WB-01 | Partial — 4 layers exist; layer not enforced on edges |
| REQ-WB-02 | Partial — LHS/RHS exist, edge binding and add/remove semantics broken |
| REQ-WB-03 | Partial — caches exist, overwrite bug breaks exact reversal |
| REQ-WB-04 | Partial — is_hidden/weight present, empty pool accepted |
| REQ-WB-05 | Partial — default clamp bounds are unbounded |
| REQ-WB-06 | Implemented — zero-sum propagation matches the spec formula |
| REQ-WB-07 | Partial — threshold events exist, direction state inferred from global tag |
| REQ-WB-08 | **Missing** — quality categories and crisis auto-trigger not modelled |
| REQ-WB-09 | Implemented |
| REQ-BB-01 | Partial — tags/metrics only; passions and vertex attributes unqueryable |
| REQ-BB-02 | Partial — order-independent hub works; lock is per-node not global |
| REQ-BB-03 | Implemented — priority + urgency + 4 cluster types |
| REQ-BB-04 | Partial — formula implemented; weights unexpressible, no fallback, errors swallowed |
| REQ-BB-05 | **Not enforced** — empty mask bypasses the gate by design |
| REQ-BB-06 | Partial — Dijkstra + penalties work; transition asset unused; cycle hang |
| REQ-FG-01 | Partial |
| REQ-FG-02 | Implemented |
| REQ-FG-03 | **Broken** — non-flat token, budget/deferral semantics, no cycle cap |
| REQ-FG-04 | Partial — ALL gates deadlock with >1 input pin |
| REQ-FG-05 | Partial — all three endpoints exist; inverse caches have gaps |
| REQ-SC-01 | Partial — clock exists; negative durations panic, re-entry does not reset |
| REQ-SC-02 | Implemented |
| REQ-SC-03 | Implemented |
| REQ-SC-04 | Partial — boundary differs from spec; no consumer |
| REQ-SC-05 | Partial — START/END decomposition incomplete |
| REQ-SC-06 | **Missing** — no cross-instance arbitration |
| REQ-CH-01 | Partial — history model exists; checkout duplicates entries |
| REQ-CH-02 | Partial — save carries history + seeds, no snapshots |
| REQ-CH-03 | **Broken** — replay discards the recorded input pin and pending latent state |
| REQ-CH-04 | Partial — isolation works; rollback unreachable outside tests |
| REQ-CH-05 | Partial — invoke works on linear graphs; ALL/route branches unverified |
| REQ-CH-06 | **Broken** — LCA is always 0, mission tail loss, checkout double-push |
| REQ-GP-01 | **Unreachable** — revision pin rejects every rebase |
| REQ-GP-02 | Unverified — no caller, no test |
| REQ-GP-03 | **Broken** — BFS one-hop only, spliced pins inverted |

## Recommended fix order

1. `chrono_runtime.rs:2479` — add a per-tick execution cap and reject zero-cost cycles in `validate()`. (Hang, blocks all testing.)
2. `chrono_runtime.rs:1682` — write `arrived_input_pins` back so ALL gates complete.
3. `chrono_runtime.rs:2400` / `L2490` — carry `frame_budget_ms` into the seed token and defer exactly one frame; then re-run the costed-node probe.
4. `chrono_runtime.rs:3614` — pass `executed_input_pin` into `force_complete_with_input` on replay and reconcile.
5. `chrono_runtime.rs:3668` / `L3727` / `L3710` — fix the LCA offset, drop the duplicate push, and rebuild via `force_complete`.
6. `chrono_runtime.rs:721` / `L785` — rebuild bridge BFS over all edges and store `(entering target_pin, leaving source_pin)`.
7. `chrono_playtest.rs:432` — let the revision pin adopt a rebased history (fingerprint check instead of equality).
8. Then the REQ-SC-06 arbitration, REQ-WB-08 menace crisis, and the §11 test-procedure upgrades.
