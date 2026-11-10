# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Custom thumbnails for editable traits, using the entity image picker; saved
  images appear in the trait editor, project tree, and folder views. Existing
  traits without images remain compatible.
- Tree Graph plugin with TreeGraph and TreeItem traits, an infinite pan-and-zoom
  canvas, editable entity connections, multiple roots, directional and radial
  layouts, configurable canvas backgrounds, nodes resizable from any edge or
  corner, visible connection zones, configurable shapes and colors, normal,
  dashed, dotted and brush wires, reconnection and cutting, entity thumbnail
  backgrounds, graph-wide defaults and scoped CSS customization, a responsive
  canvas title, in-canvas view controls and JSON export, local display
  preferences, and three example graphs.
- Tree canvas image layers with move, resize and lock controls, and node locks.
- Named tree-wire relationships, editable by double-click or the wire menu and
  retained when wires are reconnected, saved, or exported.
- Tree list view with editable TreeItem trait columns, hierarchy and relationship
  references, plus default gameplay fields and non-negative level and cost.
- Tree position storylet nodes with a read-only graph picker and per-tree playtest pointer.
- Shared color wheel picker for tree graph and vector canvas colors.
- Split-pane studio workspace: drag any project-tree entry onto a pane edge (or use
  the pane header, or the tree's "Open in split view") to open it beside or below
  the current view; panes resize, maximize, close, and restore after reload, and
  each pane keeps its own navigation and unsaved-change guard. Drag a pane's
  title bar onto another pane's edge to move it there, or onto its middle to swap
  the two; moved panes keep their unsaved edits.
- Appearance settings in the toolbar: seven game-inspired themes (Kojima
  Productions, the default; Mass Effect, Cyberpunk, RPG, Death Stranding, Anime,
  Modern Warfare) that retune surfaces, borders, text, corner radii, heading type
  and accent, plus an optional background image with a blur toggle. Stored
  locally per browser.
- Shared studio UI components: EmptyState, IconButton, SearchField,
  SegmentedControl and ToggleSwitch, plus a consistent outline icon set.
- A Fit button in the story graph toolbar zooms out so every node is in view.
- `bun run test:visual:stress`: a worst-case data audit that seeds long, one-letter,
  accented, CJK, emoji and very large values and scans every view at desktop,
  1024px, phone and three-pane sizes for clipped, spilled, overlapping, covered
  or low-contrast text.

### Changed

- The studio is now branded Erzählstudio, with a new logo (an "E" whose middle arm
  forks into two outcomes) used as the toolbar mark and favicon.
- Reworked studio visual system: floating rounded panes on an atmospheric
  backdrop, a compact glass toolbar and project tree, self-hosted Sora UI font,
  icon-based actions in place of emoji, quieter destructive buttons, and
  container-query layouts so every view adapts to narrow split panes.
- Trait and asset editors share the entity editor's centered layout, header,
  section navigation, and content-card spacing.
- Rebranded the README around Erzählstudio's vision, with a plain-language
  dungeon example and screenshots of the studio and built-in plugins.

### Fixed

- Choice text fields in story graph nodes take focus again; their wire handle's hit
  area covered the whole row and started a connection instead.
- Graph context menus stay inside the canvas, story nodes keep a readable minimum
  width, and toolbar-added nodes land in view without covering existing cards.
- Menus, pickers and the graph toolbar are opaque, so text underneath no longer
  shows through.
- Narrow panes: inspector field pairs, folder headers, entity titles and entity
  tabs no longer overflow or clip; empty property lists hide their column heads.
- Node type badges and tag placeholders meet text contrast in every theme;
  project-tree menus near the bottom open upward.
- Long unbroken values (file paths, URLs, emails) wrap inside asset version cards,
  asset titles and dialogue previews instead of running past their card;
  dialogue previews end in an ellipsis after three lines instead of a hard cut.
- Long runtime ids yield space to the entity name; folder tiles show names on two
  lines; breadcrumbs shrink ancestors before the current folder; truncated pane
  titles, breadcrumbs, tiles and inspector titles show the full text on hover.
- Thumbnail initials keep emoji and accented letters whole, the project tree
  sorts numbered names naturally ("Clerk 2" before "Clerk 10"), and counts read
  "1 section" rather than "1 sections", with digit grouping for large numbers.
- The trait and asset editors are wide enough for property names, keys and
  defaults such as "(none)" to stay readable.
- Entity headers keep their names and actions visible on narrow screens.
- Deleting tree trait templates checks both entities and entity templates in use.
- Storylet precondition graphs now lower and raise plugin nodes, including Is Tree Position.
- Added tactile active press feedback to tree view switches and hierarchy disclosure toggles.
- Tree context menus open at the clicked row and stay inside the viewport.
- Locked tree canvas images open their menu with Enter.
- Clamped tree canvas context menus within visible viewport bounds to prevent clipping actions.
- Improved tree list horizontal scroll indicators and select dropdown chevrons.
- Added hover glow and press feedback to tree pointer preview nodes and zoom tools.
- Added keyboard zoom (+/-), fit (0/f), and Shift+arrow navigation to the tree canvas.
- Synchronized table row hover states across sticky tree item columns in the list view.
- Added keyboard viewport zoom and live region status announcements to the tree pointer inspector.
- Added quick zoom buttons and live zoom level display to the tree canvas toolbar.
- Added Expand All and Collapse All hierarchy controls to the tree list view.
- Enhanced tree canvas node hover glow, tactile options button, and Esc cancel for relationship edits.
- Ensured default tree dimensions immediately reflect clamped values in the defaults editor.
- Resolved fieldset accessibility compliance across all tree settings panels.
- Added search and filter capability to the tree list view for rapid item lookup.
- Enabled keyboard Enter/Space unlocking for locked tree canvas nodes and Delete key removal for unlocked nodes, wires, and images.
- Restored focus to triggering elements when dismissing tree canvas context menus.
- Added Home and End keyboard selection navigation and live zoom percentage display to the tree pointer preview inspector.
- Added inline keyboard shortcuts reference card in tree canvas view options.
- Added instant clear button and matched result counter to tree list search.
- Added visual selection outline and glowing focus state on canvas nodes.
- Enabled Enter and Space keyboard context menu triggering on locked canvas images.
- Added unselected status guidance to the storylet tree pointer inspector.
- Tree pointer references now protect selected items and tree owners from invalidating saved storylets; scenario overrides cannot alter internal tree state.
- Tree position previews preserve node shapes, images and wire styles, with readable labels, keyboard selection, and zoom focus.
- Connecting, reconnecting, or removing tree wires preserves node positions.
- The tree canvas grid follows pan and zoom.
- Resizing a tree node no longer moves other nodes in the current layout.
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
