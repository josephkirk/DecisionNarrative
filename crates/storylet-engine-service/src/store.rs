//! Single SQLite store implementation for DecisionNarrative Storylet Studio.

use std::path::Path;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::contracts::{
    CharacterRecord, EventRecord, GenerationJobRecord, LibraryPageResult, LibraryQuery,
    LibrarySummaryItem, LocationRecord, NpcBehaviorRecord, ProjectBundleContent, ProjectMetadata,
    QualityRecord, ScenarioRecord, StoryletRecord,
};

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

             CREATE TABLE IF NOT EXISTS generation_jobs (
                 id TEXT PRIMARY KEY,
                 revision INTEGER NOT NULL,
                 status TEXT NOT NULL,
                 job_json TEXT NOT NULL,
                 updated_at INTEGER NOT NULL
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
             );"
        )?;

        // Ensure default project metadata exists
        let exists: bool = self.conn.query_row(
            "SELECT 1 FROM project_metadata WHERE singleton = 1",
            [],
            |_| Ok(true),
        ).optional()?.unwrap_or(false);

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

    pub fn get_content_json(&self, entity: &str, id: &str) -> Result<Option<String>, rusqlite::Error> {
        self.conn.query_row(
            "SELECT data_json FROM content_records WHERE entity = ?1 AND id = ?2",
            params![entity, id],
            |row| row.get(0),
        ).optional()
    }

    pub fn list_content_json(&self, entity: &str) -> Result<Vec<String>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT data_json FROM content_records WHERE entity = ?1 ORDER BY id",
        )?;
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
        Ok(json_list.into_iter().filter_map(|s| serde_json::from_str(&s).ok()).collect())
    }

    pub fn save_quality(&self, q: &QualityRecord) -> Result<(), rusqlite::Error> {
        let json = serde_json::to_string(q).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        self.save_content("qualities", &q.name, &json, 1, None)
    }

    pub fn list_characters(&self) -> Result<Vec<CharacterRecord>, rusqlite::Error> {
        let json_list = self.list_content_json("characters")?;
        Ok(json_list.into_iter().filter_map(|s| serde_json::from_str(&s).ok()).collect())
    }

    pub fn save_character(&self, c: &CharacterRecord) -> Result<(), rusqlite::Error> {
        let json = serde_json::to_string(c).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        self.save_content("characters", &c.id, &json, 1, None)
    }

    pub fn list_locations(&self) -> Result<Vec<LocationRecord>, rusqlite::Error> {
        let json_list = self.list_content_json("locations")?;
        Ok(json_list.into_iter().filter_map(|s| serde_json::from_str(&s).ok()).collect())
    }

    pub fn save_location(&self, l: &LocationRecord) -> Result<(), rusqlite::Error> {
        let json = serde_json::to_string(l).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        self.save_content("locations", &l.id, &json, 1, None)
    }

    pub fn list_npc_behaviors(&self) -> Result<Vec<NpcBehaviorRecord>, rusqlite::Error> {
        let json_list = self.list_content_json("npc_behaviors")?;
        let mut list: Vec<NpcBehaviorRecord> = json_list.into_iter().filter_map(|s| serde_json::from_str(&s).ok()).collect();
        list.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        Ok(list)
    }

    pub fn save_npc_behavior(&self, b: &NpcBehaviorRecord) -> Result<(), rusqlite::Error> {
        let json = serde_json::to_string(b).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        self.save_content("npc_behaviors", &b.id, &json, 1, None)
    }

    pub fn list_events(&self) -> Result<Vec<EventRecord>, rusqlite::Error> {
        let json_list = self.list_content_json("events")?;
        Ok(json_list.into_iter().filter_map(|s| serde_json::from_str(&s).ok()).collect())
    }

    pub fn save_event(&self, e: &EventRecord) -> Result<(), rusqlite::Error> {
        let json = serde_json::to_string(e).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        self.save_content("events", &e.id, &json, 1, None)
    }

    pub fn list_scenarios(&self) -> Result<Vec<ScenarioRecord>, rusqlite::Error> {
        let json_list = self.list_content_json("scenarios")?;
        Ok(json_list.into_iter().filter_map(|s| serde_json::from_str(&s).ok()).collect())
    }

    pub fn save_scenario(&self, s: &ScenarioRecord) -> Result<(), rusqlite::Error> {
        let json = serde_json::to_string(s).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        self.save_content("scenarios", &s.id, &json, 1, None)
    }

    pub fn list_storylets(&self) -> Result<Vec<StoryletRecord>, rusqlite::Error> {
        let json_list = self.list_content_json("storylets")?;
        Ok(json_list.into_iter().filter_map(|s| serde_json::from_str(&s).ok()).collect())
    }

    pub fn get_storylet(&self, id: &str) -> Result<Option<StoryletRecord>, rusqlite::Error> {
        let json_opt = self.get_content_json("storylets", id)?;
        Ok(json_opt.and_then(|s| serde_json::from_str(&s).ok()))
    }

    pub fn save_storylet(&self, st: &StoryletRecord) -> Result<(), rusqlite::Error> {
        let json = serde_json::to_string(st).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        self.save_content("storylets", &st.id, &json, st.revision, st.approved_revision)
    }

    pub fn approve_storylet(&self, id: &str, expected_revision: u64) -> Result<Option<StoryletRecord>, String> {
        let Some(mut st) = self.get_storylet(id).map_err(|e| e.to_string())? else {
            return Ok(None);
        };
        if st.revision != expected_revision {
            return Err(format!(
                "Revision conflict: current storylet revision is {}, expected {}",
                st.revision, expected_revision
            ));
        }
        st.approved_revision = Some(expected_revision);
        self.save_storylet(&st).map_err(|e| e.to_string())?;
        Ok(Some(st))
    }

    // --- Library Paged Query ---

    pub fn query_library(&self, query: &LibraryQuery) -> Result<LibraryPageResult, rusqlite::Error> {
        let all_storylets = self.list_storylets()?;
        let mut available_locations_set = std::collections::BTreeSet::new();
        for st in &all_storylets {
            if let Some(ref loc) = st.location {
                if !loc.trim().is_empty() {
                    available_locations_set.insert(loc.clone());
                }
            }
        }
        let available_locations: Vec<String> = available_locations_set.into_iter().collect();

        let q_needle = query.q.as_deref().unwrap_or("").trim().to_lowercase();
        let status_filter = query.status.as_deref().unwrap_or("all").trim().to_lowercase();
        let location_filter = query.location_id.as_deref().unwrap_or("").trim();
        let ending_filter = query.ending.as_deref().unwrap_or("all").trim().to_lowercase();

        let mut filtered: Vec<&StoryletRecord> = all_storylets
            .iter()
            .filter(|st| {
                if !q_needle.is_empty() {
                    let title_match = st.title.to_lowercase().contains(&q_needle);
                    let text_match = st.content.text_template.to_lowercase().contains(&q_needle)
                        || st.content.scene_opening.as_deref().unwrap_or("").to_lowercase().contains(&q_needle);
                    let tag_match = st.group_tag.as_deref().unwrap_or("").to_lowercase().contains(&q_needle);
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
                if !location_filter.is_empty() && location_filter != "all" {
                    if st.location.as_deref().unwrap_or("") != location_filter {
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
            filtered.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| a.id.cmp(&b.id)));
        } else {
            filtered.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()).then_with(|| a.id.cmp(&b.id)));
        }

        let total = filtered.len();
        let page_size = query.page_size.unwrap_or(60).clamp(1, 100);
        let total_pages = if total == 0 { 1 } else { (total + page_size - 1) / page_size };
        let page = query.page.unwrap_or(1).clamp(1, total_pages);
        let start = (page - 1) * page_size;
        let paged: Vec<LibrarySummaryItem> = filtered
            .into_iter()
            .skip(start)
            .take(page_size)
            .map(|st| {
                let text = st.content.scene_opening.as_deref().unwrap_or(&st.content.text_template);
                let excerpt = text.lines().next().unwrap_or("").chars().take(140).collect::<String>();
                let role_tags = st.parameters.iter().map(|p| p.query.clone()).collect();
                LibrarySummaryItem {
                    id: st.id.clone(),
                    title: st.title.clone(),
                    excerpt,
                    location: st.location.clone(),
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
            available_locations,
        })
    }

    // --- Generation Jobs ---

    pub fn list_generation_jobs(&self) -> Result<Vec<GenerationJobRecord>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT job_json FROM generation_jobs ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let list: Vec<GenerationJobRecord> = rows
            .filter_map(|r| r.ok())
            .filter_map(|s| serde_json::from_str(&s).ok())
            .collect();
        Ok(list)
    }

    pub fn get_generation_job(&self, id: &str) -> Result<Option<GenerationJobRecord>, rusqlite::Error> {
        let json_opt: Option<String> = self.conn.query_row(
            "SELECT job_json FROM generation_jobs WHERE id = ?1",
            params![id],
            |row| row.get(0),
        ).optional()?;
        Ok(json_opt.and_then(|s| serde_json::from_str(&s).ok()))
    }

    pub fn save_generation_job(&self, job: &GenerationJobRecord) -> Result<(), rusqlite::Error> {
        let json = serde_json::to_string(job).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        self.conn.execute(
            "INSERT INTO generation_jobs (id, revision, status, job_json, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                 revision = excluded.revision,
                 status = excluded.status,
                 job_json = excluded.job_json,
                 updated_at = excluded.updated_at",
            params![job.id, job.revision as i64, job.status, json, job.updated_at],
        )?;
        Ok(())
    }

    pub fn delete_generation_job(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let count = self.conn.execute("DELETE FROM generation_jobs WHERE id = ?1", params![id])?;
        Ok(count > 0)
    }

    // --- Transactional Project Content Replacement ---

    pub fn replace_project_content(&mut self, meta: &ProjectMetadata, content: &ProjectBundleContent) -> Result<(), rusqlite::Error> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("DELETE FROM content_records", [])?;
        tx.execute("DELETE FROM generation_jobs", [])?;

        tx.execute(
            "UPDATE project_metadata SET project_id = ?1, name = ?2, revision = ?3 WHERE singleton = 1",
            params![meta.id, meta.name, meta.revision as i64],
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
        for j in &content.generation_jobs {
            let json = serde_json::to_string(j).unwrap();
            tx.execute(
                "INSERT INTO generation_jobs (id, revision, status, job_json, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![j.id, j.revision as i64, j.status, json, j.updated_at],
            )?;
        }

        tx.commit()?;
        Ok(())
    }
}

fn chrono_now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
