use std::collections::BTreeMap;

use storylet_engine::chrono_runtime::{FlowGraphInstance, RuntimeContext};
use storylet_engine::chrono_space::ClockState;
use storylet_engine::run::RunContent;

use crate::chrono_adapter::{
    bind_played_by, compile_chrono_dispatch_with_world, compile_chronicle, compile_space,
    initialize_chrono_world,
};
use crate::chrono_graph_adapter::{
    bind_precondition_graphs, bind_storylet_start_nodes, compile_narrative_graphs,
    validate_entity_property_graph,
};
use crate::contracts::{
    CharacterRecord, EntityRecord, EntityTemplateRecord, EventRecord, LocationRecord,
    ScenarioRecord, StoryletRecord, WorldObjectRecord,
};
use crate::entities::runtime_records_with_world;
use crate::inventory::{project_inventory_entities, validate_inventory_graph};
use crate::narrative_graph::NarrativeGraph;
use crate::scenario::resolve_scenario_start;
use crate::store::Store;
use crate::tree_pointer::{project_tree_entities, validate_pointer_graph};

struct WorkspaceContent {
    world: EntityRecord,
    entities: Vec<EntityTemplateRecord>,
    locations: Vec<LocationRecord>,
    characters: Vec<CharacterRecord>,
    world_objects: Vec<WorldObjectRecord>,
    world_tags: Vec<String>,
    entity_tag_seeds: Vec<(String, Vec<String>)>,
    storylets: Vec<StoryletRecord>,
    scenarios: Vec<ScenarioRecord>,
    events: Vec<EventRecord>,
    entity_game_ids: BTreeMap<String, String>,
}

fn read_content(store: &Store) -> Result<WorkspaceContent, String> {
    let instances = store.list_entities().map_err(|e| e.to_string())?;
    let world = instances.iter().find(|entity| entity.id == "world").cloned()
        .ok_or_else(|| "missing intrinsic world entity".to_string())?;
    let mut entities = store.list_entity_templates().map_err(|e| e.to_string())?;
    let templates = store.list_trait_templates().map_err(|e| e.to_string())?;
    project_inventory_entities(&mut entities, &instances, &templates)?;
    project_tree_entities(&mut entities, &instances, &templates)?;
    let runtime = runtime_records_with_world(&world, &entities, &templates);
    let storylets = store.list_storylets().map_err(|e| e.to_string())?;
    for storylet in &storylets {
        let graph = storylet.graph.clone().unwrap_or_else(|| NarrativeGraph::from_storylet(storylet));
        validate_entity_property_graph(&graph, &entities)?;
        validate_pointer_graph(&graph, &entities, &templates)?;
        validate_inventory_graph(&graph, &entities)?;
        if let Some(precondition) = storylet.precondition_graph.as_ref() {
            validate_entity_property_graph(precondition, &entities)?;
            validate_pointer_graph(precondition, &entities, &templates)?;
            validate_inventory_graph(precondition, &entities)?;
        }
    }
    Ok(WorkspaceContent {
        entities,
        world,
        locations: runtime.locations,
        characters: runtime.characters,
        world_objects: runtime.world_objects,
        world_tags: runtime.world_tags,
        entity_tag_seeds: runtime.entity_tag_seeds,
        storylets,
        scenarios: store.list_scenarios().map_err(|e| e.to_string())?,
        events: store.list_events().map_err(|e| e.to_string())?,
        entity_game_ids: runtime.entity_game_ids,
    })
}

pub(crate) fn compile_run_content(store: &Store, scenario_id: Option<&str>, expected_revision: Option<u64>, start_storylet_id: Option<&str>) -> Result<(RunContent, u64), String> {
    let tx = store.conn.unchecked_transaction().map_err(|e|e.to_string())?;
    let meta = store.get_project_metadata().map_err(|e| e.to_string())?;
    if expected_revision.is_some_and(|expected| expected != meta.revision) { return Err("stale revision".into()); }
    let content = read_content(store)?;
    let scenario = match scenario_id {
        Some(id) => content.scenarios.iter().find(|s| s.id == id).ok_or_else(|| format!("Missing scenario '{id}'"))?,
        None => content.scenarios.first().ok_or_else(|| "Missing scenario: the project has no scenarios".to_string())?,
    };
    let resolved = resolve_scenario_start(scenario, &content.locations, &content.characters);
    if !resolved.issues.is_empty() { return Err(resolved.issues.join("; ")); }
    let mut compiled = compile_chrono_dispatch_with_world(&content.storylets, &content.locations, &content.characters, &content.world_objects, &content.world_tags, &content.entity_tag_seeds, &content.entity_game_ids)?;
    let mut seed = initialize_chrono_world(&compiled, &content.storylets, &content.world, &content.entities, &content.locations, &content.characters, &content.world_objects, &content.world_tags, &content.entity_tag_seeds, Some(scenario), &resolved.player_start_location)?;
    compiled.participant_indices = seed.state.participants.keys.iter().enumerate().map(|(i,k)| (k.clone(), i as u16)).collect();
    compiled.implicit_participant = seed.state.participants.entity.as_slice() == [u16::MAX];
    (compiled.connections, compiled.clock) = compile_space(&content.world, &content.entities, &content.locations, &seed.entity_indices, &compiled.registry)?;
    if let Some(clock) = &compiled.clock { seed.state.clock = Some(ClockState::new(clock, seed.state.participants.len())); }
    compiled.chronicle = compile_chronicle(&content.events, &compiled.registry, &seed.entity_indices)?;
    bind_played_by(&mut compiled, &content.storylets)?;
    let mut graphs = compile_narrative_graphs(&meta.id, &content.storylets, &compiled, &seed.entity_indices)?;
    bind_precondition_graphs(&mut compiled, &content.storylets, &seed.entity_indices)?;
    bind_storylet_start_nodes(&mut compiled, &graphs)?;
    let mut evaluation_world=seed.state.clone();
    let ctx=RuntimeContext::with_registries(&mut evaluation_world,&compiled.dispatcher,&compiled.registry,&compiled.passion_registry);
    let current_tags=seed.state.active_location(0).and_then(|i| seed.state.entity_tags.get(i as usize));
    let mut candidates=Vec::new();
    for storylet in &content.storylets {
        let Some(index)=compiled.storylet_index(&storylet.id) else {continue};
        let Some(def)=compiled.dispatcher.storylet(index) else {continue};
        if !graphs.storylet_start_nodes.contains_key(&storylet.id) || !compiled.dispatcher.is_eligible(def,&seed.state) {continue;}
        if let Some(graph)=&def.precondition_graph {if !FlowGraphInstance::evaluate_precondition_graph_for(graph,&ctx,0).map_err(|e|e.to_string())? {continue;}}
        let mask=compiled.storylet_location_tags.get(&index);
        let matched=mask.is_some_and(|mask|!mask.is_empty() && current_tags.is_some_and(|tags|tags.intersects(mask)));
        if mask.is_none_or(|mask|mask.is_empty()) || matched {candidates.push((!matched,storylet.id.as_str()));}
    }
    candidates.sort();
    let start=match start_storylet_id {
        Some(id) => content.storylets.iter().map(|s|s.id.as_str()).find(|s|*s==id).filter(|id|graphs.storylet_start_nodes.contains_key(*id)).ok_or_else(||format!("unknown storylet \"{id}\""))?,
        None => candidates.first().map(|(_,id)|*id).ok_or("No eligible storylet at the starting location")?,
    };
    graphs.asset.start_node_guid = graphs.storylet_start_nodes[start];
    compiled.start_storylet = compiled.storylet_index(start).ok_or("Missing starting storylet")?;
    compiled.entity_names = content
        .locations
        .iter()
        .map(|e| (&e.id, &e.name))
        .chain(content.characters.iter().map(|e| (&e.id, &e.name)))
        .chain(content.world_objects.iter().map(|e| (&e.id, &e.name)))
        .filter_map(|(id, name)| seed.entity_indices.get(id).map(|index| (*index, name.clone())))
        .collect();
    compiled.initial_world = seed.state;
    compiled.content_revision = meta.revision;
    compiled.root_asset = graphs.asset;
    compiled.mission_assets = graphs.mission_assets;
    tx.commit().map_err(|e|e.to_string())?;
    Ok((compiled, resolved.seed))
}

#[cfg(test)]
mod run_compile_revision_tests {
    use super::compile_run_content;
    use crate::store::Store;

    #[test]
    fn edit_between_precondition_and_compile_is_rejected_before_reading_content() {
        let path=std::env::temp_dir().join(format!("run-compile-race-{}.db",uuid::Uuid::new_v4()));
        let reader=Store::open(&path).unwrap();
        let expected=reader.get_project_metadata().unwrap().revision;
        assert_eq!(reader.get_project_metadata().unwrap().revision,expected);
        let writer=Store::open(&path).unwrap();
        writer.save_content("storylets","new","{}",1,None).unwrap();
        let error=compile_run_content(&reader,None,Some(expected),None).unwrap_err();
        assert_eq!(error,"stale revision");
    }
}
