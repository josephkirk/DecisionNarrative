use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::path::Path;

use crate::narrative_graph::NarrativeGraph;

use crate::contracts::{
    AssetRecord, EntityRecord, EventRecord, FolderRecord, GameplayTagRecord, LibraryPageResult,
    LibraryQuery, LibrarySummaryItem, ProjectBundleContent, ProjectMetadata,
    SYSTEM_TRAIT_TEMPLATES, ScenarioRecord, StoryletRecord, TraitTemplateRecord,
};
use serde_json::{Value, json};
fn normalize_storylet_ending(mut storylet: StoryletRecord) -> StoryletRecord {
    if let Some(graph) = storylet.graph.clone() {
        storylet.ending = graph.ending_from_graph();
    }
    storylet
}

pub struct Store {
    pub conn: Connection,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    pub fn from_connection(conn: Connection) -> Result<Self, rusqlite::Error> {
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<(), rusqlite::Error> {
        let object_count: i64 = self.conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type IN ('table','index','trigger','view') AND name NOT LIKE 'sqlite_%'",
            [], |row| row.get(0))?;
        if object_count > 0 {
            let marker: Option<String> = self
                .conn
                .query_row(
                    "SELECT value FROM store_metadata WHERE key = 'schema_version'",
                    [],
                    |row| row.get(0),
                )
                .optional()?;
            if marker.as_deref() != Some("8") {
                return Err(rusqlite::Error::InvalidQuery);
            }
            for table in [
                "project_metadata",
                "content_records",
                "interaction_context",
                "store_metadata",
                "runs",
            ] {
                let present: bool = self.conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                    [table],
                    |row| row.get(0),
                )?;
                if !present {
                    return Err(rusqlite::Error::InvalidQuery);
                }
            }
            let columns = self
                .conn
                .prepare("PRAGMA table_info(runs)")?
                .query_map([], |row| {
                    Ok((row.get::<_, String>(1)?, row.get::<_, bool>(3)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let has_column = |name: &str| {
                columns
                    .iter()
                    .any(|(column, required)| column == name && *required)
            };
            let legacy_table: bool = self.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='run_idempotency')",
                [], |row| row.get(0),
            )?;
            let unique_start_key: bool = self.conn.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM pragma_index_list('runs') AS i
                    WHERE i.[unique]=1 AND (SELECT count(*) FROM pragma_index_info(i.name))=1
                    AND EXISTS(SELECT 1 FROM pragma_index_info(i.name) WHERE name='start_key')
                )",
                [],
                |row| row.get(0),
            )?;
            if !has_column("start_key")
                || !has_column("start_request")
                || legacy_table
                || !unique_start_key
            {
                return Err(rusqlite::Error::InvalidQuery);
            }
            let has_legacy_records: bool = self.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM content_records WHERE
                    entity IN ('qualities','npc_behaviors'))",
                [],
                |row| row.get(0),
            )?;
            if has_legacy_records {
                return Err(rusqlite::Error::InvalidQuery);
            }
            let canonical_capabilities: i64 = self.conn.query_row("SELECT count(*) FROM content_records WHERE entity='trait_templates' AND id IN ('system.character','system.location','system.fact')", [], |row| row.get(0))?;
            let has_world: bool = self.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM content_records WHERE entity='entities' AND id='world')",
                [], |row| row.get(0))?;
            if !has_world || canonical_capabilities != 3 {
                return Err(rusqlite::Error::InvalidQuery);
            }
        }
        self.conn.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS project_metadata (
                 singleton INTEGER PRIMARY KEY CHECK(singleton = 1), project_id TEXT NOT NULL,
                 name TEXT NOT NULL, revision INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE IF NOT EXISTS content_records (
                 entity TEXT NOT NULL, id TEXT NOT NULL, data_json TEXT NOT NULL,
                 revision INTEGER NOT NULL DEFAULT 1, approved_revision INTEGER,
                 updated_at INTEGER NOT NULL, PRIMARY KEY(entity, id));
             CREATE INDEX IF NOT EXISTS idx_content_entity ON content_records(entity);
             CREATE TABLE IF NOT EXISTS interaction_context (
                 singleton INTEGER PRIMARY KEY CHECK(singleton = 1), context_json TEXT NOT NULL,
                 captured_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
                 is_stale INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE IF NOT EXISTS store_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS runs (id TEXT PRIMARY KEY, start_key TEXT NOT NULL UNIQUE, start_request TEXT NOT NULL, project_revision INTEGER NOT NULL, seed INTEGER NOT NULL, start_json TEXT NOT NULL, actions_json TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);")?;
        self.conn.execute(
            "INSERT OR IGNORE INTO store_metadata (key, value) VALUES ('schema_version', '8')",
            [],
        )?;
        let exists: bool = self
            .conn
            .query_row(
                "SELECT 1 FROM project_metadata WHERE singleton = 1",
                [],
                |_| Ok(true),
            )
            .optional()?
            .is_some();
        if !exists {
            self.conn.execute(
                "INSERT INTO project_metadata (singleton, project_id, name, revision)
                 VALUES (1, 'workspace-project', 'Workspace', 0)",
                [],
            )?;
            seed_system_records(&self.conn)?;
        } else {
            sync_system_trait_uniqueness(&self.conn)?;
        }
        ensure_world_entity(&self.conn)?;
        Ok(())
    }

    pub fn get_project_metadata(&self) -> Result<ProjectMetadata, rusqlite::Error> {
        self.conn.query_row(
            "SELECT project_id, name, revision FROM project_metadata WHERE singleton = 1",
            [],
            |row| {
                Ok(ProjectMetadata {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    revision: row.get::<_, i64>(2)? as u64,
                })
            },
        )
    }

    pub fn save_project_metadata(&self, meta: &ProjectMetadata) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE project_metadata SET project_id = ?1, name = ?2, revision = ?3 WHERE singleton = 1",
            params![meta.id, meta.name, meta.revision as i64],
        )?;
        Ok(())
    }

    pub fn bump_project_revision(&self) -> Result<u64, rusqlite::Error> {
        self.conn.execute(
            "UPDATE project_metadata SET revision = revision + 1 WHERE singleton = 1",
            [],
        )?;
        let rev: u64 = self.conn.query_row(
            "SELECT revision FROM project_metadata WHERE singleton = 1",
            [],
            |row| Ok(row.get::<_, i64>(0)? as u64),
        )?;
        Ok(rev)
    }

    pub fn get_content_json(
        &self,
        entity: &str,
        id: &str,
    ) -> Result<Option<String>, rusqlite::Error> {
        self.conn
            .query_row(
                "SELECT data_json FROM content_records WHERE entity = ?1 AND id = ?2",
                params![entity, id],
                |row| row.get(0),
            )
            .optional()
    }

    pub fn list_content_json(&self, entity: &str) -> Result<Vec<String>, rusqlite::Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT data_json FROM content_records WHERE entity = ?1 ORDER BY id")?;
        let rows = stmt.query_map(params![entity], |row| row.get::<_, String>(0))?;
        rows.collect()
    }

    pub fn save_content(
        &self,
        entity: &str,
        id: &str,
        data_json: &str,
        revision: u64,
        approved_revision: Option<u64>,
    ) -> Result<(), rusqlite::Error> {
        let now = chrono_now_ms();
        let tx = self.conn.unchecked_transaction()?;
        self.conn.execute(
            "INSERT INTO content_records (entity, id, data_json, revision, approved_revision, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(entity, id) DO UPDATE SET
                 data_json = excluded.data_json,
                 revision = excluded.revision,
                 approved_revision = excluded.approved_revision,
                 updated_at = excluded.updated_at",
            params![entity, id, data_json, revision as i64, approved_revision.map(|r| r as i64), now],
        )?;
        self.bump_project_revision()?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete_content(&self, entity: &str, id: &str) -> Result<bool, rusqlite::Error> {
        let tx = self.conn.unchecked_transaction()?;
        let count = self.conn.execute(
            "DELETE FROM content_records WHERE entity = ?1 AND id = ?2",
            params![entity, id],
        )?;
        if count > 0 {
            self.bump_project_revision()?;
        }
        tx.commit()?;
        Ok(count > 0)
    }

    pub fn list_entities(&self) -> Result<Vec<EntityRecord>, rusqlite::Error> {
        Ok(self
            .list_content_json("entities")?
            .into_iter()
            .filter_map(|s| serde_json::from_str(&s).ok())
            .collect())
    }

    pub fn list_gameplay_tags(&self) -> Result<Vec<GameplayTagRecord>, rusqlite::Error> {
        Ok(self
            .list_content_json("gameplay_tags")?
            .into_iter()
            .filter_map(|s| serde_json::from_str(&s).ok())
            .collect())
    }

    pub fn list_trait_templates(&self) -> Result<Vec<TraitTemplateRecord>, rusqlite::Error> {
        let json_list = self.list_content_json("trait_templates")?;
        Ok(json_list
            .into_iter()
            .filter_map(|s| serde_json::from_str(&s).ok())
            .collect())
    }

    pub fn list_assets(&self) -> Result<Vec<AssetRecord>, rusqlite::Error> {
        let json_list = self.list_content_json("assets")?;
        Ok(json_list
            .into_iter()
            .filter_map(|s| serde_json::from_str(&s).ok())
            .collect())
    }

    pub fn list_folders(&self) -> Result<Vec<FolderRecord>, rusqlite::Error> {
        let json_list = self.list_content_json("folders")?;
        Ok(json_list
            .into_iter()
            .filter_map(|s| serde_json::from_str(&s).ok())
            .collect())
    }

    pub fn update_content_json(
        &self,
        entity: &str,
        id: &str,
        data_json: &str,
    ) -> Result<bool, rusqlite::Error> {
        let tx = self.conn.unchecked_transaction()?;
        let count = self.conn.execute(
            "UPDATE content_records SET data_json = ?1, updated_at = ?2 WHERE entity = ?3 AND id = ?4",
            params![data_json, chrono_now_ms(), entity, id],
        )?;
        if count > 0 {
            self.bump_project_revision()?;
        }
        tx.commit()?;
        Ok(count > 0)
    }

    pub fn list_events(&self) -> Result<Vec<EventRecord>, rusqlite::Error> {
        let json_list = self.list_content_json("events")?;
        Ok(json_list
            .into_iter()
            .filter_map(|s| serde_json::from_str(&s).ok())
            .collect())
    }

    pub fn save_event(&self, e: &EventRecord) -> Result<(), rusqlite::Error> {
        let json = serde_json::to_string(e)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        self.save_content("events", &e.id, &json, 1, None)
    }

    pub fn list_scenarios(&self) -> Result<Vec<ScenarioRecord>, rusqlite::Error> {
        let json_list = self.list_content_json("scenarios")?;
        Ok(json_list
            .into_iter()
            .filter_map(|s| serde_json::from_str(&s).ok())
            .collect())
    }

    pub fn save_scenario(&self, s: &ScenarioRecord) -> Result<(), rusqlite::Error> {
        let json = serde_json::to_string(s)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        self.save_content("scenarios", &s.id, &json, 1, None)
    }

    pub fn list_storylets(&self) -> Result<Vec<StoryletRecord>, rusqlite::Error> {
        let json_list = self.list_content_json("storylets")?;
        Ok(json_list
            .into_iter()
            .filter_map(|s| serde_json::from_str(&s).ok())
            .map(normalize_storylet_ending)
            .collect())
    }

    pub fn get_storylet(&self, id: &str) -> Result<Option<StoryletRecord>, rusqlite::Error> {
        let json_opt = self.get_content_json("storylets", id)?;
        Ok(json_opt
            .and_then(|s| serde_json::from_str::<StoryletRecord>(&s).ok())
            .map(normalize_storylet_ending))
    }

    pub fn save_storylet(&self, st: &StoryletRecord) -> Result<(), rusqlite::Error> {
        let json = serde_json::to_string(st)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        self.save_content(
            "storylets",
            &st.id,
            &json,
            st.revision,
            st.approved_revision,
        )
    }

    pub fn approve_storylet(
        &self,
        id: &str,
        expected_revision: u64,
    ) -> Result<Option<StoryletRecord>, String> {
        let Some(mut st) = self.get_storylet(id).map_err(|e| e.to_string())? else {
            return Ok(None);
        };
        if st.revision != expected_revision {
            return Err(format!(
                "Revision conflict: current storylet revision is {}, expected {}",
                st.revision, expected_revision
            ));
        }
        let graph = match st.graph.clone() {
            Some(graph) => graph,
            None => NarrativeGraph::from_storylet(&st),
        };
        graph.validate().map_err(|issues| issues.join(" "))?;
        st.approved_revision = Some(expected_revision);
        self.save_storylet(&st).map_err(|e| e.to_string())?;
        Ok(Some(st))
    }

    pub fn query_library(
        &self,
        query: &LibraryQuery,
    ) -> Result<LibraryPageResult, rusqlite::Error> {
        let all_storylets = self.list_storylets()?;
        let available_location_tags: Vec<String> = all_storylets
            .iter()
            .flat_map(|st| st.location_tags.iter().cloned())
            .filter(|tag| !tag.trim().is_empty())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();

        let q_needle = query.q.as_deref().unwrap_or("").trim().to_lowercase();
        let status_filter = query
            .status
            .as_deref()
            .unwrap_or("all")
            .trim()
            .to_lowercase();
        let location_tag_filter = query.location_tag.as_deref().unwrap_or("").trim();
        let ending_filter = query
            .ending
            .as_deref()
            .unwrap_or("all")
            .trim()
            .to_lowercase();

        let mut filtered: Vec<&StoryletRecord> = all_storylets
            .iter()
            .filter(|st| {
                if !q_needle.is_empty() {
                    let title_match = st.title.to_lowercase().contains(&q_needle);
                    let text_match = st.content.text_template.to_lowercase().contains(&q_needle)
                        || st
                            .content
                            .scene_opening
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&q_needle);
                    let tag_match = st
                        .group_tag
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&q_needle);
                    if !title_match && !text_match && !tag_match {
                        return false;
                    }
                }
                if status_filter == "approved" {
                    if st.approved_revision != Some(st.revision) {
                        return false;
                    }
                } else if status_filter == "draft" && st.approved_revision == Some(st.revision) {
                    return false;
                }
                if !location_tag_filter.is_empty()
                    && location_tag_filter != "all"
                    && !st
                        .location_tags
                        .iter()
                        .any(|tag| tag == location_tag_filter)
                {
                    return false;
                }
                if ending_filter == "ending" {
                    if !st.ending {
                        return false;
                    }
                } else if ending_filter == "non-ending" && st.ending {
                    return false;
                }
                true
            })
            .collect();

        let sort_mode = query.sort.as_deref().unwrap_or("title");
        if sort_mode == "created_desc" {
            filtered.sort_by(|a, b| {
                b.created_at
                    .cmp(&a.created_at)
                    .then_with(|| a.id.cmp(&b.id))
            });
        } else {
            filtered.sort_by(|a, b| {
                a.title
                    .to_lowercase()
                    .cmp(&b.title.to_lowercase())
                    .then_with(|| a.id.cmp(&b.id))
            });
        }

        let total = filtered.len();
        let page_size = query.page_size.unwrap_or(60).clamp(1, 100);
        let total_pages = if total == 0 {
            1
        } else {
            total.div_ceil(page_size)
        };
        let page = query.page.unwrap_or(1).clamp(1, total_pages);
        let start = (page - 1) * page_size;
        let paged: Vec<LibrarySummaryItem> = filtered
            .into_iter()
            .skip(start)
            .take(page_size)
            .map(|st| {
                let text = st
                    .content
                    .scene_opening
                    .as_deref()
                    .unwrap_or(&st.content.text_template);
                let excerpt = text
                    .lines()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(140)
                    .collect::<String>();
                let role_tags = st.parameters.iter().map(|p| p.query.clone()).collect();
                LibrarySummaryItem {
                    id: st.id.clone(),
                    title: st.title.clone(),
                    origin: st.origin.clone(),
                    excerpt,
                    location_tags: st.location_tags.clone(),
                    role_tags,
                    window: st.window.clone(),
                    played_by: st.played_by.clone(),
                    choice_count: st.choices.len(),
                    revision: st.revision,
                    approved_revision: st.approved_revision,
                    ending: st.ending,
                    created_at: st.created_at,
                }
            })
            .collect();

        Ok(LibraryPageResult {
            items: paged,
            total,
            page,
            page_size,
            total_pages,
            available_location_tags,
        })
    }

    pub fn save_interaction_context(
        &self,
        context: &serde_json::Value,
    ) -> Result<(), rusqlite::Error> {
        let now = chrono_now_ms();
        let captured_at = context
            .get("capturedAt")
            .and_then(|v| v.as_i64())
            .unwrap_or(now);
        let json = serde_json::to_string(context)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        self.conn.execute(
            "INSERT INTO interaction_context (singleton, context_json, captured_at, updated_at, is_stale)
             VALUES (1, ?1, ?2, ?3, 0)
             ON CONFLICT(singleton) DO UPDATE SET
                 context_json = excluded.context_json,
                 captured_at = excluded.captured_at,
                 updated_at = excluded.updated_at,
                 is_stale = 0",
            params![json, captured_at, now],
        )?;
        Ok(())
    }

    pub fn mark_interaction_context_stale(&self) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE interaction_context SET is_stale = 1 WHERE singleton = 1",
            [],
        )?;
        Ok(())
    }

    pub fn get_interaction_context(
        &self,
    ) -> Result<Option<(serde_json::Value, i64, i64, bool)>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT context_json, captured_at, updated_at, is_stale FROM interaction_context WHERE singleton = 1",
        )?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let raw: String = row.get(0)?;
            let captured_at: i64 = row.get(1)?;
            let updated_at: i64 = row.get(2)?;
            let is_stale: bool = row.get::<_, i64>(3)? != 0;
            if let Ok(parsed) = serde_json::from_str(&raw) {
                return Ok(Some((parsed, captured_at, updated_at, is_stale)));
            }
        }
        Ok(None)
    }

    pub fn replace_project_content(
        &mut self,
        meta: &ProjectMetadata,
        content: &ProjectBundleContent,
    ) -> Result<u64, rusqlite::Error> {
        if !crate::entities::validate_bundle_content(content).is_empty() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("DELETE FROM content_records", [])?;
        let current_revision: i64 = tx
            .query_row(
                "SELECT revision FROM project_metadata WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0);
        let next_revision = (current_revision + 1).max(meta.revision as i64);
        tx.execute(
            "UPDATE project_metadata SET project_id = ?1, name = ?2, revision = ?3 WHERE singleton = 1",
            params![meta.id, meta.name, next_revision],
        )?;

        let now = chrono_now_ms();
        insert_rows(
            &tx,
            "entities",
            content.entities.iter().map(|entity| {
                Ok((
                    entity.id.clone(),
                    serde_json::to_string(entity)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                    1,
                    None,
                ))
            }),
            now,
        )?;

        insert_rows(
            &tx,
            "gameplay_tags",
            content.gameplay_tags.iter().map(|tag| {
                Ok((
                    tag.id.clone(),
                    serde_json::to_string(tag)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                    1,
                    None,
                ))
            }),
            now,
        )?;
        insert_rows(
            &tx,
            "folders",
            content.folders.iter().map(|folder| {
                Ok((
                    folder.id.clone(),
                    serde_json::to_string(folder)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                    1,
                    None,
                ))
            }),
            now,
        )?;
        insert_rows(
            &tx,
            "trait_templates",
            content.trait_templates.iter().map(|template| {
                Ok((
                    template.id.clone(),
                    serde_json::to_string(template)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                    1,
                    None,
                ))
            }),
            now,
        )?;
        insert_rows(
            &tx,
            "assets",
            content.assets.iter().map(|asset| {
                Ok((
                    asset.id.clone(),
                    serde_json::to_string(asset)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                    1,
                    None,
                ))
            }),
            now,
        )?;
        insert_rows(
            &tx,
            "storylets",
            content.storylets.iter().map(|storylet| {
                Ok((
                    storylet.id.clone(),
                    serde_json::to_string(storylet)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                    storylet.revision,
                    storylet.approved_revision,
                ))
            }),
            now,
        )?;
        insert_rows(
            &tx,
            "events",
            content.events.iter().map(|event| {
                Ok((
                    event.id.clone(),
                    serde_json::to_string(event)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                    1,
                    None,
                ))
            }),
            now,
        )?;
        insert_rows(
            &tx,
            "scenarios",
            content.scenarios.iter().map(|scenario| {
                Ok((
                    scenario.id.clone(),
                    serde_json::to_string(scenario)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                    1,
                    None,
                ))
            }),
            now,
        )?;
        seed_system_records(&tx)?;
        ensure_world_entity(&tx)?;
        tx.commit()?;
        Ok(next_revision as u64)
    }
}

fn insert_rows(
    tx: &rusqlite::Transaction<'_>,
    entity: &str,
    rows: impl Iterator<Item = rusqlite::Result<(String, String, u64, Option<u64>)>>,
    now: i64,
) -> Result<(), rusqlite::Error> {
    let mut statement = tx.prepare(
        "INSERT INTO content_records (entity, id, data_json, revision, approved_revision, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for row in rows {
        let (id, json, revision, approved_revision) = row?;
        statement.execute(params![
            entity,
            id,
            json,
            revision as i64,
            approved_revision.map(|value| value as i64),
            now
        ])?;
    }
    Ok(())
}

fn chrono_now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn sync_system_trait_uniqueness(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    let tx = conn.unchecked_transaction()?;
    let mut changed = 0;
    for template in SYSTEM_TRAIT_TEMPLATES {
        let id = format!("system.{}", template.key);
        changed += tx.execute(
            "UPDATE content_records SET data_json = json_set(data_json, '$.unique', json(?1)), revision = revision + 1 WHERE entity = 'trait_templates' AND id = ?2 AND COALESCE(json_extract(data_json, '$.unique'), 0) != ?3",
            params![if template.unique { "true" } else { "false" }, id, template.unique],
        )?;
    }
    if changed > 0 {
        tx.execute(
            "UPDATE project_metadata SET revision = revision + 1 WHERE singleton = 1",
            [],
        )?;
    }
    tx.commit()?;
    Ok(())
}

fn seed_system_records(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    for template in SYSTEM_TRAIT_TEMPLATES {
        let properties: Vec<Value> = template
            .properties
            .iter()
            .map(|spec| {
                let mut property = json!({
                "key": spec.key,
                "name": spec.name,
                "type": spec.property_type,
                "is_array": spec.is_array,
                "default": serde_json::from_str::<Value>(spec.default_json)
                    .expect("system trait default JSON must be valid"),
                });
                if let Some(editor) = template
                    .editors
                    .iter()
                    .find(|editor| editor.property_key == spec.key)
                {
                    property["editor"] = json!({
                        "option_source": editor.option_source,
                        "selection_only": editor.selection_only,
                        "entity_trait_key": editor.entity_trait_key,
                    });
                }
                property
            })
            .collect();
        let id = format!("system.{}", template.key);
        let record = json!({
            "id": id,
            "key": template.key,
            "key_auto": false,
            "name": template.name,
            "description": "",
            "folder_id": null,
            "properties": properties,
            "allow_multiple": template.allow_multiple,
            "unique": template.unique,
            "system": true,
            "created_at": 0
        });
        conn.execute("INSERT OR IGNORE INTO content_records (entity,id,data_json,revision,updated_at) VALUES ('trait_templates',?1,?2,1,0)",
            params![id, record.to_string()])?;
    }
    Ok(())
}

pub fn ensure_world_entity(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM content_records WHERE entity='entities' AND id='world'",
            [],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    if !exists {
        let world = serde_json::to_value(crate::entities::default_world_entity())
            .expect("world entity serializes");
        conn.execute(
            "INSERT INTO content_records (entity,id,data_json,revision,updated_at)
             VALUES ('entities','world',?1,1,0)",
            params![world.to_string()],
        )?;
    }
    Ok(())
}
