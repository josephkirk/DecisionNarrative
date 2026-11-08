# DecisionNarrative

**Version 0.2.0-alpha.1** — alpha prerelease. APIs, bundle schemas, and databases may change without migration.

DecisionNarrative is a local authoring and playtest studio for nonlinear, storylet-driven narrative games. It is designed to scale from a compact detective scenario to a large systemic RPG without imposing a built-in setting, cast, or linear plot structure.

The platform couples a host-independent Rust decision core with a unified local Rust service and a modern Vue 3 MVVM authoring studio styled after quiet macOS creative tools.

## Architecture

- **Narrative Core (`crates/storylet-engine`)**: Host-independent Rust library with zero I/O dependencies. ChronoFlow v5 uses data-oriented world state, fixed gameplay-tag bitsets, reversible mutation deltas, persistent call history, and deterministic QBN, salience, waypoint, token, timeline, invoke, and reconciliation runtime APIs. Adds participants with per-participant knowledge, a canon chronicle with per-participant telling and `Reveal` nodes, `Learn` / `Move` / `Knows` / `Colocated` nodes, and a world clock with scheduled connections and occupancy rules. A deterministic run kernel replays an action log against content, so runs can be rewound, reloaded after edits, and explained. Exports a C ABI for native embedding (Unreal / Unity / Godot / .NET), including `dn_run_*` entry points for deterministic runs.
- **Unified Local Service (`crates/storylet-engine-service`)**: Axum, Tokio, and rusqlite service providing single-store SQLite persistence, REST endpoints, Server-Sent Events (SSE) live streams, and stdio/HTTP Model Context Protocol (MCP) server.
- **Authoring Studio (`src/web`)**: Vue 3 (Composition API) + Vite + Vue Router application. Articy:draft-style project tree (Storylets, intrinsic Entities, Entity Templates, Traits, Assets) with custom folders/subfolders, live search, drag-and-drop hierarchy, and grid/list folder browsing. Generic entity-template authoring (characters, locations, items, facts, other) with Blueprint-style custom properties, composable traits, thumbnail storage, and bidirectional reference tracking with graph jump-to-node. Full-width graph storylet editor with a contextual property inspector, Tag Manager reference tracking, world/canon events, and starting-condition scenarios. A **Play** view (`/play`) reads and plays the story directly; see [docs/design/playtest-ui.md](docs/design/playtest-ui.md).
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

On Windows you can run `run.bat` (or `run.bat fresh`) instead, which stops any previous server, installs dependencies if needed, then builds and starts the service.

Open `http://127.0.0.1:3000` in your browser (e.g. Microsoft Edge, Chrome, Safari). `bun run play` uses `data/storylet_studio_v6.db`; set `STORYLET_STUDIO_DB_PATH` to select a different file or `:memory:` for isolated testing.

`bun run play:fresh` opens the separate `data/storylet_studio_v6_clean.db` workspace and binds on port 3001; it does not modify the default database. Earlier schema versions are rejected without migration. The MCP process inherits `STORYLET_STUDIO_DB_PATH` when the agent client passes it through.

## Environment configuration

| Variable | Default | Purpose |
| :--- | :--- | :--- |
| `STORYLET_STUDIO_ADDR` | `127.0.0.1:3000` | HTTP service bind address (`play:fresh` defaults to port 3001) |
| `STORYLET_STUDIO_DB_PATH` | `data/storylet_studio_v6.db` | Unified SQLite database file path |

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
- **Story Codex & World Bible**: Author world and canon events. Character settings, location settings, and passion data are composed from reusable traits.
- **Project Tree & Entities**: Keep the intrinsic World entity under Entities; author characters, locations, items, facts, and other game objects as Entity Templates under the separate Entity Templates root. Classify characters, locations, and facts with composable `system.character`, `system.location`, and `system.fact` trait instances; entities may carry multiple classification traits. Items use the inventory plugin Item trait. Gameplay Tag namespaces are independent of entity classification, and the intrinsic World is entity `world`. Compose reusable traits with default values and entity-specific overrides, and manage append-only Assets with versioning and external storage references.
- **Trait uniqueness**: The **Unique trait** toggle defaults off. An entity can contain at most one trait instance marked unique, including duplicate instances of the same trait. Non-unique traits may coexist; `allow_multiple` controls whether the same trait may be applied more than once.
- **Play**: A Play button in the top toolbar (and **Play from here** in the storylet editor) starts a run. The prose column is the centre of the screen; the run advances itself until it needs a choice, a move, or an ending. A *where & when* strip shows the clock, place, and participant tabs (hot-seat). Every step is on a timeline: click an earlier step to rewind, and choosing differently branches automatically. When content changes, **Reload run** replays the same actions on the new content and reports where it diverged. A closed-by-default *Inspect* panel shows knowledge provenance and the chronicle, and *Why?* explains the nearest misses when nothing is available.
- **Coverage Explorer**: `workspace.coverage` / `/api/v4/coverage` deterministically explores the compiled project by replaying runs, so unreachable content can be found without hand-playing.
- **Participants & Knowledge**: Characters act through storylets on the world clock. Knowledge is tracked per participant with provenance, co-presence witnesses what happens, and `Reveal` nodes tell canon chronicle events to chosen participants.
- **Project Authoring Aids**: Storylets can carry time windows and participant badges; world connections and the clock are authored as system traits and validated.
- **Project Interchange**: Export and import complete standalone project bundles (`.storylet.json`, schema v6) containing entities, entity templates, folders, traits, assets, storylets, and scenarios, with transactional validation. Earlier bundle schemas are rejected; use a fresh schema-v6 database rather than migrating a legacy database.

## Plugins

The studio follows an *everything is a plugin* model (after DeepSeek Harness / Cordis). A plugin is
`definePlugin({ name, apply(ctx) })`; everything it registers through `ctx` is removed when it is disposed.
Code lives in `src/web/studio/plugins/`; see [docs/design/studio-plugin-system.md](docs/design/studio-plugin-system.md) for the extension contracts and lifecycle.

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
storylets now have a **References** tab in the editor. Connect a configured **Get Entity** node to the **Modify Canvas**
node's Entity input, then open the canvas editor on that entity's working copy (undo only, no history): draw, move,
delete, add markers and references, or change icons. **Save version** stages the result in the entity's canvas history
until the storylet is saved; saving the version again replaces it. When the node runs it sets the
entity's `canvas_version` to that version and applies the gameplay differences (marker/reference presence, marker
icons) as Apply Command nodes, so several nodes on one canvas compose. Node changes are temp history until the storylet
is saved: a new or edited version is marked *storylet not saved* and a deleted node's version *removal pending*;
**Save draft** commits them, and closing or reopening the storylet without saving discards them.

**Built-in: Inventory.** The plugin creates the `InventoryOwner` and `Item` traits on workspace load. Add the `InventoryOwner` trait to an entity to get an **Inventory** tab. Set rows and columns (1–20 each, default 4×4, capacity = rows × columns), then add entities carrying the `Item` trait; each item occupies one slot, must be unique, and displays its entity thumbnail when present. Save the entity to persist the layout and initial contents. A grid cannot be shrunk below its current item count.

In a story graph, connect configured **Get Entity** nodes to the owner and item inputs of **Add Item**, **Remove Item**, or **Has Item**. Add/remove are flow nodes; Has Item outputs a boolean for a Condition Node. Inputs are checked for the required traits. Adding an item already present and removing an absent item are no-ops; adding to a full inventory fails. Runtime changes are part of the playtest state, not edits to the authored inventory.

**Built-in: Tree Graph.** Add `TreeGraph` to an entity to open its **Tree** tab. Add entities carrying `TreeItem`, then drag from a node's outer edge to another node to connect them. Multiple roots, shared nodes, cycles, and disconnected components are supported without game-specific progression rules. Pan and zoom the canvas; switch between four directional views and radial view, drag nodes or use arrow keys, and save a separate layout for each view. **Save layout** saves the entity draft; **Export JSON** downloads the graph, item metadata, and current coordinates.

Right-click the canvas to add TreeItems or create skill-tree, roguelike-map, and relationship-tree examples. Right-click a node to change its shape, colors, outline width, and dimensions; drag its resize handle to resize it. Nodes use their entities' thumbnails as backgrounds. Open node options to connect without a mouse.

Right-click a wire to change its color or thickness, or remove it. Drag a wire to reconnect its target; drop it on empty canvas to choose a target from a menu. Alt-drag across wires to cut them. Focus a wire and press Enter to open its options by keyboard.

Expand **View options** to customize the grid and thumbnails; these preferences are stored in the local browser, separate from the saved graph.

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

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for release notes.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
