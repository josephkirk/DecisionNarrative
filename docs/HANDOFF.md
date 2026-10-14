# DecisionNarrative handoff

**Workspace:** `G:\Projects\DecisionNarrative`  
**Last updated:** 2026-10-14  
**Purpose:** Give the next agent enough product and implementation context to continue without redoing completed work or overwriting project state.

## Product objective

DecisionNarrative is being adapted into a generic authoring and playtest platform for nonlinear, storylet-driven games. It should support both a small detective story and a large systemic RPG. A designer authors characters, locations, events, qualities, NPC behavior, and storylets; describes the roles needed, trigger conditions, and intended outcomes; generates dialogue, action, and interaction through a staged Strata workflow; edits and approves the executable contract and prose; then playtests with an inspectable drama director and causal graph.

The Rust engine is the deterministic authority and must remain embeddable in game engines such as Unreal through a C ABI. The web studio is the authoring/playtest experience. A local model service supplies embeddings and optional ranking artifacts; it must never own world state or bypass hard eligibility. Literary quality is a goal (Fallen London was cited as a quality bar), not a promise made by a model.

User clarification to preserve: **keep the platform setting-neutral and support nonlinear games of different complexity.** Also, **do not keep test database state; every test/playtest should start clean.** The implementation uses `:memory:` for test playtest services and default-new-project flows. Persistent local authoring projects remain an intended product feature; don't confuse that with persistent test runs.

## Source of truth and working rules

- Main design: [storylet-studio.md](design/storylet-studio.md). Read it first for product scope, system boundaries, contracts, and acceptance criteria.
- Social simulation foundations: [social-simulation.md](design/social-simulation.md), covering motives, relationships, local knowledge, institutions, authored routes, and causal consequences.
- Authoring checklist: [storylet-authoring.md](guides/storylet-authoring.md).
- Model setup and training: [model/README.md](../model/README.md).
- Benchmark context: [benchmarks/README.md](benchmarks/README.md) and [rust-runtime-2026-10-14.json](benchmarks/rust-runtime-2026-10-14.json).
- Repository `AGENTS.md` says: when researching, deeply verify sources and prototype algorithms; when coding, use Graphify for project knowledge, `uv` for Python, Bun for TypeScript, `pnpm` for JavaScript, Rust for low-level/high-performance work, C/C++ only when needed for Unreal, and Unsloth for LoRA training.

## Architecture and implementation map

### Rust core and service

- `crates/storylet-engine/`: host-independent core. It validates project/scenario data, evaluates predicates and eligibility, binds roles, executes choices/effects transactionally, manages lifetimes/cooldowns, simulates NPC behaviors, records causal events, supports export/restore/forks, and handles deterministic and semantic director ranking.
- `crates/storylet-engine-service/`: loopback HTTP service and local session persistence. The web client normally launches this service through the Bun client.
- `src/engine/RustRuntimeClient.ts`: Bun-side service client. Its default database path for disposable sessions is `:memory:`.
- `crates/storylet-engine/src/ffi.rs` and the crate headers: C ABI surface. `examples/native-host/` is a native sample; `examples/native-host-dotnet/` is a .NET smoke host. Full Unreal Editor plugin work is explicitly future scope.
- Important runtime rule: only Rust decides hard eligibility and commits state. External models rank the Rust-approved candidate set for an exact context revision; Rust validates the returned result.

### Web studio and authoring pipeline

- `src/web/`, `public/`: Bun/TypeScript studio. The product direction is a clean writing workspace with project library, codex, storylet editor, staged generation, playtest, director inspection, and butterfly/cause graph.
- `src/web/workspaceProject.ts`: maps designer-authored generic records into a validated Rust project/scenario.
- `src/web/projectBundle.ts`: versioned import/export and validation of portable project bundles.
- `src/agents/GenerationWorkflow.ts`: resumable, staged authoring jobs. Generated content remains a draft until explicit review/approval; upstream edits stale dependent stages.
- `src/agents/StoryCraftResources.ts`: story-skills adapted into generic craft resources, with source/prompt hash provenance.
- `src/agents/UnslopStoryletPipeline.ts`, `src/agents/UnslopAudit.ts`: staged alternatives/audit/revision process and prose pattern findings. Audit reports; it does not silently rewrite designer prose.
- `src/web/semanticDirector.ts`: gathers current context, requests query/candidate embeddings, derives live feature values, optionally fetches a learned ranking head, and submits the ranking request to Rust.

### Model service and training

- `model/embedding_protocol.py`: prompt-aware embedding provider, validation, normalization, caching.
- `model/embedding_service.py`: loopback JSON service. `POST /v1/embeddings` serves vectors. Optional `GET /v1/ranking-head` reads and validates the artifact named by `STORYLET_RANKING_HEAD`; absent artifact returns 404 so the studio uses the baseline.
- `model/ranking_head.py`: nine-feature schema, validation, pairwise logistic training/scoring/evaluation.
- `model/storylet_rank_data.py` and `model/ranking-example.schema.json`: preference example validation/provenance. Scenario families must be disjoint by split; validation and held-out labels must be human-reviewed; synthetic Strata/rule labels cannot be used as human evaluation.
- `model/train_embedding_ranker.py`: EmbeddingGemma 2 + Unsloth fine-tuning; computes baseline and post-training encoder/head metrics, recomputes semantic similarity using the loaded encoder, writes adapter, `ranking-head.json`, and metadata.
- `scripts/smoke-semantic-director.ts`: local end-to-end embedding/Rust smoke path.

The nine feature names are: `semantic_relevance`, `urgency`, `pacing_arc_alignment`, `novelty`, `waiting_time`, `goal_progress`, `role_binding_quality`, `predicted_consequence_alignment`, `no_action`. Rust recomputes semantic relevance from vectors; feature vectors are bounded to `[0,1]`. Candidate hard eligibility remains deterministic.

## Genericity / legacy boundary

New-project and studio flows start with empty project data, and the generic authoring guide describes project-owned content. Do not reintroduce a fixed cast, setting, quality names, or premise in code paths used by new projects. The former TypeScript prototype and its setting-specific fixture content have been removed; the generic Rust runtime and its engine tests are the supported playtest path.

The social-simulation principles are retained in [social-simulation.md](design/social-simulation.md): separate state dimensions, actor motives and obligations, directional relationships, witness-scoped knowledge, institutional power through systems, multiple authored progress routes, and causal feedback. Keep examples as opt-in project data rather than engine defaults.

## Current model-ranking behavior and limitations

The trained head is now wired from trainer artifact to service endpoint to Bun studio to Rust core. Rust can apply it and marks the source `trained_ranking_head_uncalibrated`; confidence stays zero because calibration is not implemented. If no head is configured, Rust uses its uncalibrated embedding baseline. This is deliberate and should remain clearly labeled in the UI.

Live feature derivation in `semanticDirector.ts` currently uses project urgency, candidate pacing alignment when available (neutral default otherwise), prior storylet events for novelty/waiting time, and role bindings if the evaluation result exposes them. `goal_progress` and `predicted_consequence_alignment` are currently set to `0.5` because the generic project/runtime contract does not yet define goal and predicted-effect alignment models. This is a known modeling limitation; do not present those values as observed truth. Follow-up options: add explicit goal definitions/progress and predicted-consequence signals to the project contract and director evaluation, or retain neutral live values until that contract is designed.

**No reviewed training dataset or accepted trained artifact was present during this work.** The pipeline and artifact path are implemented, but model acceptance is not complete until there is family-held-out human-reviewed data, the adapter/head is trained, baseline-vs-trained ranking and calibration are reviewed, and cold/warm latency and memory are measured. Do not claim a trained, calibrated Jev model based on synthetic fixtures or the smoke test.

## UX and playtest status

The web test suite covers workspace creation, authoring, API playtest, director ranking, NPC consequences, causal branching, and replay. Repeat the full browser workflow after material UI or ranking changes.

The intended world model is event-driven and supports NPC actions and causal consequences while the player is elsewhere; player movement can alter co-presence, witnesses, knowledge, and later events. The Rust benchmark recorded in the docs is deterministic CPU engine performance, not model inference, memory, or full web-turn performance. The spec's <200 ms warm p95 selection target is not a trained-model acceptance result.

## Verification evidence

The latest recorded full verification passed after model artifact integration:

- `bun test` — 42 passed, 0 failed after removing obsolete prototype suites.
- `bunx tsc --noEmit` — passed.
- `bun run build` — passed; `public/dist/client.js` regenerated.
- `uv run --project model --no-sync python -m unittest discover -s model/tests -v` — 16 passed.
- `uv lock --check --project model` — passed.
- `cargo test --manifest-path crates/storylet-engine/Cargo.toml` — passed.
- `cargo test --manifest-path crates/storylet-engine-service/Cargo.toml` — passed (10 service integration tests).
- `cargo fmt --manifest-path crates/storylet-engine/Cargo.toml -- --check` and service equivalent — passed.
- `git diff --check` — passed (only line-ending warnings).
- On Windows, run Rust build/tests before `bun test`; Bun tests may launch the Rust service executable.

Useful commands (PowerShell, workspace root):

```powershell
cargo test --manifest-path crates/storylet-engine/Cargo.toml
cargo test --manifest-path crates/storylet-engine-service/Cargo.toml
uv run --project model --no-sync python -m unittest discover -s model/tests -v
bun test
bunx tsc --noEmit
bun run build
uv lock --check --project model
```

For the live smoke, use `bun run scripts/smoke-semantic-director.ts`. It owns child processes and uses in-memory playtest state. Do not run the main studio with its default authoring database just to test disposable playtest behavior. When starting a test server directly, use fresh temporary/in-memory settings; do not delete or reset a user/project database.

## Workspace safety

Start with `git status --short` before making changes. Preserve local project databases; do not reset, delete, or regenerate them during test cleanup. Use in-memory settings and fresh session IDs for disposable tests. Refresh generated Graphify output only when needed and review its diff before staging.

## Recommended next work

1. Define explicit project/runtime signals for goal progress and predicted-consequence alignment, or keep both ranking features neutral and labeled.
2. Collect human-reviewed preference examples across distinct scenario families, with eligible hard negatives and no-action comparisons. Validate that evaluation splits do not leak families.
3. Train and evaluate the EmbeddingGemma adapter and ranking head against deterministic and encoder baselines. Report held-out metrics, calibration, latency, and memory; do not treat a successful training command as acceptance.
4. Add calibrated confidence only after suitable held-out labels and calibration metrics are available.
5. Repeat the browser workflow after ranking/UI changes: create a generic project, author and approve content, playtest NPC consequences, inspect Director and Butterfly views, and fork/replay a run.

The design spec remains the requirements authority. If an implementation detail conflicts with the user's genericity/clean-test-state clarification, follow the user clarification and update the spec/hand-off accordingly.
