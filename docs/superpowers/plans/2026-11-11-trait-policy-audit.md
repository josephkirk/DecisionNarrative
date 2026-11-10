# Trait policy and capability audit plan

**Goal:** Make authoring rules explicit in trait definitions and prevent mutable names from implicitly changing unrelated policy.

**Architecture:** Generic editors consume typed metadata; built-in and plugin definitions declare their own policy. Generic uniqueness validation continues to use the `unique` field. Runtime capabilities with explicit schema contracts are audited separately from policy: changing those identifiers requires explicit capability metadata and coordinated changes to projection, validation, and plugin dispatch. Existing data migration is outside this request; use a fresh project when testing changed built-in definitions.

**Tech stack:** Rust/serde service, Vue/TypeScript studio, Bun and Cargo tests.

## Audit and execution

- [x] Remove editor decisions tied to `character_settings` and property names. Typed `editor` metadata (`option_source`, `selection_only`, `entity_trait_key`) lives on `TraitPropertyDefinition`, is seeded by built-in definitions, validated on save, round-trips through bundles, and is consumed by `EntityView.vue` and `PropertyListEditor.vue`. Covered by `trait_property_editor_metadata_round_trips_and_is_seeded` and `tests/visual/trait-editor-visual.ts` (built-in, custom trait, look-alike without metadata).
- [x] Inventory plugin only creates missing traits; it never rewrites an existing trait. Covered by the `inventory plugin creates only missing traits` bun test and the visual script (fresh project provisioning).
- [x] Numeric `min`/`max` are property data: validated on definitions and defaults, enforced on entity and entity-template saves (`validate_trait_value_bounds`) and bundle validation, used by the tree list and clamped in `PropertyValueInput`. TreeItem `level`/`cost` and InventoryOwner `columns`/`rows` declare bounds. Covered by `trait_numeric_bounds_reject_out_of_range_instance_values` and the visual script.
- [x] Domain dispatch audit (no data-driven replacement; each site is a runtime capability tied to a fixed schema):
  - `inventory_owner`/`item`: now named `inventory::INVENTORY_OWNER_KEY`/`ITEM_KEY`; used by runtime projection (`entities::runtime_attributes`, `tree_pointer`), graph validation, and the studio inventory plugin. Schema keys `__inventory_*` are projection contracts.
  - `tree_graph`/`tree_item`: `TREE_GRAPH_KEY`, tree validation and pointer projection in `tree_graph.rs`/`tree_pointer.rs`/`service.rs`.
  - `character`, `character_settings`, `location`, `location_settings`, `passion*`: `entities::character_record`/`location_record` map fixed trait properties onto engine record fields.
  - `system.participant|clock|connection|milestone|intent_probe|location|fact`: `chrono_adapter`, `runs.rs`, studio `storyletModel.ts`.
  Not replaced: a capability contract would have to carry the property schema, ownership, and validation for every site above. That is a separate design; do not change those keys without it.
- [x] Verification recorded in the final report; unimplemented: capability contract for the sites above.

## Review focus

- Renaming an ordinary trait must not acquire unrelated editor or uniqueness policy.
- A plugin must not silently overwrite a user-defined trait's settings.
- Metadata must survive service serialization and bundle export/import.
- A custom trait using editor metadata must receive the same behavior as a built-in.
- Runtime capability and schema dispatch must not silently change projection, validation, or authored data.
