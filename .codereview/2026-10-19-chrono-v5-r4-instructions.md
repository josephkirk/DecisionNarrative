# ChronoFlow v5 — r3 verification and open-issue instructions

Date: 2026-10-19
Spec: `docs/featuresbacklog/chrono-decision-engine/chronoflow_narrative_engine_technical_specification_v5.md`
Previous review: `.codereview/2026-10-19-chrono-v5-conformance-r3.md`

Verified this pass:
- `cargo test` (storylet-engine): 99 pass. `cargo check --all-targets`: no warnings (`_locations_count` fixed).
- `cargo test` (storylet-engine-service, `target/review-tests`): 57 pass.
- `try_is_eligible` / `try_is_playable` / `try_query_qbn_deck` / `try_match_most_salient` exist (`chrono_dispatch.rs:680,824,850,1130`) and are used in the runtime deck/selection/coverage/invoke paths (`chrono_runtime.rs:2200,2916,2967,3053`) with `GraphError::Dispatch`.
- 9 v2 request types carry `deny_unknown_fields`; misspelling test present.
- Priority docs and `beats()` agree ("higher wins", `chrono_runtime.rs:75,1574`).
- Latency assertion is gated to optimized builds with a 50 ms bound (`chrono_v5_acceptance.rs:443`); `signal_wake_leaves_one_inverse_record` passes.
- Identity test now has `MetricRegistry`, `SetMetric`, a second NPC, cross-NPC zero-sum and a passion threshold rule.

r3 findings are closed. Below are the remaining items, ordered, with implementation instructions.

---

## 1. 🔴 Flat `SignalToken` (REQ-FG-03) — staging plan

**Current shape** (`chrono_runtime.rs:120`): `token_id: u64, graph_instance_id: u64, target_node_guid: u32, target_pin_name: String, time_budget_ms: i64, is_active: bool, deferred_until_ms: i64, is_deferred: bool`. With `String` the struct is 56+ bytes plus a heap allocation per token; the "flat ring buffer" requirement is not met.

**Constraint that shapes the design:** tokens are persisted verbatim in `PendingState.tokens` (`chrono_playtest.rs:1185`) and exposed as JSON by `tokens_value` (`:847`). Changing the field breaks live sessions.

### Stage 1 — index the pin (do this first, no persistence break)

1. Add to `SignalToken`: `pub target_pin_index: u16` replacing `target_pin_name: String`.
2. Add `FlowGraphAsset::input_pin_index(&self, node_guid: u32, name: &str) -> Option<u16>` returning the position in `FlowNodeDef.input_pins`. Add `input_pin_name(&self, node_guid, index) -> Option<&str>` for display.
3. `enqueue_token` (`:2577`) resolves name → index once; return `GraphError::PinNotFound` when absent. `emit_tokens` (`:2563`) and the deferred-target path at `:2495` pass the resolved index.
4. `execute_input` (`:2780`) gains `input_pin_index: u16`; resolve the name internally for `find_pin`/`plan`. Keep a thin public `execute_input_by_name` wrapper so external callers and tests do not change.
5. Persistence: add a service-side DTO
   ```rust
   #[derive(Serialize, Deserialize)]
   struct PersistedToken { token_id: u64, graph_instance_id: u64, target_node_guid: u32, target_pin_name: String, time_budget_ms: i64, is_active: bool, deferred_until_ms: i64, is_deferred: bool }
   ```
   `PendingState.tokens: Vec<PersistedToken>`; convert in `instance_pending_state` (write) and `restore_pending` (read) using the session asset. `tokens_value` reads `input_pin_name(...)` and emits `target_pin_name` as today, so HTTP JSON is unchanged.
6. Engine tests construct `SignalToken` literals in ~8 places (e.g. `chrono_runtime.rs:4947`, `chrono_v5_acceptance.rs:1390`); add a `SignalToken::at_pin(node, name, budget)` helper and use it, so future shape changes touch one function.

### Stage 2 — pack the struct (after stage 1 is green)

- `target_node_guid: u32`, `target_pin_index: u16`, `flags: u16` (`is_active` bit 0, `is_deferred` bit 1), `time_budget_ms: i32`, `deferred_until_ms: i32`, `token_id: u64`, `graph_instance_id: u32` (instance ids are allocated sequentially; truncate with a checked conversion, or keep `u64` if you prefer to preserve the id space — that yields 32 bytes).
- Acceptance check: `#[test] fn signal_token_is_flat() { assert!(size_of::<SignalToken>() <= 32); assert_eq!(align_of::<SignalToken>(), 8); }` plus a `Vec<SignalToken>` allocation-count assertion is optional.

**Risk:** deferred/tick ordering is sensitive — after stage 1, run the full engine suite and re-run the costed-node and zero-cost-cycle probes from the r2 review.

---

## 2. 🔴 Cross-section `SceneComposer` arbitration (REQ-SC-06)

**Problem.** `tick` (`chrono_runtime.rs:2715-2730`) calls `process_time_window_inner` per active `SceneSection`, and each call runs `execute_sorted_action_parts(node_guid, parts, …)` (`:1483`). Arbitration groups by `combiner_entity_path`, but only within that one node's parts, so two sections acting on the same actor in one frame both win; `conflicts` never spans nodes.

**Design — split planning from application.**

1. Add
   ```rust
   struct SectionFrame {
       node_guid: u32,
       is_reverse: bool,
       parts: Vec<ActionPart>,
       t_start: i64,
       t_end: i64,
       just_completed: bool,
       reopened: bool,
       previous_poses: Vec<ScenePoseUndo>,
   }
   ```
2. Extract `fn plan_section_frame(&self, node_guid, delta_ms) -> Result<SectionFrame, GraphError>` from the first ~90 lines of `process_time_window_inner` (`:3430`). It must not mutate `section_clock_ms`, `call_data`, history or `composer`.
3. Extract `fn apply_section_frame(&mut self, frame: SectionFrame, record_completion, emit_completion, budget) -> Result<Vec<SignalToken>, GraphError>` from the remainder.
4. Add `SceneComposer::execute_sorted_action_parts_multi(&mut self, owners: &[(u32, &[ActionPart])], is_reverse: bool) -> Result<Vec<ComposerCommand>, GraphError>`:
   - build commands tagged with their owning `node_guid`;
   - sort by `(action_key_priority, combiner_entity_path, node_guid, event_id)`;
   - group by entity, pick winner with the existing `beats()`;
   - push **one** `ComposerConflict` per entity with all rejected commands, sorted;
   - apply winners.
   Reimplement the single-node `execute_sorted_action_parts` as a one-entry call to the new function (no duplicated logic).
5. In `tick`: collect frames for every active section in node-GUID order, call `execute_sorted_action_parts_multi` once, then apply frames — forward in GUID order, reverse (`delta < 0`) in descending GUID order so pose restoration is symmetrical. `process_time_window` (public, `:3422`) keeps working as a single-frame wrapper.
6. `force_complete` / replay paths that call the single-node API must keep their current semantics; only the per-frame tick path becomes cross-section.

**Acceptance test** (new, `chrono_v5_acceptance.rs`):
two active sections (GUID 10 and 11) with events on the same `combiner_entity_path` inside one frame, priorities 2 and 4. Assert: exactly one command applied, `conflicts.len() == 1`, `conflicts[0].winner.node_guid == 11`, `rejected[0].node_guid == 10`; then scrub backward one frame and assert both prior poses are restored.

**Risk:** `conflicts` is an append-only log; running per frame will accumulate. Cap or dedupe by `(entity, winner.event_id, rejected.event_ids)` if tests grow.

---

## 3. 🔴 `chrono_state.rs` correctness items

File unchanged since pass 2; all five still present.

### 3.1 Greedy LHS matching (`:1119`)
`match_graph_pattern` binds each vertex to the first candidate and never reconsiders, so a pattern whose second vertex needs a different first assignment returns `PatternNotMatched`.

Implement backtracking:
```rust
fn match_graph_pattern(&self, pattern) -> Result<GraphMatch, ChronoStateError> {
    let mut chosen = Vec::with_capacity(pattern.vertices.len());
    let mut used = BTreeSet::new();
    let mut budget = 4096usize;
    if self.bt_vertices(pattern, 0, &mut used, &mut chosen, &mut budget)? { self.bt_edges(pattern, chosen) }
    else { Err(ChronoStateError::PatternNotMatched { pattern_index: 0 }) }
}
```
- `bt_vertices`: try each vertex in declaration order that satisfies layer / literal id / attribute predicates and is unused; recurse; undo on failure. When an edge pattern references `GraphBinding::Vertex(i)`, prune candidates that cannot satisfy it.
- `bt_edges`: after vertices bind, match edges with backtracking too (a literal `edge_id` narrows to one candidate; relation/source/target may still conflict across patterns).
- Add `ChronoStateError::PatternSearchExhausted` for the budget and a `Display` arm.
- Tests: disconnected candidate pair (first vertex must be second in declaration order), and a 3-vertex chain where the second candidate arrangement is required.

### 3.2 Empty passion pool (`:2080`)
Add at the top of `validate_star_npc`:
```rust
if npc.micro_passions.is_empty() { return Err(ChronoStateError::EmptyPassionPool { npc_id: npc.npc_id }); }
```
No existing test builds an empty pool (verified), so this is safe. Add the variant + `Display` + one negative test.

### 3.3 Macro clamp bounds (`:1949`)
`raw.clamp(npc.macro_bounds.min, npc.macro_bounds.max)` with `MetricBound::default()` unbounded. Clamp to the intersection with the spec range:
```rust
let min = npc.macro_bounds.min.max(-100.0);
let max = npc.macro_bounds.max.min(100.0);
if min > max { return Err(ChronoStateError::InvalidMetricBound { index: npc.npc_id }); }
macro_plan.insert(npc.npc_id, raw.clamp(min, max));
```
Do **not** change `MetricBound::default()` — it is shared with metrics. Test: macro sum of 400 with default bounds yields exactly 100.

### 3.4 Threshold suppression via shared tag (`:1955-1980`)
The rule both owns the bit and infers its prior state from it, so any external write to that bit suppresses or duplicates the crossing event. Compute crossings from the value transition only:
```rust
let old = state.macro_values.get(rule.npc_id as usize).copied().unwrap_or(-100.0);
let new = macro_plan.get(&rule.npc_id).copied().unwrap_or(old);
let crossed_up   = old <  rule.trigger_value && new >= rule.trigger_value;
let crossed_down = old >= rule.trigger_value && new <  rule.trigger_value;
```
Set/clear the bit from the crossing, and emit `PassionThresholdEvent` whenever `crossed_up || crossed_down`. Document that the bit is rule-owned. Test: write the tag bit externally, mutate the passion across the threshold, assert exactly one activate event and that the bit ends in the correct state.

### 3.5 `MacroThresholdRule` naming (`:381`)
It indexes `state.metrics` (`rule.star_macro_id`) but is named for macro passions; the adapter uses it for MENACE (`chrono_adapter.rs:185`). Rename to `MetricThresholdRule` with field `metric_id`, rename `PassionRegistry::with_threshold_rules` callers accordingly, and update the adapter + tests. If macro-passion thresholds are wanted later, add a separate `MacroThresholdRule { npc_id, passion_trigger, … }` rather than overloading this one.

---

## 4. 🟡 §11 test procedures still shallower than the spec

### 4.1 `TEST_INVOKE_EQUIVALENCE` (`chrono_v5_acceptance.rs:292`)
Current graph is linear `Mutation → QbnHub → Poi`, the manual side uses `force_complete`, and `declared_routes` is empty. Extend:
1. Add a `LogicAll` fan-in: two upstream branches (one from the QBN hub exit, one from a `Choice` node) both feeding the POI.
2. Drive the manual side through normal execution (`start` + `tick`, selecting via `present_deck`/`choose`) instead of `force_complete`.
3. Pass `declared_routes = { choice_node_guid: "choice_b" }` to invoke and assert equality with the manual run.
4. Assert `instance.node(poi_guid).state == Completed` and that the final history entry is the POI.

### 4.2 `TEST_WAYPOINT_DIVERSION` (`:559`)
Start the node through `start` + `tick` so it is `ActiveLatent` before diverting, and add a negative case: divert into `Revolution` before `PopularUnrest` exists and assert the diversion is refused (`GraphError::…`), then reveal the clue and assert it is accepted.

---

## 5. 🟡 Spec/implementation divergences — decide and record

### 5.1 Bonus-tag popcount (`chrono_dispatch.rs:1085`)
`salience_score` adds `popcount(global_tags ∩ salience_bonus_tags)` to specificity; REQ-BB-04 defines specificity as predicate weights only. Implementation is deterministic and tested, so the cheaper correct move is to **amend the spec**: add the term to §5.4 step C and to the REQ-BB-04 formula as an explicit extension, with a note that bonus tags carry weight 1.0 each. Do not silently keep the divergence.

### 5.2 Ideal-vector weights (`chrono_adapter.rs:333`)
`IdealMetricTarget.weight` exists in the engine but the adapter hardcodes `1.0`. Add to `ChronoStoryletSettings` (`contracts.rs:223`):
```rust
#[serde(default)]
pub ideal_state_weights: BTreeMap<String, f64>,
```
and use `settings.ideal_state_weights.get(target).copied().unwrap_or(1.0)`, validating finite and `> 0`. Add one test proving a weight of 4.0 changes which candidate wins when distances are otherwise equal.

---

## Execution order and gates

1. 3.2, 3.3, 3.5 (small, isolated) → `cargo test` engine + service.
2. 3.4, then 3.1 (backtracking is the largest state change) → add the three new state tests.
3. 2 (scene arbitration) → engine suite + new conflict test.
4. 1 stage 1 (token index) → engine suite + service suite; re-run the r2 costed-node/cycle probes.
5. 1 stage 2 (packing) → size assertion.
6. 4.1/4.2 test upgrades, 5.1 spec amendment, 5.2 weights.

Do not start the token refactor before 2 lands: both touch `chrono_runtime.rs` and the tick loop, and doing them together makes bisection hard.

Done check after all steps: engine and service suites green, no warnings, and a manual `bun run play` smoke that opens a chrono playtest, ticks a scene, scrubs back one frame, and reloads the page (persistence path exercised by the new token DTO).
