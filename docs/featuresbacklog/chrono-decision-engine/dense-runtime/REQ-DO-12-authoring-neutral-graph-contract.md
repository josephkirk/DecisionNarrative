# REQ-DO-12 — Authoring-Neutral Graph Contract

**Status:** Proposed  
**Target:** ChronoFlow v6  
**DOD relationship:** Preserves the existing studio graph contract; removes backend story-shape assumptions from validation and compilation.  
**Depends on:** `REQ-DO-02`, `REQ-DO-06`  
**Related:** `REQ-DO-08`, `REQ-DO-09`, `REQ-DO-10`, `REQ-DO-11`

## 1. Problem

The studio is a graph editor, not a genre template. The backend must preserve that graph and accept any supported story shape.

Current gaps that remain after the backend fix:

1. Scenario playtests still require a start location.
2. The studio UI does not yet expose the backend role `location_policy`; agents and JSON authoring can set it.

Resolved: both validators accept multiple End nodes; `ending` is derived from the graph on save/read; a jump-only or mixed End graph stays on the normal path; role binding can opt out of co-location with `location_policy: "any"`.

## 2. Requirement

**REQ-DO-12:** Narrative designer flexibility is the primary constraint. The backend SHALL treat the authored graph as the canonical narrative contract and SHALL NOT impose a particular genre, plot shape, or required ending structure.

Specific rules:

1. The persisted `graph` is canonical for storylet flow.
2. `content`, `choices`, and `effects` are projections for search/preview and legacy callers; they SHALL NOT replace or discard the graph.
3. Validation SHALL require exactly one Start and **at least one End**.
4. Validation SHALL allow multiple End nodes and any mix of terminal and jump exits for one storylet.
5. Every node SHALL be reachable from Start and able to reach at least one End.
6. An End SHALL have no outgoing edges.
7. `ending` SHALL be true only when the graph has at least one End, every End has `is_terminal: true`, and every End has no `jump_to_storylet`.
8. A jump takes precedence over a leftover `is_terminal` flag. A jump-only or mixed End graph SHALL remain a normal-path storylet and SHALL NOT set `ending`.
9. A project MAY have zero ending storylets. No ending phase is required.
10. Authoring metadata SHALL remain optional when the graph does not need it: location tags, role parameters, quality preconditions, choices, cluster type, repeatability, and effect projections are not mandatory genre fields.
11. Role parameters MAY declare `location_policy` as `current` (default) or `any`; `any` permits remote or abstract actors.
12. Backend validation SHALL accept every registered executable node type regardless of story shape, including linear, QBN hub, salience, waypoint, mission, conditional, graph-transform, and loop-with-exit graphs.
13. Unknown or non-executable node types SHALL fail validation explicitly; they SHALL NOT be silently ignored or rewritten.
14. The backend SHALL NOT require a fixed number of beats, endings, acts, locations, roles, or choices for a project or storylet.
15. Runtime selection remains policy-driven: normal deck, ending profile, salience, QBN, waypoint, or host-driven invocation.

## 3. Graph contract

```text
NarrativeGraph
  nodes: Start | supported node types | End...
  edges: authored transitions

Invariants
  exactly one Start
  >= 1 End
  IDs unique
  every node reachable from Start
  every node can reach some End
  no outgoing edges from End
```

End variants:

```text
End { is_terminal: true,  jump_to_storylet: None }
End { is_terminal: false, jump_to_storylet: Some }
End { is_terminal: false, jump_to_storylet: None }  // graph-complete, not a run ending
```

- The first variant is a terminal ending; a jump target overrides a leftover terminal flag and is treated as an exit.
- The second is an exit to another storylet.
- The third completes the storylet graph without ending the run; the dispatcher chooses the next storylet.
- A mix is valid when branches have different outcomes.

## 4. Storylet lifecycle, not story shape

Only these storylet-level policies affect selection:

- repeatability/cooldown;
- eligibility predicates;
- ending membership;
- display priority/weight;
- project-scoped path pacing, if enabled.

The backend SHALL NOT infer a linear plot, a three-act arc, a required number of choices, or a mandatory ending from those policies.

A project may choose:

- a single terminal ending;
- many terminal endings;
- only jumps and a normal-deck loop;
- no ending at all.

Gates from `REQ-DO-09` apply only when a project/profile opts in.

## 5. Authoring boundary

The studio may expose a curated node palette. That is a UI choice, not a backend contract.

The backend contract is:

- accepted node types execute deterministically;
- unknown types fail with an actionable validation error;
- a future node type can be added without changing the storylet/graph envelope;
- node payloads round-trip without backend rewriting.

Do not add genre-specific fields to `StoryletRecord` to satisfy one game type. Express the requirement through graph nodes, predicates, tags, qualities, roles, items, or project-owned data.

## 6. Validation matrix

| Case | Result |
|---|---|
| Zero Start | error |
| More than one Start | error |
| Zero End | error |
| Multiple End | valid |
| End with outgoing edge | error |
| Jump-only End on an `ending` storylet | invalid ending flag |
| Terminal End | sets `ending` |
| Non-ending storylet with jump exits | valid |
| Zero ending storylets in project | valid |
| Every node reachable and reaches an End | required |
| Unknown node type | error with type name |
| Optional metadata absent | valid |
| Fixed beat/ending count | not required |

## 7. Acceptance tests

- `multiple_end_nodes_validate`: one Start and two End nodes pass.
- `mixed_terminal_and_jump_ends`: a terminal branch and a jump branch compile as `ending = false`.
- `jump_only_is_not_ending`: jump-only End nodes compile as `ending = false`.
- `all_terminal_ends`: an all-terminal graph compiles as `ending = true`.
- `zero_ending_project`: project load and playtest with no `ending` storylets.
- `graph_is_canonical`: save/load preserves every authored node type and field without backend rewriting.
- `service_owns_ending`: a contradictory client `ending` value is replaced by the graph-derived value on save/read.
- `unknown_node_type_rejected`: unknown type fails with its name.
- `optional_metadata_absent`: a graph-only storylet with no location tags, roles, or choices validates.
- `role_location_policy`: default binds at `location.current`; `any` binds without a location constraint.
- `no_fixed_story_shape`: linear, QBN-hub, salience, waypoint, mission, and loop-with-exit graphs validate without shape-specific errors.
- `projection_does_not_erase_graph`: compiling legacy content fields leaves `graph` intact.

## 8. Migration

1. Change graph validation to require at least one End.
2. Reverse-reachability walks backward from every End.
3. Derive `ending` from the graph: at least one End and every End terminal.
4. Keep mixed and jump-only graphs on the normal path.
5. Remove any requirement that a project has an ending.
6. Add round-trip and optional-metadata tests.
7. Document that node palette membership is a UI/runtime capability, not a genre rule.

## 9. Risks

- Multiple End nodes complicate author-facing “ending” labels; the label must be derived, not hand-maintained separately.
- A mixed terminal/jump storylet can end on one branch and continue on another; this is intentional, but selection policy must not assume every branch is terminal.
- A zero-ending project needs an explicit “no ending phase” policy; otherwise the ending gate can appear to be missing.
- Allowing arbitrary node types at runtime would be unsafe; unknown types must fail validation. The extension point is registered executable node types, not free-form payload execution.
- Legacy fields can drift from the graph; they must remain projections, never the authority.

## 10. References

- `crates/storylet-engine-service/src/narrative_graph.rs` — graph validation and projection.
- `src/web/studio/narrativeGraphTypes.ts` — studio validation and compilation.
- `crates/storylet-engine/src/chrono_runtime.rs` — runtime already supports multiple `NodeKind::End` nodes.
- `REQ-DO-06` — ending storylets and pool.
- `REQ-DO-09` — optional selection gates and pacing.
