# Entity, Trait, and Storylet Property Model

**Status:** Implemented contract model.

DecisionNarrative stores authored narrative state as typed properties. There is no separate quality model or numeric quality store.

## Entities and properties

`EntityRecord.properties` contains `EntityProperty` values. Each property has `id`, `key`, `name`, `type`, `is_array`, `multiline`, and `value`. Supported scalar types are `Bool`, `String`, `Float`, `Int`, `Asset`, and `Entity`; arrays use the same declared scalar type with `is_array: true`. Values are JSON-encoded at the service boundary and validated against the declared type.

Characters retain their passion fields (`passion_pool`, active passions, thresholds, and zero-sum links), but they do not have quality fields. Character-specific data is held in the character section and ordinary typed entity properties.

## Traits

A `TraitTemplateRecord` declares reusable `TraitPropertyDefinition` entries. A trait property definition wraps the same typed schema as an entity property and supplies a typed default value. Traits may include other traits; inclusion cycles and duplicate keys are content errors. `TraitInstance.values` is a map of typed JSON values keyed by property key. Trait values are projections of entity state, not a second numeric store.

Property defaults and overrides are checked against the declared type. Entity and trait references are validated against the project index. Runtime projection exposes custom entity properties as `prop.<key>` attributes.

## Storylets and graphs

A storylet is an authored entity with stable runtime identity `storylet:<id>`. Its graph is the story graph; `precondition_graph`, when present, is evaluated against the current world state before candidate consideration. Flow and data edges are distinct graph semantics.

Storylets have no local-variable definitions. Graph data nodes read typed entity properties and commands write declared properties. Tags remain a boolean producer; legacy quality predicates and quality effects are not part of the contract.

## Scenarios

Scenario initial state is represented by:

```rust
BTreeMap<entity_id, BTreeMap<property_key, serde_json::Value>>
```

`ResolvedScenarioStart.effective_properties` has the same shape. There are no initial-quality or effective-quality fields.

## Contract boundaries

The service contract deliberately contains no `QualityRecord`, quality content root, quality authoring option, `ChronoQualityPredicate`, `ChronoStoryletEffect`, variable definition, or legacy migration field. Chrono metadata that is unrelated to properties remains available, as do passion fields and ordinary storylet content/effects.
