# Storylet Generative Engine and Designer Studio

**Status:** Current product and system design

**Updated:** 2026-10-14

## Summary

DecisionNarrative is a reusable storylet authoring and playtest platform. Designers create characters, locations, qualities, events, NPC behavior, and approved storylets for projects they own. Strata generates candidate scene content during authoring. Designers inspect and approve both the executable storylet contract and its prose before the runtime can use them. During playtest, the engine selects from approved content, simulates autonomous NPC actions, and records why the world changed.

The runtime is a host-independent Rust library. A Rust local service uses that library for the web studio and persistence. A C ABI and native sample host demonstrate engine embedding. The web studio uses the existing TypeScript and Bun toolchain. A local Python service runs EmbeddingGemma 2. New projects and runtime code are setting-neutral; projects provide their own cast, institutions, state vocabulary, and rules.

The product is a local studio, not a hosted collaboration service. Playtests advance in action-driven turns. Designers read scenes and navigate a location map, then inspect storylet selection and causal consequences in dedicated views.

## User needs and success

The primary user is a narrative designer who can describe a scene in ordinary language, review and edit its trigger and effect rules, generate dialogue and action, and test how the scene combines with other approved content. Success means a second, unrelated story project can use the same engine without runtime code edits.

The engine must make consequences inspectable. A designer can see why a scene was eligible, which roles were bound, which model or rule selected it, which events changed state, who observed those events, and how later effects followed. A playtest can be saved, replayed, rewound into a branch, and compared with another branch.

Literary quality is a production goal rather than a model guarantee. The workflow should encourage concrete, legible, character-specific scenes, meaningful choices, and implied connective action. Designers own final approval.

## Decisions and scope

The following choices are fixed for this design:

- Local web studio with persistent local projects; no hosted accounts or team collaboration in this release.
- Rust core and local HTTP service; Bun-built TypeScript studio; Python model service and training scripts managed with uv.
- Action-driven simulation: moving, waiting, and resolving a player choice advance world time. Reading and inspection do not.
- Strata generates at authoring time. Only designer-approved content is playable.
- EmbeddingGemma 2 is the starting encoder for narrative relevance; the deliverable includes a trained adapter and ranking head, reproducible training inputs, evaluation, and benchmarks.
- Playtest uses text scenes, a location map, director inspection, and a butterfly-effect canvas. A navigable 3D view is outside the first release.
- The Rust deliverable includes a reusable library, stable C ABI, headers, and a native host example. A full Unreal Editor plugin is a later integration.
- Sample projects are optional imports and are never automatically loaded into new projects.

The first release includes project and scenario authoring, entity and storylet management, staged generation and approval, persistent playtest sessions, autonomous NPC actions, storylet lifecycle management, decision ranking, replay and branching, model training and evaluation, import/export, and the native SDK example.

The first release excludes hosted collaboration, a full Unreal plugin, combat simulation, real-time ticking while scenes are read, open-ended generation during play, and inference that changes authoritative world state. Project-specific unresolved story questions remain under the designer's control and are not settled by the platform.

## System architecture

### Rust engine library

The engine owns versioned project contracts, predicate evaluation, role binding, choice validation, effect application, world revisions, event provenance, autonomous simulation, storylet instance lifetimes, candidate evaluation, decision-result validation, deterministic replay, and forks. It has no dependency on HTTP, SQLite, Python, browser APIs, or Strata.

Core operations accept typed Rust structures and return typed results. World commands include an idempotency key and expected world revision. A command that is stale, invalid, or unaffordable returns a typed error and makes no partial state change. All state changes go through one transaction path.

The engine is built as a Rust library with a stable C ABI facade. The ABI uses opaque playtest handles and UTF-8 JSON buffers for broad schema compatibility. It exports scenario start, snapshot, evaluation, semantic ranking, externally resolved decisions, world commands, storylet choice execution, session export/restore, and explicit buffer-free operations. Inputs are borrowed during calls; each returned buffer is released exactly once with the matching free function. The generic native sample host invokes a complete scenario through this ABI. A .NET P/Invoke smoke test verifies this cross-language path; C++/Unreal toolchain compilation remains a platform integration check.

### Rust local service

The local service serves the API, stores projects and sessions in SQLite, hosts generation jobs, communicates with configured local model services, and calls the Rust core. Use Axum and Tokio for HTTP and asynchronous jobs, and rusqlite for SQLite access. Bind to loopback by default. Configure ports and model endpoints through local settings rather than hardcoding model addresses.

Persistent changes are committed atomically before the service acknowledges a world command. Session state is loaded from a snapshot plus later events. Migration and project import run transactionally and preserve a recoverable original database or source bundle.

### Web studio

The browser client remains TypeScript built with Bun. Define API payloads from versioned Rust schemas and generate TypeScript types from those contracts. Keep the existing user-facing intent—project codex, storylet workbench, director, and butterfly trace—but reorganize the controller into focused modules. Replace the current playtest viewport with readable scene text and a clickable location map; retain map positions and participant locations as inspectable simulation data.

### Model services

The local Python process loads EmbeddingGemma 2 and serves query and document embeddings. Unsloth manages the fine-tuning path. Rust performs final ranking using returned embeddings and a versioned trained ranking head. Strata serves the authoring pipeline through a configured local endpoint. Neither service owns authoritative world state.

## Data and public contracts

Project content is independent from runtime sessions. Project bundles are versioned, portable JSON documents containing metadata, qualities, resources, characters, locations, relationships, events, NPC behavior rules, storylets, style resources, and scenario definitions. Stable IDs link records. Entity references are validated when a project is saved or approved.

Core contracts are:

- **ProjectBundle:** bundle format version, project settings, content definitions, assets or asset references, and model/style metadata.
- **StoryletDefinition:** stable ID and revision, purpose, tags, location constraints, role queries, trigger predicates, generation status, scene content, choices, explicit choice requirements and effects, pacing values, and lifetime policy.
- **StoryletInstance:** definition revision, bound roles, status, cursor, start turn, expiry, cooldown, reservations, interruption state, and causal event IDs.
- **WorldCommand:** session ID, idempotency key, expected revision, actor, action, and input payload.
- **WorldEvent:** event ID, turn, type, actor, location, structured state delta, actual witnesses, observation provenance, rule/model provenance, and causal parent IDs.
- **WorldSnapshot:** project and content revision, full world state, active and queued storylet instances, delayed events, schedule state, and deterministic random state.
- **DecisionRequest / DecisionResult:** candidate IDs and features, context revision, ranking output, normalized candidate probabilities, urgency score, actionability gate, confidence, source, and model version.
- **GenerationJob:** inputs, current stage, stage outputs, candidate provenance, audit findings, failures, and eventual approval revision.

All HTTP resources are under /api/v1. Provide project, entity, storylet, approval, scenario, generation-job, session, command, snapshot, event, and fork endpoints. Use SSE for generation progress and session changes. A command carries an expected revision and idempotency key. Reject stale commands with a conflict response, and never partially apply them.

Storylet predicates use typed operators against defined qualities, entity properties, location, schedule, relationship, status, time, evidence, and event history. Do not evaluate user-authored JavaScript, arbitrary SQL, or arbitrary string expressions. Effects declare whether they set, add, transfer, reserve, release, schedule, or emit an observation. Source and destination balances for transfers must reconcile.

## Authoring and generation behavior

Designers may enter a scene description, required roles, trigger conditions, and desired outcomes in plain language. The system converts this brief into editable structured logic. It shows the proposed rules before generation and flags unknown properties, unsupported operations, missing references, or non-deterministic conditions. Designers can edit the rules directly. Generation uses the current edited contract.

Generation follows nine resumable stages:

1. Gather the project's approved canon, style, character voice, relationship, location, and knowledge constraints.
2. Establish or accept a specific physical or situational seed.
3. Produce three distinct premise candidates, each with a strength and a concern.
4. Plan the scene's character wants, conflict, subtext, event sequence, and outcome shape.
5. Draft or refine the structured storylet skeleton and its outcome contracts.
6. Produce three scene variants with dialogue, action, and interaction constrained by the contract.
7. Audit continuity, point of view, character voice, choice clarity, branch consequences, and prose patterns.
8. Revise the chosen candidate while preserving its logic contract.
9. Present a side-by-side review package for designer approval.

Every stage output is persisted with its inputs, prompt/template version, provider/model identity, response, timestamps, and audit findings. If an upstream input changes, mark dependent outputs stale. Limit automatic repair attempts to two retries per failed generation call; after that, mark the job failed with a readable error. A failed job never silently creates procedural prose. Approval explicitly covers both content and executable rules. Only approved revisions may enter runtime bundles.

Adapt relevant story-skills as versioned prompt and review resources: character management for goals, boundaries, relationships, and voice; worldbuilding for setting facts; scene craft for wants, conflict, subtext, and outcome; interactive fiction for choice legibility and branch continuity; voice-style and line-editing for distinct dialogue and prose audits; revision-continuity for knowledge, event, and relationship consistency. Translate chapter-centric guidance into scene and playtest context. Keep project canon separate from general craft resources. Store source references and content hashes; do not run source-skill maintenance scripts as part of the product.

Apply Unslop as a quality and revision process: generate alternatives, compare strengths and concerns, audit specified style patterns, revise, and retain a review trail. The contest-specific premise in the Unslop repository is not adopted as platform canon. A prose audit reports findings and may propose changes; it must not silently erase text.

## World simulation and drama direction

A playtest begins from a scenario's authored initial state and deterministic seed. Each accepted player action commits one world event and advances the scenario's turn count. A waiting action advances a designer-selected number of turns within the scenario's allowed limit. Opening, scrolling, and inspecting do not advance time.

NPC behaviors are authored rules with role/capability preconditions, goals, available actions, urgency, personal commitments, and constraints. At each advancing turn, resolve pending consequences and commitments in stable order, then NPC actions. Actors consult their own knowledge. Information reaches a character only through witnessed events, records they can access, or a rule-backed communication path. The director can inspect full state, while player-facing scene generation receives only permitted context.

Candidate discovery considers the approved library and evaluates location, roles, triggers, relationship and status conditions, schedule, evidence, resources, and instance lifetime. Dependency indexes reduce reevaluation work, but the result must equal a complete scan. Show the designer rejection reasons for non-candidates. Rank all eligible candidates; semantic relevance does not bypass hard conditions.

Use seeded weighted shuffling without replacement for wheels. Support once-ever, repeatable, cooldown, persistent, queued aftermath, and interruption policies. A scene that requires participants reserves them when it starts. The first release supports one foreground player-facing scene and multiple nonconflicting background instances. Suspended scenes save their cursor, role bindings, and prior events, release exclusive reservations, and revalidate trigger conditions and role availability when resuming. Invalid instances follow their declared expiry or interruption outcome. Aftermaths receive priority only after eligibility, resources, and participant reservations validate.

Commit player and NPC effects through the same transaction and event log. Reuse of an incident ID or command ID cannot award a reward or transfer resources twice. Bound the number of automatic cascades per turn; emit an explainable diagnostic if that bound is reached.

## Jev-like selection and model training

Use EmbeddingGemma 2 text embeddings for narrative context and approved storylet descriptions. Store normalized document embeddings with the source revision, model revision, and embedding dimensions. Start at 768 dimensions. Recompute only when source content or the embedding model changes.

Train a compact ranking head that combines context/storylet similarity with explicit features: urgency, pacing-arc alignment, novelty, waiting time, goal progress, role-binding quality, and predicted consequence alignment. Hard eligibility remains deterministic in Rust. The model may rank eligible content and estimate urgency/actionability; it cannot invent candidates or effects. The output provides typed Choice, Score, and Noul values, normalized probabilities, confidence calibrated against held-out scenarios, source, and model version. Add a no-action option so the director can defer a scene when no candidate is suitable.

The Rust runtime accepts a versioned `DecisionResult` only for the exact world revision used to rank it. The result carries source/model metadata, confidence, a probability and score for every eligible storylet, and a no-action probability. Rust re-evaluates eligibility, rejects stale revisions, duplicate/unknown/ineligible IDs, non-finite values, incomplete rankings, and probabilities that do not sum to one. Selection uses the highest probability, with no-action winning ties. Hosts submit this result through the Rust library or `POST /api/v1/playtests/{session_id}/director/decision`; ordinary evaluation continues to use the deterministic baseline when no external ranker is available.

For the local embedding path, the host submits the current query vector and one document vector for each eligible storylet through `POST /api/v1/playtests/{session_id}/director/semantic-rank` or the Rust library. Rust validates and normalizes the vectors, computes cosine relevance, combines it with deterministic storylet features, assigns the no-action probability, and selects the result. The first base-encoder weights are explicitly uncalibrated and return zero confidence; they must not be represented as a trained Jev model.

Create training and validation data from seeded simulation runs across distinct scenario families. Include positive selections, hard negative eligible alternatives, and no-action examples. Strata-generated preference labels must be marked as synthetic and may not contaminate human-reviewed held-out evaluation. Split evaluation by scenario family and retain a fixed benchmark set. Train with Unsloth's feature-extraction path and contrastive ranking objective, using BF16 on supported GPU hardware and never FP16 for EmbeddingGemma 2.

The model-training deliverable includes reproducible uv commands, dataset schemas and provenance, trained adapters, exported ranking-head weights, evaluation metrics, model/version metadata, and runtime instructions. Benchmark deterministic ranking, base EmbeddingGemma relevance, and trained ranking separately. Report held-out ranking metrics, calibration, cold and warm latency, memory, and throughput. For the reference workload of 10,000 definitions, 100 NPCs, and 20 locations, target warm p95 selection below 200 ms. Record the actual test hardware and report complete simulation-turn latency separately; this target is not a claim until measured.

The encoder service must return the expected dimension and finite values. A timeout, malformed vector, model mismatch, or unavailable process invokes an explicit deterministic fallback and records that reason. Fallback keeps playtests usable but does not count as model-trained acceptance. Replay uses stored decision results and never calls a live model.

## Event history, butterfly graph, and replay

Every state-changing event records provenance: command or rule, source entity, affected entities, witnesses, evidence/knowledge changes, and causal parents. Keep observed facts and inferred beliefs distinct. Rumor propagation retains the originating evidence lineage; repeated retellings of one source are not independent confirmations.

Use append-only event history and periodic full snapshots. Include random-generator state and director lifecycle state so replay reconstructs candidate decks and delayed events, not only visible qualities. Rewind creates a fork from a selected event or snapshot. Fork comparison displays divergent events and affected qualities/entities; hypothetical branches are not confused with the recorded run.

The butterfly canvas supports event-time navigation, cause/consequence tracing, filters by entity/location/type, and collapsed content versus differentiated instance views. Selecting a node opens its event details, rule/model provenance, and state delta. A separate designer inspector may show full world state; player view only shows observations they can access.

## Studio interface

Present a clean writing studio with a left navigation/library rail, central work surface, and contextual inspector. Main views are Projects, Codex, Storylets, Generation, Playtest, Director, and Butterfly Graph. The Codex stores project reference material. Character and location editors use descriptive prose plus explicit fields. The storylet editor is a graph-only canvas with a contextual property inspector: storylet metadata and local variables appear when no node is selected; selecting a node exposes its type-specific fields. Dialogue nodes contain ordered sections with dialogue lines and idle actions, role bindings, tags, and audio metadata. Variable and condition nodes choose global or storylet-local scope, and the jump-precondition helper writes supported target requirements into source effects. Right-click adds nodes or deletes nodes and wires; Alt-drag cuts wires.

The playtest view pairs the scene and choices with a location map, participant positions, and current turn. Designers can move, wait, choose, inspect eligible and rejected candidates, and inspect event history. Choice requirements and invalid effects are shown in designer mode. Generation progress and approval status remain visible without hiding scene content.

Keep the player observation lens distinct from the full-state designer lens. Provide clear empty states for projects without approved storylets and clear service-health status for Strata and EmbeddingGemma. The product remains usable for authoring and heuristic playtesting when model services are disconnected.

## Import, examples, and migration

New projects begin empty. Example projects are versioned, opt-in bundles that demonstrate social simulation patterns without creating defaults in the runtime. Designers may import legacy project data into a separate bundle; preserve the source database and map fields explicitly. Mark migrated content as draft until it passes validation and receives designer approval.

Translate numeric choice effects only when their intended operation is known. Preserve boolean values as explicit set operations. Flag unsupported role queries and effects for manual review. Do not change ambiguous behavior silently. Provide a migration report listing translated, unsupported, and omitted fields.

Import and export validate schema versions, IDs, references, predicates, and effects. Reject incompatible bundles with a useful report. Provide an explicit migration path for supported old data rather than silently accepting unknown fields.

## Failure handling

- Invalid or stale commands fail without partial effects and return current revision information.
- Duplicate command and incident IDs are idempotent.
- Missing entities, unbound roles, unavailable resources, conflicting reservations, and expired instances have typed outcomes and designer-visible explanations.
- Generation or audit errors preserve prior drafts and job state.
- Inference outages or invalid model output use the recorded deterministic fallback.
- SQLite failure prevents acknowledging a world mutation; restart restores the latest durable snapshot and event tail.
- Incomplete causal cycles or cascade limits produce diagnostics instead of hanging the turn.
- Imported project content is data, never executable code.
- The local service binds to loopback by default and does not expose API credentials in the browser.

## Validation and acceptance

**Engine:** Predicate/operator cases, role binding, choice-time revalidation, atomic transfers, duplicate transactions, cooldown/retirement, seeded wheel order, reservation conflicts, suspension/resumption, expiry, aftermath priority, cascade bounds, deterministic snapshots, and fork replay.

**Simulation:** An NPC acts while the player is elsewhere; player movement changes co-presence and witnesses; knowledge arrives only through a valid path; distinct initial locations produce distinct downstream events; a player who never learned a secret never receives secret-dependent dialogue.

**Generation:** Stage persistence and resumption, stale-stage invalidation, malformed JSON, missing references, contract preservation through revision, complete field auditing, approval gating, provider timeout, bounded retries, and no procedural silent success.

**Model:** Dataset provenance and leakage checks, held-out family evaluation, output shape, normalization and finite-value checks, calibrated probabilities, explicit no-action, unavailable-service fallback, and repeatable latency/memory reports. Training acceptance requires actual trained weights and measured evaluation.

**Web/API:** Project CRUD and import/export, entity/storylet authoring, generation and approval, scenario creation, action-driven playtest, stale revisions, SSE progress, director diagnostics, causal graph navigation, replay, and branch comparison.

**Embedding and migration:** The C ABI native host and local web service execute the same fixture and produce equivalent event traces. Legacy import preserves source data and reports unsupported fields. A fresh empty project and an opt-in generic example both run without engine code changes.

**Reference workload:** Run warm and cold benchmarks with 10,000 storylet definitions, 100 NPCs, and 20 locations. Measure director selection and complete-turn latency separately. Do not use a synthetic microbenchmark as evidence of model quality or full web latency. The current Rust-only measurement and its hardware details are recorded in [the benchmark report](../benchmarks/README.md); it does not measure encoder inference, trained ranking quality, memory, or end-to-end service latency.

## Research references

- dbohdan, Unslop repository and staged story pipeline: https://github.com/dbohdan/unslop
- Novelcrafter, reference for a focused writing workspace: https://www.novelcrafter.com/
- Failbetter Games, quality parsimony and “fires in the desert”: https://www.failbettergames.com/news/narrative-snippets-parsimony-2
- Google DeepMind, EmbeddingGemma 2 model card: https://ai.google.dev/gemma/docs/embeddinggemma/model_card_2?hl=en
- Unsloth, EmbeddingGemma 2 inference and fine-tuning guide: https://unsloth.ai/docs/models/embeddinggemma-2
- Axum Rust API documentation: https://docs.rs/axum/latest/axum/
- rusqlite Rust API documentation: https://docs.rs/rusqlite/latest/rusqlite/
- Local Storylet.md and story-skills are the source for the quality economy, clusters, lifetimes, scene craft, continuity, prose, and authoring workflows.
