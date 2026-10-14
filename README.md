# DecisionNarrative

DecisionNarrative is a local authoring and playtest studio for nonlinear, storylet-driven narrative games. It is designed to scale from a compact detective scenario to a large systemic RPG without imposing a built-in setting or cast.

Designers define a project's characters, locations, qualities, events, NPC behavior, scenarios, and storylets. A staged authoring pipeline turns a scene brief into editable trigger, role, outcome, choice, and effect contracts plus dialogue and action drafts. Designers review and approve the result before playtest. During playtest, the Rust engine evaluates eligible storylets, simulates player and NPC actions, records causal events, and exposes director diagnostics and butterfly-effect traces.

The runtime is a host-independent Rust library with a C ABI for native embedding. The Bun/TypeScript studio calls a local Rust service. An optional local Python service provides EmbeddingGemma embeddings and can serve an optional learned ranking head. Models may rank Rust-eligible storylets; they do not decide hard eligibility or mutate authoritative world state.

## Requirements

- Windows, macOS, or Linux with Rust 1.90 or later (edition 2024 support).
- Bun 1.3.14 or later.
- Python 3.11–3.13 and `uv` for model contract tests. GPU training requires a supported CUDA environment and the optional training dependencies.
- Optional: a configured Strata-compatible generation endpoint for model-assisted authoring. The editor, deterministic playtest, and project bundle workflows remain usable without it.

## Run the studio

From the repository root:

```sh
bun install
bun run play
```

Open `http://127.0.0.1:3000`. The studio starts with an empty workspace when no project data exists. Authoring records use a local SQLite database at `data/storylet_studio.db` by default; set `STORYLET_DB_PATH` to choose another path. Playtest sessions are managed by the Rust service. For an isolated disposable run, set `STORYLET_ENGINE_DB_PATH=:memory:`. Do not point test runs at a project database you want to preserve.

The Rust service binds to loopback. `STORYLET_WEB_PORT`, `STORYLET_ENGINE_ADDR`, `STRATA_BASE_URL`, and `STORYLET_EMBEDDING_URL` can configure the local endpoints. The optional embedding service defaults to `127.0.0.1:4318` and loads its model on the first request.

## Build and test

```sh
bun run build
bun test
bunx tsc --noEmit
cargo test --manifest-path crates/storylet-engine/Cargo.toml
cargo test --manifest-path crates/storylet-engine-service/Cargo.toml
uv run --project model --no-sync python -m unittest discover -s model/tests -v
```

Run Rust service builds/tests before `bun test` on Windows. Bun tests may launch the service; compiling its executable concurrently can cause a Windows file-lock failure. Tests and the semantic director smoke test use in-memory playtest state. To exercise the local encoder path, run:

```sh
bun run scripts/smoke-semantic-director.ts
```

## Create a project and playtest

Create or import a project in the studio, add locations and characters, define storylets with role requirements, trigger predicates, choices, and explicit effects, then approve the content revisions you want in runtime. Save a scenario with its start state and selected approved content. Start a playtest and use the Director view to inspect eligible and rejected candidates. Player movement, waiting, choices, and NPC behaviors create revisioned events. The Butterfly view follows cause and consequence; sessions can be exported, restored, and forked for comparison.

Project bundles are portable, versioned JSON. Use the studio's export/validate/import actions to move authored projects. A new project is generic and empty; example content is opt-in.

## Storylet generation

Generation is an authoring-time process. A designer provides the scene purpose, required roles, trigger conditions, and desired outcomes. The workflow develops alternatives, plans scene beats, drafts executable structure and prose, audits voice/continuity/choice clarity, and presents a review package. Outputs remain drafts until the designer approves both the storylet logic and its content. See the [storylet authoring guide](docs/guides/storylet-authoring.md) and the [system design](docs/design/storylet-studio.md) for the complete workflow and contracts.

The social model is setting-neutral. Its reusable principles—actor motives, obligations, directional relationships, witness-scoped knowledge, institutional access, resilient narrative routes, and causal event history—are described in the [social simulation design](docs/design/social-simulation.md).

## Optional semantic ranking

See [model/README.md](model/README.md) for the EmbeddingGemma 2 service, training data contract, Unsloth setup, evaluation requirements, and ranking-head configuration. Set `STORYLET_RANKING_HEAD` to a validated, reviewed `ranking-head.json` artifact when starting the embedding service to enable the trained feature head. Without it, the director uses its deterministic or uncalibrated embedding baseline. No production-trained, calibrated model is bundled; human-reviewed family-held-out evaluation is required before treating a model as accepted.

Current live feature extraction uses neutral values for goal progress and predicted consequence alignment until those signals have explicit project/runtime contracts. These neutral values and all current model probabilities are uncalibrated.

## Native embedding

`crates/storylet-engine/` builds as Rust library forms (`rlib`, `cdylib`, and `staticlib`) and exports a versioned C ABI. Headers and example hosts are under `crates/` and `examples/`. Run `bun run smoke:native-host` for the native host smoke path. The Rust library is designed to be embedded by engines such as Unreal; a full Unreal Editor plugin is not included yet.

## Project state and test data

Local project authoring data persists by default; disposable test/playtest state should not. Set database paths to `:memory:` for test processes and use fresh scenario/session IDs within the process. Database files are ignored by Git. Keep backups of authored project databases and do not delete them as part of test cleanup.

## Scope and current limitations

- The engine supports deterministic eligibility and causal world changes, while model ranking is an optional layer over eligible candidates.
- Ranking confidence and probabilities are explicitly uncalibrated until reviewed calibration data and evaluation exist.
- Goal progress and predicted consequence alignment currently use neutral live values.
- Full Unreal plugin packaging and hosted collaboration are outside the current local-studio scope.
- Runtime benchmark results measure the Rust deterministic engine path, not embedding inference, model quality, memory, or full web request latency. See [benchmark notes](docs/benchmarks/README.md).

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for the full text.
