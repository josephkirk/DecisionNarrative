# PROJECT MATRIX
## Product Requirements Document
### Architect-Controlled Emergent Worlds

**Product codename:** MATRIX  
**Existing foundation:** [DecisionNarrative](https://github.com/josephkirk/DecisionNarrative)  
**Document version:** 0.1 | Moonshot PRD  
**Status:** Proposed, not yet approved for implementation  
**Planning horizon:** 90-day proof of concept, 12-month product exploration, 3–5-year aspiration

---

# 1. Executive Summary

## 1.1 Vision

**Build a platform where human designers create the laws of a virtual world, autonomous agents discover how to live within those laws, and players create consequences that become part of the world's permanent history.**

Traditional game development largely manufactures content. Designers author quests, writers create dialogue, artists build environments, and gameplay programmers implement predefined interactions.

MATRIX proposes a different model.

The human designer creates the *possibility space*: physical rules, social systems, character capabilities, motivations, environmental constraints, narrative themes, and world-building components.

Autonomous agents then perceive their environment, form goals, build relationships, remember experiences, discover strategies, and interact with other agents.

The simulation determines the real consequences of their actions.

An intelligent narrative director identifies meaningful developments and turns them into playable experiences without fabricating what occurred.

The result is a world where **the designer authors possibility, agents create history, and players change the future.**

## 1.2 The Moonshot

Our long-term aspiration is to create a persistent artificial civilization capable of:

- Developing relationships, institutions, economies, cultural practices, and conflicts through interactions.
- Discovering useful strategies and behaviors that were not explicitly scripted.
- Remembering its history and adapting future decisions based on experience.
- Producing meaningful gameplay through emergent consequences.
- Visually transforming its environments as its society evolves.
- Remaining coherent, inspectable, reproducible, and governed by human-authored world rules.
- Serving as both an entertainment platform and a controlled environment for evaluating autonomous intelligence.

The ambition is not to recreate human consciousness or claim that simulated characters are living beings.

It is to build a sufficiently rich, causally consistent environment that complex and surprising behaviors can emerge from comparatively simple rules and bounded intelligence.

## 1.3 The First Product

The first product is not an infinite virtual universe.

It is a designer-operated simulation called **The Living Village**.

A small population of autonomous characters lives within a constrained world containing scarce resources, locations, basic economic activities, relationships, and limited communication methods.

Characters must solve problems, coordinate, and adapt.

The human designer can inspect why events occurred, change an initial condition, replay the simulation, and compare alternative histories.

A later playable version gives a human player the ability to intervene and experience the resulting consequences.

**First decisive proof:** Agents discover and sustain at least one useful cooperative behavior that was not explicitly prescribed as a sequence of actions.

This must be demonstrated through reproducible experiments and comparisons against simpler scripted-agent baselines.

---

# 2. First-Principles Problem Definition

## 2.1 The Industry's Fundamental Constraint

Traditional authored game content has an expensive relationship with scope.

More locations require more environment work. More quests require more writing, implementation, and testing. More character interactions produce additional animation, dialogue, and integration costs.

Procedural generation reduces some production costs but often struggles to create meaningful causality, narrative coherence, and player attachment.

Generative AI can produce individual content elements cheaply, but generating more text or geometry does not necessarily produce better gameplay.

The underlying problem is:

**How can a game create substantially more meaningful player experiences without requiring proportional increases in handcrafted content?**

## 2.2 Our Hypothesis

We hypothesize that a combination of:

1. Human-authored world rules and experience design.
2. Autonomous agents with persistent goals, memory, and planning.
3. A deterministic, authoritative simulation.
4. A narrative director that recognizes meaningful situations.
5. A procedural presentation and art pipeline.

...can produce coherent experiences at a lower marginal content-authoring cost than conventional authored pipelines for certain game genres.

This hypothesis is not assumed to be true.

MATRIX exists to test it.

## 2.3 The Critical Unknowns

We must resolve five questions in order:

**Can agents act coherently?**  
Can a character choose sensible actions given goals, limited knowledge, and constrained capabilities?

**Can agents interact meaningfully?**  
Can multiple characters cooperate, compete, negotiate, and communicate without fully scripted interactions?

**Can new patterns emerge?**  
Can agents discover useful, unanticipated combinations of permitted actions?

**Can humans enjoy the results?**  
Do emergent consequences create drama, agency, emotional investment, or compelling strategy?

**Can the system operate economically?**  
Can thousands of simulated characters eventually exist without requiring expensive model inference for every action?

If the answer to any fundamental question is no, the design must be simplified, adapted, or abandoned before expanding scope.

---

# 3. Product Aspiration

## 3.1 North-Star Statement

**Create the operating system for believable, persistent, interactive worlds.**

MATRIX should eventually allow creators to specify a universe's rules and artistic identity rather than manually authoring every event within it.

The long-term platform should support multiple worlds and game genres through shared infrastructure, not a single predetermined fictional setting.

## 3.2 Aspirational Capabilities

### Horizon 1: The Living Village

A small simulated community exhibits observable motives, believable relationships, cooperative behavior, and persistent consequences.

The designer can inspect the simulation's causal history.

### Horizon 2: The Living Society

Multiple settlements produce emergent economic relationships, institutions, social hierarchies, conflicts, and collective strategies.

Human players can meaningfully intervene.

### Horizon 3: The Living Civilization

Large populations are represented using hierarchical simulation and intelligence levels.

Agents develop strategies, social structures, shared knowledge, and evolving relationships over long simulated periods.

Different civilizations may develop distinct histories under similar initial conditions.

### Horizon 4: The Matrix

Human designers define extensible world constitutions, visual grammars, action systems, and experience constraints.

Artificial societies evolve within those boundaries while the platform preserves simulation correctness and offers powerful narrative and world-building tools.

The same foundation can support games, interactive stories, experimentation, and potentially AI-agent research.

**Horizon 4 is a research aspiration, not a scheduled feature commitment.**

---

# 4. Product Pillars

## Pillar 1: The Architect
### Human-authored possibility, not predetermined outcomes

The Architect is the designer-facing system for defining the universe.

Designers author:

- World entities, locations, resources, and relationships.
- Physical, economic, and social rules.
- Character abilities and available actions.
- Starting conditions and world constraints.
- Narrative themes and approved storylets.
- Visual and auditory identity.
- Valid and invalid state transitions.

**Core requirement:** Designers must be able to modify the world's possibility space without changing engine source code for ordinary content changes.

**First milestone:** A versioned World Constitution that defines a small settlement's resources, permitted actions, and initial actors.

**Long-term aspiration:** Designers can describe complex worlds through composable rules, constraints, and validated content modules.

## Pillar 2: The Reality Kernel
### One authoritative, causally consistent world

The simulation owns what actually happens.

Every significant action must be validated against the current world state before committing its effects.

The kernel is responsible for:

- Entity identity and state.
- Resources and conservation rules.
- Action preconditions and permissions.
- Deterministic state transitions.
- World time and scheduling.
- Event provenance and causal relationships.
- Save, replay, branching, and rollback support.
- State and simulation invariants.

**Core requirement:** An autonomous model cannot directly write authoritative world state.

**First milestone:** A validated action API that converts permitted agent intentions into trusted state transitions.

**Long-term aspiration:** Large, persistent worlds whose histories can be audited and replayed.

## Pillar 3: Autonomous Life
### Intelligence with motives, memory, and consequences

Agents must be more than dialogue generators or deterministic schedules.

An agent has:

- Identity and capabilities.
- Needs, goals, and preferences.
- Perception limited by the environment.
- Memories and beliefs with provenance.
- Relationships with other agents.
- Plans and anticipated consequences.
- A bounded set of permitted actions.
- Mechanisms for adapting future behavior.

The platform separates **world truth**, **agent belief**, and **agent intention**.

Agents may misunderstand the world, disagree, deceive, cooperate, or revise beliefs. They cannot override world rules by declaring a desired outcome.

**First milestone:** Three agents with conflicting goals, private knowledge, and memory-driven decisions.

**Long-term aspiration:** Large societies with differentiated, computationally affordable levels of intelligence.

## Pillar 4: The Oracle
### Turn real consequences into meaningful experiences

A simulation can produce interesting events without producing good entertainment.

The Oracle bridges that gap.

It identifies emerging situations with dramatic or strategic potential and decides how they should be presented.

Responsibilities include:

- Recognizing conflicts, opportunities, and significant developments.
- Ranking eligible authored storylets.
- Identifying meaningful player interventions.
- Maintaining narrative pacing.
- Explaining event significance using verified causal evidence.
- Allowing uneventful periods where appropriate.

The Oracle must not fabricate authoritative events to satisfy a narrative objective.

**First milestone:** Existing DecisionNarrative storylets react to changes produced by autonomous agents.

**Long-term aspiration:** A director capable of presenting unexpected histories as coherent, emotionally meaningful interactive experiences.

## Pillar 5: The World Compiler
### World state becomes visual content

The world should visibly change when its state changes.

Visual production begins with a human-authored design language, not unconstrained generation.

Inputs include settlement wealth, architecture, faction identity, environment condition, damage, resources, and important events.

The system uses reusable assets, procedural composition, approved generative workflows, and validation to express these changes.

**First milestone:** A simple 2D world map that updates automatically from simulated events.

**Long-term aspiration:** A production-grade OpenUSD-centered content pipeline capable of translating structured world state into coherent, high-quality 3D environments.

## Pillar 6: EmergenceGym
### Measure intelligence, not just apparent intelligence

MATRIX must distinguish genuine adaptive behavior from scripted interactions and plausible-sounding explanations.

EmergenceGym provides controlled scenarios, reproducible experiments, and baseline comparisons.

It measures:

- Goal achievement.
- Novel strategy discovery.
- Cooperation and coordination.
- Belief accuracy.
- Long-horizon consistency.
- Causal validity.
- Adaptation after failure.
- Compute and inference efficiency.
- Simulation boundary enforcement.

**First milestone:** Repeatable multi-agent coordination experiments with scripted, memoryless, and memory-enabled baselines.

**Long-term aspiration:** A model-independent benchmark for autonomous behavior in persistent simulated worlds.

---

# 5. Product Tenets

These principles govern architecture and product decisions.

| # | Tenet | Decision rule |
|---|---|---|
| 1 | **The Architect defines possibility; agents choose outcomes.** | Never substitute uncontrolled model output for established world laws. |
| 2 | **Reality is authoritative; intelligence is advisory.** | Models propose intentions. Trusted simulation code validates and applies effects. |
| 3 | **Emergence must be discovered, not pretended.** | Do not call an interaction emergent if its behavior was directly scripted or artificially imposed. |
| 4 | **Memory is evidence-based.** | Keep world truth, witnessed events, rumor, belief, and inference distinct. |
| 5 | **Every consequence has a cause.** | Significant state changes require traceable events and provenance. |
| 6 | **Reproducibility is a product feature.** | Record model decisions and simulation inputs so histories can be replayed without re-inference. |
| 7 | **Intelligence is a scarce resource.** | Use deterministic policies for routine behavior and expensive models only when valuable. |
| 8 | **Humans remain creative authorities.** | Designers own world rules, artistic standards, essential narrative truths, and production approvals. |
| 9 | **The world must be enjoyable, not just impressive.** | Player value overrides simulation complexity and agent sophistication. |
| 10 | **Safety comes from system boundaries, not promises.** | Agents receive only in-world capabilities. Host infrastructure is not an agent playground. |
| 11 | **Build vertically, not universally.** | Prove one narrow experience before developing generic systems for every genre. |
| 12 | **Delete before automating.** | Prefer fewer systems, fewer model calls, and fewer authored dependencies over sophisticated unnecessary automation. |

A further engineering rule applies to every milestone:

**No new complexity without a measured increase in product value, correctness, or efficiency.**

---

# 6. Target Users and Jobs to Be Done

## Primary User: The World Architect

A narrative designer, game designer, or systems designer who wants to create a living world.

**Job:** Define a compelling possibility space, run it, understand unexpected outcomes, and shape the experience without hand-authoring every sequence.

Needs: accessible rule authoring, causal inspection, simulation control, narrative tools, and quality validation.

## Secondary User: The Player

A person who wants agency in a world that responds meaningfully to their decisions.

**Job:** Participate in, influence, and discover a world that continues to exist beyond predefined quests.

Needs: readable consequences, meaningful interventions, memorable characters, understandable choices, and consistent world behavior.

## Tertiary User: The AI Researcher or Agent Engineer

A developer evaluating autonomous behavior over extended interactions.

**Job:** Compare agent policies in controlled environments and understand why strategies succeed or fail.

Needs: reproducible scenarios, observable state, objective metrics, model independence, bounded execution, and exportable evaluation records.

**Launch priority:** Architect first, then player. Research tooling should strengthen internal evaluation before becoming a separately marketed product.

---

# 7. Core Product Experience

## 7.1 The Architect Loop

1. Define a world constitution and desired experience.
2. Create or import characters, locations, resources, and actions.
3. Specify goals, constraints, storylets, and initial conditions.
4. Validate the project against world and content schemas.
5. Launch a simulation.
6. Observe events, agent behavior, and emerging relationships.
7. Inspect causal history and agent reasoning records.
8. Fork the world, intervene, or change initial conditions.
9. Compare outcomes and refine the authored rules.

The system must make a distinction between changes to the world configuration and actions taken within a running world.

World-rule changes create a versioned configuration or migration, never a silent rewriting of history.

## 7.2 The Agent Loop

**Perceive → Retrieve → Decide → Propose → Validate → Act → Remember → Reflect**

The world produces actor-specific observations.

The agent retrieves relevant memories and current goals, selects an intention, and submits a structured action proposal.

The Reality Kernel validates the action and commits any consequences.

The result becomes available to observers according to the world's knowledge rules.

The agent updates its memory, may revise its plan, and returns to its next activity.

Reflection should not run after every trivial action. It should be triggered by significant events, accumulated experience, or planning failures.

## 7.3 The Player Loop

**Observe → Understand → Intervene → Experience Consequences → Discover New Opportunities**

The player must be able to understand the connection between their decisions and resulting world changes.

The game does not require every consequence to happen immediately, but important delayed effects should become discoverable through the world's ordinary information systems.

---

# 8. Minimum Viable World: The Living Village

## 8.1 Initial Scenario

Three districts are connected by travel routes.

Each district has access to different resources, facilities, and services.

Characters have individual needs and goals. Cooperation can be beneficial, but communication and travel require time.

The player may eventually intervene by changing resource availability, introducing information, helping characters, or disrupting an institution.

The prototype begins with three agents, expands to six to eight, and only then targets 24.

## 8.2 Minimal World Systems

| System | Minimum implementation |
|---|---|
| Time | Discrete, deterministic simulation turns |
| Location | Connected regions with permitted movement |
| Resources | Food, materials, and one utility resource |
| Needs | Basic consumption and unmet-need penalties |
| Actions | Move, gather, transfer, use, observe, communicate |
| Relationships | Directional trust and cooperation history |
| Knowledge | Actor-specific observations, memory, and beliefs |
| Planning | Goal selection and short action sequences |
| Communication | In-person and approved asynchronous in-world methods |
| Narrative | A few approved storylets triggered by simulation events |
| Persistence | Snapshots, event history, forks, and exact replay |

The first simulation does not need combat, advanced physics, realistic economics, complex pathfinding, detailed animation, or a complete 3D environment.

## 8.3 First Emergence Experiment

**Experiment: Can residents develop a shared communication practice without being instructed to create one?**

Designers provide generic affordances such as writing, carrying objects, placing messages, observing shared spaces, and interacting with other agents.

They do not provide a hardcoded `CreatePostalSystem` action, a scripted institution-formation trigger, or a prompt instructing agents to invent a mail service.

Agents receive tasks where communication improves outcomes.

An experiment is successful when an observer can demonstrate:

- Agents discover a repeatable coordination strategy using permitted actions.
- The strategy provides a measurable advantage over an appropriate baseline.
- Multiple agents adopt or use it over time.
- Its existence and operation can be explained through the event history.
- Removing a critical component produces understandable changes in behavior.

The system must record failed experiments as well as successful ones.

Novel behavior is a hypothesis to test, not an outcome the project is allowed to manufacture for a demonstration.

---

# 9. Functional Requirements

## P0: Required for the First Proof

**FR-01: Trusted Action Catalog**

Agents submit typed intentions with action IDs and arguments, not arbitrary world effects.

The kernel validates actor authority, preconditions, resources, and expected world revision.

*Acceptance:* Unauthorized agent proposals cause zero authoritative state mutations across the test suite.

**FR-02: Actor-Specific Perception**

Each agent receives only information accessible through its position, capabilities, observations, or permitted communication.

*Acceptance:* Agents cannot obtain private facts from global simulation state through the observation API.

**FR-03: Persistent Evidence-Backed Memory**

Agents record observations, communications, conclusions, and significant outcomes with source references.

*Acceptance:* A belief can be traced to evidence; a false rumor does not become world truth merely because agents repeat it.

**FR-04: Structured Goals and Plans**

An agent can propose a goal, select valid actions, observe results, and replan after failure.

*Acceptance:* The agent can complete a simple multistep task requiring at least one intermediate action without a hardcoded task-specific script.

**FR-05: Deterministic Action Execution and Replay**

All committed actions flow through the trusted Rust world kernel.

External model responses are logged as decision inputs with model/prompt provenance.

*Acceptance:* Replaying a recorded run produces identical authoritative state and event history without invoking the model again.

**FR-06: Multi-Agent Coordination**

Agents communicate and make decisions based on information exchanged through permitted in-world mechanisms.

*Acceptance:* Agents can complete a cooperative resource-transfer task unavailable to either actor alone.

**FR-07: Causal Inspection**

The Architect can inspect why an event happened, its parents, affected entities, and relevant observations.

*Acceptance:* A world change can be traced back to the actor action and initial condition that caused it.

**FR-08: Sandbox Isolation and Resource Budgets**

Agent cognition operates without access to host credentials, production databases, unrestricted filesystem operations, arbitrary code execution, or external network tools.

*Acceptance:* Adversarial agent proposals cannot cross system boundaries and cannot exhaust unbounded compute or retry budgets.

## P1: Required for the Living Village

**FR-09: Resource Economy**

Consumption, production, transfer, and scarcity create real constraints.

**FR-10: Social Relationships**

Actor interactions change trust, obligation, and future decisions through explicit rules.

**FR-11: Narrative Director Integration**

Simulation events expose eligible authored storylets without allowing the Director to fabricate effects.

**FR-12: World Constitution Editor**

Designers configure world rules, action affordances, actors, and starting conditions through validated project data.

**FR-13: Counterfactual Comparison**

The Architect can fork a snapshot, change an intervention, rerun the world, and inspect divergences.

**FR-14: Evaluation Harness**

Run fixed scenario suites against scripted, utility-based, memoryless-model, and memory-enabled agent policies.

## P2: Required for the Playable Product

**FR-15:** State-driven 2D or 3D world presentation.

**FR-16:** Meaningful player actions and readable consequences.

**FR-17:** Narrative pacing and situation significance evaluation.

**FR-18:** Automated visual and gameplay regression checks.

**FR-19:** Hierarchical simulation and intelligence budgets for larger populations.

**FR-20:** Portable world projects and production-ready engine integration.

---

# 10. Technical Architecture and DecisionNarrative Evolution

## 10.1 Existing Foundation to Preserve

DecisionNarrative already includes:

- A host-independent Rust storylet runtime.
- Transactional commands and world revisions.
- Deterministic storylet eligibility and selection.
- NPC behavior rules.
- Causal events, session export, restore, and forks.
- Designer-approved generative authoring.
- An optional semantic ranking layer.
- A native C ABI for embedding.
- A local web studio with Director and Butterfly inspection.

These should remain the foundation rather than be rewritten.

The project's existing decision that model ranking cannot bypass Rust hard eligibility should be extended to all autonomous action execution.

## 10.2 Architecture Boundary

**Architect-owned configuration**

World rules, action schemas, content approvals, narrative boundaries, and visual grammars.

**Reality Kernel: Rust**

Authoritative world state, deterministic systems, action validation, event commits, persistence contracts, and replay.

**Cognition Layer**

Model-independent perception, memory retrieval, goal generation, planning, reflection, and action proposals.

**Oracle**

Storylet eligibility, dramatic relevance, narrative presentation, and inspectable ranking.

**World Compiler**

Maps structured world state to visual assets, scene arrangements, animation requirements, and interactive presentations.

**Evaluation Platform**

Runs experiments, baseline comparisons, adversarial tests, performance profiling, and counterfactual analysis.

The layers communicate through explicit, versioned data contracts.

## 10.3 Proposed Repository Changes

| Existing or proposed path | Direction |
|---|---|
| `crates/storylet-engine/src/lib.rs` | Preserve world authority; evolve typed state and event contracts |
| `crates/storylet-engine/src/simulation.rs` | Preserve scripted NPC rules; introduce a separate validated autonomous-action path |
| `crates/storylet-engine/src/actions.rs` | New: action definitions, permission checks, and trusted effect resolution |
| `crates/storylet-engine/src/observation.rs` | New: actor-specific world observations and evidence |
| `crates/storylet-engine/src/social.rs` | New: structured beliefs, relationships, obligations, and information propagation |
| `crates/agent-cognition/` | New: model-independent planning and memory orchestration |
| `crates/emergence-gym/` | New: experiments, comparisons, metrics, and evaluation |
| `src/web/` | Extend existing Architect, Director, Butterfly, and simulation UX |
| `model/` | Preserve ranking research; add evaluated cognition policies separately |
| `examples/` | Add small canonical world scenarios and integration examples |

New paths are architectural proposals. Implementation should preserve project compatibility where feasible and version any breaking changes.

## 10.4 Replay Contract

LLM outputs are not assumed deterministic.

Each accepted decision records the model version, policy version, relevant context hash, proposed action, decision sequence, and other provenance needed for reproduction.

A replay consumes the recorded action decisions, not fresh model responses.

The deterministic kernel must reproduce committed effects exactly.

Counterfactual experiments are new branches and may produce different agent decisions; they must not be mistaken for exact replays.

## 10.5 Learning Boundary

There are three separate forms of change:

- **World evolution:** Characters and environments change through legal simulation actions.
- **Behavioral learning:** Agent policies improve through controlled evaluation, with versioned deployments.
- **Engine evolution:** System code and world-rule capabilities change through reviewed development.

Agents do not autonomously deploy new engine code or change their own permission boundaries.

---

# 11. Small, Achievable Problems: The Problem Ladder

The moonshot should be decomposed into progressively harder proofs.

| Level | Small problem to solve | Minimal demonstration | Pass condition |
|---|---|---|---|
| 0 | Can the world reject an impossible action? | Agent requests an unavailable resource | No mutation, clear rejection |
| 1 | Can an agent observe correctly? | Two actors see different events | Private knowledge stays private |
| 2 | Can an agent remember? | Actor encounters a prior trading partner | Previous encounter influences a later decision |
| 3 | Can an agent plan? | Obtain a resource requiring two actions | Valid goal-directed sequence |
| 4 | Can an agent recover? | Planned resource becomes unavailable | Agent revises or abandons the plan |
| 5 | Can agents cooperate? | Two agents possess complementary capabilities | Shared task completed |
| 6 | Can information spread? | One actor discovers a shortage | Knowledge propagates only through valid channels |
| 7 | Can a useful convention emerge? | Communication and trade across districts | Reusable, unscripted coordination pattern |
| 8 | Can the world explain its history? | Inspect the origin of an institution | Traceable causal chain |
| 9 | Can a player change its future? | Remove or modify a critical world condition | Reproducible, meaningful divergence |
| 10 | Can the Oracle make it engaging? | Present the conflict through a storylet | Players understand the stakes and consequences |
| 11 | Can the world visually transform? | Settlement changes after events | State-driven, coherent representation |
| 12 | Can the same system scale? | Repeat with larger populations | Acceptable cost and invariant preservation |

**Do not skip levels because a more impressive demo is easier to generate.**

The point is to eliminate fundamental uncertainties in sequence.

---

# 12. Delivery Roadmap

## Phase 0: Foundation and Safety

**Proposed duration:** Week 1

Objectives:

- Document existing runtime invariants.
- Separate trusted world effects from untrusted agent intentions.
- Add action validation and permission checks.
- Establish deterministic test fixtures and benchmark baselines.

**Release gate:** An agent cannot create unauthorized effects, and existing deterministic replay and storylet tests still pass.

## Phase 1: The Thinking Character

**Proposed duration:** Weeks 2–4

Objectives:

- Introduce perception and actor-specific evidence.
- Implement minimal memory and belief stores.
- Add structured goals and bounded planning.
- Connect one cognition provider through a model-independent interface.
- Support three agents in one small environment.

**Release gate:** Agents can pursue goals, remember consequential events, fail safely, and complete simple multistep actions.

## Phase 2: The Cooperative Village

**Proposed duration:** Weeks 5–8

Objectives:

- Add a small resource economy.
- Introduce six to eight agents with complementary capabilities.
- Model communication, cooperation, and conflicts.
- Implement basic experiment fixtures and baseline policies.
- Compare agents with and without memory.

**Release gate:** Reproducible cooperative behavior is observable and causally attributable.

## Phase 3: The Emergence Experiment

**Proposed duration:** Weeks 9–12

Objectives:

- Expand the scenario toward 24 agents.
- Introduce shared communication affordances.
- Test emergence of useful conventions.
- Add repeated runs with held-out initial conditions.
- Measure compute cost, novelty, and behavioral usefulness.
- Integrate early Oracle event recognition.

**Release gate:** A credible experimental report demonstrates whether the emergence hypothesis is supported, with baselines and failures disclosed.

This phase can legitimately conclude that the proposed behavior did not emerge.

That is valuable research evidence, not a reason to misrepresent the result.

## Phase 4: The Playable World

**Proposed duration:** Months 4–6

Objectives:

- Add an attractive 2D or lightweight 3D presentation.
- Introduce player intervention.
- Extend Architect world-authoring UX.
- Create player-facing causal feedback.
- Playtest enjoyment, comprehension, and replay value.

**Release gate:** External testers voluntarily engage with the experience, understand meaningful consequences, and demonstrate interest beyond the AI novelty.

## Phase 5: The Living Society

**Exploratory horizon:** Months 6–12

Objectives:

- Multiple settlements and emerging institutions.
- Hierarchical population simulation.
- More sophisticated world-state presentation.
- Reusable rules and visual grammar.
- Improved policy evaluation.
- Commercial product and research-platform positioning.

**Release gate:** Technical economics and player demand justify an expanded product.

Long-term development continues only if earlier results support it.

---

# 13. First Engineering Backlog

These tasks should be small enough for focused implementation and code review.

| Order | Ticket | Deliverable |
|---|---|---|
| 1 | Define the agent action proposal schema | Versioned request format with actor, action, arguments, revision, and provenance |
| 2 | Add action allowlisting and authorization | Trusted resolver blocks invalid or unauthorized proposals |
| 3 | Introduce actor observation snapshots | A character receives only permitted world information |
| 4 | Create an evidence-backed memory record | Observed events and sources persist across decisions |
| 5 | Add a simple goal-driven planner | A character solves a two-step objective |
| 6 | Record cognition decision envelopes | Decisions can be replayed without another model call |
| 7 | Build a two-agent resource exchange | Cooperation follows valid resource transactions |
| 8 | Add shared in-world messaging | Characters can communicate through world-authorized affordances |
| 9 | Create three baseline policies | Scripted, utility-based, and model-driven behavior can be compared |
| 10 | Add a causal inspection scenario | A designer can trace cooperation back to its causes |
| 11 | Connect an existing storylet to the experiment | Real simulated events unlock authored narrative presentation |
| 12 | Run a controlled multi-seed experiment | Results, failures, costs, and comparisons are reproducible |

The initial execution order should favor correctness and a small playable demonstration over generalized agent tooling.

A highly capable agent can help implement these tickets, but completion requires tests and human architectural review, not merely generated code.

---

# 14. Success Metrics

## 14.1 Non-Negotiable Correctness Gates

- Zero unauthorized world mutations in the defined adversarial test suite.
- 100% exact replay consistency for recorded deterministic-core tests.
- No agent access to forbidden host capabilities.
- No unbounded agent action or inference loops.
- No untraceable authoritative world effects.

These are engineering gates, not statistical claims that all defects have been eliminated.

## 14.2 Intelligence and Emergence Metrics

Measure goal completion, adaptation after failed plans, cooperation efficiency, belief accuracy, and strategy reuse.

For emergence experiments, evaluate whether an observed coordination practice is:

1. Not explicitly scripted.
2. Valid under world rules.
3. Adopted or reproduced by more than one agent.
4. Useful relative to an appropriate baseline.
5. Traceable to observable interactions.

Use held-out scenarios, controlled seeds, repeated trials, and multiple policy configurations to reduce benchmark overfitting.

## 14.3 Entertainment Metrics

During external playtests, measure:

- Whether players understand meaningful consequences.
- Whether they care about individual characters or outcomes.
- Whether they voluntarily continue or replay.
- Whether their interventions produce satisfying agency.
- Whether they prefer the experience to a simpler scripted baseline.

Human evaluation should distinguish genuine engagement from temporary fascination with AI novelty.

## 14.4 Production Economics

Track:

- Inference cost per agent decision.
- Inference cost per simulated day.
- Total cost per player-hour.
- CPU time per world update.
- Memory and event-log growth.
- Human time to author a new scenario.
- Human time to debug unexpected outcomes.
- Cost per approved, playable world scene.

Initial numeric targets must be established after profiling representative workloads. Existing deterministic-core benchmarks must not be represented as end-to-end autonomous-agent performance.

---

# 15. Primary Risks and Mitigations

| Risk | Mitigation |
|---|---|
| Emergent behavior never appears | Test narrowly; preserve simpler utility-based and authored gameplay alternatives |
| Agents generate plausible but incoherent behavior | Typed actions, evidence-backed memory, causal validation |
| Agent autonomy exceeds permitted boundaries | Capability isolation, authorization, resource limits, adversarial tests |
| LLM usage becomes prohibitively expensive | Hierarchical intelligence, caching, compact policies, event-driven inference |
| Emergent worlds aren't fun | Human-designed gameplay loops, Oracle, repeated player tests |
| The system becomes too generic | Deliver one high-quality scenario before platform expansion |
| AI-generated art lacks coherence | Human-approved visual grammar, reusable assets, state-driven composition |
| The benchmark rewards exploits | Held-out tasks, invariant checks, external review, anti-reward-hacking tests |
| World histories become expensive to store | Snapshots, event partitioning, retention policies, archived simulation states |
| Project evolution breaks existing DecisionNarrative users | Versioned contracts, schema migrations, regression tests, compatibility tests |
| Third-party content or model licensing is unclear | Explicit provenance, rights review, approved production inputs |

The Hugging Face agent incident provides motivation for testing unforeseen coordination and objective-driven boundary violations, but MATRIX must study these behaviors in its own isolated simulation rather than expose real external systems.

---

# 16. Non-Goals

The first 12 months do not require:

- Simulating human consciousness.
- Building a proprietary foundation language model.
- Replacing Unreal Engine or developing a custom photorealistic renderer.
- A universal physical simulation.
- Fully generated cinematic-quality characters and environments.
- A massive multiplayer metaverse.
- Autonomous modification of production engine source code.
- Unrestricted agent access to external websites, machines, or services.
- A complete virtual economy with real-money financial instruments.
- A creator marketplace or large-scale commercial live service.
- Perfectly realistic human psychology.
- Fully autonomous narrative generation without editorial governance.

These are not prerequisites for validating the central hypothesis.

---

# 17. Strategic Commercial Direction

MATRIX should initially prove value through a compelling game experience.

The recommended starting market is players interested in simulation, emergent storytelling, colony management, mystery, and systemic strategy.

The first commercial offering, if playtesting supports it, should be a focused premium experience rather than a massive free-to-play platform.

If the core technology becomes reliable and sufficiently general, two adjacent opportunities can be explored.

**Creator technology:** A tool that allows other studios and designers to develop causally consistent, autonomous narrative worlds.

**AI evaluation infrastructure:** A controlled environment for comparing autonomous-agent reasoning, planning, cooperation, memory, and adaptation.

Neither should distract the initial team before demonstrating player value.

The strategic advantage will not come from attaching an LLM to an NPC.

It will come from the integrated capability to **author, simulate, understand, and present worlds where meaningful events emerge from autonomous behavior**.

---

# 18. Investment and Go/No-Go Principles

The company should fund the project through progressively larger proofs.

### Gate A: Valid Autonomous Action

Can an agent safely interact with the simulation and pursue a simple goal?

If not, stop expanding the cognitive layer.

### Gate B: Meaningful Multi-Agent Coordination

Can agents cooperate under actual world constraints, rather than simply describe cooperation?

If not, improve the action and goal architecture before adding more agents.

### Gate C: Emergence

Can the system produce useful, unanticipated interaction patterns that survive controlled evaluation?

If not, determine whether simpler authored or utility-based behaviors create equal or better player value.

### Gate D: Entertainment

Does the simulation create a genuinely compelling experience for people who are not impressed by AI technology for its own sake?

If not, refine game design before expanding scale.

### Gate E: Economics

Can the system maintain quality and reliability at a cost compatible with the intended business model?

If not, improve intelligence allocation and the production pipeline before pursuing larger worlds.

The project should not graduate to the next stage based solely on a visually impressive demonstration.

---

# 19. Definition of the First Breakthrough

MATRIX reaches its first meaningful breakthrough when a small society:

1. Receives a human-authored world constitution.
2. Experiences a problem with real simulated consequences.
3. Discovers a useful strategy through autonomous interaction.
4. Preserves that strategy as usable individual or shared knowledge.
5. Adapts when the player changes a critical condition.
6. Produces a causally reproducible history.
7. Creates a situation that is compelling for a human observer or participant.

The designer should be able to inspect the entire history and answer:

**Why did this happen?**

The player should be able to intervene and ask:

**What if I had done something different?**

And the world should provide a real, causally grounded answer.

---

# 20. The Founding Promise

**We are not building infinite AI-generated content.**

We are building a new way to create interactive experiences.

Human creators define worlds with intention, structure, meaning, and artistic identity.

Autonomous characters act within those worlds according to their own goals, knowledge, and circumstances.

The simulation preserves the consequences.

The narrative system discovers the drama.

Players change the history.

Our long-term measure of success is not how many assets, dialogues, or quests an AI model can generate.

It is how much **meaningful, coherent, emotionally engaging gameplay** can emerge from a small amount of carefully authored possibility.

**The Architect creates the laws.**

**The agents create the history.**

**The player changes the future.**

*Project MATRIX: Don't author every story. Build a world capable of having its own.*
