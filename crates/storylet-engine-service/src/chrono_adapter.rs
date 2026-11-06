//! Lossless compilation of authored Studio storylets into the dense ChronoFlow dispatcher.
//!
//! The adapter keeps existing project records as the authoring boundary. Runtime IDs are
//! assigned in stable storylet-ID order and bounded by the engine's fixed completion mask.

use std::collections::{BTreeMap, BTreeSet};
use serde_json::Value;

use storylet_engine::chrono_catalog::{StoryletCatalog, MAX_STORYLETS};
use storylet_engine::chrono_dispatch::{
    RoleRequirement, StoryletClusterType, StoryletDef, UnifiedDispatcher,
};
use storylet_engine::chrono_state::{
    AttributeValue, GameplayTagRegistry, GraphLayer, MAX_ENTITIES, MAX_PASSIONS,
    MicroPassionDef, PassionBounds, PassionDirection, PassionRegistry, PassionThresholdRule,
    RivalPassionLink, StarNpcDef, TagBitset, WorldEdge, WorldStateTable, WorldVertex,
};

use crate::contracts::{
    CharacterRecord, ChronoStoryletSettings, EntityRecord, EntityProperty, EntityTemplateRecord,
    LocationRecord, PropertyType,
    ScenarioRecord, StoryletRecord, WorldObjectLayer, WorldObjectRecord,
};

pub use storylet_engine::run::RunContent as CompiledChronoDispatch;

pub fn compile_chrono_dispatch(
    storylets: &[StoryletRecord],
    locations: &[LocationRecord],
    characters: &[CharacterRecord],
) -> Result<CompiledChronoDispatch, String> {
    compile_chrono_dispatch_with_world(
        storylets, locations, characters, &[], &[], &[], &BTreeMap::new(),
    )
}

pub fn compile_chrono_dispatch_for_storylet_validation(
    storylets: &[StoryletRecord], locations: &[LocationRecord], characters: &[CharacterRecord],
) -> Result<CompiledChronoDispatch, String> {
    compile_chrono_dispatch(storylets, locations, characters)
}

pub fn compile_chrono_dispatch_with_world(
    storylets: &[StoryletRecord], locations: &[LocationRecord], characters: &[CharacterRecord],
    world_objects: &[WorldObjectRecord], world_tags: &[String],
    entity_tag_seeds: &[(String, Vec<String>)], custom_entity_game_ids: &BTreeMap<String, String>,
) -> Result<CompiledChronoDispatch, String> {
    let mut ordered: Vec<&StoryletRecord> = storylets.iter().collect();
    ordered.sort_by_key(|s| s.id.as_str());
    if ordered.len() > MAX_STORYLETS { return Err(format!("ChronoFlow supports at most {MAX_STORYLETS} storylets")); }
    let mut catalog = StoryletCatalog::new();
    for s in &ordered { catalog.insert(&s.id, s.ending, 0, s.urgency_weight as f32).map_err(|e| e.to_string())?; }
    catalog.freeze(0).map_err(|e| e.to_string())?;
    let mut names = BTreeSet::from(["location.current".to_owned()]);
    let mut entity_game_ids = custom_entity_game_ids.clone(); entity_game_ids.insert("world".into(), "world".into());
    for l in locations {
        entity_game_ids.entry(l.id.clone()).or_insert_with(|| l.id.clone());
        names.insert(format!("location.{}", l.id));
        names.extend(l.tags.iter().cloned());
    }
    for c in characters {
        entity_game_ids.entry(c.id.clone()).or_insert_with(|| c.id.clone());
        names.insert(format!("character.{}", c.id));
        if !c.role.trim().is_empty() { names.insert(c.role.clone()); }
        names.extend(c.thresholds.iter().enumerate().map(|(i,t)| passion_threshold_tag(c,t,i)));
    }
    for o in world_objects {
        entity_game_ids.entry(o.id.clone()).or_insert_with(|| o.id.clone());
    }
    names.extend(world_tags.iter().map(|t|crate::chrono_graph_adapter::resolve_entity_tag("world",t,Some(&entity_game_ids))));
    names.extend(entity_tag_seeds.iter().flat_map(|(gid,tags)|tags.iter().map(move|t|crate::chrono_graph_adapter::resolve_entity_tag(gid,t,None))));
    for storylet in &ordered {
        names.extend(storylet.location_tags.iter().cloned());
        if let Some(settings) = &storylet.chrono {
            collect_settings_names(settings, &mut names);
        }
        if let Some(graph) = &storylet.graph {
            collect_graph_tag_names(graph, &mut names, &entity_game_ids)?;
        }
        if let Some(graph) = &storylet.precondition_graph {
            collect_precondition_graph_tag_names(graph, &mut names, &entity_game_ids);
        }
    }
    names.retain(|n|!n.trim().is_empty());
    let registry=GameplayTagRegistry::compile(names.into_iter()).map_err(|e|e.to_string())?;
    let (passion_registry,passion_indices)=compile_passion_registry(characters,locations,&registry)?;
    let mut defs=Vec::with_capacity(ordered.len());
    for(i,s)in ordered.iter().enumerate(){let set=s.chrono.as_ref();let mut d=StoryletDef{storylet_id:i as u16,is_ending:s.ending,display_priority:set.map_or(0,|x|x.display_priority),urgency_tag:set.map_or("Standard".into(),|x|x.urgency_color_tag.clone()),cluster_type:map_cluster_type(&s.cluster_type)?,payload_node_guid:i as u32,is_repeatable:s.repeatability!="once_ever"&&set.map_or(true,|x|x.is_repeatable),cooldown_ms:set.map_or(0,|x|i64::try_from(x.cooldown_ms).unwrap_or(i64::MAX)),cooldown_turns:if s.repeatability=="cooldown"{s.cooldown_turns as u64}else{0},required_roles:s.parameters.iter().filter_map(|p|crate::projection::parse_role_query(&p.query).map(|r|RoleRequirement{role_tag:r,location_tag:(p.location_policy.trim()!="any").then(||"location.current".into()),..Default::default()})).collect(),..StoryletDef::default()};if let Some(set)=set{add_tags(&registry,&mut d.required_tags,&set.required_tags,&s.id,"required_tags")?;add_any_tags(&registry,&mut d.required_any_tags,&set.required_any_tags,&s.id,"required_any_tags")?;add_any_tags(&registry,&mut d.forbidden_tags,&set.forbidden_tags,&s.id,"forbidden_tags")?;add_any_tags(&registry,&mut d.required_arc_phases,&set.required_arc_phases,&s.id,"required_arc_phases")?;for tag in &set.salience_bonus_tags{let id=registry.id(tag).ok_or_else(||format!("unknown salience tag {tag}"))?;registry.add_hierarchical(&mut d.salience_bonus_tags,id).map_err(|e|e.to_string())?;}}defs.push(d);}
    let mut dispatcher=UnifiedDispatcher::new(defs);dispatcher.resolve(&registry).map_err(|e|e.to_string())?;
    let storylet_titles = ordered.iter().enumerate().map(|(i, s)| (i as u16, s.title.clone())).collect();
    let mut storylet_location_tags = BTreeMap::new();
    for (i, storylet) in ordered.iter().enumerate() {
        let mut mask = TagBitset::new();
        for tag in &storylet.location_tags {
            if let Some(id) = registry.id(tag) { mask.set_bit(id).map_err(|error| error.to_string())?; }
        }
        storylet_location_tags.insert(i as u16, mask);
    }
    Ok(CompiledChronoDispatch{registry,dispatcher,catalog,passion_registry,passion_indices,entity_game_ids,participant_indices:BTreeMap::new(),implicit_participant:false,storylet_titles,storylet_location_tags,..Default::default()})
}

fn passion_threshold_tag(
    character: &CharacterRecord,
    threshold: &crate::contracts::MacroPassionThresholdRecord,
    index: usize,
) -> String {
    let threshold_id = if threshold.threshold_id.trim().is_empty() {
        index.to_string()
    } else {
        threshold.threshold_id.trim().to_owned()
    };
    format!("passion.threshold.{}.{}", character.id, threshold_id)
}

fn compile_passion_registry(
    characters: &[CharacterRecord],
    locations: &[LocationRecord],
    tags: &GameplayTagRegistry,
) -> Result<(PassionRegistry, BTreeMap<(String, String), u16>), String> {
    let mut ordered: Vec<&CharacterRecord> = characters.iter().collect();
    ordered.sort_by(|left, right| left.id.cmp(&right.id));
    let mut npc_indices = BTreeMap::new();
    for (index, character) in ordered.iter().enumerate() {
        let npc_id = locations
            .len()
            .checked_add(index)
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| (*value as usize) < MAX_ENTITIES)
            .ok_or_else(|| "ChronoFlow passion NPC index exceeds the entity limit".to_string())?;
        npc_indices.insert(character.id.clone(), npc_id);
    }

    let mut passion_indices = BTreeMap::new();
    let mut next_passion_id = 0usize;
    for character in &ordered {
        let mut local_ids = BTreeSet::new();
        for passion in &character.passion_pool {
            if passion.passion_id.trim().is_empty() || !local_ids.insert(passion.passion_id.clone())
            {
                return Err(format!(
                    "character {} has an empty or duplicate passion id",
                    character.id
                ));
            }
            if !passion.current_value.is_finite()
                || !(-100.0..=100.0).contains(&passion.current_value)
            {
                return Err(format!(
                    "character {} passion {} value must be finite and in [-100, 100]",
                    character.id, passion.passion_id
                ));
            }
            if !passion.weight.is_finite() {
                return Err(format!(
                    "character {} passion {} weight must be finite",
                    character.id, passion.passion_id
                ));
            }
            let passion_id = u16::try_from(next_passion_id).map_err(|_| {
                "ChronoFlow passion registry exceeds its fixed passion limit".to_string()
            })?;
            passion_indices.insert(
                (character.id.clone(), passion.passion_id.clone()),
                passion_id,
            );
            next_passion_id += 1;
        }
        if !character.passion_pool.is_empty() {
            if character.active_passions.is_empty() {
                return Err(format!(
                    "character {} has a passion pool but no active passions",
                    character.id
                ));
            }
            for active in &character.active_passions {
                if !local_ids.contains(active) {
                    return Err(format!(
                        "character {} activates unknown passion {active}",
                        character.id
                    ));
                }
            }
        }
        if !character.trust_inertia.is_finite()
            || !(0.0..=1.0).contains(&character.trust_inertia)
        {
            return Err(format!(
                "character {} trust inertia must be finite and in [0, 1]",
                character.id
            ));
        }
    }
    if next_passion_id > MAX_PASSIONS {
        return Err(format!(
            "ChronoFlow supports at most {MAX_PASSIONS} micro passions"
        ));
    }

    let mut stars = Vec::new();
    let mut thresholds = Vec::new();
    for character in ordered {
        if character.passion_pool.is_empty() {
            continue;
        }
        let npc_id = npc_indices[&character.id];
        let mut micro_passions = Vec::with_capacity(character.passion_pool.len());
        for passion in &character.passion_pool {
            micro_passions.push(MicroPassionDef {
                npc_id,
                passion_id: passion_indices[&(character.id.clone(), passion.passion_id.clone())],
                weight: passion.weight as f32,
                is_hidden: passion.is_hidden,
            });
            if !(passion.weight as f32).is_finite() {
                return Err(format!(
                    "character {} passion {} weight exceeds f32 range",
                    character.id, passion.passion_id
                ));
            }
        }
        let mut zero_sum_links = Vec::with_capacity(character.zero_sum_passion_links.len());
        for link in &character.zero_sum_passion_links {
            let target_id = *npc_indices.get(&link.target_character_id).ok_or_else(|| {
                format!(
                    "character {} links to unknown rival {}",
                    character.id, link.target_character_id
                )
            })?;
            let source_passion_id = *passion_indices
                .get(&(character.id.clone(), link.source_passion_id.clone()))
                .ok_or_else(|| {
                    format!(
                        "character {} links from unknown passion {}",
                        character.id, link.source_passion_id
                    )
                })?;
            let target_passion_id = *passion_indices
                .get(&(
                    link.target_character_id.clone(),
                    link.target_passion_id.clone(),
                ))
                .ok_or_else(|| {
                    format!(
                        "rival {} has no passion {}",
                        link.target_character_id, link.target_passion_id
                    )
                })?;
            if !link.weight.is_finite() || !(link.weight as f32).is_finite() {
                return Err(format!(
                    "character {} has non-finite zero-sum link weight",
                    character.id
                ));
            }
            zero_sum_links.push(RivalPassionLink {
                target_character_id: target_id,
                source_passion_id,
                target_passion_id,
                weight: link.weight as f32,
            });
        }
        stars.push(StarNpcDef {
            npc_id,
            trust_inertia: character.trust_inertia as f32,
            micro_passions,
            macro_bounds: PassionBounds {
                min: -100.0,
                max: 100.0,
            },
            zero_sum_links,
        });
        for (index, threshold) in character.thresholds.iter().enumerate() {
            let direction = match threshold.direction.trim().to_ascii_lowercase().as_str() {
                "positive" | "up" => PassionDirection::Up,
                "negative" | "down" => PassionDirection::Down,
                other => {
                    return Err(format!(
                        "character {} has invalid passion threshold direction {other}",
                        character.id
                    ));
                }
            };
            let name = passion_threshold_tag(character, threshold, index);
            let threshold_tag_bit = tags
                .id(&name)
                .ok_or_else(|| format!("unregistered passion threshold tag {name}"))?;
            thresholds.push(PassionThresholdRule {
                npc_id,
                threshold_tag_bit,
                trigger_value: threshold.trigger_value as f32,
                direction,
                linked_graph_asset_id: threshold.linked_graph_asset_id.clone(),
            });
        }
    }
    let registry = PassionRegistry::compile(stars, thresholds)
        .map_err(|error| format!("Invalid ChronoFlow passion registry: {error}"))?;
    Ok((registry, passion_indices))
}


fn collect_graph_tag_names(
    graph: &crate::narrative_graph::NarrativeGraph,
    names: &mut BTreeSet<String>,
    entity_game_ids: &BTreeMap<String, String>,
) -> Result<(), String> {
    let value = serde_json::to_value(graph)
        .map_err(|error| format!("cannot serialize storylet graph tags: {error}"))?;
    collect_graph_tag_value_names(&value, names, entity_game_ids);
    Ok(())
}

fn collect_graph_tag_value_names(
    value: &serde_json::Value,
    names: &mut BTreeSet<String>,
    entity_game_ids: &BTreeMap<String, String>,
) {
    match value {
        serde_json::Value::Object(fields) => {
            let is_entity_tag_node = fields
                .get("type")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|t| matches!(t, "add_entity_tag" | "remove_entity_tag" | "has_entity_tag"));

            if is_entity_tag_node {
                let target = fields.get("target_entity_id").and_then(serde_json::Value::as_str).unwrap_or("world");
                if let Some(tag) = fields.get("tag").and_then(serde_json::Value::as_str) {
                    let clean = tag.trim();
                    if !clean.is_empty() {
                        names.insert(crate::chrono_graph_adapter::resolve_entity_tag(target, clean, Some(entity_game_ids)));
                    }
                }
            }
            for (key, value) in fields {
                if is_entity_tag_node && key == "tag" {
                    continue;
                }
                let is_tag_field = matches!(
                    key.as_str(),
                    "fact_tag" | "tag"
                        | "tags"
                        | "emotion_tag"
                        | "action_tag"
                        | "emote"
                        | "arc_phase"
                        | "urgency_tag"
                        | "urgency_color_tag"
                        | "speaker_role"
                        | "actor_role"
                        | "target_role"
                        | "action_target"
                ) || key.ends_with("_tags");
                if is_tag_field {
                    match value {
                        serde_json::Value::String(tag) if !tag.trim().is_empty() => {
                            names.insert(tag.clone());
                        }
                        serde_json::Value::Array(tags) => {
                            names.extend(
                                tags.iter()
                                    .filter_map(serde_json::Value::as_str)
                                    .map(str::to_owned),
                            );
                        }
                        _ => {}
                    }
                }
                collect_graph_tag_value_names(value, names, entity_game_ids);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_graph_tag_value_names(item, names, entity_game_ids);
            }
        }
        _ => {}
    }
}
fn collect_precondition_graph_tag_names(
    graph: &crate::narrative_graph::NarrativeGraph,
    names: &mut BTreeSet<String>,
    entity_game_ids: &BTreeMap<String, String>,
) {
    for node in &graph.nodes {
        match &node.data {
            crate::narrative_graph::NarrativeNodeData::HasEntityTag { target_entity_id, tag, .. } => {
                if !tag.trim().is_empty() { names.insert(crate::chrono_graph_adapter::resolve_entity_tag(target_entity_id, tag, Some(entity_game_ids))); }
            }
            crate::narrative_graph::NarrativeNodeData::Knows { fact_tag, .. } => { if !fact_tag.trim().is_empty() { names.insert(fact_tag.clone()); } }
            _ => {}
        }
    }
}


fn collect_settings_names(settings: &ChronoStoryletSettings, names: &mut BTreeSet<String>) {
    names.extend(settings.required_tags.iter().cloned());
    names.extend(settings.required_any_tags.iter().cloned());
    names.extend(settings.forbidden_tags.iter().cloned());
    names.extend(settings.required_arc_phases.iter().cloned());
    names.extend(settings.salience_bonus_tags.iter().cloned());
}

fn add_tags(
    registry: &GameplayTagRegistry,
    target: &mut TagBitset,
    names: &[String],
    storylet_id: &str,
    field: &str,
) -> Result<(), String> {
    for name in names {
        let index = registry.id(name).ok_or_else(|| {
            format!("storylet {storylet_id} {field} references unknown tag {name}")
        })?;
        registry
            .add_hierarchical(target, index)
            .map_err(|error| format!("storylet {storylet_id} {field}: {error}"))?;
    }
    Ok(())
}

fn add_any_tags(
    registry: &GameplayTagRegistry,
    target: &mut TagBitset,
    names: &[String],
    storylet_id: &str,
    field: &str,
) -> Result<(), String> {
    let mut words = *target.words();
    for name in names {
        let index = registry.id(name).ok_or_else(|| {
            format!("storylet {storylet_id} {field} references unknown tag {name}")
        })?;
        let mask = registry.descendants_mask(index).ok_or_else(|| {
            format!("storylet {storylet_id} {field} has invalid tag index {index}")
        })?;
        for (word, descendants) in words.iter_mut().zip(mask.words()) {
            *word |= *descendants;
        }
    }
    *target = TagBitset::from_words(words);
    Ok(())
}

fn map_cluster_type(value: &str) -> Result<StoryletClusterType, String> {
    match value {
        "linear_progression" => Ok(StoryletClusterType::LinearProgression),
        "wheel_deck" => Ok(StoryletClusterType::WheelDeck),
        "aftermath_accelerator" => Ok(StoryletClusterType::AftermathAccelerator),
        "player_passage" => Ok(StoryletClusterType::PlayerPassage),
        unsupported => Err(format!("unsupported cluster type '{unsupported}'")),
    }
}

#[derive(Clone, Debug)]
pub struct ChronoWorldSeed {
    pub state: WorldStateTable,
    /// Stable dense row for each location, character, item, or fact entity.
    pub entity_indices: BTreeMap<String, u16>,
}

/// Build the fixed-capacity world rows from the current project and resolved start.
pub fn initialize_chrono_world(
    compiled: &CompiledChronoDispatch,
    storylets: &[StoryletRecord],
    world: &EntityRecord,
    entities: &[EntityTemplateRecord],
    locations: &[LocationRecord],
    characters: &[CharacterRecord],
    world_objects: &[WorldObjectRecord],
    world_tags: &[String],
    entity_tag_seeds: &[(String, Vec<String>)],
    scenario: Option<&ScenarioRecord>,
    start_location_id: &str,
) -> Result<ChronoWorldSeed, String> {
    let mut ordered_locations: Vec<&LocationRecord> = locations.iter().collect();
    ordered_locations.sort_by_key(|location| location.id.as_str());
    let mut ordered_characters: Vec<&CharacterRecord> = characters.iter().collect();
    ordered_characters.sort_by_key(|character| character.id.as_str());
    let mut ordered_world_objects: Vec<&WorldObjectRecord> = world_objects.iter().collect();
    ordered_world_objects.sort_by_key(|object| object.id.as_str());
    let entity_count = ordered_locations.len() + ordered_characters.len() + ordered_world_objects.len();
    if entity_count > MAX_ENTITIES {
        return Err(format!(
            "ChronoFlow supports at most {MAX_ENTITIES} location/character/tool entities; got {entity_count}"
        ));
    }
    let location_indices: BTreeMap<String, u16> = ordered_locations
        .iter()
        .enumerate()
        .map(|(index, location)| (location.id.clone(), index as u16))
        .collect();
    let mut entity_indices = BTreeMap::new();
    for (index, location) in ordered_locations.iter().enumerate() {
        entity_indices.insert(location.id.clone(), index as u16);
    }
    let mut character_indices = BTreeMap::new();
    for (offset, character) in ordered_characters.iter().enumerate() {
        let index = (ordered_locations.len() + offset) as u16;
        entity_indices.insert(character.id.clone(), index);
        character_indices.insert(character.id.clone(), index);
    }
    for (offset, object) in ordered_world_objects.iter().enumerate() {
        let index = (ordered_locations.len() + ordered_characters.len() + offset) as u16;
        entity_indices.insert(object.id.clone(), index);
    }
    let current_location_entity = location_indices
        .get(start_location_id)
        .copied()
        .ok_or_else(|| format!("ChronoFlow start location {start_location_id} does not exist"))?;
    let mut state = WorldStateTable::with_capacity(entity_count);
    state.current_location_entity = Some(current_location_entity);
    let mut participant_rows = Vec::new();
    for entity in entities {
        if let Some(trait_instance) = entity.traits.iter().find(|t| t.template_id == "system.participant") {
            let entity_index = *entity_indices.get(&entity.id).ok_or_else(|| format!("participant {} has no runtime entity", entity.id))?;
            let controller = trait_instance.values.get("controller").and_then(|v| v.as_str()).unwrap_or("human");
            let seat = match trait_instance.values.get("seat") { None | Some(Value::Null) => 0, Some(value) => value.as_u64().ok_or_else(|| format!("participant {} has invalid seat", entity.id))? };
            let seat = u8::try_from(seat).map_err(|_| format!("participant {} has invalid seat", entity.id))?;
            let control = match controller { "human" => storylet_engine::chrono_participants::Controller::Human { seat }, "rule" => storylet_engine::chrono_participants::Controller::Rule, _ => return Err(format!("participant {} has invalid controller", entity.id)) };
            let location = match trait_instance.values.get("start_location").and_then(|v| v.as_str()).filter(|v| !v.is_empty()) {
                Some(id) => Some(*location_indices.get(id).ok_or_else(|| format!("participant {} has unknown start location {id}", entity.id))?),
                None => None,
            };
            participant_rows.push((if controller == "human" { 0 } else { 1 }, seat, entity.id.as_str(), entity_index, control, location));
        }
    }
    participant_rows.sort_by(|a,b| (a.0,a.1,a.2).cmp(&(b.0,b.1,b.2)));
    if participant_rows.is_empty() { state.ensure_participants(); } else {
        for (_,_,id,index,control,location) in participant_rows { state.participants.push(id,index,control,location); }
        if state.participants.location[0].is_none() { state.participants.location[0] = Some(current_location_entity); }
        state.current_location_entity = state.participants.location[0];
    }


    // Seed the dramatic arc phase so arc-gated content is reachable without an
    // unrelated mutation writing the tag.
    if let Some(phase) = scenario
        .map(|scenario| scenario.current_arc_phase.trim())
        .filter(|phase| !phase.is_empty())
    {
        let phase_id = compiled.registry.id(phase).ok_or_else(|| {
            format!("scenario current_arc_phase '{phase}' is not a registered tag")
        })?;
        compiled
            .registry
            .add_hierarchical(&mut state.global_tags, phase_id)
            .map_err(|error| format!("scenario current_arc_phase: {error}"))?;
    }

    for tag in world_tags {
        let resolved = crate::chrono_graph_adapter::resolve_entity_tag("world", tag, None);
        if let Some(tag_id) = compiled.registry.id(&resolved) {
            let _ = compiled.registry.add_hierarchical(&mut state.global_tags, tag_id);
        }
    }

    let runtime_entities: BTreeMap<&str, &EntityTemplateRecord> = entities
        .iter()
        .map(|entity| (entity.id.as_str(), entity))
        .collect();
    let mut vertices = Vec::with_capacity(entity_count + storylets.len() + 1);
    let mut world_attributes = BTreeMap::new();
    world_attributes.insert("name".into(), AttributeValue::String(world.name.clone()));
    insert_entity_properties(&world.properties, &mut world_attributes)?;
    vertices.push(WorldVertex {
        vertex_id: "world".into(),
        layer: GraphLayer::Fact,
        entity_index: None,
        attributes: world_attributes,
    });
    let mut edges = Vec::new();
    for location in &ordered_locations {
        let entity_index = location_indices[&location.id];
        add_entity_tag(
            &compiled.registry,
            &mut state,
            entity_index,
            &format!("location.{}", location.id),
        )?;
        for tag in &location.tags {
            add_entity_tag(&compiled.registry, &mut state, entity_index, tag)?;
        }
        let mut attributes = BTreeMap::new();
        if let Some(value) = &location.environmental_params {
            insert_json_attributes(value, "environment", &mut attributes);
        }
        if let Some(value) = &location.devices {
            insert_json_attributes(value, "device", &mut attributes);
        }
        insert_json_attributes(&location.attributes, "", &mut attributes);
        if let Some(entity) = runtime_entities.get(location.id.as_str()) {
            insert_entity_properties(&entity.properties, &mut attributes)?;
            add_entity_tag(&compiled.registry, &mut state, entity_index, &entity.root_tag)?;
            for tag in &entity.tags {
                let resolved = tag.trim();
                if compiled.registry.id(resolved).is_some() {
                    add_entity_tag(&compiled.registry, &mut state, entity_index, resolved)?;
                }
            }
        }
        vertices.push(WorldVertex {
            vertex_id: location.id.clone(),
            layer: GraphLayer::Location,
            entity_index: Some(entity_index),
            attributes,
        });
        for target_id in location.connected_zones.as_deref().unwrap_or_default() {
            if !location_indices.contains_key(target_id) {
                return Err(format!(
                    "location {} connects to unknown location {target_id}",
                    location.id
                ));
            }
            edges.push(WorldEdge {
                edge_id: format!("connected:{}:{target_id}", location.id),
                source_id: location.id.clone(),
                target_id: target_id.clone(),
                relation_type: "connected".into(),
                weight: 1.0,
            });
        }
    }
    state.current_location_tag_mask = state.current_location_entity.map(|id| state.entity_tags[id as usize]).unwrap_or_default();
    state.global_tags.merge(&state.current_location_tag_mask);

    for character in &ordered_characters {
        let entity_index = character_indices[&character.id];
        add_entity_tag(
            &compiled.registry,
            &mut state,
            entity_index,
            &format!("character.{}", character.id),
        )?;
        if let Some(entity) = runtime_entities.get(character.id.as_str()) {
            add_entity_tag(&compiled.registry, &mut state, entity_index, &entity.root_tag)?;
            for tag in &entity.tags {
                let resolved = tag.trim();
                if compiled.registry.id(resolved).is_some() {
                    add_entity_tag(&compiled.registry, &mut state, entity_index, resolved)?;
                }
            }
        }
        if !character.role.trim().is_empty() {
            add_entity_tag(&compiled.registry, &mut state, entity_index, &character.role)?;
        }
        let mut attributes = BTreeMap::new();
        insert_json_attributes(&character.status_axes, "status", &mut attributes);
        insert_json_attributes(&character.needs, "need", &mut attributes);
        if !character.agenda.is_empty() {
            attributes.insert(
                "agenda".into(),
                AttributeValue::String(character.agenda.clone()),
            );
        }
        if !character.red_line.is_empty() {
            attributes.insert(
                "red_line".into(),
                AttributeValue::String(character.red_line.clone()),
            );
        }
        insert_json_attributes(&character.attributes, "", &mut attributes);
        if let Some(entity) = runtime_entities.get(character.id.as_str()) {
            insert_entity_properties(&entity.properties, &mut attributes)?;
        }
        vertices.push(WorldVertex {
            vertex_id: character.id.clone(),
            layer: GraphLayer::Character,
            entity_index: Some(entity_index),
            attributes,
        });
        let actor_location = scenario
            .and_then(|scenario| scenario.actor_locations.get(&character.id))
            .map(String::as_str)
            .filter(|location| !location.is_empty())
            .unwrap_or(&character.current_location);
        if !actor_location.is_empty() {
            if !location_indices.contains_key(actor_location) {
                return Err(format!(
                    "character {} references unknown current location {actor_location}",
                    character.id
                ));
            }
            if actor_location == start_location_id {
                add_entity_tag(
                    &compiled.registry,
                    &mut state,
                    entity_index,
                    "location.current",
                )?;
            }
            edges.push(WorldEdge {
                edge_id: format!("located:{}:{actor_location}", character.id),
                source_id: character.id.clone(),
                target_id: actor_location.to_owned(),
                relation_type: "located_at".into(),
                weight: 1.0,
            });
        }
    }
    for (gid, tags) in entity_tag_seeds {
        let entity_index = entity_indices.get(gid).copied();
        if let Some(entity_index) = entity_index {
            for tag in tags {
                let resolved = tag.trim().to_owned();
                if compiled.registry.id(&resolved).is_some() {
                    add_entity_tag(&compiled.registry, &mut state, entity_index, &resolved)?;
                }
            }
        }
    }
    seed_passion_state(
        compiled,
        &ordered_characters,
        &character_indices,
        &mut state,
    )?;
    for object in ordered_world_objects {
        let layer = match object.layer {
            WorldObjectLayer::Item => GraphLayer::Item,
            WorldObjectLayer::Fact => GraphLayer::Fact,
        };
        let mut attributes = BTreeMap::new();
        attributes.insert("name".into(), AttributeValue::String(object.name.clone()));
        insert_json_attributes(&object.attributes, "", &mut attributes);
        if let Some(entity) = runtime_entities.get(object.id.as_str()) {
            insert_entity_properties(&entity.properties, &mut attributes)?;
        }
        let entity_index = entity_indices[&object.id];
        vertices.push(WorldVertex {
            vertex_id: object.id.clone(),
            layer,
            entity_index: Some(entity_index),
            attributes,
        });
    }
    vertices.sort_by(|left, right| left.vertex_id.cmp(&right.vertex_id));
    edges.sort_by(|left, right| left.edge_id.cmp(&right.edge_id));
    state.vertices = vertices;
    for (entity_id, values) in scenario
        .map(|scenario| &scenario.initial_properties)
        .into_iter()
        .flatten()
    {
        let entity = runtime_entities.get(entity_id.as_str()).ok_or_else(|| {
            format!("scenario initial_properties references unknown entity {entity_id}")
        })?;
        let vertex_id = entity_id.clone();
        let vertex = state.vertices.iter_mut().find(|vertex| vertex.vertex_id == vertex_id)
            .ok_or_else(|| format!("scenario initial_properties entity {entity_id} has no runtime vertex"))?;
        for (key, value) in values {
            if key.trim().is_empty() {
                return Err(format!("scenario initial_properties for {entity_id} contains an empty property key"));
            }
            let property = entity.properties.iter().find(|property| property.key == *key)
                .ok_or_else(|| format!("scenario initial_properties for {entity_id} references undeclared property {key}"))?;
            if !crate::entities::value_shape_matches(property.property_type, property.is_array, value) {
                return Err(format!("scenario initial_properties for {entity_id}.{key} has invalid {:?} value", property.property_type));
            }
            vertex.attributes.insert(format!("prop.{key}"), attribute_from_property_value(value, property.property_type, property.is_array)?);
        }
    }
    for storylet in storylets {
        state.vertices.push(WorldVertex {
            vertex_id: format!("storylet:{}", storylet.id),
            layer: GraphLayer::Fact,
            entity_index: None,
            attributes: BTreeMap::new(),
        });
    }
    Ok(ChronoWorldSeed {
        state,
        entity_indices,
    })
}

fn seed_passion_state(
    compiled: &CompiledChronoDispatch,
    characters: &[&CharacterRecord],
    character_indices: &BTreeMap<String, u16>,
    state: &mut WorldStateTable,
) -> Result<(), String> {
    for character in characters {
        if character.passion_pool.is_empty() {
            continue;
        }
        let npc_id = character_indices[&character.id];
        let mut weighted_sum = 0.0f32;
        let mut total_weight = 0.0f32;
        for passion in &character.passion_pool {
            let passion_id = *compiled
                .passion_indices
                .get(&(character.id.clone(), passion.passion_id.clone()))
                .ok_or_else(|| {
                    format!(
                        "character {} passion {} was not compiled",
                        character.id, passion.passion_id
                    )
                })?;
            let value = passion.current_value as f32;
            let weight = passion.weight as f32;
            state.passions.micro[passion_id as usize] = value;
            let active = character
                .active_passions
                .iter()
                .any(|id| id == &passion.passion_id);
            state.passions.active[passion_id as usize] = active;
            if active {
                weighted_sum += value * weight;
                total_weight += weight;
            }
        }
        if !weighted_sum.is_finite() || !total_weight.is_finite() {
            return Err(format!(
                "character {} has overflowing active passion weights",
                character.id
            ));
        }
        if total_weight > 0.0 {
            let macro_value = weighted_sum / total_weight + character.trust_inertia as f32;
            if !macro_value.is_finite() || npc_id as usize >= state.passions.macro_values.len() {
                return Err(format!(
                    "character {} has invalid initial macro passion",
                    character.id
                ));
            }
            state.passions.macro_values[npc_id as usize] = macro_value.clamp(-100.0, 100.0);
        }
        for (index, threshold) in character.thresholds.iter().enumerate() {
            let direction = threshold.direction.trim().to_ascii_lowercase();
            let trigger = threshold.trigger_value as f32;
            let macro_value = state.passions.macro_values[npc_id as usize];
            let active = match direction.as_str() {
                "positive" | "up" => macro_value >= trigger,
                "negative" | "down" => macro_value <= trigger,
                _ => {
                    return Err(format!(
                        "character {} has invalid passion threshold direction {direction}",
                        character.id
                    ));
                }
            };
            let tag = passion_threshold_tag(character, threshold, index);
            let tag_id = compiled
                .registry
                .id(&tag)
                .ok_or_else(|| format!("unregistered passion threshold tag {tag}"))?;
            if active {
                state
                    .global_tags
                    .set_bit(tag_id)
                    .map_err(|error| error.to_string())?;
            }
        }
    }
    Ok(())
}

fn add_entity_tag(
    registry: &GameplayTagRegistry,
    state: &mut WorldStateTable,
    entity_index: u16,
    tag: &str,
) -> Result<(), String> {
    let index = registry
        .id(tag)
        .ok_or_else(|| format!("unregistered entity tag {tag}"))?;
    let entity = state
        .entity_tags
        .get_mut(entity_index as usize)
        .ok_or_else(|| {
            format!("entity index {entity_index} is outside the ChronoFlow state table")
        })?;
    registry
        .add_hierarchical(entity, index)
        .map_err(|error| error.to_string())
}


fn attribute_from_property_value(value: &serde_json::Value, property_type: PropertyType, is_array: bool) -> Result<AttributeValue, String> {
    if !crate::entities::value_shape_matches(property_type, is_array, value) {
        return Err(format!("invalid {:?} property value", property_type));
    }
    if is_array {
        return Ok(AttributeValue::Array(value.as_array().expect("validated array").iter()
            .map(|item| attribute_from_json(item, property_type)).collect::<Result<Vec<_>, _>>()?));
    }
    attribute_from_json(value, property_type)
}

fn attribute_from_json(value: &serde_json::Value, property_type: PropertyType) -> Result<AttributeValue, String> {
    if property_type == PropertyType::Json {
        return Err("ChronoFlow does not support JSON entity properties".into());
    }
    match property_type {
        PropertyType::Bool => value.as_bool().map(AttributeValue::Bool),
        PropertyType::Int => value.as_i64().map(AttributeValue::Int),
        PropertyType::Float => value.as_f64().map(AttributeValue::Float),
        PropertyType::Asset => if value.is_null() { Some(AttributeValue::Asset(String::new())) } else { value.get("asset_id").and_then(serde_json::Value::as_str).map(|v| AttributeValue::Asset(v.to_owned())) },
        PropertyType::Entity => if value.is_null() { Some(AttributeValue::EntityRef(String::new())) } else { value.as_str().map(|v| AttributeValue::EntityRef(v.to_owned())) },
        PropertyType::Storylet => if value.is_null() { Some(AttributeValue::String(String::new())) } else { value.as_str().map(|v| AttributeValue::String(v.to_owned())) },
        PropertyType::Tag => value.as_str().map(|v| AttributeValue::String(v.to_owned())),
        PropertyType::String => value.as_str().map(|v| AttributeValue::String(v.to_owned())),
        PropertyType::Json => return Err("ChronoFlow does not support JSON entity properties".into()),
    }.ok_or_else(|| format!("invalid {:?} property value", property_type))
}

fn insert_entity_properties(properties: &[EntityProperty], attributes: &mut BTreeMap<String, AttributeValue>) -> Result<(), String> {
    for property in properties {
        if !property.key.trim().is_empty() {
            let value = if property.value.is_null() {
                crate::entities::default_value(property.property_type, property.is_array)
            } else {
                property.value.clone()
            };
            attributes.insert(format!("prop.{}", property.key), attribute_from_property_value(&value, property.property_type, property.is_array)?);
        }
    }
    Ok(())
}
fn insert_json_attributes(
    value: &serde_json::Value,
    prefix: &str,
    attributes: &mut BTreeMap<String, AttributeValue>,
) {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                insert_json_attributes(value, &path, attributes);
            }
        }
        serde_json::Value::Array(items) => {
            for (index, value) in items.iter().enumerate() {
                let path = format!("{prefix}.{index}");
                insert_json_attributes(value, &path, attributes);
            }
        }
        serde_json::Value::Bool(value) => {
            attributes.insert(prefix.to_owned(), AttributeValue::Bool(*value));
        }
        serde_json::Value::Number(value) => {
            if let Some(integer) = value.as_i64() {
                attributes.insert(prefix.to_owned(), AttributeValue::Int(integer));
            } else if let Some(float) = value.as_f64().filter(|float| float.is_finite()) {
                attributes.insert(prefix.to_owned(), AttributeValue::Float(float));
            }
        }
        serde_json::Value::String(value) => {
            attributes.insert(prefix.to_owned(), AttributeValue::String(value.clone()));
        }
        serde_json::Value::Null => {}
    }
}

#[cfg(test)]
mod world_object_vertex_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn runtime_cluster_type_accepts_only_canonical_values() {
        assert_eq!(map_cluster_type("player_passage"), Ok(StoryletClusterType::PlayerPassage));
        assert_eq!(map_cluster_type("linear_progression"), Ok(StoryletClusterType::LinearProgression));
        assert_eq!(map_cluster_type("wheel_deck"), Ok(StoryletClusterType::WheelDeck));
        assert_eq!(map_cluster_type("aftermath_accelerator"), Ok(StoryletClusterType::AftermathAccelerator));
        assert!(map_cluster_type("player_controlled_passage").is_err());
    }


    #[test]
    fn world_item_and_fact_entities_seed_typed_properties() {
        let location: LocationRecord = serde_json::from_value(json!({
            "id": "square",
            "name": "Square",
            "description": "",
            "tags": []
        }))
        .expect("valid location fixture");
        let compiled = compile_chrono_dispatch(&[], std::slice::from_ref(&location), &[])
            .expect("empty storylet deck compiles");
        let item: WorldObjectRecord = serde_json::from_value(json!({
            "id": "lantern",
            "name": "Rusted Lantern",
            "layer": "item",
            "attributes": { "prop": { "Rusty": true, "Weight": 2 } }
        }))
        .expect("valid item fixture");
        let fact: WorldObjectRecord = serde_json::from_value(json!({
            "id": "rumor",
            "name": "Rumor",
            "layer": "fact",
            "attributes": { "prop": { "Verified": false } }
        }))
        .expect("valid fact fixture");
        let world: EntityRecord = serde_json::from_value(json!({
            "id": "world",
            "name": "World",
            "root_tag": "world",
            "tags": [],
            "properties": [{
                "id": "mood",
                "key": "mood",
                "name": "Mood",
                "type": "string",
                "value": "calm"
            }],
        }))
        .expect("valid world entity fixture");

        let seed = initialize_chrono_world(
            &compiled,
            &[],
            &world,
            &[],
            std::slice::from_ref(&location),
            &[],
            &[item, fact],
            &[],
            &[],
            None,
            "square",
        )
        .expect("world seeds with item and fact vertices");
        let world_vertex = seed.state.vertex("world").expect("world property vertex");
        assert_eq!(
            world_vertex.attributes.get("prop.mood"),
            Some(&AttributeValue::String("calm".into()))
        );

        let item_vertex = seed.state.vertex("lantern").expect("item vertex");
        assert_eq!(
            item_vertex.attributes.get("prop.Rusty"),
            Some(&AttributeValue::Bool(true))
        );
        assert_eq!(
            item_vertex.attributes.get("prop.Weight"),
            Some(&AttributeValue::Int(2))
        );
        assert_eq!(
            item_vertex.attributes.get("name"),
            Some(&AttributeValue::String("Rusted Lantern".into()))
        );
        assert_eq!(
            item_vertex.entity_index,
            seed.entity_indices.get("lantern").copied()
        );

        let fact_vertex = seed.state.vertex("rumor").expect("fact vertex");
        assert_eq!(
            fact_vertex.attributes.get("prop.Verified"),
            Some(&AttributeValue::Bool(false))
        );
        assert!(seed.entity_indices.contains_key("lantern"));
    }

    #[test]
    fn entity_property_types_survive_every_runtime_entity_seed_and_scenario_overrides() {
        let world: EntityRecord = serde_json::from_value(json!({
            "id": "world", "name": "World", "root_tag": "world", "tags": [],
            "properties": [{ "id": "mood", "key": "mood", "name": "Mood", "type": "float", "value": 3 }]
        })).expect("valid World fixture");
        let entities: Vec<EntityTemplateRecord> = serde_json::from_value(json!([
            {
                "id": "square", "name": "Square", "root_tag": "location", "tags": [],
                "properties": [{ "id": "visited", "key": "visited", "name": "Visited", "type": "bool", "value": false }]
            },
            {
                "id": "guard", "name": "Guard", "root_tag": "character", "tags": [],
                "properties": [
                    { "id": "score", "key": "score", "name": "Score", "type": "float", "value": 3 },
                    { "id": "owner", "key": "owner", "name": "Owner", "type": "entity", "value": "square" },
                    { "id": "empty_asset", "key": "empty_asset", "name": "Empty Asset", "type": "asset", "value": null },
                    { "id": "empty_entity", "key": "empty_entity", "name": "Empty Entity", "type": "entity", "value": null },
                    { "id": "unset_count", "key": "unset_count", "name": "Unset Count", "type": "int", "value": null }
                ]
            },
            {
                "id": "lantern", "name": "Lantern", "root_tag": "item", "tags": [],
                "properties": [
                    { "id": "heat", "key": "heat", "name": "Heat", "type": "float", "value": 5 },
                    { "id": "art", "key": "art", "name": "Art", "type": "asset", "value": { "asset_id": "asset-lantern", "version": "v1" } },
                    { "id": "owner", "key": "owner", "name": "Owner", "type": "entity", "value": "guard" },
                    { "id": "steps", "key": "steps", "name": "Steps", "type": "int", "is_array": true, "value": [1, 2] }
                ]
            },
            {
                "id": "rumor", "name": "Rumor", "root_tag": "fact", "tags": [],
                "properties": [{ "id": "verified", "key": "verified", "name": "Verified", "type": "bool", "value": true }]
            }
        ])).expect("valid Entity fixtures");
        let records = crate::entities::runtime_records_with_world(
            &world,
            &entities,
            &[],
        );
        let compiled = compile_chrono_dispatch_with_world(
            &[],
            &records.locations,
            &records.characters,
            &records.world_objects,
            &records.world_tags,
            &records.entity_tag_seeds,
            &records.entity_game_ids,
        )
        .expect("Chrono dispatch compiles");
        let seed = initialize_chrono_world(
            &compiled,
            &[],
            &world,
            &entities,
            &records.locations,
            &records.characters,
            &records.world_objects,
            &[],
            &records.entity_tag_seeds,
            None,
            "square",
        ).expect("typed entity properties seed");

        assert_eq!(
            seed.state.vertex("world").unwrap().attributes.get("prop.mood"),
            Some(&AttributeValue::Float(3.0))
        );
        assert_eq!(
            seed.state.vertex("square").unwrap().attributes.get("prop.visited"),
            Some(&AttributeValue::Bool(false))
        );
        let character = seed.state.vertex("guard").unwrap();
        assert_eq!(character.attributes.get("prop.score"), Some(&AttributeValue::Float(3.0)));
        assert_eq!(character.attributes.get("prop.owner"), Some(&AttributeValue::EntityRef("square".into())));
        assert_eq!(character.attributes.get("prop.empty_asset"), Some(&AttributeValue::Asset(String::new())));
        assert_eq!(character.attributes.get("prop.empty_entity"), Some(&AttributeValue::EntityRef(String::new())));
        assert_eq!(character.attributes.get("prop.unset_count"), Some(&AttributeValue::Int(0)));

        let item = seed.state.vertex("lantern").unwrap();
        assert_eq!(item.attributes.get("prop.heat"), Some(&AttributeValue::Float(5.0)));
        assert_eq!(item.attributes.get("prop.art"), Some(&AttributeValue::Asset("asset-lantern".into())));
        assert_eq!(item.attributes.get("prop.owner"), Some(&AttributeValue::EntityRef("guard".into())));
        assert_eq!(
            item.attributes.get("prop.steps"),
            Some(&AttributeValue::Array(vec![AttributeValue::Int(1), AttributeValue::Int(2)]))
        );
        assert_eq!(
            seed.state.vertex("rumor").unwrap().attributes.get("prop.verified"),
            Some(&AttributeValue::Bool(true))
        );

        let scenario: ScenarioRecord = serde_json::from_value(json!({
            "id": "typed-override",
            "name": "Typed Override",
            "seed": 1,
            "start_location_policy": { "mode": "fixed", "location_id": "square" },
            "initial_properties": { "guard": { "score": 9 } }
        })).expect("valid scenario fixture");
        let overridden = initialize_chrono_world(
            &compiled,
            &[],
            &world,
            &entities,
            &records.locations,
            &records.characters,
            &records.world_objects,
            &[],
            &records.entity_tag_seeds,
            Some(&scenario),
            "square",
        ).expect("typed scenario property override");
        assert_eq!(
            overridden.state.vertex("guard").unwrap().attributes.get("prop.score"),
            Some(&AttributeValue::Float(9.0))
        );
    }
}

#[cfg(test)]
mod participant_seed_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn participant_trait_compiles_rows_in_seat_order() {
        let world: EntityRecord = serde_json::from_value(json!({"id":"world","name":"World","root_tag":"world"})).unwrap();
        let entities: Vec<EntityTemplateRecord> = serde_json::from_value(json!([
            {"id":"square","name":"Square","root_tag":"location"},
            {"id":"player","name":"A","root_tag":"character","traits":[{"template_id":"system.participant","values":{"controller":"human","seat":1,"start_location":"square"}}]},
            {"id":"b","name":"B","root_tag":"character","traits":[{"template_id":"system.participant","values":{"controller":"human","seat":0}}]}
        ])).unwrap();
        let records = crate::entities::runtime_records_with_world(&world, &entities, &[]);
        let compiled = compile_chrono_dispatch_with_world(&[], &records.locations, &records.characters, &records.world_objects, &records.world_tags, &records.entity_tag_seeds, &records.entity_game_ids).unwrap();
        let seed = initialize_chrono_world(&compiled, &[], &world, &entities, &records.locations, &records.characters, &records.world_objects, &[], &records.entity_tag_seeds, None, "square").unwrap();
        assert_eq!(seed.state.participants.keys, vec!["b", "player"]);
        assert_eq!(seed.state.active_location(0), Some(seed.entity_indices["square"]));
        assert_ne!(seed.state.participants.entity[1], u16::MAX);
    }
}

/// Compiles canon events with a story time into the chronicle, sorted by
/// `(story_time, id)`. Fact tags must be registered; a location must be a
/// known entity.
pub fn compile_chronicle(
    events: &[crate::contracts::EventRecord],
    registry: &storylet_engine::chrono_state::GameplayTagRegistry,
    entity_indices: &BTreeMap<String, u16>,
) -> Result<storylet_engine::chrono_chronicle::ChronicleTable, String> {
    use storylet_engine::chrono_chronicle::{ChronicleEntry, ChronicleTable};
    let mut entries = Vec::new();
    for event in events.iter().filter(|event| event.is_chronicle_entry()) {
        let facts = event
            .facts
            .iter()
            .map(|tag| registry.id(tag).ok_or_else(|| format!("Event \"{}\": fact tag \"{tag}\" is not registered", event.id)))
            .collect::<Result<Vec<_>, _>>()?;
        let location = event
            .location_id
            .as_deref()
            .map(|id| entity_indices.get(id).copied().ok_or_else(|| format!("Event \"{}\": unknown location \"{id}\"", event.id)))
            .transpose()?;
        entries.push(ChronicleEntry {
            key: event.id.clone(),
            story_time: event.story_time.unwrap_or_default(),
            label: event.story_time_label.clone().filter(|label| !label.is_empty()).unwrap_or_else(|| event.title.clone()),
            location,
            facts,
            text: event.text.clone().unwrap_or_default(),
        });
    }
    Ok(ChronicleTable::new(entries))
}

#[cfg(test)]
mod chronicle_compile_tests {
    use super::*;
    use serde_json::json;

    fn event(id: &str, kind: &str, time: Option<i64>, facts: &[&str]) -> crate::contracts::EventRecord {
        serde_json::from_value(json!({
            "id": id, "title": id, "description": "", "event_type": kind, "trigger": null, "effects": null,
            "story_time": time, "facts": facts,
        })).unwrap()
    }

    #[test]
    fn canon_events_compile_sorted_by_story_time() {
        let registry = storylet_engine::chrono_state::GameplayTagRegistry::empty();
        let events = vec![
            event("late", "canon", Some(30), &[]),
            event("early", "canon", Some(10), &[]),
            event("world-event", "world", Some(5), &[]),
            event("untimed", "canon", None, &[]),
        ];
        let table = compile_chronicle(&events, &registry, &BTreeMap::new()).unwrap();
        let keys: Vec<_> = table.entries.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, ["early", "late"]);
    }

    #[test]
    fn unregistered_fact_tag_is_an_error() {
        let registry = storylet_engine::chrono_state::GameplayTagRegistry::empty();
        let events = vec![event("e", "canon", Some(1), &["fact.nope"])];
        let error = compile_chronicle(&events, &registry, &BTreeMap::new()).unwrap_err();
        assert!(error.contains("fact.nope"), "{error}");
    }
}

/// Compiles scheduled connections and the clock from entity traits.
/// `connected_zones` become always-open connections.
pub fn compile_space(
    world: &crate::contracts::EntityRecord,
    entities: &[EntityTemplateRecord],
    locations: &[crate::contracts::LocationRecord],
    entity_indices: &BTreeMap<String, u16>,
    registry: &storylet_engine::chrono_state::GameplayTagRegistry,
) -> Result<(Vec<storylet_engine::chrono_space::Connection>, Option<storylet_engine::chrono_space::Clock>), String> {
    use storylet_engine::chrono_space::{Clock, Connection, OnClose};
    let clock = match world.traits.iter().find(|t| t.template_id == "system.clock") {
        None => None,
        Some(instance) => {
            let labels: Vec<String> = instance
                .values
                .get("slot_labels")
                .and_then(Value::as_array)
                .map(|items| items.iter().filter_map(Value::as_str).map(str::to_owned).collect())
                .unwrap_or_default();
            if labels.is_empty() {
                return Err("Clock: slot_labels must not be empty".into());
            }
            let per_slot = match instance.values.get("actions_per_slot") {
                None | Some(Value::Null) => 2,
                Some(value) => value.as_u64().filter(|n| (1..=255).contains(n)).ok_or("Clock: actions_per_slot must be between 1 and 255")?,
            };
            Some(Clock { slot_labels: labels, actions_per_slot: per_slot as u8 })
        }
    };
    let slot_count = clock.as_ref().map_or(0, |c| c.slot_labels.len());
    let index_of = |id: &str, from: &str| entity_indices.get(id).copied().ok_or_else(|| format!("Connection \"{from}\": unknown location \"{id}\""));
    let mut connections = Vec::new();
    for location in locations {
        let from = index_of(&location.id, &location.id)?;
        for target in location.connected_zones.as_deref().unwrap_or_default() {
            connections.push(Connection { from, to: index_of(target, &location.id)?, open_slots: None, requires_fact: None, on_close: None });
        }
        let Some(entity) = entities.iter().find(|e| e.id == location.id) else { continue };
        for instance in entity.traits.iter().filter(|t| t.template_id == "system.connection") {
            let text = |key: &str| instance.values.get(key).and_then(Value::as_str).filter(|v| !v.is_empty());
            let to_id = text("to").ok_or_else(|| format!("Connection \"{}\": a connection must set to", location.id))?;
            let to = index_of(to_id, &location.id)?;
            let label = format!("Connection \"{}\" → \"{to_id}\"", location.id);
            let mut mask = 0u64;
            let slots: Vec<u64> = instance.values.get("open_slots").and_then(Value::as_array).map(|items| items.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
            for slot in &slots {
                if *slot as usize >= slot_count || *slot >= 64 {
                    return Err(format!("Clock: open_slots index {slot} is outside slot_labels (0..{slot_count})"));
                }
                mask |= 1u64 << slot;
            }
            let scheduled = !slots.is_empty();
            let on_close = match text("on_close") {
                None if scheduled => return Err(format!("{label}: a scheduled connection must set on_close (return or strand)")),
                None => None,
                Some("strand") => Some(OnClose::Strand),
                Some("return") => {
                    let back = text("return_to").ok_or_else(|| format!("{label}: on_close \"return\" needs return_to"))?;
                    Some(OnClose::Return { to: index_of(back, &location.id)? })
                }
                Some(other) => return Err(format!("{label}: on_close \"{other}\" must be return or strand")),
            };
            let requires_fact = match text("requires_fact") {
                None => None,
                Some(tag) => Some(registry.id(tag).ok_or_else(|| format!("{label}: fact tag \"{tag}\" is not registered"))?),
            };
            connections.push(Connection { from, to, open_slots: scheduled.then_some(mask), requires_fact, on_close });
        }
    }
    Ok((connections, clock))
}

#[cfg(test)]
mod space_compile_tests {
    use super::*;
    use serde_json::json;

    fn location(id: &str, traits: Value, zones: Value) -> Value {
        let mut all = traits.as_array().cloned().unwrap_or_default();
        if zones.as_array().is_some_and(|z| !z.is_empty()) {
            all.push(json!({"template_id": "system.location_settings", "values": {"connected_zones": zones}}));
        }
        json!({"id": id, "name": id, "root_tag": "location", "traits": all})
    }

    fn compile(entities: Value, world_traits: Value) -> Result<(Vec<storylet_engine::chrono_space::Connection>, Option<storylet_engine::chrono_space::Clock>), String> {
        let world: EntityRecord = serde_json::from_value(json!({"id": "world", "name": "World", "root_tag": "world", "traits": world_traits})).unwrap();
        let entities: Vec<EntityTemplateRecord> = serde_json::from_value(entities).unwrap();
        let templates = crate::store::Store::open_in_memory().unwrap().list_trait_templates().unwrap();
        let records = crate::entities::runtime_records_with_world(&world, &entities, &templates);
        let indices: BTreeMap<String, u16> = records.locations.iter().enumerate().map(|(i, l)| (l.id.clone(), i as u16)).collect();
        compile_space(&world, &entities, &records.locations, &indices, &storylet_engine::chrono_state::GameplayTagRegistry::empty())
    }

    fn connection_trait(values: Value) -> Value {
        json!([{"template_id": "system.connection", "values": values}])
    }
    fn clock_trait() -> Value {
        json!([{"template_id": "system.clock", "values": {"slot_labels": ["a", "b", "c"], "actions_per_slot": 3}}])
    }

    #[test]
    fn scheduled_connection_without_on_close_is_rejected() {
        let entities = json!([location("lighthouse", json!([]), json!([])), location("rock", connection_trait(json!({"to": "lighthouse", "open_slots": [1], "on_close": ""})), json!([]))]);
        let error = compile(entities, clock_trait()).unwrap_err();
        assert_eq!(error, "Connection \"rock\" → \"lighthouse\": a scheduled connection must set on_close (return or strand)");
    }

    #[test]
    fn return_without_return_to_is_rejected() {
        let entities = json!([location("lighthouse", json!([]), json!([])), location("rock", connection_trait(json!({"to": "lighthouse", "open_slots": [1], "on_close": "return"})), json!([]))]);
        let error = compile(entities, clock_trait()).unwrap_err();
        assert_eq!(error, "Connection \"rock\" → \"lighthouse\": on_close \"return\" needs return_to");
    }

    #[test]
    fn open_slot_out_of_range_is_rejected() {
        let entities = json!([location("lighthouse", json!([]), json!([])), location("rock", connection_trait(json!({"to": "lighthouse", "open_slots": [1, 5], "on_close": "strand"})), json!([]))]);
        let error = compile(entities, clock_trait()).unwrap_err();
        assert_eq!(error, "Clock: open_slots index 5 is outside slot_labels (0..3)");
    }

    #[test]
    fn connected_zones_compile_as_always_open() {
        let entities = json!([location("lighthouse", json!([]), json!(["harbor"])), location("harbor", json!([]), json!([]))]);
        let (connections, clock) = compile(entities, json!([])).unwrap();
        assert!(clock.is_none());
        assert_eq!(connections.len(), 1);
        let c = &connections[0];
        // Entity indices are alphabetical: harbor = 0, lighthouse = 1.
        assert_eq!((c.from, c.to, c.open_slots, c.requires_fact, c.on_close), (1, 0, None, None, None));
    }

    #[test]
    fn valid_scheduled_connection_and_clock_compile() {
        let entities = json!([
            location("lighthouse", json!([]), json!([])),
            location("rock", connection_trait(json!({"to": "lighthouse", "open_slots": [0, 2], "on_close": "return", "return_to": "lighthouse"})), json!([])),
        ]);
        let (connections, clock) = compile(entities, clock_trait()).unwrap();
        let clock = clock.unwrap();
        assert_eq!((clock.slot_labels.len(), clock.actions_per_slot), (3, 3));
        let c = &connections[0];
        assert_eq!((c.from, c.to, c.open_slots), (1, 0, Some(0b101)));
        assert_eq!(c.on_close, Some(storylet_engine::chrono_space::OnClose::Return { to: 0 }));
    }
}

/// Resolves each storylet's authored `played_by` entity ids to participant
/// indices. Must run after `participant_indices` is known.
pub fn bind_played_by(compiled: &mut storylet_engine::run::RunContent, storylets: &[crate::contracts::StoryletRecord]) -> Result<(), String> {
    for storylet in storylets.iter().filter(|s| !s.played_by.is_empty()) {
        let Some(index) = compiled.storylet_index(&storylet.id) else { continue };
        let mut played_by = Vec::new();
        for id in &storylet.played_by {
            let participant = compiled
                .participant_indices
                .get(id)
                .copied()
                .ok_or_else(|| format!("Storylet \"{}\": played_by \"{id}\" is not a participant", storylet.id))?;
            played_by.push(participant);
        }
        if let Some(def) = compiled.dispatcher.storylets.iter_mut().find(|d| d.storylet_id == index) {
            def.played_by = played_by;
        }
    }
    Ok(())
}

#[cfg(test)]
mod played_by_tests {
    use super::*;
    use serde_json::json;

    fn storylets(played_by: Value) -> Vec<crate::contracts::StoryletRecord> {
        serde_json::from_value(json!([{"id": "gossip", "title": "Gossip", "content": {"text_template": "x"}, "created_at": 1, "played_by": played_by}])).unwrap()
    }

    #[test]
    fn played_by_binds_participant_indices() {
        let records = storylets(json!(["rosa"]));
        let mut compiled = compile_chrono_dispatch_with_world(&records, &[], &[], &[], &[], &[], &BTreeMap::new()).unwrap();
        compiled.participant_indices = BTreeMap::from([("rosa".to_string(), 1)]);
        bind_played_by(&mut compiled, &records).unwrap();
        let index = compiled.storylet_index("gossip").unwrap();
        assert_eq!(compiled.dispatcher.storylet(index).unwrap().played_by, vec![1]);
    }

    #[test]
    fn unknown_played_by_is_rejected() {
        let records = storylets(json!(["nobody"]));
        let mut compiled = compile_chrono_dispatch_with_world(&records, &[], &[], &[], &[], &[], &BTreeMap::new()).unwrap();
        let error = bind_played_by(&mut compiled, &records).unwrap_err();
        assert_eq!(error, "Storylet \"gossip\": played_by \"nobody\" is not a participant");
    }

    #[test]
    fn empty_played_by_means_any_human() {
        let records = storylets(json!([]));
        let mut compiled = compile_chrono_dispatch_with_world(&records, &[], &[], &[], &[], &[], &BTreeMap::new()).unwrap();
        bind_played_by(&mut compiled, &records).unwrap();
        let index = compiled.storylet_index("gossip").unwrap();
        assert!(compiled.dispatcher.storylet(index).unwrap().played_by.is_empty());
    }
}
