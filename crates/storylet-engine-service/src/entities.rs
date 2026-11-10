//! Entity templates, traits, and asset authoring rules.
//!
//! Entities are authored as generic [`EntityTemplateRecord`] values. Runtime
//! records are projections selected by the entity's gameplay tags and
//! populated from flattened typed properties.

use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use crate::contracts::{
    AssetRecord, AssetVersionRecord, CharacterRecord, EntityRecord, EntityTemplateRecord,
    FolderRecord, FOLDER_ROOTS, GameplayTagRecord, LocationRecord, PropertyType, ProjectBundle,
    STORYLET_CLUSTER_TYPES,
    ProjectBundleContent, TraitPropertyDefinition, TraitTemplateRecord, WorldObjectLayer,
    WorldObjectRecord, SYSTEM_TRAIT_TEMPLATES,
};
use crate::files;

/// Parse only the current project bundle schema.
///
/// Bundle migration is intentionally not performed: earlier schema versions must
/// be rejected rather than silently reinterpreted as current content.
pub fn parse_project_bundle(value: &Value) -> Result<ProjectBundle, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "Project bundle must be a JSON object".to_string())?;
    let schema_version = object
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| "Project bundle schema_version must be 6".to_string())?;
    if schema_version != 6 {
        return Err(format!(
            "Unsupported project bundle schema_version {schema_version}; expected 6"
        ));
    }
    serde_json::from_value(value.clone()).map_err(|error| error.to_string())
}

/// Entities, traits, assets, folders, and storylet ids used to validate
/// Authored records and references used to validate a workspace.
#[derive(Clone, Debug, Default)]
pub struct WorkspaceIndex {
    pub entities: Vec<EntityRecord>,
    pub entity_templates: Vec<EntityTemplateRecord>,
    pub gameplay_tags: Vec<GameplayTagRecord>,
    pub templates: Vec<TraitTemplateRecord>,
    pub assets: Vec<AssetRecord>,
    pub folders: Vec<FolderRecord>,
    pub storylet_ids: BTreeSet<String>,
}

/// Records projected from entities for the runtime adapters.
#[derive(Clone, Debug, Default)]
pub struct RuntimeRecords {
    pub locations: Vec<LocationRecord>,
    pub characters: Vec<CharacterRecord>,
    pub world_objects: Vec<WorldObjectRecord>,
    pub world_tags: Vec<String>,
    pub entity_tag_seeds: Vec<(String, Vec<String>)>,
    pub entity_game_ids: BTreeMap<String, String>,
}

pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub fn new_object_id() -> String {
    format!("0x{:016X}", uuid::Uuid::new_v4().as_u128() as u64)
}


pub fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn pascal_case(name: &str) -> String {
    let mut out = String::new();
    for part in name.split(|c: char| !c.is_ascii_alphanumeric()) {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out
}

fn dedupe_suffix(base: &str, used: &BTreeSet<String>) -> String {
    if !used.contains(base) {
        return base.to_string();
    }
    let mut index = 2;
    loop {
        let candidate = format!("{base}_{index}");
        if !used.contains(&candidate) {
            return candidate;
        }
        index += 1;
    }
}

pub const WORLD_ENTITY_ID: &str = "world";

pub fn default_world_entity() -> EntityRecord {
    EntityRecord {
        id: WORLD_ENTITY_ID.into(),
        object_id: None,
        game_id: Some("world".into()),
        template_id: None,
        name: "World".into(),
        tags: Vec::new(),
        description: String::new(),
        thumbnail: None,
        folder_id: None,
        properties: Vec::new(),
        traits: Vec::new(),
        created_at: Some(0),
    }
}
 

pub fn default_game_id(name: &str, used: &BTreeSet<String>) -> String {
    let pascal = pascal_case(name);
    let pascal = if pascal.is_empty() { "Entity" } else { &pascal };
    dedupe_suffix(&format!("{}_{}", "Ent", pascal), used)
}

pub fn default_template_key(name: &str, used: &BTreeSet<String>) -> String {
    let pascal = pascal_case(name);
    let pascal = if pascal.is_empty() {
        "Trait".to_string()
    } else {
        pascal
    };
    dedupe_suffix(&pascal, used)
}

fn trim_tags(tags: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for tag in tags {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !out.iter().any(|existing| existing == trimmed) {
            out.push(trimmed.to_string());
        }
    }
    out
}

/// The asset-link shape stored in `PropertyType::Asset` values.
/// Links reference the asset only; the latest version is resolved at read time.
pub fn asset_link(value: &Value) -> Option<String> {
    value.get("asset_id")?.as_str().map(str::to_string)
}

fn scalar_shape_matches(property_type: PropertyType, value: &Value) -> bool {
    match property_type {
        PropertyType::Bool => value.is_boolean(),
        PropertyType::String => value.is_string(),
        PropertyType::Float => value.as_f64().is_some_and(f64::is_finite),
        PropertyType::Int => value.as_i64().is_some(),
        PropertyType::Json => value.is_object(),
        PropertyType::Asset => value.is_null() || asset_link(value).is_some(),
        PropertyType::Entity => value.is_null() || value.is_string(),
        PropertyType::Storylet => value.is_null() || value.is_string(),
        PropertyType::Tag => value.is_string(),
    }
}

pub fn value_shape_matches(property_type: PropertyType, is_array: bool, value: &Value) -> bool {
    if is_array {
        return match value {
            Value::Array(items) => items
                .iter()
                .all(|item| !item.is_null() && scalar_shape_matches(property_type, item)),
            _ => false,
        };
    }
    scalar_shape_matches(property_type, value)
}

/// Generic playtests have no entity-scoped custom-property representation.
pub fn generic_playtest_projection_issues(entities: &[EntityTemplateRecord]) -> Vec<String> {
    entities.iter().filter(|entity| !entity.properties.is_empty())
        .map(|entity| format!("entity '{}': generic playtests cannot represent typed custom properties; use Chrono playtests", entity.id))
        .collect()
}


pub fn default_value(property_type: PropertyType, is_array: bool) -> Value {
    if is_array {
        return Value::Array(Vec::new());
    }
    match property_type {
        PropertyType::Bool => Value::Bool(false),
        PropertyType::String => Value::String(String::new()),
        PropertyType::Tag => Value::String(String::new()),
        PropertyType::Float => Value::from(0.0_f64),
        PropertyType::Int => Value::from(0_i64),
        PropertyType::Json => Value::Object(Map::new()),
        PropertyType::Asset | PropertyType::Entity | PropertyType::Storylet => Value::Null,
    }
}

fn trait_values<'a>(
    entity: &'a EntityTemplateRecord,
    templates: &'a [TraitTemplateRecord],
    key: &str,
) -> Vec<&'a BTreeMap<String, Value>> {
    entity
        .traits
        .iter()
        .filter_map(|instance| {
            templates
                .iter()
                .find(|template| template.id == instance.template_id && template.key == key)
                .map(|_| &instance.values)
        })
        .collect()
}

fn trait_values_as<T: serde::de::DeserializeOwned>(
    entity: &EntityTemplateRecord,
    templates: &[TraitTemplateRecord],
    key: &str,
) -> Vec<T> {
    trait_values(entity, templates, key)
        .into_iter()
        .filter_map(|values| {
            let value = serde_json::to_value(values).ok()?;
            serde_json::from_value(value).ok()
        })
        .collect()
}

fn setting_value(
    values: &[&BTreeMap<String, Value>],
    key: &str,
) -> Option<Value> {
    values.iter().find_map(|values| values.get(key).cloned())
}

pub fn character_record(
    entity: &EntityTemplateRecord,
    templates: &[TraitTemplateRecord],
    attributes: Value,
) -> Option<CharacterRecord> {
    if !entity.traits.iter().any(|instance| templates.iter().any(|template| template.id == instance.template_id && template.key == "character")) {
        return None;
    }
    let settings = trait_values(entity, templates, "character_settings");
    let decode = |key: &str| setting_value(&settings, key);
    Some(CharacterRecord {
        id: entity.id.clone(),
        name: entity.name.clone(),
        role: decode("role").and_then(|value| value.as_str().map(str::to_owned)).unwrap_or_default(),
        home_location: decode("home_location").and_then(|value| value.as_str().map(str::to_owned)).unwrap_or_default(),
        faction_id: None,
        current_location: decode("current_location").and_then(|value| value.as_str().map(str::to_owned)).unwrap_or_default(),
        status_axes: Value::Null,
        needs: Value::Null,
        agenda: String::new(),
        red_line: String::new(),
        passion_pool: trait_values_as(entity, templates, "passion"),
        active_passions: Vec::new(),
        schedule: None,
        trust_inertia: 0.0,
        thresholds: trait_values_as(entity, templates, "passion_threshold"),
        zero_sum_passion_links: trait_values_as(entity, templates, "passion_link"),
        created_at: entity.created_at,
        attributes,
    })
}

pub fn location_record(
    entity: &EntityTemplateRecord,
    templates: &[TraitTemplateRecord],
    attributes: Value,
) -> Option<LocationRecord> {
    if !entity.traits.iter().any(|instance| templates.iter().any(|template| template.id == instance.template_id && template.key == "location")) {
        return None;
    }
    let settings = trait_values(entity, templates, "location_settings");
    Some(LocationRecord {
        id: entity.id.clone(),
        name: entity.name.clone(),
        description: entity.description.clone(),
        tags: entity.tags.clone(),
        connected_zones: setting_value(&settings, "connected_zones")
            .and_then(|value| serde_json::from_value(value).ok()),
        environmental_params: None,
        devices: None,
        created_at: entity.created_at,
        attributes,
    })
}
/// Recursively collect typed properties from a template and its included templates.
pub fn collect_template_properties(
    template: &TraitTemplateRecord,
    templates: &[TraitTemplateRecord],
    visited: &mut BTreeSet<String>,
    out: &mut Vec<TraitPropertyDefinition>,
) {
    if !visited.insert(template.id.clone()) {
        return;
    }
    for property in &template.properties {
        if !out.iter().any(|existing| existing.key() == property.key()) {
            out.push(property.clone());
        }
    }
    let _ = templates;
}

/// Flattened runtime attributes: `prop.<key>` and trait property namespaces.
fn property_value_matches(property_type: PropertyType, is_array: bool, value: &Value) -> bool {
    value_shape_matches(property_type, is_array, value)
}

/// Flattened runtime attributes: `prop.<key>` and trait property namespaces.
pub fn runtime_attributes(entity: &EntityTemplateRecord, templates: &[TraitTemplateRecord]) -> Value {
    let mut properties = Map::new();
    let mut traits = Map::new();
    let has_inventory_trait = entity.traits.iter().any(|instance| templates.iter().any(|template|
        template.id == instance.template_id && matches!(template.key.as_str(), "inventory_owner" | "item")));
    for property in &entity.properties {
        // Inventory's typed runtime projection is inserted separately; flattening
        // its array into indexed JSON attributes would leave stale entries after mutation.
        if has_inventory_trait && ["__inventory_items", "__inventory_rows", "__inventory_columns", "__inventory_item"].contains(&property.key.as_str()) { continue; }
        let value = if property.value.is_null() {
            default_value(property.property_type, property.is_array)
        } else {
            property.value.clone()
        };
        properties.insert(property.key.clone(), value);
    }
    for instance in &entity.traits {
        let Some(template) = templates.iter().find(|t| t.id == instance.template_id) else { continue };
        // The mutable inventory state lives in the typed runtime projection.
        if template.key == "inventory_owner" { continue; }
        let mut all_properties = Vec::new();
        collect_template_properties(template, templates, &mut BTreeSet::new(), &mut all_properties);
        let trait_values = traits.entry(template.key.clone()).or_insert_with(|| Value::Object(Map::new()));
        let Value::Object(trait_values) = trait_values else { continue };
        for property in all_properties {
            let property_type = property.property.property_type;
            let is_array = property.property.is_array;
            let value = instance.values.get(property.key())
                .filter(|value| property_value_matches(property_type, is_array, value))
                .cloned()
                .or_else(|| {
                    property_value_matches(property_type, is_array, &property.property.default)
                        .then(|| property.property.default.clone())
                });
            if let Some(value) = value {
                trait_values.insert(property.key().to_owned(), value);
            }
        }
    }
    let mut root = Map::new();
    if !properties.is_empty() { root.insert("prop".into(), Value::Object(properties)); }
    if !traits.is_empty() { root.insert("trait".into(), Value::Object(traits)); }
    if root.is_empty() { Value::Null } else { Value::Object(root) }
}


pub fn runtime_records_with_world(
    world: &EntityRecord,
    entities: &[EntityTemplateRecord],
    templates: &[TraitTemplateRecord],
) -> RuntimeRecords {
    let mut records = RuntimeRecords::default();
    records.entity_game_ids.insert(WORLD_ENTITY_ID.into(), WORLD_ENTITY_ID.into());
    if !world.tags.is_empty() {
        records.world_tags.clone_from(&world.tags);
        records.entity_tag_seeds.push((WORLD_ENTITY_ID.into(), world.tags.clone()));
    }
    for entity in entities {
        let gid = if entity.game_id.trim().is_empty() {
            entity.id.clone()
        } else {
            entity.game_id.clone()
        };
        records.entity_game_ids.insert(entity.id.clone(), gid);
        if !entity.tags.is_empty() {
            records.entity_tag_seeds.push((entity.id.clone(), entity.tags.clone()));
        }
        let mut character = character_record(entity, templates, Value::Null);
        let mut location = location_record(entity, templates, Value::Null);
        let has_trait = |key: &str| entity.traits.iter().any(|instance| {
            templates.iter().any(|template| template.id == instance.template_id && template.key == key)
        });
        let item = has_trait("item");
        let fact = has_trait("fact");
        let generic = character.is_none() && location.is_none() && !item && !fact;
        let mut remaining = usize::from(character.is_some()) + usize::from(location.is_some())
            + usize::from(item) + usize::from(fact) + usize::from(generic);
        let mut attributes = runtime_attributes(entity, templates);
        let mut next_attributes = || {
            remaining -= 1;
            if remaining == 0 { std::mem::take(&mut attributes) } else { attributes.clone() }
        };
        if let Some(row) = character.as_mut() { row.attributes = next_attributes(); }
        if let Some(row) = location.as_mut() { row.attributes = next_attributes(); }
        if let Some(row) = character { records.characters.push(row); }
        if let Some(row) = location { records.locations.push(row); }
        for (present, layer) in [(item, WorldObjectLayer::Item), (fact, WorldObjectLayer::Fact), (generic, WorldObjectLayer::Generic)] {
            if present {
                records.world_objects.push(WorldObjectRecord {
                    id: entity.id.clone(), name: entity.name.clone(), layer, attributes: next_attributes(),
                });
            }
        }
    }
    records.characters.sort_by(|a, b| a.id.cmp(&b.id));
    records.locations.sort_by(|a, b| a.id.cmp(&b.id));
    records
        .world_objects
        .sort_by(|a, b| a.id.cmp(&b.id));
    records
}

/// Resolve a `prop.`/`trait.` attribute path against an entity's runtime attributes.
pub fn attribute_path(root: &Value, key: &str) -> Option<Value> {
    let trimmed = key.trim();
    if !(trimmed.starts_with("prop.") || trimmed.starts_with("trait.")) {
        return None;
    }
    let mut current = root;
    for segment in trimmed.split('.') {
        if segment.is_empty() {
            return None;
        }
        let next = match segment.parse::<usize>() {
            Ok(index) => current.get(index).or_else(|| current.get(segment)),
            Err(_) => current.get(segment),
        }?;
        current = next;
    }
    Some(current.clone())
}

fn validate_property_refs(
    property_type: PropertyType,
    is_array: bool,
    value: &Value,
    index: &WorkspaceIndex,
    prefix: &str,
) -> Result<(), String> {
    let values: Vec<&Value> = if is_array {
        value.as_array().map(|items| items.iter().collect()).unwrap_or_default()
    } else {
        vec![value]
    };
    for item in values {
        match property_type {
            PropertyType::Entity => {
                if let Some(target) = item.as_str()
                    && !index.entities.iter().any(|entity| entity.id == target)
                    && !index.entity_templates.iter().any(|entity| entity.id == target)
                {
                    return Err(format!(
                        "{prefix}: entity reference '{target}' does not exist"
                    ));
                }
            }
            PropertyType::Asset => {
                if let Some(asset_id) = asset_link(item)
                    && !index.assets.iter().any(|asset| asset.id == asset_id)
                {
                    return Err(format!(
                        "{prefix}: asset reference '{asset_id}' does not exist"
                    ));
                }
            }
            PropertyType::Storylet => {
                if let Some(target) = item.as_str()
                    && !index.storylet_ids.iter().any(|id| id == target)
                {
                    return Err(format!(
                        "{prefix}: storylet reference '{target}' does not exist"
                    ));
                }
            }
            PropertyType::Tag => {
                if let Some(name) = item.as_str()
                    && !index.gameplay_tags.iter().any(|tag| tag.name.eq_ignore_ascii_case(name))
                {
                    return Err(format!(
                        "{prefix}: gameplay tag '{name}' is not registered"
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn validate_folder_ref(
    root: &str,
    folder_id: Option<&str>,
    index: &WorkspaceIndex,
) -> Result<(), String> {
    let Some(folder_id) = folder_id.filter(|id| !id.trim().is_empty()) else {
        return Ok(());
    };
    let Some(folder) = index.folders.iter().find(|folder| folder.id == folder_id) else {
        return Err(format!("folder '{folder_id}' does not exist"));
    };
    if folder.root != root {
        return Err(format!("folder '{folder_id}' does not belong to {root}"));
    }
    Ok(())
}

pub fn prepare_entity(
    payload: &Value,
    stored: Option<&EntityTemplateRecord>,
    index: &WorkspaceIndex,
) -> Result<EntityTemplateRecord, String> {
    const PREFIX: &str = "Entity validation failed";
    let mut entity: EntityTemplateRecord = serde_json::from_value(payload.clone())
        .map_err(|error| format!("{PREFIX}: {error}"))?;
    if entity.id.trim().is_empty() {
        entity.id = new_object_id();
    }
    if entity.id.eq_ignore_ascii_case(WORLD_ENTITY_ID) { return Err(format!("{PREFIX}: entity id world is reserved for the intrinsic world entity")); }
    entity.name = entity.name.trim().into();
    if entity.name.is_empty() { return Err(format!("{PREFIX}: name cannot be empty")); }
    entity.tags = trim_tags(&entity.tags);
    validate_unique_traits(&entity.traits, &index.templates, "Entity validation failed")?;
    crate::tree_graph::validate_tree_item_traits(&entity.traits, &index.templates, PREFIX)?;
    for tag in &entity.tags {
        if !index.gameplay_tags.iter().any(|registered| registered.name == *tag) {
            return Err(format!("{PREFIX}: gameplay tag '{tag}' is not registered"));
        }
    }
    let used: BTreeSet<String> = index.entity_templates.iter().filter(|e| e.id != entity.id).map(|e| e.game_id.clone()).collect();
    if entity.game_id.trim().is_empty() {
        entity.game_id = default_game_id(&entity.name, &used);
        entity.game_id_auto = true;
    } else if !is_identifier(&entity.game_id) {
        return Err(format!("{PREFIX}: invalid game ID '{}'", entity.game_id));
    }
    if entity.object_id.trim().is_empty() { entity.object_id = new_object_id(); }
    normalize_optional_id(&mut entity.folder_id);
    entity.created_at = Some(stored.and_then(|s| s.created_at).or(entity.created_at).unwrap_or_else(now_ms));
    entity.description = entity.description.trim().into();
    Ok(entity)
}

pub fn prepare_entity_record(
    payload: &Value,
    stored: Option<&EntityRecord>,
    index: &WorkspaceIndex,
) -> Result<EntityRecord, String> {
    const PREFIX: &str = "Entity validation failed";
    let mut entity: EntityRecord = serde_json::from_value(payload.clone())
        .map_err(|error| format!("{PREFIX}: {error}"))?;
    if entity.id.trim().is_empty() {
        entity.id = new_object_id();
    }
    if entity.id.eq_ignore_ascii_case(WORLD_ENTITY_ID) {
        entity.name = "World".into();
    } else {
        entity.name = entity.name.trim().into();
        if entity.name.is_empty() {
            return Err(format!("{PREFIX}: name cannot be empty"));
        }
    }
    if entity.object_id.as_deref().unwrap_or("").trim().is_empty() {
        entity.object_id = Some(new_object_id());
    }
    if entity.game_id.as_deref().unwrap_or("").trim().is_empty() && entity.id != WORLD_ENTITY_ID {
        let used: BTreeSet<String> = index.entities.iter().filter_map(|e| e.game_id.clone()).collect();
        entity.game_id = Some(default_game_id(&entity.name, &used));
    }
    normalize_optional_id(&mut entity.folder_id);
    normalize_optional_id(&mut entity.template_id);
    entity.created_at = Some(stored.and_then(|s| s.created_at).or(entity.created_at).unwrap_or_else(now_ms));
    entity.description = entity.description.trim().into();
    entity.tags = trim_tags(&entity.tags);
    validate_unique_traits(&entity.traits, &index.templates, "Entity validation failed")?;
    crate::tree_graph::validate_tree_item_traits(&entity.traits, &index.templates, PREFIX)?;
    for tag in &entity.tags {
        if !index.gameplay_tags.iter().any(|registered| registered.name == *tag) {
            return Err(format!("{PREFIX}: gameplay tag '{tag}' is not registered"));
        }
    }
    Ok(entity)
}

pub fn validate_unique_traits(traits: &[crate::contracts::TraitInstance], templates: &[TraitTemplateRecord], owner: &str) -> Result<(), String> {
    let mut has_unique = false;
    for (index, instance) in traits.iter().enumerate() {
        let template = templates.iter().find(|template| template.id == instance.template_id)
            .ok_or_else(|| format!("{owner}: unknown trait '{}'", instance.template_id))?;
        if template.unique {
            if has_unique {
                return Err(format!("{owner}: only one unique trait may be applied"));
            }
            has_unique = true;
        }
        if !template.allow_multiple && traits[..index].iter().any(|previous| previous.template_id == instance.template_id) {
            return Err(format!("{owner}: trait '{}' does not allow multiple instances", template.name));
        }
    }
    Ok(())
}

pub fn validate_trait_properties(
    properties: &mut [TraitPropertyDefinition],
    prefix: &str,
    _index: &WorkspaceIndex,
) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for definition in properties {
        definition.property.key = definition.property.key.trim().to_string();
        definition.property.name = definition.property.name.trim().to_string();
        if definition.property.key.is_empty() || !seen.insert(definition.property.key.clone()) {
            return Err(format!("{prefix}: property keys must be non-empty and unique"));
        }
    }
    Ok(())
}

pub fn prepare_trait_template(
    payload: &Value,
    stored: Option<&TraitTemplateRecord>,
    index: &WorkspaceIndex,
) -> Result<TraitTemplateRecord, String> {
    const PREFIX: &str = "Trait validation failed";
    let mut template: TraitTemplateRecord = serde_json::from_value(payload.clone())
        .map_err(|error| format!("{PREFIX}: {error}"))?;
    if let Some(previous) = stored {
        if previous.system && (template.id != previous.id || template.key != previous.key || !template.system
            || template.allow_multiple != previous.allow_multiple || template.unique != previous.unique || template.properties != previous.properties
            || template.thumbnail != previous.thumbnail) {
            return Err(format!("{PREFIX}: system traits are immutable"));
        }
    } else {
        template.system = false;
    }
    if template.thumbnail.as_deref().is_some_and(|value| value.trim().is_empty()) {
        template.thumbnail = None;
    } else if let Some(thumb) = &template.thumbnail {
        files::validate_image_ref(thumb).map_err(|error| format!("{PREFIX}: {error}"))?;
        template.thumbnail = Some(thumb.trim().to_string());
    }
    if template.id.trim().is_empty() {
        template.id = match stored {
            Some(previous) => previous.id.clone(),
            None => new_object_id(),
        };
    }
    template.name = template.name.trim().to_string();
    if template.name.is_empty() {
        return Err(format!("{PREFIX}: name cannot be empty"));
    }

    let key = template.key.trim().to_string();
    let used_keys: BTreeSet<String> = index
        .templates
        .iter()
        .filter(|existing| existing.id != template.id)
        .map(|existing| existing.key.clone())
        .collect();
    if key.is_empty() {
        template.key = default_template_key(&template.name, &used_keys);
        template.key_auto = true;
    } else {
        let mut key = key;
        // Mirrors the game-ID rule: only a server-generated key follows a rename.
        let auto = match stored {
            Some(previous) if key == previous.key => previous.key_auto,
            _ => false,
        };
        if auto {
            key = default_template_key(&template.name, &used_keys);
        }
        if !is_identifier(&key) {
            return Err(format!(
                "{PREFIX}: key '{key}' must start with a letter or underscore and contain only letters, digits, and underscores"
            ));
        }
        if let Some(owner) = index
            .templates
            .iter()
            .find(|existing| existing.id != template.id && existing.key == key)
        {
            return Err(format!(
                "{PREFIX}: key '{key}' is already used by '{}'",
                owner.name
            ));
        }
        template.key = key;
        template.key_auto = auto;
    }

    if let Some(folder_id) = template.folder_id.as_deref().filter(|id| !id.trim().is_empty()) {
        validate_folder_ref("trait_templates", Some(folder_id), index)
            .map_err(|error| format!("{PREFIX}: {error}"))?;
    } else {
        template.folder_id = None;
    }

    validate_trait_properties(&mut template.properties, PREFIX, index)?;
    crate::tree_graph::validate_tree_item_template(&template, PREFIX)?;

    template.description = template.description.trim().to_string();
    template.created_at = Some(
        stored
            .and_then(|previous| previous.created_at)
            .or(template.created_at)
            .unwrap_or_else(now_ms),
    );
    Ok(template)
}

/// Blank optional ids mean "unset"; storing `""` would hide items from both the
/// root listing and every folder.
pub fn normalize_optional_id(value: &mut Option<String>) {
    if value.as_deref().is_some_and(|id| id.trim().is_empty()) {
        *value = None;
    }
}

/// Canonicalize imported content so stored rows equal their own re-save.
pub fn canonicalize_content(content: &mut ProjectBundleContent) {
    for storylet in &mut content.storylets {
        normalize_optional_id(&mut storylet.folder_id);
    }
    for entity in &mut content.entity_templates {
        normalize_optional_id(&mut entity.folder_id);
    }
    for template in &mut content.trait_templates {
        normalize_optional_id(&mut template.folder_id);
        normalize_optional_id(&mut template.thumbnail);
        if let Some(thumbnail) = &mut template.thumbnail {
            *thumbnail = thumbnail.trim().to_string();
        }
    }
    for folder in &mut content.folders {
        normalize_optional_id(&mut folder.parent_id);
    }
    for asset in &mut content.assets {
        normalize_optional_id(&mut asset.folder_id);
        normalize_asset(asset);
    }
}

fn validate_storage(version: &mut AssetVersionRecord, prefix: &str) -> Result<(), String> {
    for storage in &mut version.storage {
        if !matches!(storage.storage_type.as_str(), "disk" | "git" | "perforce") {
            return Err(format!(
                "{prefix}: storage type '{}' must be disk, git, or perforce",
                storage.storage_type
            ));
        }
        if !matches!(
            storage.reference_type.as_str(),
            "file" | "commit" | "changelist" | "snapshot"
        ) {
            return Err(format!(
                "{prefix}: reference type '{}' must be file, commit, changelist, or snapshot",
                storage.reference_type
            ));
        }
        if storage.storage_id.trim().is_empty() {
            return Err(format!("{prefix}: storage locations need an id"));
        }
        if storage.path.trim().is_empty() {
            return Err(format!("{prefix}: storage locations need a path"));
        }
        if storage.metadata.is_null() {
            storage.metadata = Value::Object(Map::new());
        }
        if !storage.metadata.is_object() {
            return Err(format!("{prefix}: storage metadata must be an object"));
        }
    }
    Ok(())
}

/// Canonical asset values shared by authoring saves and bundle import, so a
/// stored version never differs from its own re-save (append-only safety).
pub fn normalize_asset(asset: &mut AssetRecord) {
    asset.name = asset.name.trim().to_string();
    if asset.thumbnail.as_deref().is_some_and(|value| value.trim().is_empty()) {
        asset.thumbnail = None;
    }
    asset.tags = trim_tags(&asset.tags);
    for version in &mut asset.versions {
        if version.id.trim().is_empty() {
            version.id = new_object_id();
        }
        version.file_path = version.file_path.trim().to_string();
        version.creator = version.creator.trim().to_string();
        version.tool_version = version.tool_version.trim().to_string();
        version.tags = trim_tags(&version.tags);
        if version.thumbnail.as_deref().is_some_and(|value| value.trim().is_empty()) {
            version.thumbnail = None;
        }
        if version.custom_data.is_null() {
            version.custom_data = Value::Object(Map::new());
        }
        for storage in &mut version.storage {
            storage.storage_type = storage.storage_type.trim().to_string();
            storage.reference_type = storage.reference_type.trim().to_string();
            storage.storage_id = storage.storage_id.trim().to_string();
            storage.path = storage.path.trim().to_string();
            if storage.metadata.is_null() {
                storage.metadata = Value::Object(Map::new());
            }
        }
    }
}

pub fn prepare_asset(
    payload: &Value,
    stored: Option<&AssetRecord>,
    index: &WorkspaceIndex,
) -> Result<AssetRecord, String> {
    const PREFIX: &str = "Asset validation failed";
    let mut asset: AssetRecord = serde_json::from_value(payload.clone())
        .map_err(|error| format!("{PREFIX}: {error}"))?;
    if asset.id.trim().is_empty() {
        asset.id = match stored {
            Some(previous) => previous.id.clone(),
            None => new_object_id(),
        };
    }
    asset.name = asset.name.trim().to_string();
    if asset.name.is_empty() {
        return Err(format!("{PREFIX}: name cannot be empty"));
    }
    normalize_asset(&mut asset);

    if asset.thumbnail.as_deref().is_some_and(|value| !value.trim().is_empty()) {
        let thumbnail = asset.thumbnail.clone().unwrap_or_default();
        files::validate_image_ref(&thumbnail).map_err(|error| format!("{PREFIX}: {error}"))?;
    }

    if let Some(folder_id) = asset.folder_id.as_deref().filter(|id| !id.trim().is_empty()) {
        validate_folder_ref("assets", Some(folder_id), index)
            .map_err(|error| format!("{PREFIX}: {error}"))?;
    } else {
        asset.folder_id = None;
    }

    if stored.is_some_and(|previous| previous.versions.len() > asset.versions.len()) {
        return Err(format!(
            "{PREFIX}: Asset versions are append-only: existing versions cannot be edited, reordered, or removed"
        ));
    }

    let mut version_ids = BTreeSet::new();
    for version in &mut asset.versions {
        if !version_ids.insert(version.id.clone()) {
            return Err(format!(
                "{PREFIX}: duplicate version id '{}'",
                version.id
            ));
        }
        version.file_path = version.file_path.trim().to_string();
        if version.file_path.is_empty() {
            return Err(format!("{PREFIX}: version '{}' needs a file path", version.id));
        }
        version.creator = version.creator.trim().to_string();
        if version.creator.is_empty() {
            return Err(format!("{PREFIX}: version '{}' needs a creator", version.id));
        }
        version.tool_version = version.tool_version.trim().to_string();
        if version.created_at <= 0 {
            return Err(format!(
                "{PREFIX}: version '{}' needs a creation timestamp",
                version.id
            ));
        }
        if version.thumbnail.as_deref().is_some_and(|value| !value.trim().is_empty()) {
            let thumbnail = version.thumbnail.clone().unwrap_or_default();
            files::validate_image_ref(&thumbnail).map_err(|error| format!("{PREFIX}: {error}"))?;
        }
        if !version.custom_data.is_object() {
            return Err(format!(
                "{PREFIX}: version '{}' custom data must be an object",
                version.id
            ));
        }
        validate_storage(version, PREFIX)?;
    }
    if let Some(previous) = stored {
        for (old, new) in previous.versions.iter().zip(asset.versions.iter()) {
            let old_value = serde_json::to_value(old).map_err(|error| error.to_string())?;
            let new_value = serde_json::to_value(new).map_err(|error| error.to_string())?;
            if old_value != new_value {
                return Err(format!(
                    "{PREFIX}: Asset versions are append-only: existing versions cannot be edited, reordered, or removed"
                ));
            }
        }
    }
    asset.tags = trim_tags(&asset.tags);
    asset.created_at = Some(
        stored
            .and_then(|previous| previous.created_at)
            .or(asset.created_at)
            .unwrap_or_else(now_ms),
    );
    Ok(asset)
}

pub fn prepare_folder(
    payload: &Value,
    stored: Option<&FolderRecord>,
    index: &WorkspaceIndex,
) -> Result<FolderRecord, String> {
    const PREFIX: &str = "Folder validation failed";
    let mut folder: FolderRecord = serde_json::from_value(payload.clone())
        .map_err(|error| format!("{PREFIX}: {error}"))?;
    if folder.id.trim().is_empty() {
        folder.id = match stored {
            Some(previous) => previous.id.clone(),
            None => new_object_id(),
        };
    }
    if !FOLDER_ROOTS.contains(&folder.root.as_str()) {
        return Err(format!(
            "{PREFIX}: root '{}' must be one of {}",
            folder.root,
            FOLDER_ROOTS.join(", ")
        ));
    }
    if let Some(previous) = stored
        && previous.root != folder.root
    {
        return Err(format!("{PREFIX}: a folder cannot change its root"));
    }
    folder.name = folder.name.trim().to_string();
    if folder.name.is_empty() {
        return Err(format!("{PREFIX}: name cannot be empty"));
    }
    folder.parent_id = folder
        .parent_id
        .filter(|id| !id.trim().is_empty());

    if let Some(parent_id) = &folder.parent_id {
        let Some(parent) = index.folders.iter().find(|candidate| &candidate.id == parent_id) else {
            return Err(format!("{PREFIX}: parent folder '{parent_id}' does not exist"));
        };
        if parent.root != folder.root {
            return Err(format!(
                "{PREFIX}: parent folder '{parent_id}' belongs to {}",
                parent.root
            ));
        }
        let mut visited = BTreeSet::new();
        let mut cursor = Some(parent_id.clone());
        while let Some(current) = cursor {
            if current == folder.id {
                return Err(format!(
                    "{PREFIX}: a folder cannot be moved into itself or one of its subfolders"
                ));
            }
            if !visited.insert(current.clone()) {
                return Err(format!("{PREFIX}: the parent chain forms a cycle"));
            }
            cursor = index
                .folders
                .iter()
                .find(|candidate| candidate.id == current)
                .and_then(|candidate| candidate.parent_id.clone());
        }
    }

    if let Some(sibling) = index.folders.iter().find(|candidate| {
        candidate.id != folder.id
            && candidate.root == folder.root
            && candidate.parent_id == folder.parent_id
            && candidate.name.eq_ignore_ascii_case(&folder.name)
    }) {
        return Err(format!(
            "{PREFIX}: a folder named '{}' already exists here",
            sibling.name
        ));
    }

    folder.created_at = Some(
        stored
            .and_then(|previous| previous.created_at)
            .or(folder.created_at)
            .unwrap_or_else(now_ms),
    );
    Ok(folder)
}


fn check_item_folder(
    label: &str,
    root: &str,
    name: &str,
    folder_id: Option<&str>,
    index: &WorkspaceIndex,
    issues: &mut Vec<String>,
) {
    if let Err(error) = validate_folder_ref(root, folder_id, index) {
        issues.push(format!("{label} '{name}': {error}"));
    }
}

/// Authoring-level validation of a whole bundle: folder graph, item placement,
/// identity uniqueness, and cross-references.
pub fn validate_bundle_content(content: &ProjectBundleContent) -> Vec<String> {
    let mut issues = Vec::new();
    for spec in SYSTEM_TRAIT_TEMPLATES {
        let valid = content.trait_templates.iter().any(|template| {
            template.id == format!("system.{}", spec.key)
                && template.key == spec.key
                && template.name == spec.name
                && template.system
                && template.allow_multiple == spec.allow_multiple
                && template.unique == spec.unique
                && spec.properties.iter().all(|expected| {
                    template.properties.iter().any(|property| {
                        property.key() == expected.key
                            && property.property.property_type == expected.property_type
                            && property.property.is_array == expected.is_array
                    })
                })
        });
        if !valid {
            issues.push(format!(
                "required system trait '{}' is missing or incompatible",
                spec.key
            ));
        }
    }
    let world_entities: Vec<&EntityRecord> = content.entities.iter()
        .filter(|entity| entity.id == WORLD_ENTITY_ID).collect();
    if world_entities.len() != 1
        || content.entity_templates.iter().any(|entity| entity.id == WORLD_ENTITY_ID)
    {
        issues.push("exactly one intrinsic world entity is required and world templates are forbidden".into());
    }
    let index = WorkspaceIndex {
        entities: content.entities.clone(),
        entity_templates: content.entity_templates.clone(),
        gameplay_tags: content.gameplay_tags.clone(),
        templates: content.trait_templates.clone(),
        assets: content.assets.clone(),
        folders: content.folders.clone(),
        storylet_ids: content.storylets.iter().map(|storylet| storylet.id.clone()).collect(),
    };
    issues.extend(crate::tree_graph::validate_tree_graphs(&index, None, None));
    for entity in &content.entities {
        if let Err(error) = validate_unique_traits(&entity.traits, &index.templates, &format!("entity '{}'", entity.name)) { issues.push(error); }
        if let Err(error) = crate::tree_graph::validate_tree_item_traits(&entity.traits, &index.templates, &format!("entity '{}'", entity.name)) { issues.push(error); }
        let mut property_ids = BTreeSet::new();
        let mut property_keys = BTreeSet::new();
        for property in &entity.properties {
            if property.id.trim().is_empty() || !property_ids.insert(property.id.clone()) {
                issues.push(format!("intrinsic entity '{}': missing or duplicate property id", entity.name));
            }
            if !is_identifier(&property.key) || !property_keys.insert(property.key.clone()) {
                issues.push(format!("intrinsic entity '{}': property key '{}' must be a unique identifier", entity.name, property.key));
            }
            if property.name.trim().is_empty() {
                issues.push(format!("intrinsic entity '{}': property '{}' needs a name", entity.name, property.key));
            }
            if !value_shape_matches(property.property_type, property.is_array, &property.value) {
                issues.push(format!("intrinsic entity '{}': property '{}' value does not match its type", entity.name, property.key));
                continue;
            }
            if let Err(error) = validate_property_refs(property.property_type, property.is_array, &property.value, &index, "reference") {
                issues.push(format!("intrinsic entity '{}': {error}", entity.name));
            }
        }
        if entity.id == WORLD_ENTITY_ID && !entity.traits.is_empty() {
            issues.push(format!("intrinsic entity '{}': trait instances are not supported", entity.name));
        } else if entity.id != WORLD_ENTITY_ID {
            let mut seen = BTreeSet::new();
            for instance in &entity.traits {
                let Some(template) = index.templates.iter().find(|template| template.id == instance.template_id) else {
                    issues.push(format!("entity '{}': unknown trait '{}'", entity.name, instance.template_id));
                    continue;
                };
                if !seen.insert(&instance.template_id) && !template.allow_multiple {
                    issues.push(format!("entity '{}': trait '{}' does not allow multiple instances", entity.name, instance.template_id));
                }
                let mut properties = Vec::new();
                collect_template_properties(template, &index.templates, &mut BTreeSet::new(), &mut properties);
                for (key, value) in &instance.values {
                    let Some(property) = properties.iter().find(|property| property.key() == key) else {
                        issues.push(format!("entity '{}': trait '{}' property '{}' is not declared", entity.name, template.key, key));
                        continue;
                    };
                    if !value_shape_matches(property.property.property_type, property.property.is_array, value) {
                        issues.push(format!("entity '{}': trait '{}' property '{}' value does not match its type", entity.name, template.key, key));
                    } else if let Err(error) = validate_property_refs(property.property.property_type, property.property.is_array, value, &index, "reference") {
                        issues.push(format!("entity '{}': trait '{}' {error}", entity.name, template.key));
                    }
                }
            }
        }
    }

    for folder in &content.folders {
        if !FOLDER_ROOTS.contains(&folder.root.as_str()) {
            issues.push(format!(
                "folder '{}': root '{}' must be one of {}",
                folder.name,
                folder.root,
                FOLDER_ROOTS.join(", ")
            ));
        }
        if let Some(parent_id) = &folder.parent_id {
            match index.folders.iter().find(|candidate| &candidate.id == parent_id) {
                None => issues.push(format!(
                    "folder '{}': parent folder '{parent_id}' does not exist",
                    folder.name
                )),
                Some(parent) if parent.root != folder.root => issues.push(format!(
                    "folder '{}': parent folder '{parent_id}' belongs to {}",
                    folder.name, parent.root
                )),
                Some(_) => {}
            }
        }
        let mut visited = BTreeSet::new();
        let mut cursor = folder.parent_id.clone();
        while let Some(current) = cursor {
            if current == folder.id || !visited.insert(current.clone()) {
                issues.push(format!(
                    "folder '{}': the parent chain forms a cycle",
                    folder.name
                ));
                break;
            }
            cursor = index
                .folders
                .iter()
                .find(|candidate| candidate.id == current)
                .and_then(|candidate| candidate.parent_id.clone());
        }
    }

    for entity in &content.entity_templates {
        check_item_folder("entity template", "entity_templates", &entity.name, entity.folder_id.as_deref(), &index, &mut issues);
    }
    for template in &content.trait_templates {
        check_item_folder("trait", "trait_templates", &template.name, template.folder_id.as_deref(), &index, &mut issues);
    }
    for asset in &content.assets {
        check_item_folder("asset", "assets", &asset.name, asset.folder_id.as_deref(), &index, &mut issues);
    }
    for storylet in &content.storylets {
        check_item_folder("storylet", "storylets", &storylet.title, storylet.folder_id.as_deref(), &index, &mut issues);
        if !STORYLET_CLUSTER_TYPES.contains(&storylet.cluster_type.as_str()) {
            issues.push(format!("storylet '{}': unsupported cluster_type '{}'", storylet.title, storylet.cluster_type));
        }
    }

    let mut ids = BTreeSet::new();
    let mut object_ids = BTreeSet::new();
    let mut game_ids = BTreeSet::new();
    let mut registered_tags = BTreeSet::new();

    for tag in &content.gameplay_tags {
        let name = tag.name.trim();
        if name.is_empty() {
            issues.push("gameplay tag: name is required".into());
        } else if name.split('.').any(|segment| !is_identifier(segment)) {
            issues.push(format!("gameplay tag '{name}': name must be a dot-separated identifier path"));
        }
        if !registered_tags.insert(name.to_ascii_lowercase()) {
            issues.push(format!("gameplay tag '{name}': duplicate tag name"));
        }
    }
    for entity in &content.entities {
        for tag in &entity.tags {
            if !content.gameplay_tags.iter().any(|registered| registered.name == *tag) {
                issues.push(format!("intrinsic entity '{}': gameplay tag '{}' is not registered", entity.name, tag));
            }
        }
    }
    for entity in &content.entity_templates {
        if !ids.insert(entity.id.clone()) {
            issues.push(format!("entity template '{}': duplicate entity id", entity.name));
        }
        for tag in &entity.tags {
            if !content.gameplay_tags.iter().any(|registered| registered.name == *tag) {
                issues.push(format!("entity template '{}': gameplay tag '{}' is not registered", entity.name, tag));
            }
        }
        if entity.object_id.is_empty() {
            issues.push(format!("entity template '{}': object_id is required", entity.name));
        } else if !object_ids.insert(entity.object_id.clone()) {
            issues.push(format!("entity template '{}': duplicate object_id '{}'", entity.name, entity.object_id));
        }
        if entity.game_id.is_empty() {
            issues.push(format!("entity template '{}': game_id is required", entity.name));
        } else if !game_ids.insert(entity.game_id.clone()) {
            issues.push(format!("entity template '{}': duplicate game ID '{}'", entity.name, entity.game_id));
        }
    }

    for entity in &content.entity_templates {
        if let Err(error) = validate_unique_traits(&entity.traits, &index.templates, &format!("entity template '{}'", entity.name)) { issues.push(error); }
        if let Err(error) = crate::tree_graph::validate_tree_item_traits(&entity.traits, &index.templates, &format!("entity template '{}'", entity.name)) { issues.push(error); }
        let mut property_ids = BTreeSet::new();
        let mut property_keys = BTreeSet::new();
        for property in &entity.properties {
            if property.id.trim().is_empty() || !property_ids.insert(property.id.clone()) {
                issues.push(format!(
                    "entity '{}': missing or duplicate property id",
                    entity.name
                ));
            }
            if !is_identifier(&property.key) {
                issues.push(format!(
                    "entity '{}': property key '{}' must be an identifier",
                    entity.name, property.key
                ));
            } else if !property_keys.insert(property.key.clone()) {
                issues.push(format!(
                    "entity '{}': duplicate property key '{}'",
                    entity.name, property.key
                ));
            }
            if property.name.trim().is_empty() {
                issues.push(format!("entity '{}': property '{}' needs a name", entity.name, property.key));
            }
            if !value_shape_matches(property.property_type, property.is_array, &property.value) {
                issues.push(format!(
                    "entity '{}': property '{}' value does not match its type",
                    entity.name, property.key
                ));
                continue;
            }
            if let Err(error) =
                validate_property_refs(property.property_type, property.is_array, &property.value, &index, "reference")
            {
                issues.push(format!("entity '{}': {error}", entity.name));
            }
        }

        let mut trait_instance_ids = BTreeSet::new();
        for instance in &entity.traits {
            if !trait_instance_ids.insert(instance.template_id.clone()) && !index.templates.iter().find(|t| t.id == instance.template_id).is_some_and(|t| t.allow_multiple) {
                issues.push(format!("entity '{}': trait '{}' does not allow multiple instances", entity.name, instance.template_id));
            }
            let Some(template) = index.templates.iter().find(|template| template.id == instance.template_id) else {
                continue;
            };
            let mut properties = Vec::new();
            collect_template_properties(template, &index.templates, &mut BTreeSet::new(), &mut properties);
            for (key, value) in &instance.values {
                let Some(property) = properties.iter().find(|property| property.key() == key) else {
                    issues.push(format!("entity '{}': trait '{}' property '{}' is not declared", entity.name, template.key, key));
                    continue;
                };
                if !value_shape_matches(property.property.property_type, property.property.is_array, value) {
                    issues.push(format!(
                        "entity '{}': trait '{}' property '{}' value does not match its type",
                        entity.name, template.key, key
                    ));
                    continue;
                }
                if let Err(error) = validate_property_refs(
                    property.property.property_type,
                    property.property.is_array,
                    value,
                    &index,
                    "reference",
                ) {
                    issues.push(format!("entity '{}': trait '{}' {error}", entity.name, template.key));
                }
            }
        }
    }
    for template in &content.trait_templates {
        if template.name.trim().is_empty() {
            issues.push("trait: name is required".to_string());
        }
        let mut candidate = template.clone();
        if let Err(error) = validate_trait_properties(&mut candidate.properties, "trait", &index) {
            issues.push(format!("trait '{}': {error}", template.name));
        }
        if let Err(error) = crate::tree_graph::validate_tree_item_template(template, &format!("trait '{}'", template.name)) {
            issues.push(error);
        }
        if let Some(thumb) = template.thumbnail.as_deref().filter(|t| !t.trim().is_empty()) {
            if let Err(error) = files::validate_image_ref(thumb) {
                issues.push(format!("trait '{}': {error}", template.name));
            }
        }
    }

    let mut template_ids = BTreeSet::new();
    let mut template_keys = BTreeSet::new();
    for template in &content.trait_templates {
        if template.id.trim().is_empty() || !template_ids.insert(template.id.clone()) {
            issues.push(format!(
                "trait '{}': missing or duplicate template id",
                template.name
            ));
        }
        if !is_identifier(&template.key) {
            issues.push(format!(
                "trait '{}': key '{}' must be an identifier",
                template.name, template.key
            ));
        } else if !template_keys.insert(template.key.clone()) {
            issues.push(format!(
                "trait '{}': duplicate key '{}'",
                template.name, template.key
            ));
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use crate::contracts::TraitInstance;

    #[test]
    fn unique_traits_are_limited_across_entity_kinds_and_repeated_instances() {
        let templates: Vec<TraitTemplateRecord> = vec![
            serde_json::from_value(json!({"id":"u","key":"unique","name":"Unique","unique":true,"allow_multiple":true,"properties":[]})).unwrap(),
            serde_json::from_value(json!({"id":"n","key":"normal","name":"Normal","unique":false,"allow_multiple":true,"properties":[]})).unwrap(),
        ];
        let index = WorkspaceIndex { templates: templates.clone(), ..WorkspaceIndex::default() };
        let payload = json!({"name":"Sample","traits":[{"template_id":"u"},{"template_id":"n"}]});
        assert!(prepare_entity(&payload, None, &index).is_ok());
        assert!(prepare_entity_record(&payload, None, &index).is_ok());
        let duplicate = json!({"name":"Sample","traits":[{"template_id":"u"},{"template_id":"u"}]});
        assert!(prepare_entity(&duplicate, None, &index).unwrap_err().contains("only one unique trait"));
        assert!(prepare_entity_record(&duplicate, None, &index).unwrap_err().contains("only one unique trait"));
        let repeated_non_unique = json!({"name":"Sample","traits":[{"template_id":"n"},{"template_id":"n"}]});
        assert!(prepare_entity(&repeated_non_unique, None, &index).is_ok());
        assert!(prepare_entity_record(&repeated_non_unique, None, &index).is_ok());

        let mut toggled = templates.clone();
        toggled.iter_mut().find(|template| template.id == "n").unwrap().unique = true;
        let repeated_existing: Vec<TraitInstance> = serde_json::from_value(json!([
            {"template_id":"n"},{"template_id":"n"}
        ])).unwrap();
        assert!(validate_unique_traits(&repeated_existing, &templates, "entity").is_ok());
        assert!(validate_unique_traits(&repeated_existing, &toggled, "entity").unwrap_err().contains("only one unique trait"));
    }

    #[test]
    fn tree_item_entity_and_template_save_validates_nonnegative_level_and_cost() {
        let templates: Vec<TraitTemplateRecord> = vec![
            serde_json::from_value(json!({
                "id": "tid-tree-item",
                "key": "tree_item",
                "name": "TreeItem",
                "properties": [
                    { "key": "level", "name": "Level", "type": "int", "is_array": false, "default": 0 },
                    { "key": "cost", "name": "Cost", "type": "int", "is_array": false, "default": 0 }
                ]
            })).unwrap(),
        ];
        let index = WorkspaceIndex { templates: templates.clone(), ..WorkspaceIndex::default() };

        // 1. Valid nonnegative integers pass for both entity and entity_template
        let valid_payload = json!({
            "name": "Skill Node",
            "traits": [{ "template_id": "tid-tree-item", "values": { "level": 0, "cost": 10 } }]
        });
        assert!(prepare_entity(&valid_payload, None, &index).is_ok());
        assert!(prepare_entity_record(&valid_payload, None, &index).is_ok());

        // 2. Absent values remain valid (pre-existing items)
        let absent_payload = json!({
            "name": "Skill Node",
            "traits": [{ "template_id": "tid-tree-item", "values": {} }]
        });
        assert!(prepare_entity(&absent_payload, None, &index).is_ok());
        assert!(prepare_entity_record(&absent_payload, None, &index).is_ok());

        // 3. Negative level is rejected on save independently of membership in any graph
        let neg_level_payload = json!({
            "name": "Skill Node",
            "traits": [{ "template_id": "tid-tree-item", "values": { "level": -1 } }]
        });
        let err_template = prepare_entity(&neg_level_payload, None, &index).unwrap_err();
        assert!(err_template.contains("TreeItem 'level' must be a nonnegative integer"), "got {err_template}");
        let err_record = prepare_entity_record(&neg_level_payload, None, &index).unwrap_err();
        assert!(err_record.contains("TreeItem 'level' must be a nonnegative integer"), "got {err_record}");

        // 4. Negative cost is rejected
        let neg_cost_payload = json!({
            "name": "Skill Node",
            "traits": [{ "template_id": "tid-tree-item", "values": { "cost": -2 } }]
        });
        assert!(prepare_entity(&neg_cost_payload, None, &index).unwrap_err().contains("TreeItem 'cost' must be a nonnegative integer"));
        assert!(prepare_entity_record(&neg_cost_payload, None, &index).unwrap_err().contains("TreeItem 'cost' must be a nonnegative integer"));

        // 5. Guard trait template default prevents negative displayed default
        let neg_default_trait_payload = json!({
            "id": "tid-tree-item",
            "key": "tree_item",
            "name": "TreeItem",
            "properties": [
                { "key": "level", "name": "Level", "type": "int", "is_array": false, "default": -1 }
            ]
        });
        let err_trait = prepare_trait_template(&neg_default_trait_payload, None, &index).unwrap_err();
        assert!(err_trait.contains("TreeItem 'level' default must be a nonnegative integer"), "got {err_trait}");
    }

    #[test]
    fn default_game_id_uses_pascal_case_and_deduplicates() {
        let empty = BTreeSet::new();
        assert_eq!(default_game_id("guard captain", &empty), "Ent_GuardCaptain");
        assert_eq!(default_game_id("  ", &empty), "Ent_Entity");
        let mut used = BTreeSet::new();
        used.insert("Ent_Guard".to_string());
        used.insert("Ent_Guard_2".to_string());
        assert_eq!(default_game_id("Guard", &used), "Ent_Guard_3");
    }

    #[test]
    fn runtime_attributes_fall_back_to_template_defaults_on_shape_mismatch() {
        let template: TraitTemplateRecord = serde_json::from_value(json!({
            "id": "tpl",
            "key": "Personality",
            "name": "Personality",
            "properties": [{
                "id": "bravery",
                "key": "Bravery",
                "name": "Bravery",
                "type": "float",
                "default": 1.0
            }]
        })).unwrap();
        let mut entity: EntityTemplateRecord = serde_json::from_value(json!({
            "id": "guard",
            "name": "Guard",
                    }))
        .unwrap();
        entity.traits.push(TraitInstance {
            id: "trait-instance".into(),
            template_id: "tpl".into(),
            values: BTreeMap::from([("Bravery".to_string(), json!("not a number"))]),
        });
        let attributes = runtime_attributes(&entity, &[template]);
        assert_eq!(attributes["trait"]["Personality"]["Bravery"], json!(1.0));
        assert_eq!(attribute_path(&attributes, "trait.Personality.Bravery"), Some(json!(1.0)));
        assert_eq!(attribute_path(&attributes, "name"), None);
    }

    #[test]
    fn trait_capabilities_overlap_without_adding_gameplay_tags_and_generic_entities_project() {
        let templates: Vec<TraitTemplateRecord> = ["character", "location", "fact"]
            .into_iter()
            .map(|key| serde_json::from_value(json!({
                "id": format!("system.{key}"), "key": key, "name": key,
                "system": true, "allow_multiple": false, "properties": []
            })).unwrap())
            .chain(std::iter::once(serde_json::from_value(json!({
                "id": "inventory.item", "key": "item", "name": "Inventory Item",
                "allow_multiple": false, "properties": []
            })).unwrap()))
            .collect();
        let world = default_world_entity();
        let entities: Vec<EntityTemplateRecord> = vec![
            json!({"id":"dual","name":"Dual","tags":["story.secret"],"traits":[
                {"template_id":"system.character"},{"template_id":"system.location"}]}),
            json!({"id":"generic","name":"Generic","tags":["quest.marker"]}),
            json!({"id":"key","name":"Key","traits":[{"template_id":"inventory.item"}]}),
        ].into_iter().map(|value| serde_json::from_value(value).unwrap()).collect();

        let projected = runtime_records_with_world(&world, &entities, &templates);
        let dual_character = projected.characters.iter().find(|row| row.id == "dual").unwrap();
        let dual_location = projected.locations.iter().find(|row| row.id == "dual").unwrap();
        assert_eq!(dual_character.id, dual_location.id);
        assert_eq!(projected.entity_tag_seeds.iter().find(|(id, _)| id == "dual").unwrap().1, vec!["story.secret"]);
        assert!(!projected.world_tags.iter().any(|tag| tag == "world"));
        assert_eq!(projected.world_objects.iter().find(|row| row.id == "generic").unwrap().layer, WorldObjectLayer::Generic);
        assert_eq!(projected.world_objects.iter().find(|row| row.id == "key").unwrap().layer, WorldObjectLayer::Item);
    }
    #[test]
    fn bundle_rejects_malformed_gameplay_tag_paths() {
        let mut content = ProjectBundleContent::default();
        content.gameplay_tags = [".quest", "quest.", "quest..secret", "quest.secret"]
            .into_iter()
            .map(|name| serde_json::from_value(json!({"id": name, "name": name})).unwrap())
            .collect();
        let issues = validate_bundle_content(&content);
        let tag_issues: Vec<_> = issues.iter().filter(|issue| issue.contains("dot-separated identifier path")).collect();
        assert_eq!(tag_issues.len(), 3);
        assert!(tag_issues.iter().all(|issue| !issue.contains("'quest.secret'")));
    }
    #[test]
    fn generic_projection_reports_unrepresentable_entity_properties() {
        let entity: EntityTemplateRecord = serde_json::from_value(json!({
            "id": "guard",
            "name": "Guard",
                        "properties": [{
                "id": "trust",
                "key": "trust",
                "name": "Trust",
                "type": "int",
                "value": 3
            }]
        })).unwrap();
        assert_eq!(
            generic_playtest_projection_issues(std::slice::from_ref(&entity)),
            vec!["entity 'guard': generic playtests cannot represent typed custom properties; use Chrono playtests"]
        );
        assert!(generic_playtest_projection_issues(&[]).is_empty());
    }

    #[test]
    fn trait_template_thumbnail_normalization_and_validation() {
        let index = WorkspaceIndex::default();
        // 1. Valid project-relative path
        let payload = json!({
            "id": "t1",
            "key": "skill",
            "name": "Skill",
            "thumbnail": "project-files/thumbnails/skill.png",
            "properties": []
        });
        let prepared = prepare_trait_template(&payload, None, &index).unwrap();
        assert_eq!(prepared.thumbnail.as_deref(), Some("project-files/thumbnails/skill.png"));

        // 2. Valid web URL
        let payload_url = json!({
            "id": "t2",
            "key": "buff",
            "name": "Buff",
            "thumbnail": "https://example.com/buff.jpg",
            "properties": []
        });
        let prepared_url = prepare_trait_template(&payload_url, None, &index).unwrap();
        assert_eq!(prepared_url.thumbnail.as_deref(), Some("https://example.com/buff.jpg"));

        // 3. Empty string normalizes to None
        let payload_empty = json!({
            "id": "t3",
            "key": "spell",
            "name": "Spell",
            "thumbnail": "   ",
            "properties": []
        });
        let prepared_empty = prepare_trait_template(&payload_empty, None, &index).unwrap();
        assert_eq!(prepared_empty.thumbnail, None);

        // 4. Null normalizes to None
        let payload_null = json!({
            "id": "t4",
            "key": "aura",
            "name": "Aura",
            "thumbnail": null,
            "properties": []
        });
        let prepared_null = prepare_trait_template(&payload_null, None, &index).unwrap();
        assert_eq!(prepared_null.thumbnail, None);

        // 5. Omitted field defaults to None
        let payload_omitted = json!({
            "id": "t5",
            "key": "curse",
            "name": "Curse",
            "properties": []
        });
        let prepared_omitted = prepare_trait_template(&payload_omitted, None, &index).unwrap();
        assert_eq!(prepared_omitted.thumbnail, None);

        // 6. Invalid paths are rejected
        for invalid in [
            "project-files/../secret.png",
            "/thumbnails/skill.png",
            "thumbnails\\skill.png",
            "thumbnails/skill.gif",
            "thumbnails//skill.png",
        ] {
            let payload_invalid = json!({
                "id": "t_inv",
                "key": "invalid_trait",
                "name": "Invalid",
                "thumbnail": invalid,
                "properties": []
            });
            assert!(prepare_trait_template(&payload_invalid, None, &index).is_err());
        }

        // 7. System traits cannot modify thumbnail
        let system_trait: TraitTemplateRecord = serde_json::from_value(json!({
            "id": "system.character",
            "key": "character",
            "name": "Character",
            "system": true,
            "allow_multiple": false,
            "properties": []
        })).unwrap();
        let modify_system = json!({
            "id": "system.character",
            "key": "character",
            "name": "Character",
            "system": true,
            "thumbnail": "project-files/thumbnails/char.png",
            "properties": []
        });
        assert!(prepare_trait_template(&modify_system, Some(&system_trait), &index).unwrap_err().contains("system traits are immutable"));
    }

    #[test]
    fn bundle_validation_checks_trait_thumbnails() {
        let mut content = ProjectBundleContent::default();
        for spec in SYSTEM_TRAIT_TEMPLATES {
            content.trait_templates.push(serde_json::from_value(json!({
                "id": format!("system.{}", spec.key),
                "key": spec.key,
                "name": spec.name,
                "system": true,
                "allow_multiple": spec.allow_multiple,
                "unique": spec.unique,
                "properties": spec.properties.iter().map(|p| json!({
                    "key": p.key, "name": p.name, "type": p.property_type, "is_array": p.is_array, "default": null
                })).collect::<Vec<_>>()
            })).unwrap());
        }
        content.entities.push(default_world_entity());

        // Valid custom trait with thumbnail
        content.trait_templates.push(serde_json::from_value(json!({
            "id": "custom.valid",
            "key": "valid_trait",
            "name": "Valid Trait",
            "thumbnail": "project-files/thumbnails/icon.png",
            "properties": []
        })).unwrap());
        assert!(validate_bundle_content(&content).is_empty());

        // Invalid custom trait with thumbnail
        content.trait_templates.push(serde_json::from_value(json!({
            "id": "custom.invalid",
            "key": "invalid_trait",
            "name": "Invalid Trait",
            "thumbnail": "../escape.png",
            "properties": []
        })).unwrap());
        let issues = validate_bundle_content(&content);
        assert!(issues.iter().any(|i| i.contains("trait 'Invalid Trait'") && i.contains("cannot contain empty, '.' or '..' segments")));
    }
}
