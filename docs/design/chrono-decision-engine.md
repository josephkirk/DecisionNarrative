# Chrono Decision Engine

**Status:** Current implementation
**Scope:** Deterministic narrative execution and trusted run operations in `storylet-engine`

## Runtime

`RunKernel` is the single runtime for a play session. `RunContent` supplies the authored graph, dispatcher, registries, mission assets, location connections, clock, and initial `WorldStateTable`. A kernel owns the mutable world and `FlowGraphInstance`; the world and instance are available through read-only accessors.

Graph execution uses dense participant, entity, storylet, and tag indices. `FlowGraphInstance` executes graph nodes in deterministic token order and stores reversible call history. `BijectiveDelta` records reversible state changes. The runtime maintains stable iteration order for candidate selection and replay.

`chrono_triggers`, `chrono_transactions`, `chrono_selection`, `chrono_tables`, and `chrono_features` are typed contracts and indexes that are not wired into the run lifecycle.

## Action catalog and trust boundary

Every external world change is submitted with `RunKernel::propose(content, authority, proposal)`. `ActionProposal` carries its schema version, actor, typed action, expected revision, idempotency key, and optional `DecisionProvenance`. Provenance identifies the policy and model, context hash, and decision sequence.

`Authority` is `Human { seat }`, `Policy`, or `Architect`. Human authority controls only its matching human seat. Policy authority controls rule participants. Architect authority may submit actions for any participant and is required for rewind operations.

A proposal is checked in this order: actor exists, authority controls that actor, revision matches, action is currently available, and action preconditions hold. A rejection restores the serialized kernel exactly. Each accepted proposal advances the revision. A repeated idempotency key with the same authority and proposal body returns its recorded receipt; reusing the key with a different body is rejected.

`RunKernel::rewind(authority, to_step)` requires Architect authority. Rewind restores the selected branch snapshot, including world, graph instance, revision, proposal records, idempotency records, and event log. Event identifiers remain unique as execution continues after a rewind.

## Causal event log

Each accepted step records an event id, step, revision, actor, action, causal parents, witnesses, and `BijectiveDelta` values. Witnesses are participants co-located with the actor when the event commits. Rule-participant plays triggered by a human action are separate events whose parent is that human action; their actor and storylet are recorded independently. Ordinary actions have no assumed causal parent.

`RunKernel::explain(event_id)` returns the requested event and its full ancestor chain in event-log order. Knowledge and telling rows refer to these event identifiers. `inspect().world_changes` is projected from event deltas.

## Participant observation

`RunKernel::observe(content, actor)` returns the actor's current place and legal moves, co-present participants, available actions, own knowledge rows, events caused or witnessed by that actor, and a pending scene when that actor owns the interaction. It does not expose another participant's private knowledge or unrelated event history.

`RunKernel::architect_view(content, actor)` is the global diagnostic view. It includes the run timeline and global scene information for authoring and debugging. Participant clients use `observe`.

## Knowledge, rules, and coordination

Knowledge and telling rows use the same event-id space as the event log. Mission-scoped learn and reveal nodes and successful rule-participant storylets are attributed to the event that produced them. A told knowledge row records its teller in `told_by`.

Rule participants are evaluated in deterministic participant order. Eligibility evaluation failures and storylets that cannot complete without a player choice are recorded in the triggering step's notes. A failed rule storylet is rolled back independently, so the human action still commits.

`Transfer { to, resource, amount }` and `Tell { to, fact }` require co-location. Transfers move item entity references between the existing inventory attributes and preserve the total quantity. A transfer fails if the actor lacks enough items or the recipient inventory has no capacity. Telling succeeds only for a fact the teller already knows and adds a `Told` knowledge row for the listener.

## Replay and native ABI

`RunKernel::replay(content, start, seed, proposals)` starts a kernel and applies the recorded `(Authority, ActionProposal)` sequence through `propose`; it does not call policy code. Replay preserves state, revisions, receipts, and event log when given the same content and seed.

The native ABI exposes `dn_run_start`, `dn_run_propose`, `dn_run_observe`, `dn_run_explain`, `dn_run_architect_view`, `dn_run_free`, and `dn_string_free`. `dn_engine_abi_version` reports ABI version 2. JSON is the wire format for content, starts, proposals, receipts, observations, explanations, and diagnostic views.

## Runtime surface

The crate exposes the Chrono `RunKernel`, typed content and graph structures, action catalog, actor observation, and the `dn_run_*` native interface. The separate `WorldState` command runtime, `StoryletRuntime`, project bundle runtime, legacy engine/playtest ABI, and their host and benchmark binaries are not part of the crate. Native-host example programs and the PowerShell runner are not included.
