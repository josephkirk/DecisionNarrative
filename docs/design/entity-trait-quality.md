# Entity, Trait, and Quality Model

**Status:** Target design; not yet implemented in the runtime. Section 11 maps it to current code.
**Scope:** The narrative state model behind storylet selection, graph traversal, and authoring in the DecisionNarrative studio.
**Related:** [Chrono decision engine](chrono-decision-engine.md) (current runtime), [Storylet studio design](storylet-studio.md).

This document formalizes the state model the studio is being built around: entities as graph nodes, traits as quality schemas, qualities as the only mutable state, and Quality Effect Commands (QECs) as the only way to change it.

The graph and production half of the model follows Grabska-Gradzińska, Nowak, Palacz, and Grabska, *Application of Graphs for Story Generation in Video Games*, in *2021 Australasian Computer Science Week Multiconference (ACSW '21)*, February 01–05 2021, Dunedin, New Zealand, ACM, 6 pages, [DOI 10.1145/3437378.3442693](https://doi.org/10.1145/3437378.3442693). A local copy is at `docs/design/3437378.3442693.pdf`. Section references below (§3.1–§3.3, §4.4–§4.6, Figure 1) are to that paper.

## 1. Summary

All narrative state is a typed graph of Entities. Each Entity carries quality values through the Traits granted to it. A quality value is write-protected: it changes only when a Quality Effect Command (QEC) is applied to it, and every QEC is applied inside a reversible transaction that records the old value before writing. A subscription is a QEC template that fires when another quality changes — on the same entity or on an entity reachable over a relation — so derived state, thresholds, and cross-entity coupling are all expressed as cascading commands instead of ad-hoc propagation code.

Four properties follow, and the rest of the document is the detail:

1. **One write path.** Numeric values, flags, and structure (link, unlink, spawn, despawn, grant trait, revoke trait, tag add, tag remove) all mutate through commands, so one undo log covers everything a transaction did.
2. **Every quality has an owner.** There is no global number: today's global metrics become qualities of a singleton `World` entity, and pairwise values become qualities on edges.
3. **The schema is content.** Entity kinds, traits, qualities, relations, and subscriptions are authored project data. The engine fixes only base layers with runtime semantics; a designer's kind extends one of them.
4. **Selection reads the graph.** Productions (§3.2) match their left-hand side against the entity graph; their right-hand side emits QECs. Available actions are every binding of every matching production, with detailed-over-generic precedence (§4.4–4.5).

## 2. Vocabulary

| Term | Definition |
| --- | --- |
| Entity | One game-world object: a location, character, item, fact, or designer kind. A node in the entity graph. Holds quality values through its traits. |
| Entity kind | An authored definition an entity instantiates. Fixes the base layer and the set of default traits. |
| Base layer | One of the engine-fixed categories with runtime semantics: `World`, `Location`, `Character`, `Item`, `Fact`, `Custom`. |
| Trait | A schema: a named group of quality definitions, optionally including other traits. Not a value. |
| Quality definition | An authored declaration: key, value type, default, bounds, visibility, category, metadata. Immutable per content revision. |
| Quality value | The per-entity storage slot for a quality definition. The only mutable narrative state. |
| Edge quality | A quality value whose owner is an edge instead of an entity, holding a pairwise value (A trusts B, A is a member of faction F). |
| Relation | An authored edge type: source kinds, target kinds, cardinality, and the edge traits that give it qualities. |
| Edge | One instance of a relation between two entities. |
| Derived quality | A read-only value computed from other qualities in the same transaction. Never a QEC target. |
| QEC | Quality Effect Command: `Add`, `Set`, or `Transfer` on a `QualityAddress`, applied inside a transaction. |
| Subscription | An authored rule: when a source quality changes in a way a condition accepts, emit a derived QEC against a target address. |
| Transaction | The atomic unit of state change: the root command batch plus its whole cascade, committed or reverted as one. |
| Cause | Why a command was issued: a storylet, a production, a subscription, the clock, or the host. Recorded with every applied command. |
| Binding | One concrete assignment of a production's left-hand side to graph vertices and edges. Each binding is one deck option. |
| Sheaf | A location with its contents, or an entity with its containing location, that location's contents, and everything the entity owns (paper §3.1, Figure 1). |
| Half-sheaf | An entity plus everything it owns (paper §3.1, Figure 1). |

## 3. Clarifications to the original model

Each subsection is a claim in the informal model that is missing, ambiguous, or wrong as stated, followed by the decision this project adopts.

### 3.1 "Immutable" means command-only, not read-only

Values are readable anywhere and write-protected everywhere: the only mutation path is a QEC, and every applied QEC records its before-value. There is no setter, no direct field write, and no path that mutates a quality without producing an undo entry.

The rule covers structure, not just numbers. Linking, unlinking, spawning, despawning an entity, granting or revoking a trait, and adding or removing an entity tag are commands too (`TagCommand`, `StructuralCommand`), executed in the same transaction and the same undo log as QECs. A transaction that half-applied structure and numbers would not be revertible, so the boundary is the transaction, and structure is inside it.

### 3.2 A Trait is a schema, not a value

A trait declares quality definitions plus per-trait defaults and bound overrides; granting a trait allocates the entity's slots for those qualities. Traits carry no values and no entity-specific state.

This matters for the naming collision in current code: `ActorTraitIndex` (`chrono_features.rs:15`) stores an `i8` per actor × trait, which means a value, not a schema. Under this model that index is a *projection* of quality values inside a Personality trait and must be renamed (or redefined) during implementation; keeping the name would make "trait" mean two different things in the same codebase.

Personality descriptors such as "Cocky" or "Guilt-ridden" are qualities inside a `Personality` trait, not traits: a trait is a group, and "Cocky" is a single graded property.

### 3.3 Trait composition

An entity may hold many traits, and a trait may include other traits (`includes`), so designers build large traits from small ones. The effective quality set of a trait is the union of its own qualities and the expanded `includes` closure.

Sharing is explicit and safe:

- Two traits declaring the same quality share one slot per entity, so granting both allocates once.
- Conflicting defaults or bound overrides for a shared quality are a content-compile error that names both traits.
- An `includes` cycle is a content-compile error.

### 3.4 The schema is authored, not built in

Entity kinds, traits, qualities, relations, and subscriptions are project data created by designers in the studio. The engine fixes only the base layers, because they carry runtime semantics (movement, presence, ownership, knowledge). Every designer kind extends exactly one base layer, and nothing in the engine enumerates designer kinds.

There is one fixed entity — see 3.5 — but nothing else about kinds, traits, qualities, or relations is fixed.

### 3.5 Global state is the World entity

"Global" metrics and arc/phase flags are qualities of a singleton `World` entity, created automatically, one per project. Every quality therefore has an owner, and the runtime has no ownerless numbers. This resolves the informal claim that some qualities "belong to the game" — they belong to the World entity.

### 3.6 Relationships are edge qualities

Pairwise values live on a directed edge: trust A→B, reputation with a faction, membership in a guild. They are addressed as `QualityAddress::Edge(edge_id, quality_id)`, never as an untargeted global number.

Faction standing is therefore either a quality on the `member_of` edge (per-member standing in that faction) or a quality on the faction entity (the faction's aggregate). Both are expressible; a bare `standing: 12` on the world is not, because it names no counterpart.

### 3.7 Items: entity vs. stack

- **Unique items** — a named sword, a letter, a specific key — are `Item` entities linked by `owns`/`contains`. They carry their own qualities (durability, provenance) and can be referenced individually.
- **Fungible resources** — gold, ammo, generic potions — are Count qualities on the owner. Creating 300 item entities to hold 300 gold coins is not a design the runtime can afford.

Transfer between owners is a single `QecOp::Transfer` so the pair is atomic and zero-sum: there is no intermediate state in which the gold exists twice or not at all, and the undo log reverses both sides.

### 3.8 Storylets are content, not entities

Storylets hold no quality slots and are not graph nodes with state. Locations reach storylets through anchor relations (`anchors_storylet` → storylet stable key) that traversal uses when gathering candidates.

Facts and narrative information — the paper's fourth layer (`Info` in Figure 1, §3.1) — *are* entities of the `Fact` kind, so knowledge can be owned, learned, forgotten, and transferred over edges like anything else. What stays external is the storylet itself: it is a selection unit, not a stateful object.

### 3.9 Value types

| Type | Storage | Semantics |
| --- | --- | --- |
| `Number` | `f32` | Clamped to `[min, max]` on write. The undo log records the value from before the write, never the clamped result. |
| `Count` | `i32` | Non-negative integer, clamped to the authored `[max(0, min), max]`. Used for resources and progress counters. |
| `Flag` | `bool` | Compiled to a tag bit for predicate speed. Story hooks, seen-markers, one-shot gates. |
| `Enum` | small integer | Authored label set. Milestones and phases where the order matters. |

Progress and milestones use Count or Enum; story hooks use Flag. A designer never picks a "generic variant": the value type determines clamping and predicate compilation.

### 3.10 Derived qualities

A derived quality is read-only and computed, not stored as an independent truth: a sum, average, or max of other qualities (macro passion as the weighted average of micro passions; `wealth_tier` from gold).

Two rules make them safe. A derived quality is never a QEC target — a command addressing one is a content-compile error. And it is recomputed in the same transaction, after its inputs change, so no reader can observe a stale derived value.

### 3.11 Time-driven change is still a command

Decay, regeneration, cooldowns, and scheduled events are clock-issued QECs with `Cause::Clock`. Nothing changes implicitly between commands: no field decays because a frame advanced.

This is what makes a transaction replayable — a replay that re-runs the same commands with the same causes produces the same state, and a state change with no recorded command is a bug rather than a feature.

### 3.12 Subscriptions and storylet triggers are different mechanisms

| | Subscription | Storylet trigger |
| --- | --- | --- |
| When it runs | Inside the transaction, as part of the cascade | After commit |
| What it may do | Mutate qualities and tags (emit QECs) | Only enqueue content (offer storylets) |
| Failure mode | Reverts the whole transaction | Nothing applied yet, so nothing to revert |
| Authored as | Subscription record | `TriggerTable` entry with `EventKind::Quality` (`chrono_triggers.rs`) |

A subscription never starts a storylet, and a trigger never mutates a quality directly. Mixing them would make a transaction's outcome depend on content selection and would break determinism.

### 3.13 Visibility and bounds are part of the definition

Every quality definition carries `hidden`, `min`, and `max`. Clamping happens as part of applying a QEC: the stored value is the clamped result, and the delta subscriptions observe is computed from it. The undo log records the value that was there *before* the write, so undo restores the previous state exactly — clamping cannot make an undo entry lossy.

`hidden` affects authoring and UI presentation only; it never changes evaluation.

### 3.14 Metadata and asset links are authoring data, not runtime state

Thumbnails, display names, gameplay tags on definitions, and asset links exist for the studio and for content compilation. Only *entity* tags reach the runtime table; the engine ignores `name`, `thumbnail`, and `asset_link` entirely. Section 4 defines the split precisely.

## 4. Data model

### 4.1 Built-in layers

The engine fixes these and only these:

```rust
pub enum BaseLayer {
    World,      // exactly one per project, created automatically; owns global qualities
    Location,   // participates in movement and `located_at`
    Character,  // participates in presence checks and role binding
    Item,       // participates in ownership and containment
    Fact,       // participates in knowledge edges (the paper's narrative-information layer)
    Custom,     // no runtime semantics; designer-defined kinds outside the paper's layers
}
```

### 4.2 Authored definitions

Every authored definition has an immutable generated `id` plus an editable `key` and `meta.name`, so renaming never breaks references.

```rust
pub struct Metadata {
    pub thumbnail: Option<ImageRef>,
    pub name: String,                 // free display text; not an identifier
    pub tags: Vec<GameplayTag>,       // hierarchical gameplay tags
    pub asset_link: Option<AssetLink>,
}

pub struct ImageRef {
    /// Project-relative path. Extension must be `.png`, `.jpg`, or `.jpeg`,
    /// case-insensitive; any other extension is a validation error.
    pub path: String,
}

pub struct EntityKindDef {
    pub id: EntityKindId,
    pub key: String,
    pub base_layer: BaseLayer,
    pub default_traits: Vec<TraitKey>,
    pub meta: Metadata,
}

pub struct EntityDef {
    pub id: EntityId,
    pub key: String,
    pub kind: EntityKindKey,
    /// Added to the kind's `default_traits`.
    pub traits: Vec<TraitKey>,
    /// Overrides trait defaults.
    pub initial_values: BTreeMap<QualityKey, Value>,
    pub meta: Metadata,
}

pub struct TraitDef {
    pub id: TraitId,
    pub key: String,
    pub includes: Vec<TraitKey>,
    pub qualities: Vec<TraitQuality>,
    pub meta: Metadata,
}

pub struct TraitQuality {
    pub quality: QualityKey,
    pub default: Option<Value>,
    pub min_override: Option<Value>,
    pub max_override: Option<Value>,
}

pub struct QualityDef {
    pub id: QualityId,
    pub key: String,
    pub value_type: ValueType,
    pub default: Value,
    pub min: Option<Value>,
    pub max: Option<Value>,
    pub hidden: bool,
    pub category: QualityCategory,
    pub description: String,
    pub meta: Metadata,
}

pub enum ValueType { Number, Count, Flag, Enum(Vec<EnumLabel>) }

pub enum QualityCategory {
    Attribute,
    Resource,
    Relationship,
    Progress,
    Flag,
    Menace,
}

pub struct RelationDef {
    pub id: RelationId,
    pub key: String,
    pub name: String,                 // plain display name; relations carry no other metadata
    pub from_kinds: Vec<EntityKindKey>,
    pub to_kinds: Vec<EntityKindKey>,
    pub cardinality: Cardinality,
    pub edge_traits: Vec<TraitKey>,
}

pub enum Cardinality { OneToOne, OneToMany, ManyToMany }
```

`QualityCategory` maps onto the current `QualityType` (`crates/storylet-engine/src/project.rs:53`):

| Target category | Current `QualityType` |
| --- | --- |
| `Resource` | `Currency`, `Tool` |
| `Attribute` | `Metric`, `Territory`, `Custom` |
| `Progress` | `Progress` |
| `Menace` | `Menace` |

Designer kinds are ordinary `EntityKindDef` records: a `Faction` kind on `Custom`, a `Vehicle` kind on `Item`, a `Garden` kind on `Location`.

Defaults and bounds on `QualityDef` and `TraitQuality` are `Value`s in the quality's own type, so a `Count` bound is an exact `i32` and comparison against it never round-trips through `f32` (an `f32` bound could not represent `16 777 217`). A default, bound, or override whose variant does not match the quality's `ValueType` is a content-compile error.

### 4.3 Runtime addresses, commands, and subscriptions

```rust
pub enum QualityAddress {
    Entity(EntityId, QualityId),
    Edge(EdgeId, QualityId),
}

pub struct Qec {
    pub target: QualityAddress,
    pub op: QecOp,
    pub cause: Cause,
}

pub enum QecOp {
    Add(Value),
    Set(Value),
    /// Atomic and zero-sum: both sides move in one command.
    Transfer { from: QualityAddress, amount: Value },
}

/// One quality value in its authored representation. The variant must match
/// the target quality's `ValueType`; a mismatch is a content-compile error.
pub enum Value {
    Number(f32),
    Count(i32),
    Flag(bool),
    Enum(EnumLabel),
}

pub enum Cause {
    Storylet(StoryletKey),
    Production(ProductionKey),
    Subscription(u32),   // authored index
    Clock,
    Host,
}

pub struct TagCommand {
    pub entity: EntityId,
    pub add: Vec<GameplayTag>,
    pub remove: Vec<GameplayTag>,
}

pub struct Subscription {
    pub key: String,
    pub source: QualitySelector,
    pub when: When,
    pub target: TargetSelector,
    pub derive: Derive,
}

pub struct QualitySelector {
    pub quality: QualityRef,
    pub on: Scope,
}

pub enum QualityRef {
    Key(QualityKey),
    /// Definition-level tag, e.g. `Stat.Combat`. Expands at content compile
    /// to the explicit list of matching quality ids; consumes no runtime bits.
    Tagged(GameplayTag),
}

pub enum Scope {
    AnyEntity,
    EntitiesOfKind(EntityKindKey),
    EntitiesWithTrait(TraitKey),
    Entity(EntityKey),
    /// Edge qualities of this relation, so a pairwise value can be a source.
    EdgesOfRelation(RelationKey),
}

pub enum When {
    AnyChange,
    /// The threshold's variant must match the source quality's value type.
    CrossesAbove(Value),
    CrossesBelow(Value),
    DeltaSign(Sign),
}

pub enum Sign { Positive, Negative }

pub enum TargetSelector {
    SameEntity(QualityKey),
    /// The other end of the changed edge (edge contexts only).
    OtherEndpoint(QualityKey),
    Related(RelationKey, QualityKey),
    Fixed(EntityKey, QualityKey),
    EdgeOfRelation(RelationKey, QualityKey),
}

pub enum Derive {
    /// Multiplies the observed delta: `f32` for `Number`, truncated toward
    /// zero for `Count`.
    Scale(f32),
    /// Adds this value; the variant must match the target's value type.
    Constant(Value),
    /// Assigns this value; the variant must match the target's value type.
    SetTo(Value),
}
```

A cascade carries the changed address as its context, and every target selector resolves against that context:

| Selector | Entity context (changed entity `e`) | Edge context (changed edge `u → v` of relation `r0`) |
| --- | --- | --- |
| `SameEntity(q)` | `(e, q)` | `(u, q)` — the edge's source endpoint |
| `OtherEndpoint(q)` | content-compile error | `(v, q)` — the edge's destination endpoint |
| `Related(r, q)` | every `(r, x)` out of `e` → `(x, q)` | every `(r, x)` out of `u` → `(x, q)` |
| `Fixed(e2, q)` | `(e2, q)` | `(e2, q)` |
| `EdgeOfRelation(r, q)` | the single `r` edge out of `e` → `(edge, q)`; several such edges is an ambiguity error | `(u → v, q)` when `r == r0`, otherwise a content-compile error |

`OtherEndpoint` exists because the changed edge's destination is not reachable by any other selector: with `A trusts B` and `A trusts C`, a change on `A → B` must be able to target B alone, and `SameEntity` always means the source endpoint while `Related` traverses all matching edges.

A derive produces an operand of the target's value type. `Scale` applies to `Number` targets in `f32`; on a `Count` target the scaled delta is truncated toward zero, and the operand is an integer. `Constant` and `SetTo` carry a `Value` whose variant must match the target's type, so a flag is set with `SetTo(Value::Flag(true))`, never with a number. `Add` and `Transfer` are defined only for `Number` and `Count` targets: adding to a `Flag` or `Enum`, or transferring into one, is a content-compile error. A derive that cannot produce a value of the target's type is a content-compile error.

### 4.4 Metadata rules

`Metadata` appears on `EntityKindDef`, `EntityDef`, `TraitDef`, and `QualityDef`. `RelationDef` has a plain `name` and no other metadata.

**`name`** is free display text: non-empty after trimming, may contain spaces and any Unicode. It is not an identifier — identity stays `id`, and expressions reference `key`.

**`thumbnail`** is a project-relative `ImageRef` whose extension is `.png`, `.jpg`, or `.jpeg` (case-insensitive); any other extension is a validation error. A path that does not exist on disk is a validation *warning*, and the UI shows a placeholder. Thumbnail precedence for display:

1. the item's own `thumbnail`;
2. else the thumbnail of the linked asset's resolved version (or the asset's own thumbnail);
3. else, for an entity, its kind's thumbnail;
4. else the generic placeholder.

**`tags`** are hierarchical gameplay tags using the existing name rules — dot-separated segments of ASCII letters, digits, `_`, `-`, max 128 characters (`validate_tag` / `is_valid_hierarchical_name`, `crates/storylet-engine/src/chrono_state.rs:2443`). So `Garden.Small` is valid and `Garden Small` is not.

Querying a parent tag matches descendants: the query `Garden` matches entities tagged `Garden.Big` and `Garden.Small` through the registry's descendant mask (`GameplayTagRegistry::has_hierarchical`, `chrono_state.rs:625`). The same rule drives the studio tree filter, canvas filters, and traversal filters such as `neighbors(city, links_to).with_tag("Garden")`. The editor offers only registered tags plus "create tag", and new tags are added to a project-level tag registry. That registry is new work: the existing Tag Manager (`src/web/studio/views/TagManagerView.vue`) derives the visible tag list from current content and shows each tag's usage references, but it has no registration step.

Tag semantics differ by owner:

| Owner | Semantics |
| --- | --- |
| Entity | Runtime state. Compiled into that entity's `entity_tags` `TagBitset`. Queried by predicates, role binding, traversal filters, and production left-hand sides. Mutated only by `TagCommand`, in the same transaction and undo log as QECs. |
| Entity kind, trait, quality | Definition-level. Never change at runtime. Used for authoring search and for selectors (`QualitySelector::Tagged("Stat.Combat")`), which expand at content compile to explicit id lists. They consume no runtime tag bits. |

Runtime entity tags share the existing registry capacity `MAX_TAGS = 256` (`WORD_COUNT = 4`, `chrono_state.rs:16-18`). Exceeding it is a content-compile error that names the count; see the limitation in section 11.

**`asset_link`** is `AssetLink { asset, version }` with `VersionSelector::{Latest, Pinned(VersionId)}`. The default is `Latest`. Export bundles resolve `Latest` to a concrete `Pinned` id so exported content is reproducible. A link to a missing asset or missing version is a validation error.

### 4.5 Asset library

Assets are project-level records. They are not entities, have no qualities, and never affect runtime decisions.

Fields follow the Asset Version Framework models (`Version`, `VersionStorage`, `StorageReference`, <https://github.com/josephkirk/avf>): `AssetVersion` mirrors `Version`, and `StorageLocation` merges `VersionStorage` (`storage_type`, `storage_id`, `created_at`) with `StorageReference` (`reference_type`, `path`, `metadata`), so an AVF repository maps 1:1. The `thumbnail` fields are this project's addition — AVF has no thumbnail on an asset or a version.

```rust
pub struct Asset {
    pub id: AssetId,
    pub name: String,
    pub thumbnail: Option<ImageRef>,
    pub tags: Vec<String>,
    pub versions: Vec<AssetVersion>,
}

pub struct AssetVersion {
    pub id: VersionId,
    pub file_path: String,
    pub creator: String,
    pub tool_version: String,
    pub description: Option<String>,
    pub created_at: Timestamp,
    pub tags: Vec<String>,
    pub custom_data: serde_json::Value,
    pub thumbnail: Option<ImageRef>,
    pub storage: Vec<StorageLocation>,
}

pub struct StorageLocation {
    pub storage_type: String,       // "disk" | "git" | "perforce"
    pub storage_id: String,
    pub created_at: Timestamp,      // VersionStorage.created_at
    pub reference_type: ReferenceType,
    pub path: String,
    pub metadata: serde_json::Value, // StorageReference.metadata
}

pub enum ReferenceType { File, Commit, Changelist, Snapshot }
```

Versions are append-only and ordered by `created_at`, then `id`; `Latest` is the last one. Asset and version tags are free strings (AVF semantics), not gameplay tags — they are catalog metadata, not predicate inputs.

Deleting an asset or a version that any `AssetLink` references is blocked, using the same reference-list rule as other schema items.

### 4.6 Storage layout

Stable ids at rest, dense ids in memory: one column per quality, typed by that quality's value type — `f32` for `Number`, `i32` for `Count`, one bit for `Flag`, a small integer for `Enum` — with a slot map per entity (structure-of-arrays), matching the existing data-oriented design and the `metrics: Vec<f32>` precedent. Command operands carry the matching representation, so a `Count` never passes through `f32` and never loses integer precision. Structure (`traits_of`, adjacency) stays sorted by `(relation, entity)` so fan-out order is authored-stable and cascades are deterministic.

## 5. Entity graph and traversal (paper mapping)

The paper's World State Graph is layered and hierarchical (§3.1): every node belongs to one of four layers, edges represent spatial, ownership, and affiliation dependencies, and node attributes represent object properties.

| Paper layer | Base layer | Runtime semantics |
| --- | --- | --- |
| Locations (`Loc`) | `Location` | Movement, `located_at`, current-location tag contribution |
| Characters (`Char`) | `Character` | Presence, role binding, actor rows |
| Items (`Item`) | `Item` | Ownership, containment, transfer |
| Narrative information (`Info`) | `Fact` | Knowledge edges: known by, learned from, forgotten |
| — | `World` | The singleton entity owning global qualities |
| — | `Custom` | Designer kinds outside the paper's four layers (factions, vehicles, factions-of-factions) |

Designer kinds refine a base layer; hierarchy comes from designer relations, not from the layer list. Recommended starter relations, shipped as editable defaults in new projects (not built in): `contains`, `located_at`, `owns`, `member_of`.

Sheaf and half-sheaf (§3.1, Figure 1):

- **Sheaf of an entity** = the entity's containing location, everything in that location, the entity, and everything the entity owns. For a location, the sheaf is the location and its contents.
- **Half-sheaf of an entity** = the entity + everything it owns.
- The relations used by both are configured per project; default `owns`, `contains`, `located_at`.

Traversal primitives exposed to productions, predicates, and the studio:

```text
neighbors(entity, relation)                 -> entities
sheaf(entity)                               -> entities
half_sheaf(entity)                          -> entities
path(a, b, relations, max_depth)            -> entity chains
```

Productions (§3.2) match their left-hand side over this graph and their right-hand side emits QECs and structural commands. The available action set (§4.4–4.5) is every binding of every matching production, with detailed-over-generic precedence: if a detailed production matches, its parent generic production is suppressed for that resolution. This is a deliberate narrowing of §4.6: the paper's "loosened restrictions" example has a detailed production that is *not* applicable in the circumstances still blocking its generic parent until a designer removes it from the set, while this design suppresses only on a match, so a production that cannot be applied never withholds content. The §4.6 deadlock break survives as an authoring action — remove the overly specific production from the set — rather than as an implicit rule. Section 10 measures what "every binding" costs.

## 6. Authoring: designing and composing the schema

**Workflow.** Creation order is free; references resolve at content compile. A typical flow:

1. Define qualities.
2. Group them into traits.
3. Compose traits with `includes`.
4. Create entity kinds with default traits.
5. Create entities, add extra traits, set initial values.
6. Define relations.
7. Link entities.
8. Author subscriptions.

**Effective quality set.** An entity's effective quality set is the union of the expanded `includes` closure of its kind's `default_traits` plus its own `traits`. The property panel shows values grouped per trait, and groups inherited from the kind are marked as coming from the kind.

**Value override precedence** (highest first):

1. entity `initial_values`;
2. scenario initial values (the existing scenario `initial_qualities`);
3. trait `TraitQuality.default`;
4. `QualityDef.default`.

**Validation at save and at content compile:**

- unique keys per definition type;
- unknown references;
- trait `includes` cycles;
- conflicting defaults or bounds for a quality shared by two traits (error names both traits);
- relation endpoint kinds not matching the linked kinds;
- subscription dependency cycles (section 8);
- a value outside its bounds;
- metadata rules from section 4.4: empty `name`, thumbnail extension, unregistered or malformed tag, missing asset or version link.

**Delete-blocking.** Deleting a quality, trait, kind, relation, gameplay tag, asset, or asset version that is referenced is blocked, and the error lists every referencing item — by traits, entities, storylet predicates and effects, productions, subscriptions, and asset links. This is a new capability, not existing behavior: the Tag Manager today derives a usage index, lists every referencing entity for a tag, and opens it, but it performs no deletion, so the blocking rule and its error listing have to be built.

**Schema edits bump the project content revision.** Existing entities receive new trait qualities at their default; values of removed qualities are dropped and reported in the save result; a value-type change requires an explicit editor choice of `keep numeric value` or `reset to default`.

**Templates.** An entity kind is the template layer. The mapping to articy-style authoring is: kind = template, trait = feature, quality = property.

## 7. Studio UI

The studio layout follows an articy-style three-pane arrangement (supplied as a layout reference only; none of its content is used). Implementation lives in `src/web/studio/` under MVVM per `AGENTS.md`.

**Left project tree.**

- `Entities` folder with one sub-folder per authored entity kind — generated from the schema, not hard-coded — plus `World`.
- `Schema` folder: `Entity Kinds`, `Traits`, `Qualities`, `Relations`, `Subscriptions`.
- `Assets` folder listing the asset library.

Every row shows the item's displayed thumbnail (section 4.4 precedence) and its `name`. The tree search box filters by name and by gameplay tag, with hierarchical matching: the query `Garden` also matches `Garden.Big`.

Today `CODEX_CATEGORIES` (`src/web/studio/definitions.ts:18`) is a fixed list (qualities, characters, locations, npc_behaviors, events). The target replaces those fixed entity categories with schema-driven ones: the tree is derived from `EntityKindDef` records, and adding a kind adds a folder.

**Center.**

- Entity graph canvas: entities as nodes, relation edges between them, reusing the node-canvas interaction style of `NarrativeGraphCanvas.vue`. Filters by kind and by relation, plus a "sheaf" focus mode for one entity.
- When a schema item is selected instead of an entity: that item's editor — trait composition list, quality fields, relation endpoints, subscription builder.
- Canvas nodes show the entity thumbnail and name; the canvas tag filter uses hierarchical matching.

**Right property panel** (`PropertyInspector.vue` pattern): the selected entity's kind, traits (add/remove), quality values grouped by trait, outgoing and incoming relations, and "used by" references.

**Metadata header** at the top of the property panel for every entity, kind, trait, and quality:

- thumbnail preview, click to pick a `.png`/`.jpg`/`.jpeg` file;
- `name` field;
- tag chips with autocomplete from the registered tag list, reusing `TagEditor.vue`;
- asset-link picker (asset, then `Latest` or a specific version) showing the resolved version's `file_path`, `creator`, and `created_at` read-only.

**Asset editor** (selected from the `Assets` folder): asset name, thumbnail, tags, and an append-only version list newest-first showing every `AssetVersion` field and its storage locations, plus "used by" references.

**Subscription builder:** source quality plus selector, condition (`When`), target selector, derive rule; shows the resulting dependency edge and rejects cycles inline.

**View-model:** one new `view-models/schema.ts` owns schema CRUD and validation results. Views stay presentation-only.

## 8. Execution semantics

| Concern | Decision |
| --- | --- |
| Transaction scope | One transaction per root command batch (a storylet's effects, a production application, a host call). |
| Cascade order | FIFO breadth-first over the command queue. |
| Subscription order | Authored index, ascending. |
| Related-target order | Entities in sorted order, per relation. |
| Depth cap | `max_depth = 16` per transaction; exceeding it fails the transaction. |
| Command budget | `max_commands = 4096` per transaction. |
| Cycles | Rejected statically at content compile over the quality dependency graph (source quality → every target quality), same policy as zero-cost graph cycle checks. |
| Failure | Any error reverts the whole transaction; no partial commit. |
| Undo | Reverse application of the before-value log. Extends `BijectiveDelta` (`chrono_state.rs:1609`) from metric deltas to all addresses, including edges and tags. |
| History | Records root commands plus a cascade digest. Replay re-runs the cascade and fails with a mismatch error if the digest differs — the same rule as `checkout` and `CallDataMismatch` (`chrono_runtime.rs:1156`). |
| Post-commit | Events feed the trigger table; triggers enqueue content and never mutate qualities. |

Clamping is part of applying a command, and every value type honors its authored bounds: `Number` clamps to `[min, max]` in `f32`, `Count` clamps to `[max(0, min), max]` in `i32` arithmetic, saturating at the `i32` limits — a `Count` definition that authors `min < 0` is normalized to 0 at content compile, and `max` applies to `Add` exactly as to `Set`, so `Add(20)` on a `Count` with `max = 10` stores 10. The stored value is the clamped result, and the undo log records the prior value. `Transfer` cannot be clamped per side, because independent clamping of the two endpoints creates or destroys units: the transferred amount is decided once as `min(requested, source_value − source_min, destination_max − destination_value)`, where each endpoint contributes its own effective bounds (`max(0, min)` for `Count`), and that single amount is applied to both sides atomically — so both endpoints stay inside their authored bounds and no unit is created or destroyed. A transfer that decides an amount of 0 changes nothing. A command whose stored value equals the prior value fires no subscription, so idempotent writes do not fan out.

## 9. Worked example

An illustrative designer-authored schema — qualities, traits, kinds, and subscriptions as a designer would create them, not built-in content.

**Qualities:** `gold` (Count, Resource), `reputation` (Number, Relationship, `[-100, 100]`), `trust` (Number, Relationship), `standing` (Number, Relationship), `menace` (Number, Menace), `wealth_flag` (Flag).

**Traits:** `Wealth { gold, wealth_flag }`, `Social { reputation, trust }`, `Faction { standing, menace }`.

**Kinds:** `Character` (base `Character`, defaults `Wealth`, `Social`), `Faction` (base `Custom`, defaults `Faction`), `Location` (base `Location`, no traits).

**Relations:** `member_of` (Character → Faction), `has_member` (Faction → Character, the inverse used for fan-out).

**Subscriptions:**

| Index | Source | When | Target | Derive |
| --- | --- | --- | --- | --- |
| S0 | `gold` | `AnyChange` | `SameEntity(reputation)` | `Scale(0.1)` |
| S1 | `reputation` | `AnyChange` | `Related(member_of, standing)` | `Scale(0.5)` |
| S2 | `gold` | `CrossesAbove(Count(100))` | `SameEntity(wealth_flag)` | `SetTo(Flag(true))` |
| S3 | `standing` | `CrossesAbove(Number(50.0))` | `SameEntity(menace)` | `Constant(Number(1.0))` |
| S4 | `menace` | `AnyChange` | `Related(has_member, trust)` | `Scale(-1)` |

**Root command:** `Qec { target: Entity(char.000, gold), op: Add(Count(150)), cause: Production("trade.sell") }`.

The prototype in `crates/storylet-engine/examples/entity_quality_qec_prototype.rs` stores every quality as `f32` and therefore uses float operands; the typed operands above are the target, and at the magnitudes in this example the arithmetic is identical.

**Cascade, in application order (4 commands):**

| # | Command | Depth | Why |
| --- | --- | --- | --- |
| 1 | `gold(char.000) 0 → 150` | 0 | root |
| 2 | `reputation(char.000) 0 → 15` | 1 | S0, `Scale(0.1)` of delta 150 |
| 3 | `wealth_flag(char.000) false → true` | 1 | S2, `0 < 100 ≤ 150` crosses |
| 4 | `standing(faction.200) 0 → 7.5` | 2 | S1 from reputation's delta 15, `Scale(0.5)` on the `member_of` target |

S3 does not fire: `standing` reached 7.5, below 50. S4 therefore does not fire either. Gold's two subscribers (S0, S2) run before reputation's subscriber (S1) because breadth-first order follows the queue: both depth-1 commands were enqueued while processing the root.

**Undo log** (slot, old value) in application order: `(gold(char.000), 0)`, `(reputation(char.000), 0)`, `(wealth_flag(char.000), false)`, `(standing(faction.200), 0)`. Reversing it restores every touched slot to 0.

The prototype in `crates/storylet-engine/examples/entity_quality_qec_prototype.rs` runs exactly this scenario at 300 entities and asserts the four-command order, the final values, determinism across a rebuild, reversal from a non-zero snapshot, and rejection of out-of-range addresses.

## 10. Prototype evidence

Both prototypes are standalone examples; no engine library code was changed. Numbers below are one release run on the development machine (Windows x64). The counts are the portable claim; the timings are machine-specific and vary with load (`char+item+location` matched 747–842 ms across five runs, the linear cascade 0.075–0.080 µs per transaction, and every count was identical in all runs).

Command lines:

```text
cargo run --release --manifest-path crates/storylet-engine/Cargo.toml --example production_qbn_prototype
cargo run --release --manifest-path crates/storylet-engine/Cargo.toml --example entity_quality_qec_prototype
```

### 10.1 Production / QBN matching — `production_qbn_prototype`

World: 40 locations, 160 characters, 100 items. `WorldStateTable` with 300 vertices and 359 edges (`owns` 160, `located_at` 160, `connected` 39).

| Check | Result |
| --- | --- |
| World size | `vertices=300 edges=359` |
| Detailed precedence | `generic_matches=160 detailed_matches=1 missing_matches=0`, suppressed `{trade.any}`, `options=1` |
| Generic fallback (no detailed match) | `any_matches=160 missing_matches=0 suppressed={}`, `options=160` |
| Deck growth | `bindings_per_storylet=160`, one storylet `160` options, two storylets over the same production `320` options |
| Salience over (storylet, binding) | `options=160`, selected binding `["char.00000"]` |
| All-bindings cost, 2 vertices / 1 edge | `bindings=160 vertex_assignments=16000 edge_checks=5744000 avg_ms=18.513 truncated=false` (20 iterations) |
| All-bindings cost, 3 vertices / 2 edges | `bindings=160 vertex_assignments=640000 edge_checks=232057600 avg_ms=747.033 truncated=false` (3 iterations) |
| First-match-only baseline | `successful_runs=20 avg_ms=0.0008` |

### 10.2 QEC cascade — `entity_quality_qec_prototype`

World: 300 entities (200 characters, 20 factions, 80 locations), 6 qualities, 840 allocated slots.

| Check | Result |
| --- | --- |
| Cascade | `applied=4`, `gold=150`, `reputation=15`, `wealth_flag=1`, `standing(200)=7.5` |
| Determinism | undo log identical and final values identical across a rebuild |
| Reversibility, zero baseline | all values restored to 0 |
| Reversibility, non-zero baseline | `restored_snapshot=true commands=4 gold_after_undo=60` — undo restores values that were already non-zero |
| Missing slot | `MissingSlot { entity: 250, quality: 0 }`, store unchanged |
| Out-of-range address | `quality_and_entity_addresses_rejected=true store_unchanged=true` |
| Static cycle | `Cycle { quality: 1 }` at compile |
| Depth cap | `DepthExceeded { depth: 17 }`, all 21 values left at 0 |
| Fan-out routing | `touched_members=[5, 25, 45, 65, 85, 105, 125, 145, 165, 185]` — exactly faction 205's members |
| Throughput A (linear) | 10 000 transactions, `30000` commands, `0.773 ms` total, `0.077 µs` per transaction |
| Throughput B (fan-out) | 10 000 transactions, `110000` commands, `2.483 ms` total, `0.248 µs` per transaction |
| Fan-out effects | `menace_per_faction=500 trust_per_member=-500` |

### 10.3 Conclusions the numbers support

**Deck size is multiplicative, so binding caps are mandatory.** Salience is computed over (storylet, binding) pairs, so the deck is `matching productions × bindings per production × storylets per production`. In the 300-vertex prototype one generic two-vertex production already yields 160 bindings, and a second storylet over the same production doubles the deck to 320 with no added authoring. Per-binding score computation is cheap, but content authoring is not: a handful of generic productions over a large world produces thousands of options from a few storylets. The catalog therefore needs either a per-production binding cap or role-filtered left-hand sides (specific character, specific item, sheaf-scoped matching), and the cap must be authored, not discovered at runtime.

**Naive all-bindings matching does not scale, and this is a real limit, not a tuning artifact.** Enumerating every binding of a three-vertex, two-edge pattern took `747 ms` per resolution on 300 vertices and 359 edges — an order of magnitude above the 100 ms interactive budget — with `232 057 600` edge examinations for `640 000` vertex assignments, i.e. a full scan of all 359 edges per assignment while only 160 assignments can actually succeed. The two-vertex case shows the same shape at `18.5 ms` with `5 744 000` edge checks. The conclusion is structural: the all-bindings matcher needs per-relation adjacency indexing (and a hash join on the edge relation) rather than `graph.edges` scanning. `truncated=false` in both runs, so these costs are the full enumeration, not a cap artifact.

**Cascades are not the cost problem.** The timed loops execute successful transactions of three commands (linear cascade) and eleven commands (1-to-10 fan-out) and construct the undo log for each: `0.077 µs` and `0.248 µs` per transaction — four orders of magnitude below the matching cost. Those timings do not include the correctness checks around them: static cycle rejection runs once at compile, and reversal is exercised separately by undoing the worked example's log (section 10.2). Subscription cascades can be used freely inside a transaction; production matching is where the budget goes.

**The command-only rule is affordable.** Every applied command records a before-value; a failed transaction replays that log backwards. The measured `0.248 µs` covers an eleven-command transaction including log construction, and reversal of such a log is one additional store per entry, so there is no performance argument for a direct-write escape hatch.

## 11. Mapping to current code

| Current | Target |
| --- | --- |
| `WorldStateTable.metrics: Vec<f32>` (`chrono_state.rs:939`, global only) | Qualities of the singleton `World` entity. |
| `WorldStateTable.entity_tags` (`chrono_state.rs:936`) | Kept as the runtime store of entity gameplay tags (metadata `tags` plus `TagCommand` changes) and of compiled Flag qualities. |
| `WorldVertex.attributes` holding character `quality.*`, `status.*`, `need.*` from untyped JSON (`crates/storylet-engine-service/src/chrono_adapter.rs:1291-1296`), mutated directly by `GraphTransform` `set_attributes` | Typed entity qualities written only by QECs. The current path violates command-only. |
| Star-NPC passions: `PassionState` (`chrono_state.rs:1821`), `RivalPassionLink` (`:769`), `PassionThresholdRule` (`:789`) | Character qualities with `Scale` subscriptions and crossing subscriptions; macro passion becomes a derived quality. |
| `ZeroSumLink` (`chrono_state.rs:375`) and `plan_metric_changes` (`:2259`, single-pass, no cascade) | `Scale` subscriptions with a full cascade. |
| `MetricThresholdRule` (`chrono_state.rs:384`, metric → tag bit with hysteresis) | `CrossesAbove`/`CrossesBelow` subscriptions targeting a Flag. |
| `ActorTraitIndex` (`chrono_features.rs:15`, `i8` per actor × trait) | Projection of Personality-trait quality values; rename on implementation. |
| `Effect::TransferQuality` (`crates/storylet-engine/src/lib.rs:93`) | `QecOp::Transfer`. |
| `MutationOp` (`chrono_state.rs:1575`) / `BijectiveDelta` (`:1609`) | QEC batch / undo log. |
| Fixed authoring records: `QualityRecord`, `CharacterRecord`, `LocationRecord` (`crates/storylet-engine-service/src/contracts.rs:15`, `:76`, `:111`); `QualityDefinition`, `CharacterDefinition`, `LocationDefinition` (`project.rs:64`, `:96`, `:84`); fixed `CODEX_CATEGORIES` (`src/web/studio/definitions.ts:18`) | Authored `EntityKindDef` / `TraitDef` / `QualityDef` / `RelationDef` plus generic `EntityDef`. Characters and locations become entity kinds on the `Character` / `Location` base layers. `name` fields map to `meta.name`; `LocationRecord.tags` and `CharacterDefinition.role_tags` map to `meta.tags`. |
| `match_graph_pattern` returns the first match within `PATTERN_SEARCH_BUDGET` (`chrono_state.rs:1131`) | Unchanged for graph transformations; a separate all-bindings matcher feeds the deck (section 10). |

**Limitation:** runtime gameplay tags are capped at `MAX_TAGS = 256` per project by the fixed `TagBitset` width (`WORD_COUNT = 4`). Quality definitions tagged for selectors do not consume runtime bits, but entity tags do, and a project that needs more than 256 runtime tags needs a wider bitset.

## 12. Migration order

Each step is independently shippable and leaves the runtime consistent.

1. **Authored schema definitions** — qualities, traits with `includes`, entity kinds, relations, with `Metadata`, in the project bundle under stable ids.
2. **Asset library records** and `AssetLink` resolution (`Latest` pinned on export).
3. **Generic `EntityDef`** replacing the fixed character and location records, with an importer that maps existing records onto default kinds.
4. **Dense slot storage and the `World` entity**, so every quality has an owner.
5. **QEC executor and `TagCommand`** with one undo log, replacing direct metric deltas and tag operations.
6. **Subscriptions** replacing `ZeroSumLink`, threshold rules, and passion links.
7. **Edge qualities**, for pairwise values and relation-carried traits.
8. **Studio**: schema-driven tree, metadata header, asset editor, schema editors, and the entity graph canvas.
9. **Production catalog** with all-bindings matching feeding QBN and salience, with the binding caps section 10 requires.
