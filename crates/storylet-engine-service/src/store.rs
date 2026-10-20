//! Single SQLite store implementation for DecisionNarrative Storylet Studio.

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::narrative_graph::NarrativeGraph;

use crate::contracts::{
    CharacterRecord, EventRecord, LibraryPageResult, LibraryQuery, LibrarySummaryItem,
    LocationRecord, NpcBehaviorRecord, ProjectBundleContent, ProjectMetadata, QualityRecord,
    ScenarioRecord, StoryletRecord,
};

/// Older records may carry an `ending` flag written before the graph became
/// authoritative. Normalize on read so library filters and approvals see the
/// same value the next save will persist.
fn normalize_storylet_ending(mut storylet: StoryletRecord) -> StoryletRecord {
    if let Some(value) = storylet.graph.clone()
        && let Ok(graph) = serde_json::from_value::<NarrativeGraph>(value)
    {
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
        self.conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;

             CREATE TABLE IF NOT EXISTS project_metadata (
                 singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
                 project_id TEXT NOT NULL,
                 name TEXT NOT NULL,
                 revision INTEGER NOT NULL DEFAULT 0
             );

             CREATE TABLE IF NOT EXISTS content_records (
                 entity TEXT NOT NULL,
                 id TEXT NOT NULL,
                 data_json TEXT NOT NULL,
                 revision INTEGER NOT NULL DEFAULT 1,
                 approved_revision INTEGER,
                 updated_at INTEGER NOT NULL,
                 PRIMARY KEY(entity, id)
             );

             CREATE INDEX IF NOT EXISTS idx_content_entity ON content_records(entity);

             CREATE TABLE IF NOT EXISTS interaction_context (
                 singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
                 context_json TEXT NOT NULL,
                 captured_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL,
                 is_stale INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS runtime_sessions (
                 session_id TEXT PRIMARY KEY,
                 project_id TEXT NOT NULL,
                 revision TEXT NOT NULL,
                 turn TEXT NOT NULL,
                 state_json TEXT NOT NULL,
                 updated_at TEXT NOT NULL
             );

             CREATE TABLE IF NOT EXISTS runtime_events (
                 session_id TEXT NOT NULL,
                 event_id TEXT NOT NULL,
                 revision TEXT NOT NULL,
                 event_json TEXT NOT NULL,
                 PRIMARY KEY (session_id, event_id)
             );

             CREATE TABLE IF NOT EXISTS store_metadata (
                 key TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );

             CREATE TABLE IF NOT EXISTS chrono_playtest_sessions (
                 session_id TEXT PRIMARY KEY,
                 project_id TEXT NOT NULL,
                 pinned_project_revision INTEGER NOT NULL,
                 scenario_id TEXT NOT NULL,
                 start_location_id TEXT NOT NULL,
                 start_storylet_id TEXT NOT NULL,
                 start_node_guid INTEGER NOT NULL,
                 seed INTEGER NOT NULL,
                 pinned_id_space_json TEXT NOT NULL,
                 global_call_history_json TEXT NOT NULL,
                 scoped_call_histories_json TEXT NOT NULL,
                 pending_state_json TEXT NOT NULL DEFAULT '{}',
                 last_idempotency_key TEXT NOT NULL DEFAULT '',
                 last_result_json TEXT NOT NULL DEFAULT '',
                 mutation_revision INTEGER NOT NULL DEFAULT 0,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );",
        )?;

        // Migration: mutation retries must return the original payload, and
        // optimistic concurrency needs a per-session mutation counter.
        for (column, declaration) in [
            ("last_idempotency_key", "TEXT NOT NULL DEFAULT ''"),
            ("last_result_json", "TEXT NOT NULL DEFAULT ''"),
            ("mutation_revision", "INTEGER NOT NULL DEFAULT 0"),
        ] {
            let present: bool = {
                let mut stmt = self
                    .conn
                    .prepare("PRAGMA table_info(chrono_playtest_sessions)")?;
                let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
                let mut found = false;
                for col in rows {
                    if col? == column {
                        found = true;
                        break;
                    }
                }
                found
            };
            if !present {
                let tx = self.conn.unchecked_transaction()?;
                tx.execute(
                    &format!(
                        "ALTER TABLE chrono_playtest_sessions ADD COLUMN {column} {declaration}"
                    ),
                    [],
                )?;
                tx.commit()?;
            }
        }

        // Migration: pending latent interactions are persisted alongside the
        // call history so a rebuilt session can resume a paused Choice, QBN,
        // Waiter, or scene section.
        let has_pending_state: bool = {
            let mut stmt = self
                .conn
                .prepare("PRAGMA table_info(chrono_playtest_sessions)")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
            let mut found = false;
            for col in rows {
                if col? == "pending_state_json" {
                    found = true;
                    break;
                }
            }
            found
        };
        if !has_pending_state {
            let tx = self.conn.unchecked_transaction()?;
            tx.execute(
                "ALTER TABLE chrono_playtest_sessions ADD COLUMN pending_state_json TEXT NOT NULL DEFAULT '{}'",
                [],
            )?;
            tx.commit()?;
        }

        // Migration: add is_stale to interaction_context if absent in existing DBs
        let has_is_stale: bool = {
            let mut stmt = self
                .conn
                .prepare("PRAGMA table_info(interaction_context)")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
            let mut found = false;
            for col in rows {
                if col? == "is_stale" {
                    found = true;
                    break;
                }
            }
            found
        };
        if !has_is_stale {
            let tx = self.conn.unchecked_transaction()?;
            tx.execute(
                "ALTER TABLE interaction_context ADD COLUMN is_stale INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
            tx.commit()?;
        }
        let exists: bool = self
            .conn
            .query_row(
                "SELECT 1 FROM project_metadata WHERE singleton = 1",
                [],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);

        if !exists {
            self.conn.execute(
                "INSERT INTO project_metadata (singleton, project_id, name, revision) VALUES (1, 'workspace-project', 'Workspace', 0)",
                [],
            )?;
        }

        Ok(())
    }

    // --- Project Metadata ---

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

    // --- Generic Content Records ---

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
        Ok(())
    }

    pub fn delete_content(&self, entity: &str, id: &str) -> Result<bool, rusqlite::Error> {
        let count = self.conn.execute(
            "DELETE FROM content_records WHERE entity = ?1 AND id = ?2",
            params![entity, id],
        )?;
        if count > 0 {
            self.bump_project_revision()?;
        }
        Ok(count > 0)
    }

    // --- Typed Entity Helpers ---

    pub fn list_qualities(&self) -> Result<Vec<QualityRecord>, rusqlite::Error> {
        let json_list = self.list_content_json("qualities")?;
        Ok(json_list
            .into_iter()
            .filter_map(|s| serde_json::from_str(&s).ok())
            .collect())
    }

    pub fn save_quality(&self, q: &QualityRecord) -> Result<(), rusqlite::Error> {
        let json = serde_json::to_string(q)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        self.save_content("qualities", &q.name, &json, 1, None)
    }

    pub fn list_characters(&self) -> Result<Vec<CharacterRecord>, rusqlite::Error> {
        let json_list = self.list_content_json("characters")?;
        Ok(json_list
            .into_iter()
            .filter_map(|s| serde_json::from_str(&s).ok())
            .collect())
    }

    pub fn save_character(&self, c: &CharacterRecord) -> Result<(), rusqlite::Error> {
        let json = serde_json::to_string(c)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        self.save_content("characters", &c.id, &json, 1, None)
    }

    pub fn list_locations(&self) -> Result<Vec<LocationRecord>, rusqlite::Error> {
        let json_list = self.list_content_json("locations")?;
        Ok(json_list
            .into_iter()
            .filter_map(|s| serde_json::from_str(&s).ok())
            .collect())
    }

    pub fn save_location(&self, l: &LocationRecord) -> Result<(), rusqlite::Error> {
        let json = serde_json::to_string(l)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        self.save_content("locations", &l.id, &json, 1, None)
    }

    pub fn list_npc_behaviors(&self) -> Result<Vec<NpcBehaviorRecord>, rusqlite::Error> {
        let json_list = self.list_content_json("npc_behaviors")?;
        let mut list: Vec<NpcBehaviorRecord> = json_list
            .into_iter()
            .filter_map(|s| serde_json::from_str(&s).ok())
            .collect();
        list.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        Ok(list)
    }

    pub fn save_npc_behavior(&self, b: &NpcBehaviorRecord) -> Result<(), rusqlite::Error> {
        let json = serde_json::to_string(b)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        self.save_content("npc_behaviors", &b.id, &json, 1, None)
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
            Some(value) => serde_json::from_value::<NarrativeGraph>(value)
                .map_err(|error| format!("Invalid storylet graph: {error}"))?,
            None => NarrativeGraph::from_storylet(&st),
        };
        graph.validate().map_err(|issues| issues.join(" "))?;
        st.approved_revision = Some(expected_revision);
        self.save_storylet(&st).map_err(|e| e.to_string())?;
        Ok(Some(st))
    }

    // --- Library Paged Query ---

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
                } else if status_filter == "draft" {
                    if st.approved_revision == Some(st.revision) {
                        return false;
                    }
                }
                if !location_tag_filter.is_empty() && location_tag_filter != "all" {
                    if !st
                        .location_tags
                        .iter()
                        .any(|tag| tag == location_tag_filter)
                    {
                        return false;
                    }
                }
                if ending_filter == "ending" {
                    if !st.ending {
                        return false;
                    }
                } else if ending_filter == "non-ending" {
                    if st.ending {
                        return false;
                    }
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
            (total + page_size - 1) / page_size
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
                    excerpt,
                    location_tags: st.location_tags.clone(),
                    role_tags,
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

    // --- Studio Interaction Context ---

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

    // --- Transactional Project Content Replacement ---

    pub fn replace_project_content(
        &mut self,
        meta: &ProjectMetadata,
        content: &ProjectBundleContent,
    ) -> Result<u64, rusqlite::Error> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("DELETE FROM content_records", [])?;
        // A bundle can carry the revision it was exported at. Importing it must
        // still move the counter forward so live sessions see new content.
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
        for q in &content.qualities {
            let json = serde_json::to_string(q).unwrap();
            tx.execute(
                "INSERT INTO content_records (entity, id, data_json, revision, approved_revision, updated_at) VALUES ('qualities', ?1, ?2, 1, NULL, ?3)",
                params![q.name, json, now],
            )?;
        }
        for l in &content.locations {
            let json = serde_json::to_string(l).unwrap();
            tx.execute(
                "INSERT INTO content_records (entity, id, data_json, revision, approved_revision, updated_at) VALUES ('locations', ?1, ?2, 1, NULL, ?3)",
                params![l.id, json, now],
            )?;
        }
        for c in &content.characters {
            let json = serde_json::to_string(c).unwrap();
            tx.execute(
                "INSERT INTO content_records (entity, id, data_json, revision, approved_revision, updated_at) VALUES ('characters', ?1, ?2, 1, NULL, ?3)",
                params![c.id, json, now],
            )?;
        }
        for st in &content.storylets {
            let json = serde_json::to_string(st).unwrap();
            tx.execute(
                "INSERT INTO content_records (entity, id, data_json, revision, approved_revision, updated_at) VALUES ('storylets', ?1, ?2, ?3, ?4, ?5)",
                params![st.id, json, st.revision as i64, st.approved_revision.map(|r| r as i64), now],
            )?;
        }
        for b in &content.npc_behaviors {
            let json = serde_json::to_string(b).unwrap();
            tx.execute(
                "INSERT INTO content_records (entity, id, data_json, revision, approved_revision, updated_at) VALUES ('npc_behaviors', ?1, ?2, 1, NULL, ?3)",
                params![b.id, json, now],
            )?;
        }
        for e in &content.events {
            let json = serde_json::to_string(e).unwrap();
            tx.execute(
                "INSERT INTO content_records (entity, id, data_json, revision, approved_revision, updated_at) VALUES ('events', ?1, ?2, 1, NULL, ?3)",
                params![e.id, json, now],
            )?;
        }
        for s in &content.scenarios {
            let json = serde_json::to_string(s).unwrap();
            tx.execute(
                "INSERT INTO content_records (entity, id, data_json, revision, approved_revision, updated_at) VALUES ('scenarios', ?1, ?2, 1, NULL, ?3)",
                params![s.id, json, now],
            )?;
        }
        tx.commit()?;
        Ok(next_revision as u64)
    }

    /// ChronoFlow v5 playtest sessions (REQ-CH-02): only the call-history
    /// arrays, the random seed, and pinned authoring metadata are stored — never
    /// a serialized runtime instance or world snapshot. This table is additive;
    /// the legacy `runtime_sessions` rows are never read or rewritten by it.
    pub fn save_chrono_playtest_row(&self, row: &ChronoPlaytestRow) -> Result<(), rusqlite::Error> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO chrono_playtest_sessions (
                session_id, project_id, pinned_project_revision, scenario_id,
                start_location_id, start_storylet_id, start_node_guid, seed,
                pinned_id_space_json, global_call_history_json,
                scoped_call_histories_json, pending_state_json,
                last_idempotency_key, last_result_json, mutation_revision,
                created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
             ON CONFLICT(session_id) DO UPDATE SET
                pinned_project_revision = excluded.pinned_project_revision,
                pinned_id_space_json = excluded.pinned_id_space_json,
                global_call_history_json = excluded.global_call_history_json,
                scoped_call_histories_json = excluded.scoped_call_histories_json,
                pending_state_json = excluded.pending_state_json,
                last_idempotency_key = excluded.last_idempotency_key,
                last_result_json = excluded.last_result_json,
                mutation_revision = excluded.mutation_revision,
                updated_at = excluded.updated_at",
            params![
                row.session_id,
                row.project_id,
                row.pinned_project_revision,
                row.scenario_id,
                row.start_location_id,
                row.start_storylet_id,
                row.start_node_guid,
                row.seed,
                row.pinned_id_space_json,
                row.global_call_history_json,
                row.scoped_call_histories_json,
                row.pending_state_json,
                row.last_idempotency_key,
                row.last_result_json,
                row.mutation_revision as i64,
                row.created_at,
                row.updated_at
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn get_chrono_playtest_row(
        &self,
        session_id: &str,
    ) -> Result<Option<ChronoPlaytestRow>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, project_id, pinned_project_revision, scenario_id,
                start_location_id, start_storylet_id, start_node_guid, seed,
                pinned_id_space_json, global_call_history_json,
                scoped_call_histories_json, pending_state_json, last_idempotency_key, last_result_json, mutation_revision, created_at, updated_at
             FROM chrono_playtest_sessions WHERE session_id = ?1",
        )?;
        let row = stmt
            .query_row(params![session_id], chrono_playtest_row_from_row)
            .optional()?;
        Ok(row)
    }

    pub fn list_chrono_playtest_rows(&self) -> Result<Vec<ChronoPlaytestRow>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, project_id, pinned_project_revision, scenario_id,
                start_location_id, start_storylet_id, start_node_guid, seed,
                pinned_id_space_json, global_call_history_json,
                scoped_call_histories_json, pending_state_json, last_idempotency_key, last_result_json, mutation_revision, created_at, updated_at
             FROM chrono_playtest_sessions ORDER BY session_id",
        )?;
        let rows = stmt
            .query_map([], chrono_playtest_row_from_row)?
            .collect::<Result<Vec<_>, rusqlite::Error>>()?;
        Ok(rows)
    }
}

/// One persisted ChronoFlow playtest session. JSON columns hold the global call
/// history, the per-mission scoped call histories, and the pinned ID-space
/// fingerprint; the runtime is rebuilt from them on load (REQ-CH-03).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChronoPlaytestRow {
    pub session_id: String,
    pub project_id: String,
    pub pinned_project_revision: i64,
    pub scenario_id: String,
    pub start_location_id: String,
    pub start_storylet_id: String,
    pub start_node_guid: i64,
    pub seed: i64,
    pub pinned_id_space_json: String,
    pub global_call_history_json: String,
    pub scoped_call_histories_json: String,
    /// Latent node descriptors and active tokens, so a rebuilt session resumes
    /// a paused interaction instead of replaying to an idle graph.
    pub pending_state_json: String,
    pub last_idempotency_key: String,
    pub last_result_json: String,
    pub mutation_revision: u64,
    pub created_at: i64,
    pub updated_at: i64,
}

fn chrono_playtest_row_from_row(row: &rusqlite::Row) -> Result<ChronoPlaytestRow, rusqlite::Error> {
    Ok(ChronoPlaytestRow {
        session_id: row.get(0)?,
        project_id: row.get(1)?,
        pinned_project_revision: row.get(2)?,
        scenario_id: row.get(3)?,
        start_location_id: row.get(4)?,
        start_storylet_id: row.get(5)?,
        start_node_guid: row.get(6)?,
        seed: row.get(7)?,
        pinned_id_space_json: row.get(8)?,
        global_call_history_json: row.get(9)?,
        scoped_call_histories_json: row.get(10)?,
        pending_state_json: row.get(11)?,
        last_idempotency_key: row.get(12)?,
        last_result_json: row.get(13)?,
        mutation_revision: row.get::<_, i64>(14)? as u64,
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
    })
}

fn chrono_now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
