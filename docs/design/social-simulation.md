# Social simulation and emergent narrative architecture

## Purpose

This document defines reusable principles for building nonlinear narrative games from authored storylets and a bounded social simulation. It is setting-neutral: projects supply their own actors, institutions, resources, relationships, knowledge, locations, and narrative goals.

The world should feel inhabited before it feels solvable. People encounter one another because they share spaces, services, work, obligations, dependents, and memories. A material change affects what people can do; their motives shape how they respond; witnessed actions change relationships and beliefs; those changing conditions make authored storylets available. The simulation creates access, timing, costs, witnesses, and consequences. Authors retain control over central mysteries, major revelations, and the meaning of authored scenes.

## Core principles

1. **Separate state dimensions.** Do not collapse identity, residence, permissions, service access, reputation, continuity, or institutional scrutiny into one status flag. A project defines only the dimensions its fiction needs. Each dimension has an explicit domain, owner, change rules, and consequences.
2. **Give actors local motives and obligations.** A character should have a small set of concrete needs, commitments, relationships, and constraints. A character's choices follow from their own situation and knowledge, not a global plot script.
3. **Make relationships directional and remembered.** Trust, fear, respect, obligation, affection, and resentment can differ by direction. State changes should retain a reason and relevant event history. Investing in one alliance may weaken another.
4. **Scope knowledge to evidence and witnesses.** Facts have sources, confidence, and provenance. Characters learn through witnessing, accessible records, or explicit communication. Rumor carries its origin and does not become independent confirmation through repetition. Belief remains distinct from world truth.
5. **Model power through systems and intermediaries.** Institutions exert influence through services, infrastructure, policies, access points, and people who administer them. A powerful organization need not appear as a single boss character. Control over a bottleneck creates leverage and competing interests.
6. **Use authored content for narrative intent.** Storylets provide authored voice, scene structure, choices, and outcome contracts. Simulation determines which content can occur and who is affected. Model output can assist authoring or rank eligible content, but it does not invent authoritative effects.
7. **Preserve multiple routes through essential progress.** Important narrative milestones should have more than one authored route when the design promises player resilience. People, alliances, and opportunities may be lost; the system should make the loss legible and provide a designed alternative when required for continued play.
8. **Show consequences and uncertainty.** Record why a character acted, what changed, who observed it, how information spread, and which later events depended on it. Player-facing feedback must distinguish known fact, testimony, rumor, inference, and uncertainty.

## Social simulation model

Represent the world with project-defined actors, locations, resources, institutions, relationships, commitments, schedules, and knowledge. Do not require every project to use a fixed roster or fixed status axes. Actors may be fully simulated, rule-driven, or represented as background population according to the project's scale.

Each advancing action is evaluated against the current world revision and commits atomically as an event. Events can change resources, relationships, permissions, access, beliefs, schedules, and future opportunities. Events record actor, place, turn, state delta, witnesses, evidence sources, rule provenance, and causal parents. Delayed consequences and NPC actions use the same event path as player choices. Duplicate commands are idempotent; stale commands fail without partial mutation.

An actor's available responses are constrained by personal goals, obligations, capability, local conditions, and the actor's information. NPC behavior may be authored as conditions and outcomes, with deterministic ordering and bounded cascades. Movement changes co-presence and possible witnesses; it can alter later knowledge and relationships even when the player does not select a storylet. These indirect chains are the butterfly effect and should remain traceable in the event graph.

## Storylet contract

A storylet is a reusable authored encounter with:

- purpose and tags;
- location and schedule constraints;
- required roles and role-binding rules;
- trigger predicates over project state, relationships, knowledge, time, or event history;
- authored opening, dialogue, action, and interaction;
- choices with explicit requirements and outcome contracts;
- typed effects and emitted knowledge, including scope and source;
- pacing and urgency metadata;
- repeatability, cooldown, lifetime, interruption, and aftermath policy.

The director scans or incrementally indexes the approved storylet library, explains eligibility and rejection, then ranks only eligible candidates. A no-action result is valid. Starting a storylet binds and reserves required roles according to its policy. Suspended instances preserve their cursor and history and revalidate before resuming. Completion, interruption, and aftermath behavior are explicit project data. Seeded shuffling may vary eligible content without making replay nondeterministic.

## Narrative boundaries and player fairness

The player should be able to explain who benefited, who paid a cost, how another person learned about an event, and why an opportunity changed. Journal and recap tools preserve source attribution and uncertainty. Essential information must not depend on one unrepeatable overheard line. If a character or route can be lost, the content contract should state the consequence and the author should decide whether another route remains.

The author controls what is true, what is revealed, and which outcomes are narratively acceptable. The simulation can determine whether evidence is accessible, who trusts the player, whether resources are available, or which witness is present. It should not silently settle an unresolved central mystery or turn every ordinary character into a clue dispenser.

## Prototype and acceptance approach

Begin with one small, connected area and a limited set of resources, actors, relationships, and approved storylets. Exercise one disruption through distinct player actions and NPC responses. Test that:

- different movement or timing produces understandable differences in co-presence, witnesses, and later events;
- an actor never uses information they could not have learned;
- relationship and resource changes retain causes and do not double-apply;
- essential progress has the designed alternative routes;
- a lost opportunity yields a clear, authored outcome;
- event replay and branching reproduce the same state from the same seed and commands.

Expand population, factions, schedules, and simulation detail only after the small scenario is readable and replayable. Measure deterministic engine cost, model inference, and complete turn latency separately. Performance targets are acceptance criteria only after measurement on the intended workload.

## Relationship to the platform

The Rust storylet engine and authoring/playtest studio implement these generic requirements; see [the platform design](storylet-studio.md). Each project supplies its own setting data while the engine and new-project workflow remain setting-neutral.
