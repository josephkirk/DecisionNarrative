# Non-Linear Rework Implementation Plan (Space · Time · Participants · Play)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rework DecisionNarrative into one deterministic run kernel. The kernel models participants, knowledge with provenance, a canon chronicle, a time-varying map and a world clock. Authors play it through a simple **Play** view and check it with a bot explorer. Human authoring and approval stay the source of truth.

**Architecture:**
- **One kernel.** Chrono (`chrono_*` in `crates/storylet-engine`) becomes the only runtime. A new `RunKernel` in the engine crate wraps `FlowGraphInstance` + `WorldStateTable` plus new dense tables: participants, knowledge, chronicle and telling, space, clock.
- **Service.** It persists only `(start, seed, actions[])` and rebuilds a run by deterministic replay. It exposes one `/api/v4/runs` API and matching MCP `run.*` operations.
- **Studio.** It gains an MVVM `Play` view and the authoring fields for the new concepts. The legacy deck runtime (`StoryletRuntime`, `/api/v3/playtests`, `playtest.*`) and the ChronoFlow debug endpoints are removed once Play reaches parity.

**Tech Stack:**
- Rust 2024 (≥1.90): engine and service (Axum, Tokio, rusqlite)
- Vue 3 + Vite + Vue Router, using MVVM view-models
- Bun ≥1.3.14 for TS tests and builds; the `ux` CLI for the frontend gates

**Spec:**
- `docs/design/proposal-human-first-nonlinear-narrative.md` (system)
- `docs/design/playtest-ui.md` (Play UI)
- `DESIGN.md` (visual contract)
- Evidence: `examples/proposal-weirs-crossing/explore.ts`

## Global Constraints

- Engine crate: no I/O, HTTP, SQLite or model dependency (README "Architecture").
- **No model calls anywhere in engine, service or studio.** LLM help enters only as an external MCP agent (proposal §2.4, §4.8). Runtime LLM adapters are out of scope for this plan.
- Determinism: same content revision + seed + actions → identical run state and view (proposal §0, TENNET FR-05).
- Every mutating HTTP/MCP call carries `expectedRevision` and `idempotencyKey`. A stale or duplicate call never partially mutates (storylet-studio.md "Failure handling").
- **New project data is additive with `#[serde(default)]` / optional TS fields.** An existing v5 project opens unchanged as a solo, timeless run with one implicit participant. No schema version bump and no migration code.
- The past is fixed: `Reveal` only reads canon. Changing the past is a history fork (proposal §6.1).
- Knowledge changes only through an in-world event with a source event index (proposal §3.2).
- An edge with a schedule must declare `on_close`. Validation rejects it otherwise (proposal §4.2 and §8.2).
- UI follows `DESIGN.md`:
  - 0 px radius, hairline borders
  - no glow and no `backdrop-filter`
  - EB Garamond for prose, Geist for UI
  - 44×44 px minimum targets
  - visible labels on every control
  - `ux lint` passes with no high-severity findings
- MVVM: views contain no fetch calls. All API access goes through `src/web/studio/api.ts` and a view-model in `src/web/studio/view-models/`.
- Use the existing test commands:
  - `cargo test --manifest-path crates/storylet-engine/Cargo.toml --all-targets`
  - `cargo test --manifest-path crates/storylet-engine-service/Cargo.toml`
  - `bun test`
  - `bun run typecheck`
  - `bun run build`

**Decisions taken from proposal §10 (defaults; the designer can override before Phase 3):**
1. The past is fixed.
2. The first slice is solo plus hot-seat co-op on one screen.
3. No runtime LLM.
4. Shared consequence over per-player arcs.

## Review Focus

1. **Content edited while a run is open:** the run is marked `stale`. Reload replays and reports `diverged_at`. It never shows an old scene silently. Owner: Task 2.2 (`reload_reports_divergence_after_choice_removed`).
2. **Double-clicked choice or retried request:** the same idempotency key returns the same result and appends exactly one step. Owner: Task 2.2 (`duplicate_act_appends_one_step`).
3. **A v5 project with no Participant traits, no clock and no connections:** Play still works with one implicit participant "Player" and no time strip. Owner: Task 1.2 (`v5_world_json_loads_with_implicit_player`) and Task 2.3 (`timeless_project_hides_clock`).
4. **Rewinding, then acting:** creates a new branch, and the old branch stays selectable. Owner: Task 2.1 (`act_after_rewind_creates_branch`).
5. **A scene with more than 9 choices or very long prose:** the keys 1–9 map to the first nine choices, the rest are clickable, and the column scrolls without moving the strip. Owner: Task 2.3 (`keyboard_maps_only_first_nine_choices`).

---

## File structure

| Path | Responsibility | Phase |
|---|---|---|
| `crates/storylet-engine/src/chrono_participants.rs` *(new)* | `ParticipantTable`, `KnowledgeTable`, `KnowKind`, `Controller` | 1 |
| `crates/storylet-engine/src/chrono_state.rs` *(modify)* | `WorldStateTable` holds the participant/knowledge tables; `BijectiveDelta` records knowledge and moves | 1 |
| `crates/storylet-engine/src/chrono_runtime.rs` *(modify)* | new `NodeKind`s: `Learn`, `MoveParticipant`, `Knows`, `Reveal`; `acting_participant` | 1, 3 |
| `crates/storylet-engine/src/chrono_dispatch.rs` *(modify)* | `explain_ineligible` | 2 |
| `crates/storylet-engine/src/run.rs` *(new)* | `RunKernel`, `RunContent`, `RunStart`, `RunAction`, `RunView`, `NearMiss`, `RunError` | 2 |
| `crates/storylet-engine/src/chrono_chronicle.rs` *(new)* | `ChronicleTable`, `TellingTable` | 3 |
| `crates/storylet-engine/src/chrono_space.rs` *(new)* | `Connection`, `OnClose`, `Clock`, `ClockState`, `MoveOption` | 4 |
| `crates/storylet-engine/src/chrono_explore.rs` *(new)* | `explore`, `ExploreConfig`, `ExploreReport`, personas, invariants | 5 |
| `crates/storylet-engine/tests/support/weirs_crossing.rs` *(new)* | Rust builder for the example world (shared fixture) | 1–5 |
| `crates/storylet-engine-service/src/runs.rs` *(new)* | run persistence, replay, HTTP handlers, MCP `run.*` | 2 |
| `crates/storylet-engine-service/src/contracts.rs` *(modify)* | new system traits; `EventRecord` and `StoryletRecord` additive fields | 3–6 |
| `crates/storylet-engine-service/src/narrative_graph.rs`, `chrono_graph_adapter.rs` *(modify)* | validate and compile the new graph nodes | 1, 3 |
| `src/web/studio/runModel.ts` *(new)* | pure functions over `RunView` (keyboard map, chronicle rows, near-miss text) | 2–4 |
| `src/web/studio/view-models/run.ts` *(new)* | Play view-model (state + actions) | 2 |
| `src/web/studio/views/PlayView.vue` *(new)* | Play shell | 2 |
| `src/web/studio/components/play/*.vue` *(new)* | `PlayWhereWhen`, `PlayStory`, `PlayTimeline`, `PlayInspect`, `PlayStart`, `CoverageReport` | 2–5 |

---

## Phase 0 — Baseline (≈ 2–3 hours)

### Task 0.1: Fix the drift recorded in proposal §2.3

**Files:**
- Modify: `docs/design/chrono-decision-engine.md` (§6 "Latest recorded result")
- Modify: `README.md` ("Playtest & Director" bullet)
- Modify: `examples/native-host/storylet-project.json`
- Test: `crates/storylet-engine/tests/project_bundle.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn native_host_example_project_loads() {
    let json = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/native-host/storylet-project.json")).unwrap();
    let project: storylet_engine::project::Project = serde_json::from_str(&json).expect("example must match current schema");
    assert!(project.storylets.contains_key("read-receipt"));
}
```
Use the project type that `ffi.rs` loads. Check `project.rs` for its exact name.

- [ ] **Step 2: Run it and confirm it fails.** Run `cargo test --manifest-path crates/storylet-engine/Cargo.toml --test project_bundle native_host_example_project_loads`. Expected: FAIL (unknown `set_quality` / `qualities`).
- [ ] **Step 3: Rewrite the example JSON** to the current schema: `properties` and `{"kind":"set_property","key":"clue.receipt","value":true}`. Then run `dotnet run` in `examples/native-host-dotnet` if the .NET SDK is present; otherwise note "not run" in the commit body.
- [ ] **Step 4: Update the docs.**
  - Put the measured test counts in chrono-decision-engine §6. Get them from `cargo test … 2>&1 | grep "test result"` and sum them.
  - Change the README Playtest bullet to: "Playtest runs through HTTP/MCP; the Play view is being rebuilt (docs/design/playtest-ui.md)."
- [ ] **Step 5: Run the test again.** Expected: PASS.
- [ ] **Step 6: Commit** with `git commit -m "docs: fix test counts, playtest claim, and stale native example"`.

---

## Phase 1 — Participants and knowledge (≈ 1 week)

### Task 1.1: Participant and knowledge tables

**Files:**
- Create: `crates/storylet-engine/src/chrono_participants.rs`, and register it in `lib.rs`
- Test: `crates/storylet-engine/tests/participants.rs`

**Interfaces (produces):**
```rust
pub type PartIx = u16; pub type FactIx = u16; /* FactIx = GameplayTagRegistry tag index under root "fact" */
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)] #[serde(rename_all="snake_case")]
pub enum KnowKind { Evidence, Witnessed, Told, Rumor, Recollection, Testimony, Inference }
impl KnowKind { pub fn is_confirmed(self) -> bool /* false for Rumor, Testimony */ }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)] #[serde(tag="kind", rename_all="snake_case")]
pub enum Controller { Human { seat: u8 }, Rule }
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ParticipantTable { pub keys: Vec<String>, pub entity: Vec<u16>, pub controller: Vec<Controller>, pub location: Vec<Option<u16>> }
impl ParticipantTable {
  pub fn implicit_player(location: Option<u16>) -> Self;          // key "player", entity u16::MAX, Human{seat:0}
  pub fn push(&mut self, key: &str, entity: u16, controller: Controller, location: Option<u16>) -> PartIx;
  pub fn index_of(&self, key: &str) -> Option<PartIx>;
  pub fn len(&self) -> usize;
  pub fn humans(&self) -> impl Iterator<Item = PartIx> + '_;     // seat order
  pub fn colocated(&self, p: PartIx) -> impl Iterator<Item = PartIx> + '_; // same Some(location), excluding p
  pub fn set_location(&mut self, p: PartIx, to: Option<u16>) -> Option<u16>; // returns previous
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeRow { pub holder: PartIx, pub fact: FactIx, pub kind: KnowKind, pub source_event: u32 }
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeTable { pub rows: Vec<KnowledgeRow> }        // sorted by (holder, fact); binary search
impl KnowledgeTable {
  pub fn learn(&mut self, row: KnowledgeRow) -> bool;            // false if (holder,fact) already known: first source wins
  pub fn knows(&self, holder: PartIx, fact: FactIx) -> Option<&KnowledgeRow>;
  pub fn forget(&mut self, holder: PartIx, fact: FactIx) -> Option<KnowledgeRow>; // used only by revert
  pub fn rows_for(&self, holder: PartIx) -> &[KnowledgeRow];
}
```

- [ ] **Step 1: Write the failing tests** in `tests/participants.rs`:
  - `learn_keeps_first_source`: learn `(0, 5, Rumor, 3)`, then `(0, 5, Evidence, 9)`. The second call returns `false`, and `knows(0,5).unwrap().source_event == 3`.
  - `colocated_excludes_self_and_none`: three participants at `Some(1)`, `Some(1)`, `None`. `colocated(0)` yields only `1`.
  - `implicit_player_is_single_human`: `len()==1`, `humans()` yields `[0]`, and `keys[0]=="player"`.
  - `rumor_and_testimony_are_unconfirmed`: `!Rumor.is_confirmed() && !Testimony.is_confirmed() && Evidence.is_confirmed()`.
- [ ] **Step 2: Run them and confirm they fail.** Run `cargo test --manifest-path crates/storylet-engine/Cargo.toml --test participants`. Expected: FAIL (module missing).
- [ ] **Step 3: Implement** `chrono_participants.rs` with the interface above. Keep `rows` sorted and use `binary_search_by_key(&(holder, fact))`.
- [ ] **Step 4: Run the tests again.** Expected: 4 passed.
- [ ] **Step 5: Commit** with `git commit -m "feat(engine): participant and knowledge tables"`.

### Task 1.2: World state and reversible deltas

**Files:**
- Modify: `crates/storylet-engine/src/chrono_state.rs`: `WorldStateTable` (line ~685), `BijectiveDelta` (line ~1381) and their apply/revert functions
- Test: `crates/storylet-engine/tests/participants.rs`

**Interfaces:**
- Consumes: Task 1.1.
- Produces:
  ```rust
  // WorldStateTable gains:
  #[serde(default)] pub participants: ParticipantTable,   // empty in old JSON -> normalized by `ensure_participants()`
  #[serde(default)] pub knowledge: KnowledgeTable,
  pub fn ensure_participants(&mut self);                   // if participants.len()==0 => implicit_player(current_location_entity)
  pub fn active_location(&self, p: PartIx) -> Option<u16>;
  pub fn apply_learn(&mut self, row: KnowledgeRow, delta: &mut BijectiveDelta);
  pub fn apply_move(&mut self, p: PartIx, to: Option<u16>, delta: &mut BijectiveDelta);
  // BijectiveDelta gains:
  #[serde(default)] pub knowledge_added: Vec<(PartIx, FactIx)>,
  #[serde(default)] pub participant_moves: Vec<(PartIx, Option<u16>, Option<u16>)>, // (p, from, to)
  ```
- Rule: `current_location_entity` stays a projection of participant 0, so existing nodes and tests keep working. `apply_move(0, …)` also updates it and `current_location_tag_mask`.

- [ ] **Step 1: Write the failing tests:**
  - `learn_then_revert_restores_world`: clone the world, `apply_learn`, revert the delta with the existing revert entry point, and assert `world == clone`.
  - `move_then_revert_restores_location_projection`: `apply_move(0, Some(2))` sets `current_location_entity == Some(2)`, and revert restores the original.
  - `v5_world_json_loads_with_implicit_player`: deserialize `serde_json::to_string(&WorldStateTable::new())` with the `participants`/`knowledge` keys removed, call `ensure_participants()`, and assert `participants.len()==1`.
- [ ] **Step 2: Run them and confirm they fail.** Run `cargo test … --test participants`. Expected: FAIL to compile (missing fields).
- [ ] **Step 3: Implement the fields and functions.** Extend `BijectiveDelta::is_empty` and the revert path to undo `knowledge_added` (`forget`) and `participant_moves` (in reverse order).
- [ ] **Step 4: Run all engine tests.** Run `cargo test --manifest-path crates/storylet-engine/Cargo.toml --all-targets`. Expected: all pass, including the existing chrono_v5_acceptance.
- [ ] **Step 5: Commit** with `git commit -m "feat(engine): world state carries participants and knowledge with reversible deltas"`.

### Task 1.3: Graph nodes `Learn`, `MoveParticipant`, `Knows` and witnessing

**Files:**
- Modify: `crates/storylet-engine/src/chrono_runtime.rs` (`NodeKind` at line ~277, its execution and `CancelAndRevert`, `FlowGraphInstance`)
- Test: `crates/storylet-engine/tests/participants.rs`

**Interfaces (produces):**
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)] #[serde(rename_all="snake_case")]
pub enum Visibility { Private, CoLocated }
pub enum HolderRef { Acting, Participant(PartIx) }
NodeKind::Learn { fact: FactIx, kind: KnowKind, visibility: Visibility }   // MUTATOR
NodeKind::MoveParticipant { location_entity: u16 }                          // MUTATOR, acting participant
NodeKind::Knows { holder: HolderRef, fact: FactIx, confirmed_only: bool }   // boolean data node
// FlowGraphInstance gains: #[serde(default)] pub acting_participant: PartIx
```
- `Learn` sets `source_event` to the index of the call-history entry that the node's completion appends.
- With `CoLocated`, every participant in `colocated(acting)` at execution time gets the same fact with `KnowKind::Witnessed` and the same source event.

- [ ] **Step 1: Write the failing tests:**
  - `learn_records_call_history_source`: a graph `Start→Learn(fact 7, Evidence, Private)→End`, ticked to completion. `knows(0,7).source_event` equals the index of the Learn entry in `call_history()`.
  - `colocated_learn_creates_witness_rows`: participants 0 and 1 are at location 3 and participant 2 is at location 4. Run `Learn(CoLocated)` acting as 0. Assert participant 1 has kind `Witnessed` and participant 2 has none.
  - `knows_confirmed_only_rejects_rumor`: participant 0 holds fact 7 as `Rumor`. `Knows{Acting,7,true}` evaluates `false`, and with `false` it evaluates `true`.
  - `cancel_and_revert_learn_removes_rows`: roll back the Learn call, and `knows` returns `None` for all holders.
- [ ] **Step 2: Run them and confirm they fail.** Run `cargo test … --test participants`. Expected: FAIL.
- [ ] **Step 3: Implement the three node kinds** with categories `MUTATOR`, `MUTATOR` and `LOGIC_GATE` (data), and implement `ExecuteNormal`/`ForceComplete`/`CancelAndRevert` through `apply_learn`/`apply_move` and the node's cached delta.
- [ ] **Step 4: Run all engine tests.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "feat(engine): learn/move/knows nodes with co-presence witnessing"`.

### Task 1.4: Author, validate and compile the new nodes

**Files:**
- Modify: `crates/storylet-engine-service/src/narrative_graph.rs` (`NarrativeNodeData` enum at line ~157, labels at line ~442, validation at line ~811)
- Modify: `crates/storylet-engine-service/src/chrono_graph_adapter.rs` (compile at line ~414)
- Modify: `src/web/studio/narrativeGraphTypes.ts` (node type registry), and `src/web/studio/components/PropertyInspector.vue` (fields)
- Modify: `crates/storylet-engine-service/src/contracts.rs` (`SYSTEM_TRAIT_TEMPLATES` at line ~522): add `participant`
- Test: `crates/storylet-engine-service/tests/local_service.rs`, `tests/web/narrative-graph-nodes.test.ts` *(new)*

**Interfaces:**
- Narrative node data, as JSON `type`s:
  - `learn { fact_tag: string, kind: KnowKind, visibility: "private"|"co_located" }`
  - `knows { holder: "acting"|<entity id>, fact_tag: string, confirmed_only: bool }` (Preconditions and Story graphs)
- The existing `go_to_location` now compiles to `MoveParticipant` for the acting participant. Its authoring is unchanged.
- System trait `participant` (allow_multiple: false), with properties:
  - `controller`: String, `"human"`/`"rule"`, default `"human"`
  - `seat`: Int, default `0`
  - `start_location`: Entity, default `null`
- Compile: every entity with the `participant` trait becomes a participant row, in `(controller human first, seat, entity id)` order. If none exist, use `implicit_player`.
- Fact tags must be under the `fact.` root. Validation error text: `` Learn node "<label>": fact tag "<tag>" must be under "fact." ``.

- [ ] **Step 1: Write the failing tests:**
  - Rust `learn_node_requires_fact_root`: saving a storylet whose `learn` node has the tag `clue.x` returns a validation error containing `must be under "fact."`.
  - Rust `participant_trait_compiles_rows_in_seat_order`: two characters with `participant` seats 1 and 0. The compiled `WorldStateTable.participants.keys` lists seat 0 first.
  - TS `narrative-graph-nodes.test.ts`: the registry contains `learn` and `knows`; `knows` is allowed in both `preconditions` and `story` modes; `learn` is allowed only in `story`.
- [ ] **Step 2: Run them and confirm they fail.** Run `cargo test --manifest-path crates/storylet-engine-service/Cargo.toml learn_node participant_trait` and `bun test tests/web/narrative-graph-nodes.test.ts`. Expected: FAIL.
- [ ] **Step 3: Implement:**
  - the enum variants
  - the validator
  - compilation to `NodeKind`s through the tag registry
  - the TS registry entries and inspector fields: a tag picker filtered to `fact.`, a kind select, a visibility select
  - the system trait spec
- [ ] **Step 4: Run all service tests, `bun test`, and `bun run typecheck`.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "feat: author learn/knows nodes and participant trait"`.

---

## Phase 2 — Run kernel, Run API, Play MVP (≈ 2 weeks)

### Task 2.1: `RunKernel`

**Files:**
- Create: `crates/storylet-engine/src/run.rs`
- Modify: `crates/storylet-engine/src/chrono_dispatch.rs`: add `explain_ineligible`
- Modify: `crates/storylet-engine-service/src/chrono_adapter.rs`: `CompiledChronoDispatch` becomes a re-export or alias of `RunContent`
- Test: `crates/storylet-engine/tests/run_kernel.rs`

**Interfaces (produces):**
```rust
pub struct RunContent { pub registry: GameplayTagRegistry, pub dispatcher: UnifiedDispatcher,
  pub passion_registry: PassionRegistry, pub storylet_titles: BTreeMap<u16, String>,
  pub entity_names: BTreeMap<u16, String>, pub start_storylet: u16, pub initial_world: WorldStateTable,
  pub content_revision: u64 }                       // moved from service CompiledChronoDispatch (non-I/O fields)
#[serde(tag="kind", rename_all="snake_case")]
pub enum RunStart { Beginning, Storylet { storylet: u16 } } // Storylet => invoke_story_to_poi
#[serde(tag="kind", rename_all="snake_case")]
pub enum RunAction { Choose { node_guid: u32, output_pin: u16 }, PlayStorylet { node_guid: u32, storylet: u16 },
  Topic { node_guid: u32, edge_id: u32 }, Move { to: u16 }, Wait }
pub struct RunStep { pub index: u32, pub branch: u32, pub participant: PartIx, pub action: RunAction, pub label: String }
pub struct RunView { pub participant: PartIx, pub participants: Vec<ParticipantView>, pub place: Option<PlaceView>,
  pub scene: Option<SceneView>, pub also_here: Vec<StoryletRef>, pub nothing_here: bool, pub ending: Option<StoryletRef>,
  pub steps: Vec<RunStep>, pub current_step: u32, pub branches: Vec<BranchView> }
pub struct SceneView { pub storylet: StoryletRef, pub lines: Vec<LineView>, pub choices: Vec<ChoiceView>, pub node_guid: u32 }
pub struct ChoiceView { pub action: RunAction, pub label: String, pub enabled: bool, pub requirement: Option<String> }
pub struct NearMiss { pub storylet: StoryletRef, pub reason: String }     // plain language, <= 120 chars
pub enum RunError { InvalidAction(String), Stalled { storylet: Option<u16>, node_guid: u32 }, Engine(String) }
pub const MAX_AUTO_ADVANCE_MS: i64 = 600_000;    // safety cap for auto-advance per act
impl RunKernel {
  pub fn start(content: &RunContent, start: RunStart, seed: u64) -> Result<Self, RunError>;
  pub fn view(&self, content: &RunContent, participant: PartIx) -> RunView;
  pub fn act(&mut self, content: &RunContent, participant: PartIx, action: RunAction) -> Result<u32, RunError>; // new step index
  pub fn rewind(&mut self, to_step: u32) -> Result<(), RunError>;     // checkout; next act creates a branch
  pub fn why_not(&self, content: &RunContent, participant: PartIx, limit: usize) -> Vec<NearMiss>;
  pub fn replay(content: &RunContent, start: RunStart, seed: u64, actions: &[(PartIx, RunAction)]) -> (Result<Self, RunError>, Option<u32> /*diverged_at*/);
}
// chrono_dispatch.rs
pub fn explain_ineligible(&self, def: &StoryletDef, table: &WorldStateTable, registry: &GameplayTagRegistry) -> Option<String>;
```
- **Auto-advance:** after `start` and after every `act`, call `FlowGraphInstance::tick` repeatedly until one of: a pending interaction exists, the instance ended, or the cumulative clock exceeds `MAX_AUTO_ADVANCE_MS` (in which case return `Stalled`).
- **Rewind:** wraps `FlowGraphInstance::checkout`. A branch ID is assigned when an act follows a rewind.
- **Why not:** `why_not` uses `explain_ineligible` on storylets whose location tags match the participant's place. Its first failing requirement reads, e.g., `needs fact "light doused"` or `needs Benedek absent`.

- [ ] **Step 1: Write the failing tests** (fixture: `tests/support/weirs_crossing.rs` builds a `RunContent` with lamp_soot, ilse_logbook, harbor_master_tides, press_teodor (with Teodor as a rule participant at the harbor) and watch_the_water first; it is extended in Phases 3 and 4):
  - `start_auto_advances_to_first_interaction`: `start(Beginning)`, then `view(0).scene.is_some() || !view(0).also_here.is_empty()`.
  - `act_choose_appends_step_and_changes_view`: `steps.len()` goes from 0 to 1 and `current_step == 1`.
  - `act_after_rewind_creates_branch`: act, act, `rewind(1)`, act with a different choice. `branches.len()==2`, and both branches' steps are retrievable.
  - `replay_is_deterministic`: run 20 seeded random valid actions, `replay`, and the views are equal (`assert_eq!` on serialized JSON).
  - `why_not_explains_missing_fact`: at the harbor with no `light_doused`, `why_not` contains the reason `needs fact "light doused"` for `press_teodor`.
  - `stalled_graph_returns_stalled`: a fixture storylet whose waiter never fires makes `act` return `Err(RunError::Stalled{..})`.
- [ ] **Step 2: Run them and confirm they fail.** Run `cargo test … --test run_kernel`. Expected: FAIL.
- [ ] **Step 3: Implement** `run.rs` and `explain_ineligible`. Move the `CompiledChronoDispatch` fields into `RunContent`, and make the service import the engine type so there is one definition.
- [ ] **Step 4: Run all engine and service tests.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "feat(engine): RunKernel with auto-advance, rewind/branch, replay, why-not"`.

### Task 2.2: Run persistence, HTTP and MCP

**Files:**
- Create: `crates/storylet-engine-service/src/runs.rs`
- Modify: `store.rs` (table `runs`), `http.rs` (routes), `mcp.rs` (operation enum at line ~149), `lib.rs`
- Test: `crates/storylet-engine-service/tests/runs_api.rs` *(new)*

**Interfaces:**
- SQLite: `runs(id TEXT PK, project_revision INTEGER, seed INTEGER, start_json TEXT, actions_json TEXT, revision INTEGER, last_key TEXT, last_result TEXT, created_at INTEGER, updated_at INTEGER)`. Only actions are stored; the kernel is rebuilt with `RunKernel::replay` and cached in memory per run.
- HTTP, with camelCase JSON:
  - `POST /api/v4/runs` `{start, scenarioId?, seed?}` → `RunEnvelope`
  - `GET /api/v4/runs` → `[{id, title, steps, ending?, stale, updatedAt}]`
  - `GET /api/v4/runs/{id}?participant=` → `RunEnvelope`
  - `POST /api/v4/runs/{id}/act` `{participant, action, expectedRevision, idempotencyKey}` → `RunEnvelope`
  - `POST /api/v4/runs/{id}/rewind` `{toStep, expectedRevision}` → `RunEnvelope`
  - `GET /api/v4/runs/{id}/why-not?participant=` → `{nearMisses: NearMiss[]}`
  - `GET /api/v4/runs/{id}/inspect?participant=` → `{knows: KnowRowView[], worldChanges: ChangeView[], chronicle: ChronicleRowView[], advanced: {...}}`
  - `POST /api/v4/runs/{id}/reload` `{expectedRevision}` → `RunEnvelope & {replayed: number, divergedAt: number|null}`
  - `DELETE /api/v4/runs/{id}`
- `RunEnvelope = { id, revision, stale: boolean, contentRevision: number, view: RunView }`. `stale` = `project_revision != current project revision`.
- MCP operations: `run.list`, `run.start`, `run.view`, `run.act`, `run.rewind`, `run.why_not`, `run.inspect`, `run.reload`, `run.delete`, with the same payloads.
- Errors:
  - 409 `{error:"stale revision"}` when `expectedRevision` mismatches
  - 422 `{error, issues}` for an invalid action
  - 200 with `view` + `stalled: {storyletId, nodeGuid}` for `Stalled`

- [ ] **Step 1: Write the failing tests** in `runs_api.rs`, using the in-memory service as the existing `local_service.rs` does:
  - `start_and_act_round_trip`
  - `duplicate_act_appends_one_step`: the same `idempotencyKey` twice gives the same body and the step count increases by exactly 1.
  - `stale_expected_revision_is_409_and_no_mutation`
  - `run_survives_service_restart_by_replay`: reopen the store and `GET` → identical view.
  - `reload_reports_divergence_after_choice_removed`: act on a choice, delete that choice's edge from the storylet and save; the run is `stale==true`; reload returns `divergedAt==Some(step)`.
  - `mcp_run_ops_listed`: the `tools/list` schema enum contains all nine `run.*` names.
- [ ] **Step 2: Run them and confirm they fail.** Run `cargo test --manifest-path crates/storylet-engine-service/Cargo.toml --test runs_api`. Expected: FAIL.
- [ ] **Step 3: Implement** `runs.rs`: store calls, an in-memory `BTreeMap<String, RunKernel>` cache, handlers and MCP dispatch. Compile `RunContent` from the workspace with the existing `compile_chrono_dispatch_with_world` path.
- [ ] **Step 4: Run all service tests.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "feat(service): /api/v4/runs and MCP run.* backed by deterministic replay"`.

### Task 2.3: Play view-model and pure run model

**Files:**
- Create: `src/web/studio/runModel.ts`, `src/web/studio/view-models/run.ts`
- Modify: `src/web/studio/api.ts` (add `StudioApi.runs.*`), `src/web/studio/contracts.ts` (TS mirrors of `RunEnvelope`, `RunView`, `RunAction`, `NearMiss`)
- Test: `tests/web/run-model.test.ts`, `tests/web/run-view-model.test.ts`

**Interfaces (produces):**
```ts
// runModel.ts (pure)
export function choiceForKey(view: RunView, key: string): RunAction | null;      // "1".."9" -> nth enabled-or-not choice
export function moveForKey(view: RunView, n: number): RunAction | null;           // after "G"
export function hasClock(view: RunView): boolean;
export function showParticipantTabs(view: RunView): boolean;                     // >1 human participant
export function nearMissText(m: NearMiss): string;                                // "<title> — <reason>"
export function chronicleRows(rows: ChronicleRowView[]): Array<{ label: string; told: number | null; place: string }>;
// view-models/run.ts
export interface PlayState { runId: string | null; envelope: RunEnvelope | null; activeParticipant: number;
  inspectOpen: boolean; inspectTab: "knows" | "world" | "story"; inspect: InspectView | null;
  whyOpen: boolean; nearMisses: NearMiss[]; busy: boolean; error: string | null; reloadNote: string | null }
export function usePlay(api?: typeof StudioApi.runs): { state: PlayState; start(s: RunStartInput): Promise<void>;
  open(runId: string): Promise<void>; act(a: RunAction): Promise<void>; rewind(step: number): Promise<void>;
  undo(): Promise<void>; reload(): Promise<void>; toggleWhy(): Promise<void>; toggleInspect(): void;
  setInspectTab(t: PlayState["inspectTab"]): Promise<void>; nextParticipant(dir: 1 | -1): void;
  onKey(e: { key: string; ctrlKey: boolean }): Promise<void> }
```
- `act` generates `idempotencyKey = crypto.randomUUID()` once per user intent and sets `busy` while in flight. A second `act` while `busy` is ignored.

- [ ] **Step 1: Write the failing tests:**
  - `keyboard_maps_only_first_nine_choices`: a view with 12 choices gives `choiceForKey(v,"9")` = the 9th choice; `"0"` and `"10"` give `null`.
  - `timeless_project_hides_clock`: `hasClock(view with place.clock undefined) === false`.
  - `showParticipantTabs` is false for one human and true for two.
  - `chronicleRows` keeps story-time order and puts `told` numbers on revealed rows only.
  - VM `act_while_busy_is_ignored`: a fake api resolves slowly; two `act` calls give exactly one api call.
  - VM `undo_rewinds_one_step`: `currentStep 3` → `rewind(2)` called.
  - VM `stale_envelope_sets_banner_and_reload_note`: after `reload()` returns `{replayed:12, divergedAt:13}`, `reloadNote` is `"Replayed 12 of 14 steps; diverged at step 13"`.
- [ ] **Step 2: Run them and confirm they fail.** Run `bun test tests/web/run-model.test.ts tests/web/run-view-model.test.ts`. Expected: FAIL.
- [ ] **Step 3: Implement** `runModel.ts`, the view-model, the API client methods and the contracts.
- [ ] **Step 4: Run `bun test` and `bun run typecheck`.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "feat(studio): play view-model and run model"`.

### Task 2.4: Play view (MVP: story, steps, why-not, start screen)

**Files:**
- Create: `src/web/studio/views/PlayView.vue`, `src/web/studio/components/play/PlayStart.vue`, `PlayStory.vue`, `PlayTimeline.vue`, `PlayWhereWhen.vue`
- Modify: `src/web/studio/router.ts` (`/play`, `/play/:runId`), `src/web/App.vue` (primary **Play** button in `.studio-toolbar`), `src/web/studio/views/StoryletEditorView.vue` (toolbar button **Play from here** → `start({kind:"storylet", storyletId})`), `src/web/styles.css` (play tokens only, from `DESIGN.md`)
- Test: `tests/web/play-routes.test.ts`

**Interfaces:**
- Consumes `usePlay` (Task 2.3).
- Implements `docs/design/playtest-ui.md` §3.1, §3.3, §3.4 and §5, and the parts of §3.2 that need no clock: participant tabs and place.

- [ ] **Step 1: Run the frontend gates** listed in AGENTS.md:
  - Run `ux discover`. Brief: "Play view for a narrative authoring studio; readers are narrative designers".
  - Run `ux recommend` with `--project-type "desktop authoring tool playtest panel" --tone "austere, IDE-like, literary" --forbidden "glow, blur, gradients"`. On Windows, set `PYTHONUTF8=1`, because `ux` crashes on cp1252 output otherwise.
  - Keep `DESIGN.md` as the source of truth. Take from the recommendation only motion (120–200 ms ease-out) and component a11y notes (tooltip delay, status dot with label).
- [ ] **Step 2: Write the failing test** `tests/web/play-routes.test.ts`. Assert that `router.ts` routes include `play` and `play-run`, and that `NAVIGATION_ITEMS` does **not** contain Play: Play is the toolbar's primary action, not a nav link.
- [ ] **Step 3: Run it and confirm it fails.** Run `bun test tests/web/play-routes.test.ts`. Expected: FAIL.
- [ ] **Step 4: Implement the components** following spec §3:
  - The story column has `max-width: 70ch`, with EB Garamond `body-lg`.
  - Choices are `<button>`s at least 44 px tall, prefixed by a number `<kbd>`.
  - *Nothing happens here right now.* has a **Why?** disclosure (`aria-expanded`) listing `nearMissText` and links to `/storylets/:id/edit`.
  - The timeline dots are `<button aria-label="Step n: …">`.
  - An `aria-live="polite"` region announces the new scene title.
  - The stale banner offers **Reload run**.
- [ ] **Step 5: Verify:**
  - `bun test`, `bun run typecheck` and `bun run build` all pass.
  - `ux lint src/web/studio/views/PlayView.vue src/web/studio/components/play` reports no high-severity findings.
- [ ] **Step 6: Manual check.** Run `bun run play:fresh`, then follow this script:
  1. Create a storylet.
  2. Click **Play** and confirm the scene shows.
  3. Press 1 and confirm a step is added.
  4. Press Ctrl+Z and confirm it rewinds.
  5. Choose differently and confirm the branch selector appears.
  6. Edit the storylet in another tab and confirm the stale banner appears.
  7. Click **Reload run** and confirm the note appears.
- [ ] **Step 7: Commit** with `git commit -m "feat(studio): Play view MVP"`.

---

## Phase 3 — Chronicle and telling (≈ 1 week)

### Task 3.1: Chronicle and telling tables plus the `Reveal` node

**Files:**
- Create: `crates/storylet-engine/src/chrono_chronicle.rs`
- Modify: `chrono_state.rs` (`WorldStateTable.telling`, `BijectiveDelta.tellings_added`), `chrono_runtime.rs` (`NodeKind::Reveal`), `run.rs` (`RunContent.chronicle`)
- Test: `crates/storylet-engine/tests/chronicle.rs`

**Interfaces (produces):**
```rust
pub type ChronIx = u16;
pub struct ChronicleEntry { pub key: String, pub story_time: i64, pub label: String, pub location: Option<u16>, pub facts: Vec<FactIx>, pub text: String }
pub struct ChronicleTable { pub entries: Vec<ChronicleEntry> }   // content (immutable); sorted by (story_time, key)
pub struct TellingRow { pub holder: PartIx, pub entry: ChronIx, pub source_event: u32 }
pub struct TellingTable { pub rows: Vec<TellingRow> }            // world state; append order = telling order
impl TellingTable { pub fn reveal(&mut self, row: TellingRow) -> bool; pub fn order_for(&self, holder: PartIx) -> Vec<ChronIx>; }
NodeKind::Reveal { entry: ChronIx, kind: KnowKind, visibility: Visibility }
// Reveal = telling row for the acting participant (+ witnesses if CoLocated) and Learn(fact, kind) for each entry fact.
```

- [ ] **Step 1: Write the failing tests:**
  - `reveal_order_is_free_and_canon_unchanged`: reveal P3 then P1. `order_for(0)==[P3,P1]`, and `entries` is unchanged and still sorted by story time.
  - `reveal_twice_is_idempotent`: the second reveal returns `false` and adds no new knowledge rows.
  - `reveal_learns_entry_facts_with_kind`: the facts of P2 are known with `Recollection`.
  - `revert_reveal_removes_telling_and_facts`.
- [ ] **Step 2: Run them and confirm they fail.** Run `cargo test … --test chronicle`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run the engine tests.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "feat(engine): canon chronicle, per-participant telling, reveal node"`.

### Task 3.2: Author canon events as chronicle entries

**Files:**
- Modify: `crates/storylet-engine-service/src/contracts.rs` (`EventRecord` at line ~106: add `story_time: Option<i64>`, `story_time_label: Option<String>`, `location_id: Option<String>`, `facts: Vec<String>`, `text: Option<String>`, all `#[serde(default)]`)
- Modify: `narrative_graph.rs`, `chrono_graph_adapter.rs` (node `reveal { event_id, kind, visibility }`)
- Modify: `src/web/studio/contracts.ts` (`EventRecord`), `src/web/studio/views/CodexView.vue`, `src/web/studio/view-models/codex.ts`, `narrativeGraphTypes.ts`, `PropertyInspector.vue`
- Test: `crates/storylet-engine-service/tests/local_service.rs`, `tests/web/codex-chronicle.test.ts` *(new)*

**Rules:**
- `event_type == "canon"` with `story_time` set compiles into a `ChronicleEntry`.
- A `reveal` node must reference such an event. Validation error: `` Reveal node "<label>": event "<id>" is not a canon event with a story time ``.
- The Codex form for canon events replaces the JSON textareas with visible-label fields:
  - Story time (number + label)
  - Place (entity picker filtered to locations)
  - Facts (tag chips under `fact.`)
  - Text (EB Garamond textarea)
- World events keep the existing trigger/effects editor.

- [ ] **Step 1: Write the failing tests:**
  - Rust `reveal_requires_canon_event_with_story_time`.
  - Rust `canon_events_compile_sorted_by_story_time`.
  - TS `codex-chronicle.test.ts`: the codex VM `isChronicleEntry(event)` is true only for canon events with `story_time`, and `sortChronicle` orders by `story_time`, then `id`.
- [ ] **Step 2: Run them and confirm they fail.**
- [ ] **Step 3: Implement the contracts, validation, compile, the Codex form and the reveal inspector** (event picker filtered to chronicle entries).
- [ ] **Step 4: Run the service tests, `bun test`, `bun run typecheck` and `ux lint src/web/studio/views/CodexView.vue`.** Expected: PASS, with no high-severity lint findings.
- [ ] **Step 5: Commit** with `git commit -m "feat: author canon chronicle in Codex and reveal nodes"`.

### Task 3.3: Play Inspect panel (Knows, World, Story so far)

**Files:**
- Create: `src/web/studio/components/play/PlayInspect.vue`
- Modify: `crates/storylet-engine/src/run.rs` (`RunKernel::inspect(&self, content, participant) -> InspectView`), `runs.rs` (`/inspect` route from Task 2.2 now returns real data), `PlayView.vue`
- Test: `crates/storylet-engine/tests/run_kernel.rs`, `tests/web/run-model.test.ts`

**Interfaces:**
```rust
pub struct InspectView { pub knows: Vec<KnowRowView>, pub world_changes: Vec<ChangeView>, pub chronicle: Vec<ChronicleRowView>, pub advanced: AdvancedView }
pub struct KnowRowView { pub fact: String, pub kind: KnowKind, pub confirmed: bool, pub source_step: Option<u32> }
pub struct ChangeView { pub key: String, pub before: String, pub after: String }   // last step only
pub struct ChronicleRowView { pub key: String, pub label: String, pub place: Option<String>, pub story_time: i64, pub told: Option<u32> }
pub struct AdvancedView { pub run_seed: u64, pub content_revision: u64, pub node_guid: Option<u32>, pub interaction: Option<String> }
```

- [ ] **Step 1: Write the failing tests:**
  - Rust `inspect_marks_testimony_unconfirmed_and_links_source_step`.
  - Rust `inspect_chronicle_numbers_told_entries_in_telling_order` (P2 told first → `told==Some(1)`).
  - TS: `chronicleRows` renders untold entries with `told===null`.
- [ ] **Step 2: Run them and confirm they fail.**
- [ ] **Step 3: Implement `inspect` and `PlayInspect.vue`** following spec §3.5:
  - closed by default; the **I** key toggles it
  - kind badges in violet, with the text label always visible
  - unconfirmed facts are labelled *not confirmed*
  - source-step links call `rewind`-preview: highlight the dot; do not rewind
- [ ] **Step 4: Verify:** all tests, `typecheck`, `build` and `ux lint` pass.
- [ ] **Step 5: Commit** with `git commit -m "feat: Play inspect panel with knowledge provenance and chronicle"`.

---

## Phase 4 — Space, clock and rule participants (≈ 1–1.5 weeks)

### Task 4.1: Connections, clock and occupancy rules

**Files:**
- Create: `crates/storylet-engine/src/chrono_space.rs`
- Modify: `run.rs` (moves and the clock in `act`/`view`), `chrono_state.rs` (`WorldStateTable.clock: Option<ClockState>`)
- Test: `crates/storylet-engine/tests/space_clock.rs`

**Interfaces (produces):**
```rust
pub struct Connection { pub from: u16, pub to: u16, pub open_slots: Option<u64> /* None = always */, pub requires_fact: Option<FactIx>, pub on_close: Option<OnClose> }
#[serde(tag="kind", rename_all="snake_case")] pub enum OnClose { Return { to: u16 }, Strand }
pub struct Clock { pub slot_labels: Vec<String>, pub actions_per_slot: u8 }
pub struct ClockState { pub slot: u32, pub actions_left: Vec<u8> /* per participant */ }
pub enum MoveBlock { ClosedUntil { slot_label: String }, NeedsFact { fact: String } }
pub struct MoveOption { pub to: u16, pub name: String, pub enabled: bool, pub block: Option<MoveBlock> }
pub fn moves_for(world: &WorldStateTable, content: &RunContent, p: PartIx) -> Vec<MoveOption>;
pub fn advance_slot(world: &mut WorldStateTable, content: &RunContent, delta: &mut BijectiveDelta) -> Vec<OccupancyEvent>;
pub struct OccupancyEvent { pub participant: PartIx, pub from: u16, pub to: Option<u16>, pub rule: OnClose }
// RunContent gains: pub connections: Vec<Connection>, pub clock: Option<Clock>
```
- Each `Move`, `Choose`, `PlayStorylet` and `Topic` act spends 1 action when a clock exists. `Wait` spends all remaining actions.
- When every human participant has 0 actions left, `advance_slot` runs: it moves the slot, applies `on_close` to occupants of now-closed `to` locations, and resets actions.
- With no clock, nothing is spent ("timeless").

- [ ] **Step 1: Write the failing tests:**
  - `closed_edge_blocks_with_until_label`: the rock is open in slots {1,4} and the current slot is 2. The rock `MoveOption` is disabled with `ClosedUntil{"D3 morning"}`.
  - `edge_requires_fact`.
  - `occupant_returned_when_edge_closes`: a participant on the rock at slot 1 → `advance_slot` → at the lighthouse, with an `OccupancyEvent` recorded as a step labelled `"tide drives Mara back"`.
  - `wait_spends_all_actions_and_advances_when_all_humans_done`.
  - `timeless_project_spends_nothing`.
- [ ] **Step 2: Run them and confirm they fail.** Run `cargo test … --test space_clock`. Expected: FAIL.
- [ ] **Step 3: Implement it, and integrate it into `RunKernel::act` and `view`.** `PlaceView` gains `clock_label`, `actions_left` and `moves`.
- [ ] **Step 4: Run the engine tests.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "feat(engine): scheduled connections, world clock, occupancy rules"`.

### Task 4.2: Author connections and the clock

**Files:**
- Modify: `crates/storylet-engine-service/src/contracts.rs` (`SYSTEM_TRAIT_TEMPLATES`)
  - add `connection` (allow_multiple: true): `to` Entity; `open_slots` Int array (empty = always); `requires_fact` String tag; `on_close` String `""|"return"|"strand"`; `return_to` Entity
  - add `clock` (allow_multiple: false, intended for the World entity): `slot_labels` String array; `actions_per_slot` Int, default 2
  - keep `location_settings.connected_zones` and compile it as always-open connections
- Modify: `entities.rs` / the compile path into `RunContent.connections` and `clock`; the validation in `service.rs` save path
- Test: `crates/storylet-engine-service/tests/local_service.rs`

**Validation messages (exact):**
- `` Connection "<from>" → "<to>": a scheduled connection must set on_close (return or strand) ``
- `` Connection "<from>" → "<to>": on_close "return" needs return_to ``
- `` Clock: open_slots index <n> is outside slot_labels (0..<len>) ``

- [ ] **Step 1: Write the failing tests:** `scheduled_connection_without_on_close_is_rejected`, `return_without_return_to_is_rejected`, `open_slot_out_of_range_is_rejected`, `connected_zones_compile_as_always_open`.
- [ ] **Step 2: Run them and confirm they fail.**
- [ ] **Step 3: Implement.** The Entity view shows these traits through the existing trait editor; no new UI is required.
- [ ] **Step 4: Run the service tests.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "feat: author connections and clock as system traits with validation"`.

### Task 4.3: Rule participants (NPC agency via storylets)

**Files:**
- Modify: `crates/storylet-engine-service/src/contracts.rs` (`StoryletRecord` at line ~223: `played_by: Vec<String>` (entity ids; empty = any human), `#[serde(default)]`)
- Modify: `run.rs` (rule pass), `chrono_dispatch.rs` (`StoryletDef.played_by: Vec<PartIx>`)
- Test: `crates/storylet-engine/tests/space_clock.rs`

**Rule pass:**
- When it runs:
  - at the start of every slot, after occupancy;
  - and, if there is no clock, after every human act.
- What it does: each `Controller::Rule` participant, ordered by participant index, plays at most one eligible storylet whose `played_by` contains it. The storylet is chosen by highest `urgency_weight`, then storylet index.
- Each play is a step with `participant = rule participant`. Its causal parent is the triggering step. The pass is capped at `participants.len()` plays per pass, mirroring `advance_npcs` in `simulation.rs`.

- [ ] **Step 1: Write the failing tests:**
  - `rule_storylet_plays_at_slot_start` (Rosa gossips → Benedek learns `fact.i_am_suspected`).
  - `rule_pass_one_play_per_participant`.
  - `rule_step_records_causal_parent` (its parent is Mara's `ask_rosa_openly` step).
- [ ] **Step 2: Run them and confirm they fail.**
- [ ] **Step 3: Implement it.** Add a "Played by" field (entity multi-picker) to the storylet inspector's metadata section in `PropertyInspector.vue`.
- [ ] **Step 4: Run all tests and typecheck.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "feat: rule participants act through storylets on the world clock"`.

### Task 4.4: Play "Where & when" strip with time and moves

**Files:**
- Modify: `src/web/studio/components/play/PlayWhereWhen.vue`, `runModel.ts`, `view-models/run.ts`
- Test: `tests/web/run-model.test.ts`, `tests/web/run-view-model.test.ts`

- [ ] **Step 1: Write the failing tests:**
  - `moveBlockText({kind:"closed_until",slotLabel:"D3 morning"}) === "Closed until D3 morning"` and `({kind:"needs_fact",fact:"tide table"}) === "Needs: tide table"`.
  - VM `end_turn_sends_wait`.
  - VM `next_participant_skips_rule_participants`.
  - `moveForKey(view, 2)` returns the second *enabled* move.
- [ ] **Step 2: Run them and confirm they fail.**
- [ ] **Step 3: Implement** spec §3.2 and the keyboard shortcuts G, E and Tab.
- [ ] **Step 4: Verify:** `bun test`, `typecheck`, `build` and `ux lint` pass.
- [ ] **Step 5: Commit** with `git commit -m "feat(studio): Play where-and-when strip with clock, moves, hot-seat"`.

---

## Phase 5 — Spine and explorer (≈ 1.5 weeks)

### Task 5.1: Explorer in Rust (port of the prototype)

**Files:**
- Create: `crates/storylet-engine/src/chrono_explore.rs`
- Modify: `crates/storylet-engine/tests/support/weirs_crossing.rs`: the full 12-storylet world, four participants and four endings, mirroring `examples/proposal-weirs-crossing/explore.ts`
- Test: `crates/storylet-engine/tests/explore.rs`

**Interfaces (produces):**
```rust
pub enum Persona { Wanderer, Investigator { epsilon_permille: u16 /* default 150 */ } }
pub enum ParticipantConfig { Solo, AllHumans }
pub struct Milestone { pub key: String, pub name: String, pub routes: Vec<u16>, pub reachable_for: Vec<ParticipantConfig>, pub deadline_slot: Option<u32> }
pub struct IntentProbe { pub name: String, pub holder_knows: Vec<FactIx>, pub colocated_with: Option<u16>, pub expected_routes: Vec<u16> }
pub struct ExploreConfig { pub runs: u32 /* default 2000, max 50000 */, pub seed: u64, pub personas: Vec<Persona>,
  pub configs: Vec<ParticipantConfig>, pub milestones: Vec<Milestone>, pub probes: Vec<IntentProbe>, pub max_steps: u32 /* default 500 */ }
pub enum Invariant { KnowledgeWithoutSource, PastWithoutTelling, OccupantBeyondClosedEdge, ContradictoryProps(String, String) }
pub struct Violation { pub invariant: Invariant, pub seed: u64, pub persona: Persona, pub config: ParticipantConfig, pub step: u32, pub actions: Vec<(PartIx, RunAction)> }
pub struct ExploreReport { pub cells: Vec<ExploreCell> /* persona × config */, pub violations: Vec<Violation> /* first 20 */, pub determinism_mismatches: u32 }
pub struct ExploreCell { pub persona: Persona, pub config: ParticipantConfig, pub endings: Vec<(u16, u32)>,
  pub milestone_reached: Vec<(String, u32)>, pub probe_hits: Vec<(String, u32, u32 /*filled*/)>,
  pub telling_orders: u32, pub dead_air_by_location: Vec<(u16, u32, u32)> }
pub fn explore(content: &RunContent, cfg: &ExploreConfig) -> ExploreReport;
pub fn content_without_validation_for_tests(...) // NOT public API; build via tests/support
```
- Bots act only through `RunKernel::act`, which keeps them honest.
- **Investigator:** with probability ε, a weighted random action; otherwise priority: ranked storylets (any milestone route), then rock-ready style moves toward an eligible storylet's location (BFS over enabled moves), then the remaining storylets, then compare notes, then a random move.
- **Determinism:** re-run the first 200 seeds of the first cell and compare the serialized views.

- [ ] **Step 1: Write the failing tests** (2,000 runs each; they must finish in < 10 s in `--release`; mark them `#[ignore]` in debug builds if slower, and run them with `--release -- --include-ignored` in CI):
  - `weirs_crossing_has_no_invariant_violations`: `violations.is_empty()`.
  - `stranding_detected_without_on_close`: the fixture is built bypassing validation, with `on_close: None` on the causeway. There is at least one `OccupantBeyondClosedEdge` with a non-empty `actions` list that reproduces it through `RunKernel::replay`.
  - `solo_justice_unreachable_is_reported`: in the `Investigator`×`Solo` cell, `milestone_reached["benedek_held"] == 0`; in `Investigator`×`AllHumans` it is greater than 0.
  - `bluff_fills_probe_in_coop`: with the bluff storylet, the probe `suspects_and_faces_benedek` has `filled > 0` in AllHumans.
  - `explore_is_deterministic`: `determinism_mismatches == 0`, and two `explore` calls return equal reports.
- [ ] **Step 2: Run them and confirm they fail.** Run `cargo test --release --manifest-path crates/storylet-engine/Cargo.toml --test explore`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them again.** Expected: PASS. Record the timing in the commit body.
- [ ] **Step 5: Commit** with `git commit -m "feat(engine): deterministic explorer with personas, invariants, milestones, probes"`.

### Task 5.2: Author the spine and expose coverage

**Files:**
- Modify: `contracts.rs` (`SYSTEM_TRAIT_TEMPLATES`):
  - add `milestone` (allow_multiple: true, for World): `key`, `name`, `routes` Storylet array, `reachable_for` String array (`"solo"`, `"coop"`), `deadline_slot` Int (-1 = none)
  - add `intent_probe` (allow_multiple: true, for World): `name`, `holder_knows` String-tag array, `colocated_with` Entity, `expected_routes` Storylet array
- Modify: `runs.rs`: `POST /api/v4/coverage {runs?, seed?}` → `ExploreReport` (JSON; at most one concurrent run per project, otherwise 409 `{error:"coverage already running"}`); MCP `workspace.coverage`
- Test: `crates/storylet-engine-service/tests/runs_api.rs`

- [ ] **Step 1: Write the failing tests:**
  - `coverage_rejects_runs_over_50000` (422).
  - `coverage_reports_milestones_from_world_traits`.
  - `mcp_workspace_coverage_listed`.
- [ ] **Step 2: Run them and confirm they fail.**
- [ ] **Step 3: Implement.** Use `tokio::task::spawn_blocking` for `explore`.
- [ ] **Step 4: Run the service tests.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "feat(service): milestone/intent-probe traits and coverage endpoint"`.

### Task 5.3: Coverage report in Play, with "Replay seed"

**Files:**
- Create: `src/web/studio/components/play/CoverageReport.vue`
- Modify: `PlayStart.vue` (secondary button **Let bots play**), `view-models/run.ts` (`runCoverage(runs: number)`, `replayViolation(v)` → `POST /api/v4/runs` with `{start, seed, actions}`; `RunStart` itself is unchanged), `runModel.ts`
- Modify: `run.rs`/`runs.rs`: `POST /api/v4/runs` accepts optional `actions: [(participant, action)]`, replayed at creation, so the run opens at the violating step.
- Test: `tests/web/run-model.test.ts`, `runs_api.rs`

- [ ] **Step 1: Write the failing tests:**
  - TS `endingBars(report)` returns percentages that sum to 100 ±0.1 per cell.
  - TS `invariantSeverity(report) === "fail"` iff `violations.length > 0`.
  - Rust `start_with_recorded_actions_opens_at_last_step`.
- [ ] **Step 2: Run them and confirm they fail.**
- [ ] **Step 3: Implement** spec §6. Invariant failures show in rose with the text label `"N invariant violations"`.
- [ ] **Step 4: Verify:** all tests, `typecheck`, `build` and `ux lint` pass.
- [ ] **Step 5: Commit** with `git commit -m "feat(studio): coverage report with replayable violations"`.

---

## Phase 6 — Authoring polish and the sample project (≈ 1 week)

### Task 6.1: Storylet "Who & when" inspector section and library badges

**Files:**
- Modify: `contracts.rs` (`StoryletRecord.window: Vec<u32>`; empty = any slot), `chrono_dispatch.rs` (eligibility respects the window), `PropertyInspector.vue` (section "Who & when": Played by, Window as a slot multi-select from the Clock labels, default Learn visibility), `LibraryView.vue` (badges: `D3 morning…`, played-by avatar)
- Test: `crates/storylet-engine/tests/space_clock.rs`, `tests/web/library-badges.test.ts` *(new)*

- [ ] **Step 1: Write the failing tests:**
  - Rust `storylet_window_gates_eligibility`.
  - TS `windowBadge([4,5], labels) === "D3 morning, D3 evening"` and `windowBadge([]) === null`.
- [ ] **Step 2: Run them and confirm they fail.**
- [ ] **Step 3: Implement.** Every new control has a visible `<label>` (UI review P1).
- [ ] **Step 4: Verify:** tests, typecheck, build and `ux lint` pass.
- [ ] **Step 5: Commit** with `git commit -m "feat: storylet who-and-when authoring and library badges"`.

### Task 6.2: The *Weir's Crossing* sample project, authored through the human-first loop

**Files:**
- Create: `examples/weirs-crossing/weirs-crossing.storylet.json` (exported bundle), `docs/guides/llm-authoring-assist.md`
- Test: `crates/storylet-engine-service/tests/runs_api.rs`

- [ ] **Step 1: Write the failing test** `weirs_crossing_bundle_plays_to_keeper_comes_home`: import the bundle and start a solo run with seed 1011. Apply this action script:
  1. `listen_quietly`
  2. `ask_rosa_openly`
  3. move harbor
  4. `harbor_master_tides`
  5. wait
  6. move lighthouse
  7. `lamp_soot`
  8. `ilse_logbook`
  9. move rock
  10. `find_ilse`

  Assert that `view.ending.title == "The Keeper Comes Home"` and that the step for Benedek taking the ferry has a parent chain that reaches the `ask_rosa_openly` step.
- [ ] **Step 2: Run it and confirm it fails.** Expected: FAIL (no file).
- [ ] **Step 3: Author the project** in the studio:
  - entities and traits
  - canon events P1–P4
  - 12 storylets
  - milestones and probes

  An MCP agent may draft storylets from the coverage report, following `docs/guides/llm-authoring-assist.md`; a human edits and approves each one. Export it with Project → Export.
- [ ] **Step 4: Write the guide** `docs/guides/llm-authoring-assist.md`. It must cover:
  1. Read `workspace.context`.
  2. Run `workspace.coverage`.
  3. Pick one hole.
  4. `content.save` a draft. It stays unapproved; never approve it.
  5. Run `workspace.validate`.
  6. Report to the human with the hole name and the draft ID.

  Include an explicit *never* list copied from proposal §4.8.
- [ ] **Step 5: Run the test again.** Expected: PASS. Then run `POST /api/v4/coverage` on the bundle and paste the summary into the commit body.
- [ ] **Step 6: Commit** with `git commit -m "docs+example: Weir's Crossing sample project and LLM authoring-assist guide"`.

### Task 6.3: Mark agent drafts

**Files:**
- Modify: `contracts.rs` (`StoryletRecord.origin: String`, `"human"` or `"agent"`, default `"human"`), `mcp.rs` (`content.save` for storylets sets `origin="agent"` on create), `LibraryView.vue` and `StoryletEditorView.vue` (badge "Agent draft" while unapproved)
- Test: `crates/storylet-engine-service/tests/local_service.rs`, `tests/web/library-badges.test.ts`

- [ ] **Step 1: Write the failing tests:**
  - Rust `mcp_created_storylet_has_agent_origin`.
  - Rust `http_created_storylet_has_human_origin`.
  - TS `originBadge({origin:"agent", approved_revision:null}) === "Agent draft"`; once approved it returns `null`.
- [ ] **Step 2: Run them and confirm they fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Verify.**
- [ ] **Step 5: Commit** with `git commit -m "feat: mark agent-drafted storylets until human approval"`.

---

## Phase 7 — Retire legacy runtimes (≈ 1 week)

### Task 7.1: Point the C ABI at `RunKernel`

**Files:**
- Modify: `crates/storylet-engine/src/ffi.rs` (it currently wraps `StoryletRuntime`), `crates/storylet-engine/tests/ffi_abi.rs`, `examples/native-host/*`, `examples/native-host-dotnet/Program.cs`

**Interfaces (C ABI, JSON in and out, UTF-8, caller frees with `dn_string_free`):**
```c
dn_run* dn_run_start(const char* content_json, const char* start_json, uint64_t seed, char** error_out);
char*   dn_run_view(dn_run*, uint16_t participant);
char*   dn_run_act(dn_run*, uint16_t participant, const char* action_json);   // returns RunView JSON or {"error":...}
int32_t dn_run_rewind(dn_run*, uint32_t to_step);
void    dn_run_free(dn_run*); void dn_string_free(char*);
```
- `content_json` = a serialized `RunContent`. The service gains `GET /api/v4/run-content`, which returns the compiled content for native hosts.

- [ ] **Step 1: Write the failing tests** `ffi_run_round_trip` (start → act → view through the C ABI) and `ffi_invalid_action_returns_error_json`.
- [ ] **Step 2: Run them and confirm they fail.**
- [ ] **Step 3: Implement it, and update the example hosts.**
- [ ] **Step 4: Run `cargo test … --test ffi_abi`.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "feat(ffi): C ABI over RunKernel"`.

### Task 7.2: Delete the legacy paths

**Files:**
- Delete: `crates/storylet-engine/src/runtime.rs`, `simulation.rs`, the legacy `WorldState`/`apply_command` in `lib.rs`, and the parts of `storylets.rs` used only by them. Also delete the tests `runtime_session.rs`, `autonomous_simulation.rs`, `storylet_lifecycle.rs` and `storylet_selection.rs`, after moving any assertion still relevant to the run kernel into `run_kernel.rs`.
- Modify: `http.rs`: remove `/api/v3/playtests*` and `/api/v3/chrono/playtests*`. `mcp.rs`: remove `playtest.*` and `chrono.playtest.*`. `service.rs`: remove the `StoryletRuntime` field. `chrono_playtest.rs`: delete it; its tick/scrub debug features are not carried over (spec §1).
- Test: all suites.

- [ ] **Step 1: Write the failing test** `legacy_routes_are_gone` in `runs_api.rs`: `GET /api/v3/playtests` returns 404, and the MCP `tools/list` contains no `playtest.` or `chrono.playtest.` names.
- [ ] **Step 2: Run it and confirm it fails.**
- [ ] **Step 3: Delete the code**, then fix compile errors by pointing them at `run.rs`.
- [ ] **Step 4: Run every suite:** both `cargo test` commands, `bun test`, `bun run typecheck` and `bun run build`. Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "refactor: remove legacy deck runtime and chrono debug playtest"`.

### Task 7.3: Docs

**Files:**
- Modify: `README.md`, `docs/HANDOFF.md`, `docs/design/chrono-decision-engine.md` (§4 status table: participants, chronicle, space/clock, run kernel, explorer marked "Wired"; §6 test counts), `docs/design/storylet-studio.md` (Play replaces Playtest; MCP `run.*` and `workspace.coverage`), `docs/design/proposal-human-first-nonlinear-narrative.md` (status → "Accepted; implemented through Phase 7")

- [ ] **Step 1: Check for stale references.** Run `grep -rn "playtest\.\|/api/v3/playtests\|chrono\.playtest\|StoryletRuntime\|Tick 16" README.md docs src crates --include=*.md --include=*.ts --include=*.vue --include=*.rs`. Expected after the edits: no matches outside `docs/superpowers/plans/` and the git history.
- [ ] **Step 2: Edit the docs.** Re-run the grep. Expected: no matches.
- [ ] **Step 3: Commit** with `git commit -m "docs: document the run kernel, Play, and coverage"`.

---

## Self-review notes (completed)

**Spec coverage:** each proposal section maps to tasks.

| Proposal section | Tasks |
|---|---|
| §4.2 space | 4.1, 4.2 |
| §4.3 time | 3.1, 3.2, plus rewind and branch in 2.1 |
| §4.4 participants | 1.1–1.4, 4.3 |
| §4.5 storylet contract | 1.4, 3.2, 4.3, 6.1 |
| §4.6 spine | 5.2 |
| §4.7 director: hard eligibility | existing behaviour + `explain_ineligible` |
| §4.7 director: sifting | not in this plan (see below) |
| §4.8 authoring LLM | 6.2, 6.3 |
| §4.8 runtime LLM | out of scope (decision 3) |
| §4.9 verification | 5.1–5.3 |
| §4.10 authoring loop | 6.2 guide |
| §2.3 drift | 0.1 |
| `playtest-ui.md` §2–§7 | 2.3, 2.4, 3.3, 4.4, 5.3 |

**Deliberately not in this plan:**
- **Story sifting** (proposal §4.7, step 5): it needs authored sift patterns. Write a follow-up plan after Phase 5 shows which emergent chains authors want to recognise.
- **Simultaneous networked participants** (proposal §4.4 transactions). Hot-seat co-op covers the first slice; the write-set policies stay contract-only until a networked target exists.

**Time estimate:** about 8–10 weeks for one engineer who knows the codebase. Phases 0–2 (≈ 3 weeks) give a usable Play view on today's content.
