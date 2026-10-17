//! Single-run migration from legacy SQLite databases into the unified Store.

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

use crate::contracts::{
    ActorPopulationPolicy, CharacterRecord, EventRecord, LocationRecord, NpcBehaviorRecord,
    ProjectMetadata, QualityRecord, ScenarioRecord, StartLocationPolicy, StoryletChoiceRecord,
    StoryletContentRecord, StoryletParameterRecord, StoryletRecord,
};
use crate::store::Store;

fn has_column(conn: &Connection, table: &str, column: &str) -> Result<bool, rusqlite::Error> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for row in rows {
        if row? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn legacy_storylet_query(conn: &Connection) -> Result<String, rusqlite::Error> {
    let location_column = if has_column(conn, "storylets", "location")? {
        "location"
    } else {
        "NULL"
    };
    let location_tags_column = if has_column(conn, "storylets", "location_tags")? {
        "location_tags"
    } else {
        "NULL"
    };
    Ok(format!(
        "SELECT id, title, {location_column}, {location_tags_column}, group_tag, cluster_type, urgency_weight, repeatability, cooldown_turns, parameters, preconditions, content, choices, effects, created_at, revision, approved_revision, ending FROM storylets"
    ))
}

fn parse_tags(value: Option<String>) -> Vec<String> {
    value
        .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
        .unwrap_or_default()
}

fn storylet_location_tags(
    tags: Vec<String>,
    location_id: Option<String>,
    legacy_locations: &BTreeMap<String, Vec<String>>,
) -> Vec<String> {
    if tags.is_empty() {
        if let Some(location_id) = location_id {
            return legacy_locations
                .get(&location_id)
                .cloned()
                .unwrap_or_else(|| vec![location_id]);
        }
    }
    tags
}

pub fn maybe_import_legacy_database(
    store: &mut Store,
    legacy_studio_path: impl AsRef<Path>,
    legacy_engine_path: impl AsRef<Path>,
) -> Result<bool, String> {
    // Check if migration has already been executed
    let already_imported: Option<String> = store
        .conn
        .query_row(
            "SELECT value FROM store_metadata WHERE key = 'legacy_import_v1_done'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if already_imported.is_some() {
        return Ok(false);
    }

    let studio_path = legacy_studio_path.as_ref();
    if !studio_path.exists() {
        // No legacy studio db to import
        store
            .conn
            .execute(
                "INSERT INTO store_metadata (key, value) VALUES ('legacy_import_v1_done', 'true')",
                [],
            )
            .map_err(|e| e.to_string())?;
        return Ok(false);
    }

    let legacy_studio = Connection::open_with_flags(
        studio_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("Failed to open legacy studio db: {e}"))?;

    let tx = store
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;

    // 1. Project Metadata
    let meta: Option<ProjectMetadata> = legacy_studio
        .query_row(
            "SELECT project_id, name, revision FROM project_metadata WHERE singleton = 1",
            [],
            |r| {
                Ok(ProjectMetadata {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    revision: r.get::<_, i64>(2)? as u64,
                })
            },
        )
        .optional()
        .map_err(|e| format!("Failed to read project_metadata: {e}"))?;

    if let Some(m) = meta {
        tx.execute(
            "UPDATE project_metadata SET project_id = ?1, name = ?2, revision = ?3 WHERE singleton = 1",
            params![m.id, m.name, m.revision as i64],
        )
        .map_err(|e| e.to_string())?;
    }

    // 2. Qualities
    {
        let mut stmt = legacy_studio
            .prepare("SELECT name, type, value, description, metadata FROM qualities")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                let meta_str: Option<String> = r.get(4)?;
                let metadata = meta_str.and_then(|s| serde_json::from_str(&s).ok());
                Ok(QualityRecord {
                    name: r.get(0)?,
                    kind: r.get(1)?,
                    value: r.get(2)?,
                    description: r.get(3)?,
                    metadata,
                })
            })
            .map_err(|e| e.to_string())?;

        for row in rows {
            let q = row.map_err(|e| e.to_string())?;
            let json = serde_json::to_string(&q).map_err(|e| e.to_string())?;
            tx.execute(
                "INSERT OR REPLACE INTO content_records (entity, id, data_json, revision, approved_revision, updated_at)
                 VALUES ('qualities', ?1, ?2, 1, NULL, 0)",
                params![q.name, json],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    // 3. Characters
    {
        let mut stmt = legacy_studio
            .prepare("SELECT id, name, role, home_location, current_location, status_axes, needs, agenda, red_line, schedule, qualities, created_at FROM characters")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                let status_axes_str: String = r.get(5)?;
                let needs_str: String = r.get(6)?;
                let sched_str: Option<String> = r.get(9)?;
                let qual_str: Option<String> = r.get(10)?;
                Ok(CharacterRecord {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    role: r.get(2)?,
                    home_location: r.get(3)?,
                    current_location: r.get(4)?,
                    status_axes: serde_json::from_str(&status_axes_str).unwrap_or(Value::Null),
                    needs: serde_json::from_str(&needs_str).unwrap_or(Value::Null),
                    agenda: r.get(7)?,
                    red_line: r.get(8)?,
                    schedule: sched_str.and_then(|s| serde_json::from_str(&s).ok()),
                    qualities: qual_str.and_then(|s| serde_json::from_str(&s).ok()),
                    created_at: r.get(11)?,
                })
            })
            .map_err(|e| e.to_string())?;

        for row in rows {
            let c = row.map_err(|e| e.to_string())?;
            let json = serde_json::to_string(&c).map_err(|e| e.to_string())?;
            tx.execute(
                "INSERT OR REPLACE INTO content_records (entity, id, data_json, revision, approved_revision, updated_at)
                 VALUES ('characters', ?1, ?2, 1, NULL, 0)",
                params![c.id, json],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    // 4. Locations
    let mut legacy_location_tags = BTreeMap::<String, Vec<String>>::new();
    {
        let tags_column =
            if has_column(&legacy_studio, "locations", "tags").map_err(|e| e.to_string())? {
                "tags"
            } else {
                "NULL"
            };
        let query = format!(
            "SELECT id, name, description, {tags_column}, connected_zones, environmental_params, devices, created_at FROM locations"
        );
        let mut stmt = legacy_studio.prepare(&query).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                let id: String = r.get(0)?;
                let mut tags = parse_tags(r.get(3)?);
                if tags.is_empty() {
                    tags.push(id.clone());
                }
                let conn_str: Option<String> = r.get(4)?;
                let env_str: Option<String> = r.get(5)?;
                let dev_str: Option<String> = r.get(6)?;
                Ok(LocationRecord {
                    id,
                    name: r.get(1)?,
                    description: r.get(2)?,
                    tags,
                    connected_zones: conn_str.and_then(|s| serde_json::from_str(&s).ok()),
                    environmental_params: env_str.and_then(|s| serde_json::from_str(&s).ok()),
                    devices: dev_str.and_then(|s| serde_json::from_str(&s).ok()),
                    created_at: r.get(7)?,
                })
            })
            .map_err(|e| e.to_string())?;

        for row in rows {
            let l = row.map_err(|e| e.to_string())?;
            legacy_location_tags.insert(l.id.clone(), l.tags.clone());
            let json = serde_json::to_string(&l).map_err(|e| e.to_string())?;
            tx.execute(
                "INSERT OR REPLACE INTO content_records (entity, id, data_json, revision, approved_revision, updated_at)
                 VALUES ('locations', ?1, ?2, 1, NULL, 0)",
                params![l.id, json],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    // 5. NPC Behaviors
    {
        let mut stmt = legacy_studio
            .prepare(
                "SELECT id, actor_id, priority, conditions, action, effects FROM npc_behaviors",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                let cond_str: String = r.get(3)?;
                let eff_str: String = r.get(5)?;
                Ok(NpcBehaviorRecord {
                    id: r.get(0)?,
                    actor_id: r.get(1)?,
                    priority: r.get(2)?,
                    conditions: serde_json::from_str(&cond_str).unwrap_or(Value::Null),
                    action: r.get(4)?,
                    effects: serde_json::from_str(&eff_str).unwrap_or_default(),
                })
            })
            .map_err(|e| e.to_string())?;

        for row in rows {
            let b = row.map_err(|e| e.to_string())?;
            let json = serde_json::to_string(&b).map_err(|e| e.to_string())?;
            tx.execute(
                "INSERT OR REPLACE INTO content_records (entity, id, data_json, revision, approved_revision, updated_at)
                 VALUES ('npc_behaviors', ?1, ?2, 1, NULL, 0)",
                params![b.id, json],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    // 6. Storylets (Import all text and logic; set approved_revision = None per user decision)
    {
        let query = legacy_storylet_query(&legacy_studio).map_err(|e| e.to_string())?;
        let mut stmt = legacy_studio.prepare(&query).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                let params_str: String = r.get(9)?;
                let pre_str: String = r.get(10)?;
                let content_str: String = r.get(11)?;
                let choices_str: String = r.get(12)?;
                let effects_str: String = r.get(13)?;

                let parameters: Vec<StoryletParameterRecord> =
                    serde_json::from_str(&params_str).unwrap_or_default();
                let preconditions: Value = serde_json::from_str(&pre_str).unwrap_or(Value::Null);
                let content: StoryletContentRecord =
                    serde_json::from_str(&content_str).unwrap_or(StoryletContentRecord {
                        text_template: String::new(),
                        speaker_id: None,
                        scene_opening: None,
                        dialogue_quips: None,
                    });
                let choices: Vec<StoryletChoiceRecord> =
                    serde_json::from_str(&choices_str).unwrap_or_default();
                let effects: Value = serde_json::from_str(&effects_str).unwrap_or(Value::Null);

                let rev: i64 = r.get(15)?;
                let ending_int: i64 = r.get(17)?;
                let location_id: Option<String> = r.get(2)?;
                let location_tags = storylet_location_tags(
                    parse_tags(r.get(3)?),
                    location_id,
                    &legacy_location_tags,
                );

                Ok(StoryletRecord {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    location_tags,
                    group_tag: r.get(4)?,
                    cluster_type: r.get(5)?,
                    urgency_weight: r.get(6)?,
                    repeatability: r.get(7)?,
                    cooldown_turns: r.get::<_, i64>(8)? as u32,
                    parameters,
                    preconditions,
                    content,
                    choices,
                    effects,
                    created_at: r.get(14)?,
                    revision: rev as u64,
                    approved_revision: None, // User selected: Require fresh approval!
                    ending: ending_int != 0,
                    position: None,
                    size: None,
                    local_variables: None,
                    graph: None,
                })
            })
            .map_err(|e| e.to_string())?;

        for row in rows {
            let st = row.map_err(|e| e.to_string())?;
            let json = serde_json::to_string(&st).map_err(|e| e.to_string())?;
            tx.execute(
                "INSERT OR REPLACE INTO content_records (entity, id, data_json, revision, approved_revision, updated_at)
                 VALUES ('storylets', ?1, ?2, ?3, NULL, 0)",
                params![st.id, json, st.revision as i64],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    // 7. Scenarios & Lifted Events
    {
        let mut stmt = legacy_studio
            .prepare("SELECT id, name, scenario_json FROM scenarios")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;

        for row in rows {
            let (scen_id, scen_name, json_str) = row.map_err(|e| e.to_string())?;
            if let Ok(raw) = serde_json::from_str::<Value>(&json_str) {
                // Lift embedded event_definitions into project events
                if let Some(events_arr) = raw.get("event_definitions").and_then(|v| v.as_array()) {
                    for ev_val in events_arr {
                        if let Ok(ev) = serde_json::from_value::<EventRecord>(ev_val.clone()) {
                            let ev_json = serde_json::to_string(&ev).unwrap();
                            tx.execute(
                                "INSERT OR REPLACE INTO content_records (entity, id, data_json, revision, approved_revision, updated_at)
                                 VALUES ('events', ?1, ?2, 1, NULL, 0)",
                                params![ev.id, ev_json],
                            )
                            .map_err(|e| e.to_string())?;
                        }
                    }
                }

                // Translate start location policy
                let mut start_policy: StartLocationPolicy = raw
                    .get("start_location_policy")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();

                if start_policy.location_id.is_none() {
                    if let Some(fixed_loc) =
                        raw.get("player_start_location").and_then(|v| v.as_str())
                    {
                        if !fixed_loc.trim().is_empty() {
                            start_policy.mode = "fixed".to_string();
                            start_policy.location_id = Some(fixed_loc.to_string());
                        }
                    }
                }

                let actor_policy: ActorPopulationPolicy = raw
                    .get("actor_population_policy")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();

                let initial_qualities = raw
                    .get("initial_qualities")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();

                let actor_locations = raw
                    .get("actor_locations")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();

                let seed = raw.get("seed").and_then(|v| v.as_u64()).unwrap_or(1);

                let scenario_rec = ScenarioRecord {
                    id: scen_id,
                    name: scen_name,
                    seed,
                    start_location_policy: start_policy,
                    initial_qualities,
                    actor_locations,
                    actor_population_policy: actor_policy,
                };

                let scen_json = serde_json::to_string(&scenario_rec).unwrap();
                tx.execute(
                    "INSERT OR REPLACE INTO content_records (entity, id, data_json, revision, approved_revision, updated_at)
                     VALUES ('scenarios', ?1, ?2, 1, NULL, 0)",
                    params![scenario_rec.id, scen_json],
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }

    // 8. Generation Jobs
    {
        let mut stmt = legacy_studio
            .prepare("SELECT id, revision, status, job_json, updated_at FROM generation_jobs")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            })
            .map_err(|e| e.to_string())?;

        for row in rows {
            let (id, rev, status, job_json, updated_at) = row.map_err(|e| e.to_string())?;
            tx.execute(
                "INSERT OR REPLACE INTO generation_jobs (id, revision, status, job_json, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, rev, status, job_json, updated_at],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    // 9. Runtime Sessions and Events (from legacy engine db if exists)
    let engine_path = legacy_engine_path.as_ref();
    if engine_path.exists() {
        if let Ok(legacy_engine) = Connection::open_with_flags(
            engine_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            // Sessions
            if let Ok(mut stmt) = legacy_engine.prepare("SELECT session_id, project_id, revision, turn, state_json, updated_at FROM runtime_sessions") {
                if let Ok(rows) = stmt.query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, String>(5)?,
                    ))
                }) {
                    for row in rows.flatten() {
                        let _ = tx.execute(
                            "INSERT OR REPLACE INTO runtime_sessions (session_id, project_id, revision, turn, state_json, updated_at)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                            params![row.0, row.1, row.2, row.3, row.4, row.5],
                        );
                    }
                }
            }

            // Events
            if let Ok(mut stmt) = legacy_engine
                .prepare("SELECT session_id, event_id, revision, event_json FROM runtime_events")
            {
                if let Ok(rows) = stmt.query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                    ))
                }) {
                    for row in rows.flatten() {
                        let _ = tx.execute(
                            "INSERT OR REPLACE INTO runtime_events (session_id, event_id, revision, event_json)
                             VALUES (?1, ?2, ?3, ?4)",
                            params![row.0, row.1, row.2, row.3],
                        );
                    }
                }
            }
        }
    }

    // Mark migration complete
    tx.execute(
        "INSERT OR REPLACE INTO store_metadata (key, value) VALUES ('legacy_import_v1_done', 'true')",
        [],
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{legacy_storylet_query, storylet_location_tags};
    use rusqlite::Connection;
    use std::collections::BTreeMap;

    #[test]
    fn storylet_import_query_accepts_tag_only_schema() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE storylets (id TEXT, title TEXT, location_tags TEXT, group_tag TEXT, cluster_type TEXT, urgency_weight REAL, repeatability TEXT, cooldown_turns INTEGER, parameters TEXT, preconditions TEXT, content TEXT, choices TEXT, effects TEXT, created_at INTEGER, revision INTEGER, approved_revision INTEGER, ending INTEGER);",
        )
        .unwrap();

        let query = legacy_storylet_query(&conn).unwrap();
        assert!(query.contains("SELECT id, title, NULL, location_tags,"));
        conn.prepare(&query).unwrap();
    }

    #[test]
    fn legacy_location_bindings_resolve_to_location_tags() {
        let locations = BTreeMap::from([(
            String::from("archive-id"),
            vec![String::from("archive"), String::from("indoors")],
        )]);
        assert_eq!(
            storylet_location_tags(Vec::new(), Some(String::from("archive-id")), &locations),
            vec![String::from("archive"), String::from("indoors")],
        );
        assert_eq!(
            storylet_location_tags(
                vec![String::from("custom")],
                Some(String::from("archive-id")),
                &locations,
            ),
            vec![String::from("custom")],
        );
    }
}
