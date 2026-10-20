# REQ-DO-09 — Selection Gates and Pacing

**Status:** Proposed  
**Target:** ChronoFlow v6  
**DOD relationship:** Adds opt-in gates to `SelectionProfile`; does not change QBN, salience, or scene-composer algorithms.  
**Depends on:** `REQ-DO-01`, `REQ-DO-02`, `REQ-DO-08`  
**Related:** `REQ-DO-03`, `REQ-DO-06`, `REQ-DO-10`

## 1. Problem

A profile may need to wait before it is eligible:

- an ending profile should not fire at beat 20;
- a chapter or act should not end before its minimum beats;
- a tutorial deck may require a minimum elapsed time;
- a sandbox may need no gate at all.

The gate is not an ending concept. It is a reusable condition on a profile. Hard-coding `150 storylets / 300 minutes` as an engine rule would make the backend decide the game's shape.

## 2. Requirement

**REQ-DO-09:** A `SelectionProfile` MAY declare ordered `SelectionGate` rules; gates are evaluated before the profile draws a candidate.

Rules:

1. A gate is data on the profile, not a hard-coded engine branch.
2. Gates are opt-in. An empty gate list means no extra restriction.
3. A gate MAY check completed traversal count, elapsed play time, a flag, a metric, a tag, or a custom registered predicate.
4. Gates are evaluated after exact eligibility and before ranking/drawing.
5. A gate MAY be bypassed by an explicitly authored shortcut rule; it is never bypassed implicitly.
6. A project may define zero ending profiles; no gate is required.
7. Defaults are project/scope settings. `0` disables a bound.
8. Gate evaluation is deterministic and returns a typed rejection reason for diagnostics.

## 3. Gate model

```text
SelectionGate
  key:             GateKey
  kind:            Pacing | Count | Flag | Metric | Tag | Custom
  min_completed:   u32       // 0 = no minimum
  max_completed:   u32       // 0 = no maximum
  min_minutes:     f32       // 0 = no minimum
  max_minutes:     f32       // 0 = no maximum
  predicate:       Option<CompiledPredicate>
  shortcut_allowed: bool
```

`PacingGate` is the common case:

```text
PacingGate
  min_completed = project minimum
  min_minutes   = project minimum
  shortcut_allowed = false
```

The `150 storylets / 300 minutes` target is an example project configuration:

```text
profile: ending
gates: [PacingGate { min_completed: 150, min_minutes: 300 }]
```

It is not an engine default and not a required content count.

## 4. Runtime counters

Per scope:

```text
traversal_completed: u32
traversal_minutes:   f32
```

Update only when a traversal storylet completes. Do not count ending/profile storylets, barks, or utility beats unless the project defines them as traversal.

A project may use a different counter source; the gate reads counters, not storylet prose.

## 5. Evaluation

```text
fn gate_passes(scope, profile, ending_shortcut):
    for gate in profile.gates:
        if gate.shortcut_allowed && ending_shortcut: continue
        if gate.min_completed > 0 && scope.traversal_completed < gate.min_completed: return false
        if gate.max_completed > 0 && scope.traversal_completed > gate.max_completed: return false
        if gate.min_minutes > 0 && scope.traversal_minutes < gate.min_minutes: return false
        if gate.max_minutes > 0 && scope.traversal_minutes > gate.max_minutes: return false
        if gate.predicate and not predicate_matches(scope): return false
    return true
```

The profile then applies its eligibility filter and selection policy.

## 6. Validation

The content validator SHALL report, for each profile with gates:

- minimum and maximum reachable path lengths;
- minimum and maximum estimated minutes;
- reachable states where the profile has no eligible candidate;
- dead ends that are not marked as a valid terminal profile;
- profiles whose `min_completed` exceeds their reachable path length.

Failure cases:

| Case | Result |
|---|---|
| Gate minimum exceeds reachable path | warning or error per project policy |
| Profile has no reachable candidate and no `Storylet`/`Continue` fallback | error unless `FallbackPolicy::Error` is explicitly selected |
| Non-terminal dead end | error |
| Gate references an unknown counter/flag | error |
| `min > max` for a gate bound | error |
| Missing duration when a pacing gate requires it | error |
| Pacing disabled (`0` bounds) | valid |

## 7. Acceptance tests

- `gate_empty_allows`: empty gates do not reject.
- `gate_min_completed`: 149 rejects, 150 passes when configured.
- `gate_min_minutes`: 299 rejects, 300 passes when configured.
- `gate_zero_disables`: a zero bound disables that side.
- `gate_shortcut`: an authored shortcut bypasses a gate.
- `gate_no_implicit_shortcut`: missing shortcut cannot bypass.
- `gate_max_bound`: max count/time rejects after the ceiling.
- `gate_unknown_reference`: unknown predicate/counter fails load.
- `gate_diagnostic`: rejection reason is deterministic and typed.
- `gate_zero_ending_profile`: a project with no ending profile has no ending gate.

## 8. Migration

1. Add `SelectionGate` to the profile schema (`REQ-DO-08`).
2. Add scope counters.
3. Add gate evaluation before ranking/drawing.
4. Add validator path analysis.
5. Backfill duration only for content that opts into pacing.
6. Do not add engine-wide 150/300 constants.

## 9. Risks

- Estimated minutes are subjective; use them as authored metadata, not measured wall-clock truth.
- Long cutscenes or non-storylet beats need their own counter policy.
- Per-profile gates can create many exceptions; prefer shared project gates.
- A gate that rejects every candidate needs a fallback policy or an explicit dead-end outcome.
- Gates must not become a second eligibility system; exact predicates still decide hard availability.

## 10. References

- `SelectionProfile` — `REQ-DO-08`.
- `StoryletDef` fields — `crates/storylet-engine/src/chrono_dispatch.rs:226`.
- Service `StoryletRecord` — `crates/storylet-engine-service/src/contracts.rs:285`.
- Existing graph validation — `src/web/workspaceProject.ts` / `narrative_graph.rs`.
- `REQ-DO-06` — ending profile.
- `REQ-DO-10` — catalog/working set.
