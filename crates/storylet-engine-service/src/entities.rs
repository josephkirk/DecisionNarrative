//! Entity, folder, trait-template, and asset authoring rules.
//!
//! Characters and locations are authored as one generic `entities` kind; the
//! typed `character`/`location` sections carry the engine-facing fields and are
//! projected back into [`CharacterRecord`]/[`LocationRecord`] at runtime.

use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

use crate::contracts::{
    AssetRecord, AssetVersionRecord, CharacterRecord, CharacterSection, EntityRecord, EntityType,
    FolderRecord, FOLDER_ROOTS, LocationRecord, LocationSection, NpcBehaviorRecord,
    PropertyDefinition, PropertyType, ProjectBundle, ProjectBundleContent, QualityRecord,
    ScenarioRecord, StoryletRecord, TraitInstance, TraitTemplateRecord, WorldObjectLayer,
    WorldObjectRecord,
};
use crate::files;

/// Entities, trait templates, assets, folders, and storylet ids used to validate
/// a single authored record or a whole bundle.
#[derive(Clone, Debug, Default)]
pub struct WorkspaceIndex {
    pub entities: Vec<EntityRecord>,
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

fn is_object_id(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("0x") else {
        return false;
    };
    hex.len() == 16
        && hex
            .chars()
            .all(|c| c.is_ascii_digit() || ('A'..='F').contains(&c))
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

fn type_prefix(entity_type: EntityType) -> &'static str {
    match entity_type {
        EntityType::Character => "Chr",
        EntityType::Location => "Loc",
        EntityType::Item => "Itm",
        EntityType::Fact => "Fct",
        EntityType::Other => "Ent",
    }
}

pub fn default_game_id(entity_type: EntityType, name: &str, used: &BTreeSet<String>) -> String {
    let pascal = pascal_case(name);
    let pascal = if pascal.is_empty() {
        "Entity".to_string()
    } else {
        pascal
    };
    dedupe_suffix(&format!("{}_{pascal}", type_prefix(entity_type)), used)
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
pub fn asset_link(value: &Value) -> Option<(String, String)> {
    let asset_id = value.get("asset_id")?.as_str()?.to_string();
    let version = value.get("version")?.as_str()?.to_string();
    Some((asset_id, version))
}

fn scalar_shape_matches(property_type: PropertyType, value: &Value) -> bool {
    match property_type {
        PropertyType::Bool => value.is_boolean(),
        PropertyType::String => value.is_string(),
        PropertyType::Float => value.as_f64().is_some_and(f64::is_finite),
        PropertyType::Int => value.as_i64().is_some(),
        PropertyType::Asset => value.is_null() || asset_link(value).is_some(),
        PropertyType::Entity => value.is_null() || value.is_string(),
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

pub fn default_value(property_type: PropertyType, is_array: bool) -> Value {
    if is_array {
        return Value::Array(Vec::new());
    }
    match property_type {
        PropertyType::Bool => Value::Bool(false),
        PropertyType::String => Value::String(String::new()),
        PropertyType::Float => Value::from(0.0_f64),
        PropertyType::Int => Value::from(0_i64),
        PropertyType::Asset | PropertyType::Entity => Value::Null,
    }
}

pub fn character_record(entity: &EntityRecord, attributes: Value) -> Option<CharacterRecord> {
    let section = entity.character.clone()?;
    Some(CharacterRecord {
        id: entity.id.clone(),
        name: entity.name.clone(),
        role: section.role,
        home_location: section.home_location,
        faction_id: section.faction_id,
        current_location: section.current_location,
        status_axes: section.status_axes,
        needs: section.needs,
        agenda: section.agenda,
        red_line: section.red_line,
        schedule: section.schedule,
        qualities: section.qualities,
        passion_pool: section.passion_pool,
        active_passions: section.active_passions,
        trust_inertia: section.trust_inertia,
        thresholds: section.thresholds,
        zero_sum_passion_links: section.zero_sum_passion_links,
        created_at: entity.created_at,
        attributes,
    })
}

pub fn location_record(entity: &EntityRecord, attributes: Value) -> Option<LocationRecord> {
    let section = entity.location.clone()?;
    Some(LocationRecord {
        id: entity.id.clone(),
        name: entity.name.clone(),
        description: entity.description.clone(),
        tags: section.tags,
        connected_zones: section.connected_zones,
        environmental_params: section.environmental_params,
        devices: section.devices,
        created_at: entity.created_at,
        attributes,
    })
}

pub fn world_object(entity: &EntityRecord, attributes: Value) -> Option<WorldObjectRecord> {
    let layer = match entity.entity_type {
        EntityType::Item => WorldObjectLayer::Item,
        EntityType::Fact => WorldObjectLayer::Fact,
        _ => return None,
    };
    Some(WorldObjectRecord {
        id: entity.id.clone(),
        name: entity.name.clone(),
        layer,
        attributes,
    })
}

pub fn entity_from_legacy_character(record: CharacterRecord, object_id: String, game_id: String) -> EntityRecord {
    EntityRecord {
        id: record.id.clone(),
        object_id,
        game_id,
        game_id_auto: false,
        name: record.name.clone(),
        entity_type: EntityType::Character,
        description: String::new(),
        thumbnail: None,
        folder_id: None,
        properties: Vec::new(),
        traits: Vec::new(),
        character: Some(CharacterSection {
            role: record.role,
            home_location: record.home_location,
            faction_id: record.faction_id,
            current_location: record.current_location,
            status_axes: record.status_axes,
            needs: record.needs,
            agenda: record.agenda,
            red_line: record.red_line,
            schedule: record.schedule,
            qualities: record.qualities,
            passion_pool: record.passion_pool,
            active_passions: record.active_passions,
            trust_inertia: record.trust_inertia,
            thresholds: record.thresholds,
            zero_sum_passion_links: record.zero_sum_passion_links,
        }),
        location: None,
        created_at: record.created_at,
    }
}

pub fn entity_from_legacy_location(record: LocationRecord, object_id: String, game_id: String) -> EntityRecord {
    EntityRecord {
        id: record.id.clone(),
        object_id,
        game_id,
        game_id_auto: false,
        name: record.name.clone(),
        entity_type: EntityType::Location,
        description: record.description.clone(),
        thumbnail: None,
        folder_id: None,
        properties: Vec::new(),
        traits: Vec::new(),
        character: None,
        location: Some(LocationSection {
            tags: record.tags,
            connected_zones: record.connected_zones,
            environmental_params: record.environmental_params,
            devices: record.devices,
        }),
        created_at: record.created_at,
    }
}

/// Flattened runtime attributes: `prop.<key>` and `trait.<templateKey>.<key>`.
pub fn runtime_attributes(entity: &EntityRecord, templates: &[TraitTemplateRecord]) -> Value {
    let mut properties = Map::new();
    for property in &entity.properties {
        if property.value.is_null() {
            continue;
        }
        properties.insert(property.key.clone(), property.value.clone());
    }

    let mut traits = Map::new();
    for instance in &entity.traits {
        let Some(template) = templates.iter().find(|t| t.id == instance.template_id) else {
            continue;
        };
        let mut values = Map::new();
        for definition in &template.properties {
            let effective = match instance.values.get(&definition.id) {
                Some(value)
                    if value_shape_matches(definition.property_type, definition.is_array, value) =>
                {
                    value.clone()
                }
                _ => definition.default.clone(),
            };
            if effective.is_null() {
                continue;
            }
            values.insert(definition.key.clone(), effective);
        }
        if values.is_empty() {
            continue;
        }
        traits.insert(template.key.clone(), Value::Object(values));
    }

    if properties.is_empty() && traits.is_empty() {
        return Value::Null;
    }
    let mut root = Map::new();
    if !properties.is_empty() {
        root.insert("prop".into(), Value::Object(properties));
    }
    if !traits.is_empty() {
        root.insert("trait".into(), Value::Object(traits));
    }
    Value::Object(root)
}

pub fn runtime_records(entities: &[EntityRecord], templates: &[TraitTemplateRecord]) -> RuntimeRecords {
    let mut records = RuntimeRecords::default();
    for entity in entities {
        let attributes = runtime_attributes(entity, templates);
        if let Some(character) = character_record(entity, attributes.clone()) {
            records.characters.push(character);
        }
        if let Some(location) = location_record(entity, attributes.clone()) {
            records.locations.push(location);
        }
        if let Some(object) = world_object(entity, attributes) {
            records.world_objects.push(object);
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
                {
                    return Err(format!(
                        "{prefix}: entity reference '{target}' does not exist"
                    ));
                }
            }
            PropertyType::Asset => {
                if let Some((asset_id, version)) = asset_link(item) {
                    let Some(asset) = index.assets.iter().find(|asset| asset.id == asset_id) else {
                        return Err(format!(
                            "{prefix}: asset reference '{asset_id}' does not exist"
                        ));
                    };
                    if version != "latest"
                        && !asset.versions.iter().any(|existing| existing.id == version)
                    {
                        return Err(format!(
                            "{prefix}: asset '{asset_id}' has no version '{version}'"
                        ));
                    }
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
    stored: Option<&EntityRecord>,
    index: &WorkspaceIndex,
) -> Result<EntityRecord, String> {
    const PREFIX: &str = "Entity validation failed";
    let mut entity: EntityRecord = serde_json::from_value(payload.clone())
        .map_err(|error| format!("{PREFIX}: {error}"))?;

    let requested_object_id = entity.object_id.trim().to_string();
    match stored {
        Some(previous) => {
            if !requested_object_id.is_empty() && requested_object_id != previous.object_id {
                return Err(format!("{PREFIX}: object_id is immutable"));
            }
            entity.object_id = previous.object_id.clone();
        }
        None => {
            if requested_object_id.is_empty() {
                entity.object_id = new_object_id();
            } else if !is_object_id(&requested_object_id) {
                return Err(format!(
                    "{PREFIX}: object_id '{requested_object_id}' must be 0x followed by 16 hex digits"
                ));
            } else if index
                .entities
                .iter()
                .any(|existing| existing.object_id == requested_object_id)
            {
                return Err(format!(
                    "{PREFIX}: object_id '{requested_object_id}' is already in use"
                ));
            } else {
                entity.object_id = requested_object_id;
            }
        }
    }

    if entity.id.trim().is_empty() {
        entity.id = match stored {
            Some(previous) => previous.id.clone(),
            None => entity.object_id.clone(),
        };
    }

    entity.name = entity.name.trim().to_string();
    if entity.name.is_empty() {
        return Err(format!("{PREFIX}: name cannot be empty"));
    }

    let game_id = entity.game_id.trim().to_string();
    let used_game_ids: BTreeSet<String> = index
        .entities
        .iter()
        .filter(|existing| existing.id != entity.id)
        .map(|existing| existing.game_id.clone())
        .collect();
    if game_id.is_empty() {
        entity.game_id = default_game_id(entity.entity_type, &entity.name, &used_game_ids);
        entity.game_id_auto = true;
    } else {
        let mut game_id = game_id;
        // Only an ID this service generated may follow a rename; an ID the author
        // supplied is theirs, even when it happens to look generated.
        let auto = match stored {
            Some(previous) if game_id == previous.game_id => previous.game_id_auto,
            _ => false,
        };
        if auto {
            game_id = default_game_id(entity.entity_type, &entity.name, &used_game_ids);
        }
        if !is_identifier(&game_id) {
            return Err(format!(
                "{PREFIX}: game ID '{game_id}' must start with a letter or underscore and contain only letters, digits, and underscores"
            ));
        }
        if let Some(owner) = index
            .entities
            .iter()
            .find(|existing| existing.id != entity.id && existing.game_id == game_id)
        {
            return Err(format!(
                "{PREFIX}: game ID '{game_id}' is already used by '{}'",
                owner.name
            ));
        }
        entity.game_id = game_id;
        entity.game_id_auto = auto;
    }

    match entity.entity_type {
        EntityType::Character => {
            entity.location = None;
            if entity.character.is_none() {
                entity.character = Some(CharacterSection::default());
            }
        }
        EntityType::Location => {
            entity.character = None;
            if entity.location.is_none() {
                entity.location = Some(LocationSection::default());
            }
        }
        EntityType::Item | EntityType::Fact | EntityType::Other => {
            entity.character = None;
            entity.location = None;
        }
    }

    if entity.thumbnail.as_deref().is_some_and(|value| !value.trim().is_empty()) {
        let thumbnail = entity.thumbnail.clone().unwrap_or_default();
        files::validate_image_ref(&thumbnail).map_err(|error| format!("{PREFIX}: {error}"))?;
    } else {
        entity.thumbnail = None;
    }

    normalize_optional_id(&mut entity.folder_id);
    validate_folder_ref("entities", entity.folder_id.as_deref(), index)
        .map_err(|error| format!("{PREFIX}: {error}"))?;

    // Self references are legal, so validate against the record being saved.
    let mut ref_index = index.clone();
    ref_index.entities.push(entity.clone());

    let mut property_ids = BTreeSet::new();
    let mut property_keys = BTreeSet::new();
    for property in &mut entity.properties {
        if property.id.trim().is_empty() {
            property.id = new_object_id();
        }
        if !property_ids.insert(property.id.clone()) {
            return Err(format!(
                "{PREFIX}: duplicate property id '{}'",
                property.id
            ));
        }
        property.key = property.key.trim().to_string();
        if !is_identifier(&property.key) {
            return Err(format!(
                "{PREFIX}: property key '{}' must start with a letter or underscore and contain only letters, digits, and underscores",
                property.key
            ));
        }
        if !property_keys.insert(property.key.clone()) {
            return Err(format!(
                "{PREFIX}: duplicate property key '{}'",
                property.key
            ));
        }
        property.name = property.name.trim().to_string();
        if property.name.is_empty() {
            return Err(format!("{PREFIX}: property '{}' needs a name", property.key));
        }
        if !value_shape_matches(property.property_type, property.is_array, &property.value) {
            return Err(format!(
                "{PREFIX}: property '{}' value does not match its type",
                property.key
            ));
        }
        validate_property_refs(
            property.property_type,
            property.is_array,
            &property.value,
            &ref_index,
            PREFIX,
        )?;
    }

    let mut template_ids = BTreeSet::new();
    let mut traits = Vec::with_capacity(entity.traits.len());
    for instance in &entity.traits {
        if !template_ids.insert(instance.template_id.clone()) {
            return Err(format!(
                "{PREFIX}: trait template '{}' is applied more than once",
                instance.template_id
            ));
        }
        let Some(template) = index
            .templates
            .iter()
            .find(|template| template.id == instance.template_id)
        else {
            return Err(format!(
                "{PREFIX}: trait template '{}' does not exist",
                instance.template_id
            ));
        };
        let mut values = BTreeMap::new();
        for (key, value) in &instance.values {
            let Some(definition) = template
                .properties
                .iter()
                .find(|definition| &definition.id == key)
            else {
                // Unknown property ids are authoring debris after a template edit.
                continue;
            };
            if !value_shape_matches(definition.property_type, definition.is_array, value) {
                continue;
            }
            validate_property_refs(
                definition.property_type,
                definition.is_array,
                value,
                &ref_index,
                PREFIX,
            )?;
            values.insert(key.clone(), value.clone());
        }
        traits.push(TraitInstance {
            template_id: instance.template_id.clone(),
            values,
        });
    }
    entity.traits = traits;

    if let Some(section) = &mut entity.location {
        section.tags = trim_tags(&section.tags);
        if let Some(zones) = &mut section.connected_zones {
            zones.retain(|zone| !zone.trim().is_empty());
            let mut checked = BTreeSet::new();
            for zone in zones.iter() {
                if !checked.insert(zone.clone()) {
                    continue;
                }
                let exists = index.entities.iter().any(|candidate| {
                    candidate.id == *zone
                        && candidate.entity_type == EntityType::Location
                        && candidate.id != entity.id
                });
                if !exists {
                    return Err(format!(
                        "{PREFIX}: connected zone '{zone}' is not another location"
                    ));
                }
            }
        }
    }

    entity.created_at = Some(
        stored
            .and_then(|previous| previous.created_at)
            .or(entity.created_at)
            .unwrap_or_else(now_ms),
    );
    entity.description = entity.description.trim().to_string();
    Ok(entity)
}

fn prepare_property_definitions(
    definitions: &mut [PropertyDefinition],
    prefix: &str,
    index: &WorkspaceIndex,
) -> Result<(), String> {
    let mut ids = BTreeSet::new();
    let mut keys = BTreeSet::new();
    for definition in definitions.iter_mut() {
        if definition.id.trim().is_empty() {
            definition.id = new_object_id();
        }
        if !ids.insert(definition.id.clone()) {
            return Err(format!("{prefix}: duplicate property id '{}'", definition.id));
        }
        definition.key = definition.key.trim().to_string();
        if !is_identifier(&definition.key) {
            return Err(format!(
                "{prefix}: property key '{}' must start with a letter or underscore and contain only letters, digits, and underscores",
                definition.key
            ));
        }
        if !keys.insert(definition.key.clone()) {
            return Err(format!(
                "{prefix}: duplicate property key '{}'",
                definition.key
            ));
        }
        definition.name = definition.name.trim().to_string();
        if definition.name.is_empty() {
            return Err(format!("{prefix}: property '{}' needs a name", definition.key));
        }
        if !value_shape_matches(definition.property_type, definition.is_array, &definition.default) {
            return Err(format!(
                "{prefix}: property '{}' default does not match its type",
                definition.key
            ));
        }
        validate_property_refs(
            definition.property_type,
            definition.is_array,
            &definition.default,
            index,
            prefix,
        )?;
    }
    Ok(())
}

pub fn prepare_trait_template(
    payload: &Value,
    stored: Option<&TraitTemplateRecord>,
    index: &WorkspaceIndex,
) -> Result<TraitTemplateRecord, String> {
    const PREFIX: &str = "Trait template validation failed";
    let mut template: TraitTemplateRecord = serde_json::from_value(payload.clone())
        .map_err(|error| format!("{PREFIX}: {error}"))?;
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

    prepare_property_definitions(&mut template.properties, PREFIX, index)?;
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
    for quality in &mut content.qualities {
        normalize_optional_id(&mut quality.folder_id);
    }
    for storylet in &mut content.storylets {
        normalize_optional_id(&mut storylet.folder_id);
    }
    for entity in &mut content.entities {
        normalize_optional_id(&mut entity.folder_id);
    }
    for template in &mut content.trait_templates {
        normalize_optional_id(&mut template.folder_id);
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

fn read_rows(conn: &rusqlite::Connection, kind: &str) -> Result<Vec<(String, String)>, String> {
    let mut stmt = conn
        .prepare("SELECT id, data_json FROM content_records WHERE entity = ?1 ORDER BY id")
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([kind], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

/// Convert legacy `characters`/`locations` rows into `entities` rows and drop
/// retired storylet canvas fields. Idempotent: a migrated workspace has no
/// legacy rows left, so the second run is a no-op.
pub fn migrate_legacy_rows(conn: &rusqlite::Connection) -> Result<usize, String> {
    let mut existing_ids = BTreeSet::new();
    let mut existing_object_ids = BTreeSet::new();
    let mut used_game_ids = BTreeSet::new();
    for (_, json) in read_rows(conn, "entities")? {
        let Ok(entity) = serde_json::from_str::<EntityRecord>(&json) else {
            continue;
        };
        existing_ids.insert(entity.id.clone());
        existing_object_ids.insert(entity.object_id.clone());
        used_game_ids.insert(entity.game_id.clone());
    }

    let mut converted: Vec<EntityRecord> = Vec::new();
    for (kind, rows) in [
        ("locations", read_rows(conn, "locations")?),
        ("characters", read_rows(conn, "characters")?),
    ] {
        for (id, json) in rows {
            if !existing_ids.insert(id.clone()) {
                return Err(format!(
                    "Cannot migrate {kind} '{id}': an entity with the same id already exists"
                ));
            }
            let object_id = new_object_id();
            existing_object_ids.insert(object_id.clone());
            let entity = match kind {
                "locations" => {
                    let record: LocationRecord = serde_json::from_str(&json)
                        .map_err(|error| format!("Cannot migrate location '{id}': {error}"))?;
                    let game_id = default_game_id(
                        EntityType::Location,
                        &record.name,
                        &used_game_ids,
                    );
                    used_game_ids.insert(game_id.clone());
                    entity_from_legacy_location(record, object_id, game_id)
                }
                _ => {
                    let record: CharacterRecord = serde_json::from_str(&json)
                        .map_err(|error| format!("Cannot migrate character '{id}': {error}"))?;
                    let game_id = default_game_id(
                        EntityType::Character,
                        &record.name,
                        &used_game_ids,
                    );
                    used_game_ids.insert(game_id.clone());
                    entity_from_legacy_character(record, object_id, game_id)
                }
            };
            converted.push(entity);
        }
    }

    let mut storylet_updates: Vec<(String, String)> = Vec::new();
    for (id, json) in read_rows(conn, "storylets")? {
        let Ok(Value::Object(mut map)) = serde_json::from_str::<Value>(&json) else {
            continue;
        };
        let mut changed = false;
        if map.remove("position").is_some() {
            changed = true;
        }
        if map.remove("size").is_some() {
            changed = true;
        }
        if changed {
            storylet_updates.push((
                id,
                serde_json::to_string(&Value::Object(map)).map_err(|error| error.to_string())?,
            ));
        }
    }

    let changed = converted.len() + storylet_updates.len();
    if changed == 0 {
        return Ok(0);
    }

    for entity in &converted {
        let json = serde_json::to_string(entity).map_err(|error| error.to_string())?;
        conn.execute(
            "INSERT INTO content_records (entity, id, data_json, revision, approved_revision, updated_at)
             VALUES ('entities', ?1, ?2, 1, NULL, ?3)",
            rusqlite::params![entity.id, json, now_ms()],
        )
        .map_err(|error| error.to_string())?;
    }
    for kind in ["locations", "characters"] {
        conn.execute("DELETE FROM content_records WHERE entity = ?1", [kind])
            .map_err(|error| error.to_string())?;
    }
    for (id, json) in &storylet_updates {
        conn.execute(
            "UPDATE content_records SET data_json = ?1 WHERE entity = 'storylets' AND id = ?2",
            rusqlite::params![json, id],
        )
        .map_err(|error| error.to_string())?;
    }
    conn.execute(
        "UPDATE project_metadata SET revision = revision + 1 WHERE singleton = 1",
        [],
    )
    .map_err(|error| error.to_string())?;
    Ok(changed)
}

#[derive(serde::Deserialize)]
struct LegacyBundleContentV2 {
    #[serde(default)]
    qualities: Vec<QualityRecord>,
    #[serde(default)]
    locations: Vec<LocationRecord>,
    #[serde(default)]
    characters: Vec<CharacterRecord>,
    #[serde(default)]
    storylets: Vec<StoryletRecord>,
    #[serde(default, rename = "npcBehaviors")]
    npc_behaviors: Vec<NpcBehaviorRecord>,
    #[serde(default)]
    events: Vec<crate::contracts::EventRecord>,
    #[serde(default)]
    scenarios: Vec<ScenarioRecord>,
}

#[derive(serde::Deserialize)]
struct LegacyBundleV2 {
    #[serde(default)]
    format: String,
    project: crate::contracts::ProjectMetadata,
    #[serde(default)]
    content: Option<LegacyBundleContentV2>,
}

fn legacy_records_to_entities(
    locations: Vec<LocationRecord>,
    characters: Vec<CharacterRecord>,
) -> Result<Vec<EntityRecord>, String> {
    let mut seen = BTreeSet::new();
    let mut used_game_ids = BTreeSet::new();
    let mut entities = Vec::with_capacity(locations.len() + characters.len());
    for location in locations {
        if !seen.insert(location.id.clone()) {
            return Err(format!(
                "Cannot migrate location '{}': an entity with the same id already exists",
                location.id
            ));
        }
        let game_id = default_game_id(EntityType::Location, &location.name, &used_game_ids);
        used_game_ids.insert(game_id.clone());
        entities.push(entity_from_legacy_location(
            location,
            new_object_id(),
            game_id,
        ));
    }
    for character in characters {
        if !seen.insert(character.id.clone()) {
            return Err(format!(
                "Cannot migrate character '{}': an entity with the same id already exists",
                character.id
            ));
        }
        let game_id = default_game_id(EntityType::Character, &character.name, &used_game_ids);
        used_game_ids.insert(game_id.clone());
        entities.push(entity_from_legacy_character(
            character,
            new_object_id(),
            game_id,
        ));
    }
    Ok(entities)
}

pub fn parse_project_bundle(value: &Value) -> Result<ProjectBundle, String> {
    let schema_version = value.get("schema_version").and_then(Value::as_u64);
    let mut bundle = match schema_version {
        Some(3) => serde_json::from_value::<ProjectBundle>(value.clone())
            .map_err(|error| format!("Bundle does not match schema version 3: {error}")),
        Some(2) => {
            let legacy: LegacyBundleV2 = serde_json::from_value(value.clone())
                .map_err(|error| format!("Bundle does not match schema version 2: {error}"))?;
            let content = legacy.content.unwrap_or(LegacyBundleContentV2 {
                qualities: Vec::new(),
                locations: Vec::new(),
                characters: Vec::new(),
                storylets: Vec::new(),
                npc_behaviors: Vec::new(),
                events: Vec::new(),
                scenarios: Vec::new(),
            });
            let entities = legacy_records_to_entities(content.locations, content.characters)?;
            Ok(ProjectBundle {
                format: legacy.format,
                schema_version: 3,
                project: legacy.project,
                content: ProjectBundleContent {
                    qualities: content.qualities,
                    entities,
                    folders: Vec::new(),
                    trait_templates: Vec::new(),
                    assets: Vec::new(),
                    storylets: content.storylets,
                    npc_behaviors: content.npc_behaviors,
                    events: content.events,
                    scenarios: content.scenarios,
                },
            })
        }
        Some(other) => Err(format!(
            "Unsupported schema_version {other}; expected 2 or 3"
        )),
        None => Err("Bundle is missing schema_version; expected 2 or 3".to_string()),
    }?;
    canonicalize_content(&mut bundle.content);
    Ok(bundle)
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
    let index = WorkspaceIndex {
        entities: content.entities.clone(),
        templates: content.trait_templates.clone(),
        assets: content.assets.clone(),
        folders: content.folders.clone(),
        storylet_ids: content.storylets.iter().map(|storylet| storylet.id.clone()).collect(),
    };

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

    for quality in &content.qualities {
        check_item_folder("quality", "qualities", &quality.name, quality.folder_id.as_deref(), &index, &mut issues);
    }
    for entity in &content.entities {
        check_item_folder("entity", "entities", &entity.name, entity.folder_id.as_deref(), &index, &mut issues);
    }
    for template in &content.trait_templates {
        check_item_folder("trait template", "trait_templates", &template.name, template.folder_id.as_deref(), &index, &mut issues);
    }
    for asset in &content.assets {
        check_item_folder("asset", "assets", &asset.name, asset.folder_id.as_deref(), &index, &mut issues);
    }
    for storylet in &content.storylets {
        check_item_folder("storylet", "storylets", &storylet.title, storylet.folder_id.as_deref(), &index, &mut issues);
    }

    let mut ids = BTreeSet::new();
    let mut object_ids = BTreeSet::new();
    let mut game_ids = BTreeSet::new();
    for entity in &content.entities {
        if !ids.insert(entity.id.clone()) {
            issues.push(format!("entity '{}': duplicate entity id", entity.name));
        }
        if entity.object_id.is_empty() {
            issues.push(format!("entity '{}': object_id is required", entity.name));
        } else if !object_ids.insert(entity.object_id.clone()) {
            issues.push(format!(
                "entity '{}': duplicate object_id '{}'",
                entity.name, entity.object_id
            ));
        }
        if entity.game_id.is_empty() {
            issues.push(format!("entity '{}': game_id is required", entity.name));
        } else if !game_ids.insert(entity.game_id.clone()) {
            issues.push(format!(
                "entity '{}': duplicate game ID '{}'",
                entity.name, entity.game_id
            ));
        }
    }

    for entity in &content.entities {
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
        for instance in &entity.traits {
            let Some(template) = index
                .templates
                .iter()
                .find(|template| template.id == instance.template_id)
            else {
                issues.push(format!(
                    "entity '{}': unknown trait template '{}'",
                    entity.name, instance.template_id
                ));
                continue;
            };
            for (key, value) in &instance.values {
                let Some(definition) = template
                    .properties
                    .iter()
                    .find(|definition| &definition.id == key)
                else {
                    continue;
                };
                if let Err(error) = validate_property_refs(
                    definition.property_type,
                    definition.is_array,
                    value,
                    &index,
                    "reference",
                ) {
                    issues.push(format!(
                        "entity '{}': trait '{}' {error}",
                        entity.name, template.key
                    ));
                }
            }
        }
    }

    for template in &content.trait_templates {
        if template.name.trim().is_empty() {
            issues.push("trait template: name is required".to_string());
        }
        // Reuse the authoring validator so an imported template cannot carry a
        // definition that a later save would reject. Ids are not generated here:
        // entity overrides key off them, so a missing id would silently detach.
        for definition in &template.properties {
            if definition.id.trim().is_empty() {
                issues.push(format!(
                    "trait template '{}': property '{}' is missing an id",
                    template.name, definition.key
                ));
            }
        }
        let mut candidate = template.clone();
        if let Err(error) = prepare_property_definitions(&mut candidate.properties, "trait template", &index)
        {
            issues.push(format!("trait template '{}': {error}", template.name));
        }
        for definition in &template.properties {
            if let Err(error) = validate_property_refs(
                definition.property_type,
                definition.is_array,
                &definition.default,
                &index,
                "reference",
            ) {
                issues.push(format!(
                    "trait template '{}': {error}",
                    template.key
                ));
            }
        }
    }

    let mut template_ids = BTreeSet::new();
    let mut template_keys = BTreeSet::new();
    for template in &content.trait_templates {
        if template.id.trim().is_empty() || !template_ids.insert(template.id.clone()) {
            issues.push(format!(
                "trait template '{}': missing or duplicate template id",
                template.name
            ));
        }
        if !is_identifier(&template.key) {
            issues.push(format!(
                "trait template '{}': key '{}' must be an identifier",
                template.name, template.key
            ));
        } else if !template_keys.insert(template.key.clone()) {
            issues.push(format!(
                "trait template '{}': duplicate key '{}'",
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

    #[test]
    fn default_game_id_uses_pascal_case_and_deduplicates() {
        let empty = BTreeSet::new();
        assert_eq!(
            default_game_id(EntityType::Character, "guard captain", &empty),
            "Chr_GuardCaptain"
        );
        assert_eq!(
            default_game_id(EntityType::Character, "  ", &empty),
            "Chr_Entity"
        );
        let mut used = BTreeSet::new();
        used.insert("Chr_Guard".to_string());
        used.insert("Chr_Guard_2".to_string());
        assert_eq!(default_game_id(EntityType::Character, "Guard", &used), "Chr_Guard_3");
    }

    #[test]
    fn runtime_attributes_fall_back_to_template_defaults_on_shape_mismatch() {
        let template: TraitTemplateRecord = serde_json::from_value(json!({
            "id": "tpl",
            "key": "Personality",
            "name": "Personality",
            "properties": [
                { "id": "bravery", "key": "Bravery", "name": "Bravery", "type": "int", "default": 1 }
            ]
        }))
        .unwrap();
        let mut entity: EntityRecord = serde_json::from_value(json!({
            "id": "guard",
            "name": "Guard",
            "type": "character"
        }))
        .unwrap();
        entity.traits.push(TraitInstance {
            template_id: "tpl".into(),
            values: BTreeMap::from([("bravery".to_string(), json!("not a number"))]),
        });
        let attributes = runtime_attributes(&entity, &[template]);
        assert_eq!(attributes["trait"]["Personality"]["Bravery"], json!(1));
        assert_eq!(attribute_path(&attributes, "trait.Personality.Bravery"), Some(json!(1)));
        assert_eq!(attribute_path(&attributes, "name"), None);
    }
}
