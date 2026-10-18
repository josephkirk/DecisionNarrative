/**
 * Storylet Studio — SQLite content and local run schema
 * Strictly implements Storylet.md specifications:
 * - Quality economy taxonomy (Currencies, Metrics, Tools, Territory, Menaces)
 * - Atomic Storylet schema with clusters, preconditions, and dynamic query bindings
 * - Call Stack / History deterministic replay architecture
 * - Multi-actor population and localized topology
 */

import { Database } from "bun:sqlite";
import * as path from "node:path";
import * as fs from "node:fs";

export function migrateLocationTagColumns(db: Database): void {
  const locationColumns = db.prepare("PRAGMA table_info(locations)").all() as Array<{ name: string }>;
  if (!locationColumns.some((column) => column.name === "tags")) db.run("ALTER TABLE locations ADD COLUMN tags TEXT NOT NULL DEFAULT '[]'");

  const storyletColumns = db.prepare("PRAGMA table_info(storylets)").all() as Array<{ name: string }>;
  if (storyletColumns.some((column) => column.name === "location_tags")) return;
  db.run("ALTER TABLE storylets ADD COLUMN location_tags TEXT NOT NULL DEFAULT '[]'");
  const legacyStorylets = db.prepare("SELECT id, location FROM storylets WHERE location IS NOT NULL").all() as Array<{ id: string; location: string }>;
  const readLocationTags = db.prepare("SELECT tags FROM locations WHERE id = ?");
  const migrateLocationTags = db.prepare("UPDATE locations SET tags = ? WHERE id = ? AND tags = '[]'");
  const migrateStoryletTags = db.prepare("UPDATE storylets SET location_tags = ? WHERE id = ?");
  for (const storylet of legacyStorylets) {
    const location = readLocationTags.get(storylet.location) as { tags: string } | undefined;
    let tags: unknown;
    try { tags = JSON.parse(location?.tags ?? "[]"); } catch { tags = []; }
    const validTags = Array.isArray(tags) && tags.every((tag) => typeof tag === "string");
    const parsedTags = validTags ? tags as string[] : [];
    const locationTags = parsedTags.length ? parsedTags : [storylet.location];
    if (!parsedTags.length) migrateLocationTags.run(JSON.stringify(locationTags), storylet.location);
    migrateStoryletTags.run(JSON.stringify(locationTags), storylet.id);
  }
}

export function initializeDatabase(dbPath = "data/storylet_studio.db"): Database {
  const dir = path.dirname(dbPath);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }

  const db = new Database(dbPath);
  db.run("PRAGMA journal_mode = WAL;");
  db.run("PRAGMA foreign_keys = ON;");

  db.run(`
    CREATE TABLE IF NOT EXISTS project_metadata (
      singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
      project_id TEXT NOT NULL,
      name TEXT NOT NULL,
      revision INTEGER NOT NULL DEFAULT 0
    );
  `);

  // 1. Qualities table (The Symbolic Economy)
  db.run(`
    CREATE TABLE IF NOT EXISTS qualities (
      name TEXT PRIMARY KEY,
      type TEXT NOT NULL CHECK(type IN ('currency', 'metric', 'tool', 'territory', 'menace')),
      value REAL NOT NULL,
      description TEXT,
      metadata TEXT
    );
  `);

  // 2. Characters table
  db.run(`
    CREATE TABLE IF NOT EXISTS characters (
      id TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      role TEXT NOT NULL,
      home_location TEXT NOT NULL,
      current_location TEXT NOT NULL,
      status_axes TEXT NOT NULL,
      needs TEXT NOT NULL,
      agenda TEXT NOT NULL,
      red_line TEXT NOT NULL,
      schedule TEXT NOT NULL,
      qualities TEXT NOT NULL,
      created_at INTEGER NOT NULL
    );
  `);

  // Authored deterministic NPC behavior rules used by clean Rust playtests.
  db.run(`
    CREATE TABLE IF NOT EXISTS npc_behaviors (
      id TEXT PRIMARY KEY,
      actor_id TEXT NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
      priority INTEGER NOT NULL DEFAULT 0,
      conditions TEXT NOT NULL,
      action TEXT NOT NULL,
      effects TEXT NOT NULL
    );
  `);


  // Authored scenario starting states belong to the reusable project, not to a playtest run.
  db.run(`
    CREATE TABLE IF NOT EXISTS scenarios (
      id TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      scenario_json TEXT NOT NULL,
      updated_at INTEGER NOT NULL
    );
  `);

  // 3. Locations table
  db.run(`
    CREATE TABLE IF NOT EXISTS locations (
      id TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      description TEXT NOT NULL,
      tags TEXT NOT NULL DEFAULT '[]',
      connected_zones TEXT NOT NULL,
      environmental_params TEXT NOT NULL,
      devices TEXT NOT NULL,
      created_at INTEGER NOT NULL
    );
  `);

  // 4. Relationships table
  db.run(`
    CREATE TABLE IF NOT EXISTS relationships (
      source_id TEXT NOT NULL,
      target_id TEXT NOT NULL,
      trust REAL NOT NULL DEFAULT 0,
      fear REAL NOT NULL DEFAULT 0,
      respect REAL NOT NULL DEFAULT 0,
      obligation REAL NOT NULL DEFAULT 0,
      memories TEXT NOT NULL,
      PRIMARY KEY (source_id, target_id)
    );
  `);

  // 5. Storylets table (The Content Deck)
  db.run(`
    CREATE TABLE IF NOT EXISTS storylets (
      id TEXT PRIMARY KEY,
      title TEXT NOT NULL,
      location TEXT,
      location_tags TEXT NOT NULL DEFAULT '[]',
      group_tag TEXT,
      cluster_type TEXT NOT NULL CHECK(cluster_type IN ('linear', 'player_controlled_passage', 'wheel', 'aftermath')),
      urgency_weight REAL NOT NULL DEFAULT 50,
      repeatability TEXT NOT NULL CHECK(repeatability IN ('once_ever', 'always', 'cooldown', 'persistent')),
      cooldown_turns INTEGER NOT NULL DEFAULT 0,
      parameters TEXT NOT NULL,
      preconditions TEXT NOT NULL,
      content TEXT NOT NULL,
      choices TEXT NOT NULL,
      effects TEXT NOT NULL,
      created_at INTEGER NOT NULL,
      revision INTEGER NOT NULL DEFAULT 1,
      approved_revision INTEGER,
      ending INTEGER NOT NULL DEFAULT 0,
      graph TEXT
    );
  `);

  // Upgrade existing local databases without invalidating their current storylet data.
  migrateLocationTagColumns(db);
  const storyletColumns = db.prepare("PRAGMA table_info(storylets)").all() as Array<{ name: string }>;
  if (!storyletColumns.some((column) => column.name === "revision")) {
    db.run("ALTER TABLE storylets ADD COLUMN revision INTEGER NOT NULL DEFAULT 1");
  }
  if (!storyletColumns.some((column) => column.name === "approved_revision")) {
    db.run("ALTER TABLE storylets ADD COLUMN approved_revision INTEGER");
  }
  if (!storyletColumns.some((column) => column.name === "ending")) {
    db.run("ALTER TABLE storylets ADD COLUMN ending INTEGER NOT NULL DEFAULT 0");
  }
  if (!storyletColumns.some((column) => column.name === "graph")) {
    db.run("ALTER TABLE storylets ADD COLUMN graph TEXT");
  }

  // 6. Call History table (Deterministic Replay & Butterfly Rollback Stack)
  db.run(`
    CREATE TABLE IF NOT EXISTS call_history (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      timestamp INTEGER NOT NULL,
      shift INTEGER NOT NULL,
      storylet_id TEXT NOT NULL,
      choice_id TEXT,
      initiator_id TEXT NOT NULL,
      state_snapshot_before TEXT NOT NULL,
      state_snapshot_after TEXT NOT NULL,
      deltas TEXT NOT NULL,
      reverted INTEGER NOT NULL DEFAULT 0
    );
  `);

  // 7. World State Singleton Snapshot
  db.run(`
    CREATE TABLE IF NOT EXISTS world_state (
      id TEXT PRIMARY KEY,
      shift INTEGER NOT NULL,
      shift_name TEXT NOT NULL,
      player_location TEXT NOT NULL,
      resources TEXT NOT NULL,
      devices TEXT NOT NULL,
      tension_level REAL NOT NULL DEFAULT 0.5,
      target_tension_slope REAL NOT NULL DEFAULT 0.1,
      active_event TEXT NOT NULL,
      updated_at INTEGER NOT NULL
    );
  `);

  return db;
}
