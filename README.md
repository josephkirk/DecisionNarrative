# DecisionNarrative

DecisionNarrative is a local authoring and playtest studio for nonlinear, storylet-driven narrative games. It is designed to scale from a compact detective scenario to a large systemic RPG without imposing a built-in setting, cast, or linear plot structure.

The platform couples a host-independent Rust decision core with a unified local Rust service and a modern Vue 3 MVVM authoring studio styled after quiet macOS creative tools.

## Architecture

- **Narrative Core (`crates/storylet-engine`)**: Host-independent Rust library with zero I/O dependencies. ChronoFlow v5 uses data-oriented world state, fixed gameplay-tag bitsets, reversible mutation deltas, persistent call history, and deterministic QBN, salience, waypoint, token, timeline, invoke, and reconciliation runtime APIs. Exports a C ABI for native embedding (Unreal / Unity / Godot / .NET).
- **Unified Local Service (`crates/storylet-engine-service`)**: Axum, Tokio, and rusqlite service providing single-store SQLite persistence, REST endpoints, Server-Sent Events (SSE) live streams, and stdio/HTTP Model Context Protocol (MCP) server.
- **Authoring Studio (`src/web`)**: Vue 3 (Composition API) + Vite + Vue Router application. Articy:draft-style project tree (Storylets, Entities, Qualities, Trait Templates, Assets) with custom folders/subfolders, live search, drag-and-drop hierarchy, and grid/list folder browsing. Generic entity authoring (characters, locations, items, facts, other) with Blueprint-style custom properties, composable trait templates, thumbnail storage, and bidirectional reference tracking with graph jump-to-node. Full-width graph storylet editor with a contextual property inspector, Tag Manager reference tracking, Rules & Events (NPC rules and project events), and starting condition scenarios.
- **Drama Director (`crates/storylet-engine`)**: Deterministic eligibility and selection in Rust; no embedding ranker or model dependency.
- **Interaction Context (`src/web/studio/interactionContext.ts`)**: Shares the active studio view, project, focused entity/selection, and playtest revision with a local MCP agent. The agent works through MCP operations directly; the studio does not call a model or generate content.

## Requirements

- Windows, macOS, or Linux with Rust 1.90 or later (edition 2024 support).
- Bun 1.3.14 or later.
- No AI service, Python runtime, model download, or generation endpoint is required.

## Quick start

From the repository root:

```sh
bun install
bun run build
bun run play
```

Open `http://127.0.0.1:3000` in your browser (e.g. Microsoft Edge, Chrome, Safari). `bun run play` preserves the project database at `data/storylet_studio_v2.db`; the MCP server uses the same path.

`bun run play:fresh` opens the separate `data/storylet_studio_clean.db` workspace, skips legacy imports, and binds on port 3001; it does not modify the default v2 database. `STORYLET_STUDIO_DB_PATH` selects a different file or `:memory:` for isolated testing. The MCP process inherits that variable when the agent client passes it through.

## Environment configuration

| Variable | Default | Purpose |
| :--- | :--- | :--- |
| `STORYLET_STUDIO_ADDR` | `127.0.0.1:3000` | HTTP service bind address (`play:fresh` defaults to port 3001) |
| `STORYLET_STUDIO_DB_PATH` | `data/storylet_studio_v2.db` | Unified SQLite database file path |

No AI service or model-provider environment variables are required.

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
```

## Authoring and Starting Scenarios

- **Storylet Library**: Browsable card grid with responsive columns (60 cards per page), search by title/text/group tag, status filters (Approved/Draft), location-tag filters, and ending badges. Storylets may target location tags; with no location tags they are not location-gated. With tags, they are eligible anywhere the current location shares at least one tag. Clicking any card opens the full-width graph editor.
- **Storylet Graph Editor**: Authors edit the graph canvas and inspect storylet metadata, dialogue sections, ordered lines or idle actions, role/tag/audio metadata, local variables, and node-specific rules. Go To Location nodes move the player to a selected project location and must be directly targeted by a Choice branch. Graph validation runs before draft save and approval; valid graphs require exactly one Start and one or more End nodes, with every node reachable from Start and able to reach an End. Each End is either terminal (ends the run) or a jump to another storylet. A jump takes precedence over a leftover terminal flag; a storylet is a dedicated ending only when every End is terminal and has no jump. Variable and condition nodes select global or storylet-local scope; right-click adds or deletes graph items, and Alt-drag cuts wires.
- **Starting Scenarios**: Scenarios configure only starting conditions (player start location policy, initial qualities overrides, actor population and location policies, and random seed). Scenarios do not lock in plot arcs, storylet deck membership, or run length. All approved storylets in the project remain eligible during playtest based on dynamic world preconditions and role availability.
- **Tag Manager**: Browse role, location, emotion, and action tags by category, inspect reference counts, and open referencing entities in the Entity Editor or storylets in the Graph Editor.
- **Story Codex & World Bible**: Manage Qualities (currencies, metrics, tools, territory, menaces) with value bounds and crisis storylets, Rules & Events (autonomous NPC rules and world/canon events), and starting condition scenarios.
- **Project Tree & Entities**: Manage world records under generic Entities (characters, locations, items, facts, other) categorized in user-defined folders. Compose reusable Trait Templates with default values and entity-specific overrides, and manage append-only Assets with versioning and external storage references.
- **Playtest & Director**: ChronoFlow v5 is the primary runtime surface. Tick authored flow graphs, inspect world state and active interactions, choose storylets, signal waiters, divert waypoint conversations, invoke a target POI, and reconcile history. The context view exposes node/token state, call histories, and passion-threshold Narrative Lego events.
- **Project Interchange**: Export and import complete standalone project bundles (`.storylet.json`, schema v3) containing entities, folders, trait templates, assets, storylets, and scenarios, with transactional validation and automatic schema v2 migration on import.

## Model Context Protocol (MCP)

DecisionNarrative exposes project data and deterministic playtest operations to local MCP agents. `workspace.context` identifies the active studio view and focused entity; the agent then works through MCP without UI automation. The MCP service shares the studio's local database. No generation or model-ranking tools are exposed:

```sh
# Run MCP over stdio against the shared project database; this does not reset it
cargo run --manifest-path crates/storylet-engine-service/Cargo.toml --bin storylet-engine-service -- --mcp
```

Tools exposed:
- `narrative_platform`: Read active workspace context, inspect/validate project content, manage entities, and exercise deterministic Rust playtests.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
