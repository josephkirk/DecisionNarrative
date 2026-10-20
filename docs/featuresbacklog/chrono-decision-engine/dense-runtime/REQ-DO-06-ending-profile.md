# REQ-DO-06 — Ending Profile and Storylet Marker

**Status:** Proposed  
**Target:** ChronoFlow v6  
**DOD relationship:** Uses the existing authoring `ending` flag and graph `End` node; does not create a second ending model.  
**Depends on:** `REQ-DO-01`, `REQ-DO-02`, `REQ-DO-03`, `REQ-DO-08`, `REQ-DO-09`  
**Related:** `REQ-DO-04`, `REQ-DO-05`, `REQ-DO-10`, `REQ-DO-11`

## 1. Decision

**An ending is a storylet marked as an ending.**

It is not a separate runtime component table or a special non-storylet object. Ending composition, if used, is an **authoring-time generator** that produces ending storylets; the runtime selects one ending storylet.

This supersedes the earlier proposed runtime `EndingComponentTable` as the primary model.

## 2. Existing support and gap

The authoring/service/graph layers already have the flag:

| Layer | Evidence |
|---|---|
| Service record | `StoryletRecord.ending: bool` — `crates/storylet-engine-service/src/contracts.rs:293` |
| Projection | `ending: st.ending` — `projection.rs:246` |
| Workspace bundle | `ending: storylet.ending === true` — `src/web/workspaceProject.ts:476` |
| Studio graph | End node `is_terminal: st.ending` — `src/web/studio/narrativeGraphTypes.ts:355` |
| Service graph | `is_terminal: st.ending` — `crates/storylet-engine-service/src/narrative_graph.rs:1126` |
| Legacy runtime | `StoryletDefinition.ending`, `terminal: definition.ending` — `crates/storylet-engine/src/storylets.rs:142, 717` |

The missing link is the Chrono path:

- core `StoryletDef` has no `is_ending` (`chrono_dispatch.rs:226`);
- `chrono_adapter.rs:201` builds `StoryletDef` without copying `storylet.ending`;
- Chrono QBN has no ending profile and cannot exclude ending-marked storylets from the normal deck.

Do not add a second authoring flag. Carry the existing one into compiled Chrono metadata.

## 3. Requirement

**REQ-DO-06:** An ending SHALL be a storylet with an ending marker; the runtime SHALL select it through an ending `SelectionProfile` and apply that profile's terminal behavior.

Specific rules:

1. The compiled storylet row SHALL expose `is_ending` or an equivalent `ending_mask` bit.
2. By default, a storylet marked `is_ending` SHALL NOT enter a normal profile. A profile MAY opt in explicitly when the project wants ending-marked content on the normal path.
3. The ending profile pool SHALL contain only `is_ending` storylets.
4. The default ending profile uses `terminal: EndScope`. A project MAY configure `Continue` or `Jump` for epilogues, chapter endings, or NG+ flows; terminal behavior belongs to the profile, not the `is_ending` marker.
5. An ending storylet MAY be gated by any profile gate (`REQ-DO-09`); pacing is optional.
6. Ending selection SHALL be a `SelectionProfile` (`REQ-DO-08`), not a separate selector.
7. The selected ending storylet ID SHALL be recorded in call history and saved state.
8. An ending storylet's graph SHALL contain at least one `End` node and **every** `End` SHALL be terminal with no jump target. A mixed graph with terminal and jump exits is a normal-path storylet, not a dedicated ending; a jump target overrides a leftover terminal flag.
9. Normal selection SHALL never fall through into the ending profile pool unless the profile explicitly names it.
10. A project MAY author zero ending storylets; the ending profile is then absent and no ending phase is required.
11. When the ending profile is enabled, a failed selection SHALL follow that profile's `FallbackPolicy`: `Storylet(authored)` by default, `Continue` to keep play alive, or `Error` to fail explicitly. It SHALL NOT invent an ending.

## 4. Data layout

Add one bit per storylet. There is no separate ending table, ending index, or ending registry:

```text
StoryletCatalog (REQ-DO-10)
  ...
  is_ending: StoryletBitset
```

The ending profile's pool, weights, gates, terminal policy, and fallback live in one `SelectionProfile` (`REQ-DO-08`); see §6 for the canonical default. A profile pool is a precomputed view over `StoryletIndex`, not a second storylet store. Priority, weight, and group are profile overrides, not new table columns.

## 5. Ending eligibility

An ending storylet uses the same exact eligibility pipeline as a normal storylet:

- required/forbidden tags;
- metric gates;
- role binding;
- storylet prerequisites;
- scope completion;
- item/choice/actor features (`REQ-DO-11`);
- arc/stage gates.

The ending profile pool is filtered by exact eligibility **before** weighting. Quality distance and salience affect order/probability, not hard eligibility.

## 6. Selection

Ending selection is a `SelectionProfile` (`REQ-DO-08`), not a separate algorithm:

```text
profile = SelectionProfile {
  pool: Ending,
  eligibility: exact predicates,
  gates: [PacingGate] if the project enables pacing,
  policy: Weighted | Priority | SalienceMax,
  terminal: EndScope,
  fallback: Storylet(authored)
}
```

By default the normal and ending profile pools are disjoint; a profile may explicitly opt in to ending-marked rows. Candidate limits, gates, and fallbacks are profile settings. The ending pool may contain 20,000 storylets and is filtered once; this is not a new selection mechanism.

## 7. Terminal semantics

On ending completion:

1. Commit the ending storylet's effects.
2. Append a call-history entry with the stable ending storylet key.
3. Apply the profile's `terminal` policy:
   - `EndScope` (default): set the scope to `Completed`/`Ended` and reject later starts.
   - `Continue`: keep the scope open for an epilogue or follow-up beat.
   - `Jump`: invoke `terminal_target` as the next storylet.
4. Emit the existing completion event and a profile diagnostic; do not add a second ending event type.

The graph `End` node is still required. An `is_ending` storylet whose graph can continue is a validation error.

## 8. Capacity

The ending scenario is a stress target, not a separate capacity:

- `StoryletSpace` (`REQ-DO-02`) owns stable keys, dense indices, and the single `MAX_STORYLETS` capacity.
- `StoryletBitset` (`REQ-DO-03`) covers that same space.
- Catalog/working-set and lazy payload loading are owned by `REQ-DO-10`.
- 20,000 endings is a test target, not a required content count. A project may have zero endings.

## 9. Authoring-time composition

Composition is allowed only as a build-time generator:

```text
base/world/relationship/personality/item fragments
  -> generated ending storylets
  -> normal content validation/approval
```

The runtime sees only rows with `is_ending`. There is no runtime ending component table and no runtime ending algebra.

This keeps the user-facing rule: **ending is a storylet marked ending**.

## 10. Validation

- Every `is_ending` storylet has every reachable `End` marked terminal.
- A mixed or jump-only ending graph does not set the ending flag.
- A graph with no `End` fails validation.
- No default normal profile includes an ending storylet; an explicit opt-in is recorded in the profile.
- Every ending storylet has stable ID, text reference, and at least one effect or terminal outcome.
- Every ending storylet is reachable, or explicitly marked impossible.
- When the ending profile is enabled, its pool is non-empty for every reachable final state.
- The ending profile has a valid fallback policy; a `Storylet` fallback resolves to a terminal storylet.
- Configured gates (`REQ-DO-09`) are evaluated before ending eligibility.
- Selected ending keys survive save/load and content-patch rebase.

## 11. Acceptance tests

- `ending_flag_compiled`: service `ending: true` produces `is_ending` in the Chrono row.
- `ending_excluded_from_normal_deck`: a default normal profile does not return an ending; an explicit opt-in profile does.
- `ending_profile_pool_disjoint`: by default normal and ending profile pools share no row; explicit opt-in may overlap.
- `multiple_end_nodes`: a storylet graph may have multiple End nodes, including a mix of terminal and jump exits.
- `ending_terminal_policy`: default `EndScope` rejects; `Continue` keeps the scope; `Jump` invokes its target.
- `ending_graph_terminal_validation`: a jump-only or mixed graph does not set `ending`; an all-terminal graph does.
- `ending_gate_configured`: a configured gate rejects below its bounds and accepts at them.
- `ending_profile_weighted_selection`: a large eligible ending profile produces a deterministic seeded pick.
- `ending_fallback_storylet`: no eligible ending uses the authored fallback; `Continue` keeps the scope alive; `Error` is typed.
- `ending_save_replay`: saved ending storylet ID replays identically without a fresh draw.
- `ending_flag_not_duplicated`: no second authoring flag or ending table is required.
- `zero_ending_project`: a project with no ending storylets loads and runs without an ending phase.

## 12. Migration

1. Add `is_ending` to the compiled storylet row, sourced from `StoryletRecord.ending`.
2. Build the ending profile pool and exclude it from normal decks.
3. Keep graph `End` validation.
4. Add the profile gates (`REQ-DO-09`).
5. Add the ending profile policy (`REQ-DO-08`).
6. Migrate saved ending storylet IDs to stable keys.
7. Keep authoring-time composition as a separate generator if needed.

## 13. References

- `crates/storylet-engine-service/src/contracts.rs:293` — `StoryletRecord.ending`.
- `crates/storylet-engine-service/src/projection.rs:246`.
- `src/web/workspaceProject.ts:476`.
- `crates/storylet-engine-service/src/narrative_graph.rs:1126`.
- `crates/storylet-engine/src/storylets.rs:142, 717`.
- `crates/storylet-engine-service/src/chrono_adapter.rs:201` — current missing mapping.
- `REQ-DO-08` — weighted selection.
- `REQ-DO-09` — path gate.
- `REQ-DO-10` — capacity/catalog.
- `REQ-DO-11` — derived feature indexes.
