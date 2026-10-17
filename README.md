# DecisionNarrative

DecisionNarrative is a local authoring and playtest studio for nonlinear, storylet-driven narrative games. It is designed to scale from a compact detective scenario to a large systemic RPG without imposing a built-in setting, cast, or linear plot structure.

The platform couples a host-independent Rust decision core with a unified local Rust service and a modern Vue 3 MVVM authoring studio styled after quiet macOS creative tools.

## Architecture

- **Narrative Core (`crates/storylet-engine`)**: Host-independent Rust library with zero I/O dependencies. Owns deterministic world state, precondition matching, role resolution, choices, and causal history. Exports a C ABI for native embedding (Unreal / Unity / Godot / .NET).
- **Unified Local Service (`crates/storylet-engine-service`)**: Axum, Tokio, and rusqlite service providing single-store SQLite persistence, REST endpoints, Server-Sent Events (SSE) live streams, and stdio/HTTP Model Context Protocol (MCP) server.
- **Authoring Studio (`src/web`)**: Vue 3 (Composition API) + Vite + Vue Router application. Card-grid storylet deck library, full-width storylet editor with live unslop prose audit, Story Codex (qualities, characters, locations, NPC rules, events), and starting condition scenarios.
- **Drama Director & Ranking (`model/`)**: Deterministic priority/urgency selection with optional local embedding ranking (EmbeddingGemma 2 via Python service).

## Requirements

- Windows, macOS, or Linux with Rust 1.90 or later (edition 2024 support).
- Bun 1.3.14 or later.
- Python 3.11–3.13 and `uv` for model contract tests and optional embedding service.
- Optional: A local Strata-compatible generation endpoint (default `http://127.0.0.1:8080/v1`) for model-assisted scene drafting. The studio, deterministic playtest, and project bundle workflows remain fully functional offline.

## Quick start

From the repository root:

```sh
bun install
bun run build
bun run play
```

Open `http://127.0.0.1:3000` in your browser (e.g. Microsoft Edge, Chrome, Safari).

Authoring records use a unified SQLite database at `data/storylet_studio_v2.db` by default. Set `STORYLET_STUDIO_DB_PATH` to choose another path or `:memory:` for isolated testing. On first launch, the service automatically imports existing project records from legacy databases if present, importing storylets as drafts requiring fresh designer approval.

## Environment configuration

| Variable | Default | Purpose |
| :--- | :--- | :--- |
| `STORYLET_STUDIO_ADDR` | `127.0.0.1:3000` | HTTP service bind address |
| `STORYLET_STUDIO_DB_PATH` | `data/storylet_studio_v2.db` | Unified SQLite database file path |
| `STORYLET_RESOURCE_ROOT` | `.` | Root directory for authoring craft skills |
| `STRATA_BASE_URL` | `http://127.0.0.1:8080/v1` | Local LLM inference endpoint |
| `STRATA_MODEL` | `strata-uncensored` | Model identifier for scene generation |
| `EMBEDDING_SERVICE_URL` | `http://127.0.0.1:4318` | Optional Python embedding service endpoint |

## Build and test

```sh
# Compile Vue 3 frontend
bun run build

# Run frontend typecheck
bun run typecheck

# Run test suites
bun test
cargo test --manifest-path crates/storylet-engine/Cargo.toml
cargo test --manifest-path crates/storylet-engine-service/Cargo.toml
uv run --project model --no-sync python -m unittest discover -s model/tests -v
```

## Authoring and Starting Scenarios

- **Storylet Library**: Browsable card grid with responsive columns (60 cards per page), search by title/text/group tag, status filters (Approved/Draft), location filters, and ending badges. Clicking any card replaces the grid with a focused, full-width editor.
- **Starting Scenarios**: Scenarios configure only starting conditions (player start location policy, initial qualities overrides, actor population and location policies, and random seed). Scenarios do not lock in plot arcs, storylet deck membership, or run length. All approved storylets in the project remain eligible during playtest based on dynamic world preconditions and role availability.
- **Story Codex**: Central world bible managing Qualities (currencies, metrics, tools, territory, menaces), Characters (roles, agendas, red lines), Locations (connectivity, coordinates), NPC Behaviors (autonomous rules), and Project Events (canon and world events).
- **Playtest & Director**: Live playthrough view with current location, active scene prose, and choices. Contextual inspector exposes world qualities, candidate eligibility and rejection reasons, local encoder ranking, and causal event history with branch forking.
- **Project Interchange**: Export and import complete standalone project bundles (`.storylet.json`, schema v2) with transactional validation before replacement.

## Model Context Protocol (MCP)

DecisionNarrative provides a first-class MCP server for LLM-assisted tools and agent workflows:

```sh
# Run MCP over stdio
cargo run --manifest-path crates/storylet-engine-service/Cargo.toml --bin storylet-engine-service -- --mcp
```

Tools exposed:
- `narrative_platform`: Manage project entities, run staged generation, execute playtest commands, and inspect runtime state.
- `generate_storylet_content_batch`: Generate bounded batches (1–24) of candidate storylet records via local Strata.
- `generate_storylet_ranking_batch`: Generate synthetic training pairs matching `ranking-example.schema.json`.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
