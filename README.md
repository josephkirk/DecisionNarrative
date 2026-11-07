# DecisionNarrative

DecisionNarrative is a local authoring and playtest studio for nonlinear, storylet-driven narrative games. It is designed to scale from a compact detective scenario to a large systemic RPG without imposing a built-in setting, cast, or linear plot structure.

The platform couples a host-independent Rust decision core with a unified local Rust service and a modern Vue 3 MVVM authoring studio styled after quiet macOS creative tools.

## Architecture

- **Narrative Core (`crates/storylet-engine`)**: Host-independent Rust library with zero I/O dependencies. ChronoFlow v5 uses data-oriented world state, fixed gameplay-tag bitsets, reversible mutation deltas, persistent call history, and deterministic QBN, salience, waypoint, token, timeline, invoke, and reconciliation runtime APIs. Adds participants with per-participant knowledge, a canon chronicle with per-participant telling and `Reveal` nodes, `Learn` / `Move` / `Knows` / `Colocated` nodes, and a world clock with scheduled connections and occupancy rules. A deterministic run kernel replays an action log against content, so runs can be rewound, reloaded after edits, and explained. Exports a C ABI for native embedding (Unreal / Unity / Godot / .NET), including `dn_run_*` entry points for deterministic runs.
- **Unified Local Service (`crates/storylet-engine-service`)**: Axum, Tokio, and rusqlite service providing single-store SQLite persistence, REST endpoints, Server-Sent Events (SSE) live streams, and stdio/HTTP Model Context Protocol (MCP) server.
- **Authoring Studio (`src/web`)**: Vue 3 (Composition API) + Vite + Vue Router application. Articy:draft-style project tree (Storylets, intrinsic Entities, Entity Templates, Trait Templates, Assets) with custom folders/subfolders, live search, drag-and-drop hierarchy, and grid/list folder browsing. Generic entity-template authoring (characters, locations, items, facts, other) with Blueprint-style custom properties, composable trait templates, thumbnail storage, and bidirectional reference tracking with graph jump-to-node. Full-width graph storylet editor with a contextual property inspector, Tag Manager reference tracking, world/canon events, and starting-condition scenarios. A **Play** view (`/play`) reads and plays the story directly; see [docs/design/playtest-ui.md](docs/design/playtest-ui.md).
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

On Windows you can run `run.bat` (or `run.bat fresh`) instead, which installs dependencies if needed, then builds and starts the service.

Open `http://127.0.0.1:3000` in your browser (e.g. Microsoft Edge, Chrome, Safari). `bun run play` uses `data/storylet_studio_v5.db`; set `STORYLET_STUDIO_DB_PATH` to select a different file or `:memory:` for isolated testing.

`bun run play:fresh` opens the separate `data/storylet_studio_v5_clean.db` workspace and binds on port 3001; it does not modify the default database. Earlier schema versions are rejected without migration. The MCP process inherits `STORYLET_STUDIO_DB_PATH` when the agent client passes it through.

## Environment configuration

| Variable | Default | Purpose |
| :--- | :--- | :--- |
| `STORYLET_STUDIO_ADDR` | `127.0.0.1:3000` | HTTP service bind address (`play:fresh` defaults to port 3001) |
| `STORYLET_STUDIO_DB_PATH` | `data/storylet_studio_v5.db` | Unified SQLite database file path |

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
- **Starting Scenarios**: Scenarios configure only starting conditions (player start location policy, initial property overrides, actor population and location policies, and random seed). Scenarios do not lock in plot arcs, storylet deck membership, or run length. All approved storylets in the project remain eligible during playtest based on dynamic world preconditions and role availability.
- **Tag Manager**: Browse the hierarchical Gameplay Tag catalog, add root and child tags, inspect references, and open referenced entities or storylets.
- **Story Codex & World Bible**: Author world and canon events. Character settings, location settings, and passion data are composed from reusable trait templates.
- **Project Tree & Entities**: Keep the intrinsic World entity under Entities; author characters, locations, items, facts, and other game objects as Entity Templates under the separate Entity Templates root. Compose reusable Trait Templates with default values and entity-specific overrides, and manage append-only Assets with versioning and external storage references.
- **Play**: A Play button in the top toolbar (and **Play from here** in the storylet editor) starts a run. The prose column is the centre of the screen; the run advances itself until it needs a choice, a move, or an ending. A *where & when* strip shows the clock, place, and participant tabs (hot-seat). Every step is on a timeline: click an earlier step to rewind, and choosing differently branches automatically. When content changes, **Reload run** replays the same actions on the new content and reports where it diverged. A closed-by-default *Inspect* panel shows knowledge provenance and the chronicle, and *Why?* explains the nearest misses when nothing is available.
- **Coverage Explorer**: `workspace.coverage` / `/api/v4/coverage` deterministically explores the compiled project by replaying runs, so unreachable content can be found without hand-playing.
- **Participants & Knowledge**: Characters act through storylets on the world clock. Knowledge is tracked per participant with provenance, co-presence witnesses what happens, and `Reveal` nodes tell canon chronicle events to chosen participants.
- **Project Authoring Aids**: Storylets can carry time windows and participant badges; world connections and the clock are authored as system traits and validated.
- **Project Interchange**: Export and import complete standalone project bundles (`.storylet.json`, schema v5) containing entities, entity templates, folders, trait templates, assets, storylets, and scenarios, with transactional validation. Earlier bundle schemas are rejected; use a fresh schema-v5 database rather than migrating a legacy database.

## Plugins

The studio follows an *everything is a plugin* model (after DeepSeek Harness / Cordis). A plugin is
`definePlugin({ name, apply(ctx) })`; everything it registers through `ctx` is removed when it is disposed.
Code lives in `src/web/studio/plugins/`.

| Extension point | Registers | Shown in |
| --- | --- | --- |
| `ctx.entityPanel()` | UI attached to an entity | an extra tab in the entity editor |
| `ctx.propertyEditor()` | UI attached to a property | replaces the default value input |
| `ctx.nodeType()` | a graph node: config in, core nodes out (`lower`) | story graph toolbar, context menu, inspector |
| `ctx.on()` / `ctx.plugin()` | events, nested plugins | — |

Plugin nodes exist only while authoring. On save, each is lowered into core nodes (for example Apply Command);
the authored node is kept as `plugin` metadata on the lowered nodes, so reopening the storylet shows it again.
The engine runs only core nodes.

**Built-in: Vector Canvas.** The plugin adds a `Canvas` trait (created on first load). Entities with that trait get a **Canvas** tab: a layered vector editor (rooms, walls, text, sticky
notes, icon markers, and entity and storylet references drawn with their thumbnails). New canvases start with one
`Default` layer. Markers and references have a name shown on the canvas and a description shown on hover. Undo/redo
and Restore do not add history entries. Markers and references declare entity properties (`map_<id>_present`, plus
`map_<id>_icon` for markers). An entity or storylet placed on a canvas counts as a reference from the entity that
holds the canvas (typed `map_<id>_ref` property), so it shows in that entity's or storylet's **References** list;
storylets now have a **References** tab in the editor. Selecting a **Modify Canvas** story node opens the full
canvas editor on a working copy of the entity's canvas (undo only, no history): draw, move, delete, add markers and
references, change icons. **Save version** stores the result as one version in the entity's canvas history (marked
*storylet version*); saving again replaces that version instead of adding more. When the node runs it sets the
entity's `canvas_version` to that version and applies the gameplay differences (marker/reference presence, marker
icons) as Apply Command nodes, so several nodes on one canvas compose.
The **Plugins** tab lists installed plugins and turns them on or off.
Visual check: `bun run test:visual:canvas`.

## Model Context Protocol (MCP)

DecisionNarrative exposes project data and deterministic playtest operations to local MCP agents. `workspace.context` identifies the active studio view and focused entity; the agent then works through MCP without UI automation. The MCP service shares the studio's local database. No generation or model-ranking tools are exposed:

```sh
# Run MCP over stdio against the shared project database; this does not reset it
cargo run --manifest-path crates/storylet-engine-service/Cargo.toml --bin storylet-engine-service -- --mcp
```

Tools exposed:
- `narrative_platform`: Read active workspace context, inspect/validate project content, manage entities, and exercise deterministic Rust playtests. Run operations are `run.start`, `run.view`, `run.act`, `run.rewind`, `run.reload`, `run.inspect`, `run.why_not`, `run.list`, and `run.delete`; `workspace.coverage` explores content coverage.

The same runs are available over HTTP under `/api/v4/runs` (list/start, `/{id}` view/delete, `/{id}/act`, `/{id}/rewind`, `/{id}/reload`, `/{id}/inspect`, `/{id}/why-not`) with optimistic locking and idempotent actions, plus `/api/v4/coverage`.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
