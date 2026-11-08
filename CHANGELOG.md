# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Tree Graph plugin with TreeGraph and TreeItem traits, an infinite pan-and-zoom
  canvas, editable entity connections, multiple roots, directional and radial
  layouts, configurable canvas backgrounds, nodes resizable from any edge or
  corner, visible connection zones, configurable shapes and colors, normal,
  dashed, dotted and brush wires, reconnection and
  cutting, entity thumbnail backgrounds, JSON export, local display preferences,
  and three example graphs.
- Shared color wheel picker for tree graph and vector canvas colors.

### Fixed

- Removed unused Rust helpers and resolved compiler warnings.

### Removed

- Automatic local pull-request reviews and their pre-push hook.

## [0.2.0-alpha.1] - 2026-11-08

### Added

- Character, Location, and Fact classification traits can be combined on one
  entity; the inventory plugin supplies the Item trait.
- Traits have an optional unique flag. Each entity can have at most one unique
  trait instance, enforced during authoring, saves, and bundle imports.
- Local Codex reviews run after pushes and post commit-pinned comments to the
  pull request without an API key.

### Changed

- Removed root entity types and the associated authoring and runtime paths.
  Entities without classification traits remain valid.
- Renamed the authoring section from Trait Templates to Traits.
- Project databases and bundles use schema v6. Earlier schemas are rejected;
  use a fresh database rather than migrating an existing one.

### Fixed

- Local review agents start outside the reviewed checkout, preventing automatic
  loading of its project instructions.
- Trait definition updates cannot invalidate existing unique or repeated traits.
- Bundle imports reject malformed gameplay tag paths.
- Character roles and storylet role requirements can select any registered gameplay tag.

## [0.1.0-alpha.1] - 2026-11-07

### Added

- Studio plugin system (`src/web/studio/plugins/`): `definePlugin({ name, apply(ctx) })`
  with entity panels, property editors, and graph node types (authoring nodes lower
  to core nodes on save and raise back on open). Includes a plugin manager and a
  **Plugins** tab with per-browser enable/disable. Documented in
  [docs/design/studio-plugin-system.md](docs/design/studio-plugin-system.md).
- Built-in Vector Canvas plugin: auto-created `Canvas` trait gating a **Canvas** entity
  tab with a layered vector editor (rooms, walls, text, sticky notes, icon markers,
  entity/storylet references with thumbnails). New canvases start with a `Default`
  layer; undo/redo and Restore do not add history entries. Markers and references
  declare `map_<id>_present` (plus `map_<id>_icon` for markers) and `map_<id>_ref`
  properties. The **Modify Canvas** story node lowers to Apply Command nodes.
- Canvas reference tracking: entity/storylet placements on a canvas count as
  references and appear in **References** lists; the storylet editor has a
  **References** tab. Covered by `bun run test:visual:canvas`.
- Built-in Inventory plugin: auto-created `InventoryOwner` and `Item` traits, an
  **Inventory** entity tab (rows/columns 1–20, default 4×4, one slot per item with
  entity thumbnails, cannot shrink below item count), and **Add Item** /
  **Remove Item** / **Has Item** graph nodes wired with Get Entity owner + item
  inputs and trait validation.
- Engine inventory support: `InventoryAdd` / `InventoryRemove` / `InventoryHas` core
  nodes with deterministic chrono-runtime execution and reversible deltas (adding a
  present item and removing an absent item are no-ops; adding to a full inventory
  fails), a typed runtime projection (`__inventory_items` / `__inventory_rows` /
  `__inventory_columns` / `__inventory_item`), and service, playtest, and bundle
  validation.
- Bundle validation for trait instances on non-world intrinsic entities (unknown
  traits, `allow_multiple`, undeclared properties, value shapes, and references).

### Changed

- **Modify Canvas** now takes its canvas entity from a wired Get Entity data input
  instead of a dropdown. Lowered Apply Command nodes forward that input; if the
  input entity changes, the node must be opened and a version re-saved.
- **Modify Canvas** edits are staged as temp canvas history: **Save version** stages
  (new versions uncommitted, edits as drafts, deleted nodes as removal pending);
  **Save draft** commits them, while closing or reopening the storylet without
  saving discards them. Canvas writes are serialized and skip no-op writes.
- Deleting a **Modify Canvas** node removes the canvas version it owns when the
  storylet is saved.
- `run.bat` stops any previous `storylet-engine-service` process or port 3000/3001
  listener before rebuilding, so a stale server never holds the ports.

### Fixed

- Inventory `__inventory_*` runtime properties are no longer flattened into indexed
  JSON attributes (which left stale entries after mutation); the owner trait is
  skipped in the flat attribute projection.
- The canvas gameplay universe includes staged draft shapes, so pending node edits
  participate in property sync.
