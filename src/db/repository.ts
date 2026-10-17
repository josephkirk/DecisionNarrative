/**
 * KINESIS / ATROPOS — SQLite Data Access Repository
 * Provides atomic operations, call-history logging, and rollback mechanics.
 */

import type { Database } from "bun:sqlite";
import { initializeDatabase } from "./schema.ts";

export interface ProjectMetadata {
  id: string;
  name: string;
  revision: number;
}

export interface QualityRecord {
  name: string;
  type: "currency" | "metric" | "tool" | "territory" | "menace";
  value: number;
  description?: string;
  metadata?: Record<string, unknown>;
}

export interface StoryletRecord {
  id: string;
  revision?: number;
  approved_revision?: number | null;
  title: string;
  ending?: boolean;
  graph?: unknown;
  location_tags: string[];
  group_tag?: string;
  cluster_type: "linear" | "player_controlled_passage" | "wheel" | "aftermath";
  urgency_weight: number;
  repeatability: "once_ever" | "always" | "cooldown" | "persistent";
  cooldown_turns: number;
  parameters: Array<{ param_name: string; query: string }>;
  preconditions: {
    all_of?: Array<{ variable: string; op: string; value: unknown }>;
    any_of?: Array<{ variable: string; op: string; value: unknown }>;
  };
  content: {
    text_template: string;
    speaker_id?: string;
    sceneOpening?: string;
    dialogue_quips?: Array<{ speaker: string; text: string }>;
  };
  choices: Array<{
    choice_id: string;
    label: string;
    systemVerb?: string;
    conditions?: Array<{ variable: string; op: string; value: unknown }>;
    effects: Record<string, unknown>;
    divert_target?: string | null;
    advisorPreview?: {
      civilianTrustDelta: number;
      securityRisk: number;
      resourceCost: string;
      frictionDescription: string;
      suggestAlternativeTest?: string;
    };
  }>;
  effects: Record<string, unknown>;
  created_at: number;
}

export interface LocationRecord {
  id: string;
  name: string;
  description: string;
  tags: string[];
  connected_zones?: string[];
  environmental_params?: Record<string, unknown>;
  devices?: Record<string, unknown>;
  created_at?: number;
}

export type StartLocationMode = "fixed" | "random" | "by_attribute";

export interface StartLocationPolicy {
  mode: StartLocationMode;
  location_id?: string;
  attribute_key?: string;
  attribute_value?: unknown;
}

export type ActorPopulationMode = "all" | "random" | "by_attribute";
export type ActorLocationMode = "fixed" | "random" | "by_attribute";

export interface ActorLocationPolicy {
  mode: ActorLocationMode;
  location_id?: string;
  attribute_key?: string;
  attribute_value?: unknown;
}

export interface ActorPopulationPolicy {
  mode: ActorPopulationMode;
  count?: number;
  attribute_key?: string;
  attribute_value?: unknown;
  location_policy?: ActorLocationPolicy;
}

export interface ScenarioEventRecord {
  id: string;
  title: string;
  description: string;
  event_type: "canon" | "world";
  trigger: {
    all_of?: Array<{ variable: string; op: string; value: unknown }>;
    any_of?: Array<{ variable: string; op: string; value: unknown }>;
  };
  effects: Record<string, unknown>;
  delay_turns?: number;
  repeatable?: boolean;
}

export interface ScenarioRecord {
  id: string;
  name: string;
  player_start_location: string;
  initial_qualities: Record<string, unknown>;
  actor_locations: Record<string, string>;
  storylet_ids: string[];
  seed: number;
  start_location_policy?: StartLocationPolicy;
  actor_population_policy?: ActorPopulationPolicy;
  event_definitions?: ScenarioEventRecord[];
}

export interface CharacterRecord {
  id: string;
  name: string;
  role: string;
  home_location: string;
  current_location: string;
  status_axes: Record<string, unknown>;
  needs: Record<string, number>;
  agenda: string;
  red_line: string;
  schedule?: Record<string, string>;
  qualities?: Record<string, unknown>;
  created_at?: number;
}

export interface NpcBehaviorRecord {
  id: string;
  actor_id: string;
  priority: number;
  conditions: {
    all_of?: Array<{ variable: string; op: string; value: unknown }>;
    any_of?: Array<{ variable: string; op: string; value: unknown }>;
  };
  action: string;
  effects: Array<Record<string, unknown>>;
}

export interface CallHistoryEntry {
  id?: number;
  timestamp: number;
  shift: number;
  storylet_id: string;
  choice_id?: string | null;
  initiator_id: string;
  state_snapshot_before: Record<string, unknown>;
  state_snapshot_after: Record<string, unknown>;
  deltas: Record<string, unknown>;
  reverted?: number;
}

export class StoryletRepository {
  private db: Database;
  private storyletsCache: StoryletRecord[] | null = null;

  constructor(dbPath = "data/storylet_studio.db") {
    this.db = initializeDatabase(dbPath);
  }

  public close(): void {
    this.db.close();
  }

  public getProjectMetadata(): ProjectMetadata {
    const row = this.db.prepare("SELECT project_id, name, revision FROM project_metadata WHERE singleton = 1").get() as { project_id: string; name: string; revision: number } | undefined;
    return row
      ? { id: row.project_id, name: row.name, revision: row.revision }
      : { id: "workspace-project", name: "Workspace", revision: 0 };
  }

  public saveProjectMetadata(metadata: ProjectMetadata): void {
    this.db.prepare(`
      INSERT INTO project_metadata (singleton, project_id, name, revision)
      VALUES (1, $id, $name, $revision)
      ON CONFLICT(singleton) DO UPDATE SET
        project_id = excluded.project_id, name = excluded.name, revision = excluded.revision
    `).run({ $id: metadata.id, $name: metadata.name, $revision: metadata.revision });
  }

  private bumpProjectRevision(): void {
    this.db.prepare(`
      INSERT OR IGNORE INTO project_metadata (singleton, project_id, name, revision)
      VALUES (1, 'workspace-project', 'Workspace', 0)
    `).run();
    this.db.prepare("UPDATE project_metadata SET revision = revision + 1 WHERE singleton = 1").run();
  }

  /** Replace authored project content and metadata as one SQLite transaction. */
  public replaceProjectContent(
    metadata: ProjectMetadata,
    content: {
      qualities: QualityRecord[];
      locations: LocationRecord[];
      characters: CharacterRecord[];
      storylets: StoryletRecord[];
      npcBehaviors: NpcBehaviorRecord[];
      scenarios: ScenarioRecord[];
      generationJobs: Array<{ id: string; revision: number; status: string; updatedAt: number }>;
    },
  ): void {
    const replace = this.db.transaction(() => {
      for (const table of ["npc_behaviors", "storylets", "scenarios", "generation_jobs", "qualities", "characters", "locations"]) {
        this.db.prepare(`DELETE FROM ${table}`).run();
      }
      for (const item of content.qualities) this.saveQuality(item);
      for (const item of content.locations) this.saveLocation(item);
      for (const item of content.characters) this.saveCharacter(item);
      for (const item of content.storylets) {
        this.saveStorylet(item);
        const revision = Number.isSafeInteger(item.revision) && Number(item.revision) > 0 ? Number(item.revision) : 1;
        const approvedRevision = item.approved_revision === revision ? revision : null;
        this.db.prepare("UPDATE storylets SET revision = ?, approved_revision = ? WHERE id = ?")
          .run(revision, approvedRevision, item.id);
      }
      for (const item of content.npcBehaviors) this.saveNpcBehavior(item);
      for (const item of content.scenarios) this.saveScenario(item);
      for (const item of content.generationJobs) {
        const restored = item.status === "running"
          ? { ...item, status: "ready", revision: item.revision + 1, updatedAt: Date.now() }
          : item;
        this.saveGenerationJob(restored);
      }
      this.saveProjectMetadata(metadata);
    });
    replace();
    this.storyletsCache = null;
  }

  // --- Qualities ---
  public saveQuality(q: QualityRecord): void {
    const stmt = this.db.prepare(`
      INSERT INTO qualities (name, type, value, description, metadata)
      VALUES ($name, $type, $value, $description, $metadata)
      ON CONFLICT(name) DO UPDATE SET
        value = excluded.value,
        description = excluded.description,
        metadata = excluded.metadata;
    `);
    stmt.run({
      $name: q.name,
      $type: q.type,
      $value: q.value,
      $description: q.description ?? null,
      $metadata: q.metadata ? JSON.stringify(q.metadata) : null
    });
    this.bumpProjectRevision();
  }

  public getQuality(name: string): QualityRecord | null {
    const row = this.db.prepare("SELECT * FROM qualities WHERE name = ?").get(name) as any;
    if (!row) return null;
    return {
      name: row.name,
      type: row.type,
      value: row.value,
      description: row.description,
      metadata: row.metadata ? JSON.parse(row.metadata) : undefined
    };
  }

  public listQualities(): QualityRecord[] {
    const rows = this.db.prepare("SELECT * FROM qualities ORDER BY type, name").all() as any[];
    return rows.map((r) => ({
      name: r.name,
      type: r.type,
      value: r.value,
      description: r.description,
      metadata: r.metadata ? JSON.parse(r.metadata) : undefined
    }));
  }

  public mutateQuality(name: string, delta: number): number {
    const q = this.getQuality(name);
    const newVal = (q ? q.value : 0) + delta;
    this.saveQuality({
      name,
      type: q ? q.type : "metric",
      value: newVal,
      description: q?.description
    });
    return newVal;
  }

  // --- Locations ---
  public saveLocation(loc: LocationRecord): void {
    const stmt = this.db.prepare(`
      INSERT INTO locations (id, name, description, tags, connected_zones, environmental_params, devices, created_at)
      VALUES ($id, $name, $description, $tags, $connected_zones, $environmental_params, $devices, $created_at)
      ON CONFLICT(id) DO UPDATE SET
        name = excluded.name,
        description = excluded.description,
        tags = excluded.tags,
        connected_zones = excluded.connected_zones,
        environmental_params = excluded.environmental_params,
        devices = excluded.devices;
    `);
    stmt.run({
      $id: loc.id,
      $name: loc.name,
      $description: loc.description,
      $tags: JSON.stringify(loc.tags ?? []),
      $connected_zones: JSON.stringify(loc.connected_zones ?? []),
      $environmental_params: JSON.stringify(loc.environmental_params ?? {}),
      $devices: JSON.stringify(loc.devices ?? {}),
      $created_at: loc.created_at ?? Date.now()
    });
    this.bumpProjectRevision();
  }

  public getLocation(id: string): LocationRecord | null {
    const row = this.db.prepare("SELECT * FROM locations WHERE id = ?").get(id) as any;
    if (!row) return null;
    return {
      id: row.id,
      name: row.name,
      description: row.description,
      tags: JSON.parse(row.tags ?? "[]"),
      connected_zones: JSON.parse(row.connected_zones),
      environmental_params: JSON.parse(row.environmental_params),
      devices: JSON.parse(row.devices),
      created_at: row.created_at
    };
  }

  public listLocations(): LocationRecord[] {
    const rows = this.db.prepare("SELECT * FROM locations ORDER BY name").all() as any[];
    return rows.map((row) => ({
      id: row.id,
      name: row.name,
      description: row.description,
      tags: JSON.parse(row.tags ?? "[]"),
      connected_zones: JSON.parse(row.connected_zones),
      environmental_params: JSON.parse(row.environmental_params),
      devices: JSON.parse(row.devices),
      created_at: row.created_at
    }));
  }

  public deleteLocation(id: string): boolean {
    const res = this.db.prepare("DELETE FROM locations WHERE id = ?").run(id);
    if (res.changes > 0) this.bumpProjectRevision();
    return res.changes > 0;
  }

  // --- Characters ---
  public saveCharacter(char: CharacterRecord): void {
    const stmt = this.db.prepare(`
      INSERT INTO characters (
        id, name, role, home_location, current_location, status_axes,
        needs, agenda, red_line, schedule, qualities, created_at
      ) VALUES (
        $id, $name, $role, $home_location, $current_location, $status_axes,
        $needs, $agenda, $red_line, $schedule, $qualities, $created_at
      )
      ON CONFLICT(id) DO UPDATE SET
        name = excluded.name,
        role = excluded.role,
        home_location = excluded.home_location,
        current_location = excluded.current_location,
        status_axes = excluded.status_axes,
        needs = excluded.needs,
        agenda = excluded.agenda,
        red_line = excluded.red_line,
        schedule = excluded.schedule,
        qualities = excluded.qualities;
    `);
    stmt.run({
      $id: char.id,
      $name: char.name,
      $role: char.role,
      $home_location: char.home_location,
      $current_location: char.current_location,
      $status_axes: JSON.stringify(char.status_axes),
      $needs: JSON.stringify(char.needs),
      $agenda: char.agenda,
      $red_line: char.red_line,
      $schedule: JSON.stringify(char.schedule ?? {}),
      $qualities: JSON.stringify(char.qualities ?? {}),
      $created_at: char.created_at ?? Date.now()
    });
    this.bumpProjectRevision();
  }

  public getCharacter(id: string): CharacterRecord | null {
    const row = this.db.prepare("SELECT * FROM characters WHERE id = ?").get(id) as any;
    if (!row) return null;
    return {
      id: row.id,
      name: row.name,
      role: row.role,
      home_location: row.home_location,
      current_location: row.current_location,
      status_axes: JSON.parse(row.status_axes),
      needs: JSON.parse(row.needs),
      agenda: row.agenda,
      red_line: row.red_line,
      schedule: JSON.parse(row.schedule),
      qualities: JSON.parse(row.qualities),
      created_at: row.created_at
    };
  }

  public listCharacters(): CharacterRecord[] {
    const rows = this.db.prepare("SELECT * FROM characters ORDER BY name").all() as any[];
    return rows.map((row) => ({
      id: row.id,
      name: row.name,
      role: row.role,
      home_location: row.home_location,
      current_location: row.current_location,
      status_axes: JSON.parse(row.status_axes),
      needs: JSON.parse(row.needs),
      agenda: row.agenda,
      red_line: row.red_line,
      schedule: JSON.parse(row.schedule),
      qualities: JSON.parse(row.qualities),
      created_at: row.created_at
    }));
  }

  public deleteCharacter(id: string): boolean {
    const res = this.db.prepare("DELETE FROM characters WHERE id = ?").run(id);
    if (res.changes > 0) this.bumpProjectRevision();
    return res.changes > 0;
  }

  // --- Authored autonomous NPC rules ---
  public saveNpcBehavior(rule: NpcBehaviorRecord): void {
    this.db.prepare(`
      INSERT INTO npc_behaviors (id, actor_id, priority, conditions, action, effects)
      VALUES ($id, $actor_id, $priority, $conditions, $action, $effects)
      ON CONFLICT(id) DO UPDATE SET
        actor_id = excluded.actor_id,
        priority = excluded.priority,
        conditions = excluded.conditions,
        action = excluded.action,
        effects = excluded.effects
    `).run({
      $id: rule.id,
      $actor_id: rule.actor_id,
      $priority: rule.priority,
      $conditions: JSON.stringify(rule.conditions ?? {}),
      $action: rule.action,
      $effects: JSON.stringify(rule.effects ?? []),
    });
    this.bumpProjectRevision();
  }

  public listNpcBehaviors(): NpcBehaviorRecord[] {
    const rows = this.db.prepare("SELECT * FROM npc_behaviors ORDER BY priority DESC, id").all() as any[];
    return rows.map((row) => ({
      id: row.id,
      actor_id: row.actor_id,
      priority: row.priority,
      conditions: JSON.parse(row.conditions),
      action: row.action,
      effects: JSON.parse(row.effects),
    }));
  }

  public deleteNpcBehavior(id: string): boolean {
    const result = this.db.prepare("DELETE FROM npc_behaviors WHERE id = ?").run(id);
    if (result.changes > 0) this.bumpProjectRevision();
    return result.changes > 0;
  }

  public saveScenario(scenario: ScenarioRecord): void {
    this.db.prepare(`
      INSERT INTO scenarios (id, name, scenario_json, updated_at)
      VALUES ($id, $name, $scenario_json, $updated_at)
      ON CONFLICT(id) DO UPDATE SET
        name = excluded.name,
        scenario_json = excluded.scenario_json,
        updated_at = excluded.updated_at
    `).run({
      $id: scenario.id,
      $name: scenario.name,
      $scenario_json: JSON.stringify(scenario),
      $updated_at: Date.now(),
    });
    this.bumpProjectRevision();
  }

  public getScenario(id: string): ScenarioRecord | null {
    const row = this.db.prepare("SELECT scenario_json FROM scenarios WHERE id = ?").get(id) as { scenario_json: string } | undefined;
    return row ? JSON.parse(row.scenario_json) as ScenarioRecord : null;
  }

  public listScenarios(): ScenarioRecord[] {
    const rows = this.db.prepare("SELECT scenario_json FROM scenarios ORDER BY name COLLATE NOCASE, id").all() as Array<{ scenario_json: string }>;
    return rows.map((row) => JSON.parse(row.scenario_json) as ScenarioRecord);
  }

  public deleteScenario(id: string): boolean {
    const result = this.db.prepare("DELETE FROM scenarios WHERE id = ?").run(id);
    if (result.changes > 0) this.bumpProjectRevision();
    return result.changes > 0;
  }

  // --- Resumable storylet generation audit trail ---
  public saveGenerationJob(job: { id: string; revision: number; status: string; updatedAt: number }): void {
    this.db.prepare(`
      INSERT INTO generation_jobs (id, revision, status, job_json, updated_at)
      VALUES ($id, $revision, $status, $job_json, $updated_at)
      ON CONFLICT(id) DO UPDATE SET
        revision = excluded.revision,
        status = excluded.status,
        job_json = excluded.job_json,
        updated_at = excluded.updated_at
    `).run({
      $id: job.id,
      $revision: job.revision,
      $status: job.status,
      $job_json: JSON.stringify(job),
      $updated_at: job.updatedAt,
    });
    this.bumpProjectRevision();
  }

  public getGenerationJob<T>(id: string): T | null {
    const row = this.db.prepare("SELECT job_json FROM generation_jobs WHERE id = ?").get(id) as { job_json: string } | undefined;
    return row ? JSON.parse(row.job_json) as T : null;
  }

  public listGenerationJobs<T>(): T[] {
    const rows = this.db.prepare("SELECT job_json FROM generation_jobs ORDER BY updated_at DESC").all() as Array<{ job_json: string }>;
    return rows.map((row) => JSON.parse(row.job_json) as T);
  }

  public deleteGenerationJob(id: string): boolean {
    const result = this.db.prepare("DELETE FROM generation_jobs WHERE id = ?").run(id);
    if (result.changes > 0) this.bumpProjectRevision();
    return result.changes > 0;
  }

  // --- Storylets ---
  public saveStorylet(st: StoryletRecord): void {
    const existing = this.db.prepare("SELECT revision FROM storylets WHERE id = ?").get(st.id) as { revision: number } | undefined;
    const revision = existing ? Number(existing.revision) + 1 : 1;
    const stmt = this.db.prepare(`
      INSERT INTO storylets (
        id, title, location_tags, group_tag, cluster_type, urgency_weight,
        repeatability, cooldown_turns, parameters, preconditions, content,
        choices, effects, created_at, revision, approved_revision, ending, graph
      ) VALUES (
        $id, $title, $location_tags, $group_tag, $cluster_type, $urgency_weight,
        $repeatability, $cooldown_turns, $parameters, $preconditions, $content,
        $choices, $effects, $created_at, $revision, NULL, $ending, $graph
      )
      ON CONFLICT(id) DO UPDATE SET
        title = excluded.title,
        location_tags = excluded.location_tags,
        group_tag = excluded.group_tag,
        cluster_type = excluded.cluster_type,
        urgency_weight = excluded.urgency_weight,
        repeatability = excluded.repeatability,
        cooldown_turns = excluded.cooldown_turns,
        parameters = excluded.parameters,
        preconditions = excluded.preconditions,
        content = excluded.content,
        choices = excluded.choices,
        effects = excluded.effects,
        ending = excluded.ending,
        graph = excluded.graph,
        revision = excluded.revision,
        approved_revision = NULL;
    `);
    stmt.run({
      $id: st.id,
      $title: st.title,
      $location_tags: JSON.stringify(st.location_tags ?? []),
      $group_tag: st.group_tag ?? null,
      $cluster_type: st.cluster_type,
      $urgency_weight: st.urgency_weight,
      $repeatability: st.repeatability,
      $cooldown_turns: st.cooldown_turns,
      $parameters: JSON.stringify(st.parameters ?? []),
      $preconditions: JSON.stringify(st.preconditions),
      $content: JSON.stringify(st.content),
      $choices: JSON.stringify(st.choices),
      $effects: JSON.stringify(st.effects),
      $created_at: st.created_at ?? Date.now(),
      $revision: revision,
      $ending: st.ending === true ? 1 : 0,
      $graph: st.graph == null ? null : JSON.stringify(st.graph),
    });
    this.bumpProjectRevision();
    this.storyletsCache = null;
  }

  public approveStorylet(id: string, expectedRevision: number): boolean {
    const result = this.db.prepare(`
      UPDATE storylets SET approved_revision = revision
      WHERE id = $id AND revision = $revision
    `).run({ $id: id, $revision: expectedRevision });
    if (result.changes > 0) this.bumpProjectRevision();
    this.storyletsCache = null;
    return result.changes > 0;
  }

  public getStorylet(id: string): StoryletRecord | null {
    if (this.storyletsCache !== null) {
      const found = this.storyletsCache.find((s) => s.id === id);
      if (found) return found;
    }
    const row = this.db.prepare("SELECT * FROM storylets WHERE id = ?").get(id) as any;
    if (!row) return null;
    return this.hydrateStoryletRow(row);
  }
  public listStorylets(): StoryletRecord[] {
    if (this.storyletsCache !== null) {
      return this.storyletsCache;
    }
    const rows = this.db.prepare("SELECT * FROM storylets ORDER BY urgency_weight DESC, id").all() as any[];
    this.storyletsCache = rows.map((r) => this.hydrateStoryletRow(r));
    return this.storyletsCache;
  }

  public deleteStorylet(id: string): boolean {
    const res = this.db.prepare("DELETE FROM storylets WHERE id = ?").run(id);
    if (res.changes > 0) this.bumpProjectRevision();
    this.storyletsCache = null;
    return res.changes > 0;
  }

  private hydrateStoryletRow(row: any): StoryletRecord {
    return {
      id: row.id,
      revision: row.revision,
      approved_revision: row.approved_revision,
      ending: row.ending === 1,
      graph: row.graph == null ? undefined : JSON.parse(row.graph),
      title: row.title,
      location_tags: JSON.parse(row.location_tags ?? "[]"),
      group_tag: row.group_tag,
      cluster_type: row.cluster_type,
      urgency_weight: row.urgency_weight,
      repeatability: row.repeatability,
      cooldown_turns: row.cooldown_turns,
      parameters: JSON.parse(row.parameters),
      preconditions: JSON.parse(row.preconditions),
      content: JSON.parse(row.content),
      choices: JSON.parse(row.choices),
      effects: JSON.parse(row.effects),
      created_at: row.created_at
    };
  }

  // --- Call History & Deterministic Rollback ---
  public logCallHistory(entry: CallHistoryEntry): number {
    const stmt = this.db.prepare(`
      INSERT INTO call_history (
        timestamp, shift, storylet_id, choice_id, initiator_id,
        state_snapshot_before, state_snapshot_after, deltas, reverted
      ) VALUES (
        $timestamp, $shift, $storylet_id, $choice_id, $initiator_id,
        $state_snapshot_before, $state_snapshot_after, $deltas, 0
      );
    `);
    const res = stmt.run({
      $timestamp: entry.timestamp,
      $shift: entry.shift,
      $storylet_id: entry.storylet_id,
      $choice_id: entry.choice_id ?? null,
      $initiator_id: entry.initiator_id,
      $state_snapshot_before: JSON.stringify(entry.state_snapshot_before),
      $state_snapshot_after: JSON.stringify(entry.state_snapshot_after),
      $deltas: JSON.stringify(entry.deltas)
    });
    return Number(res.lastInsertRowid);
  }

  public getCallHistory(limit = 50): CallHistoryEntry[] {
    const rows = this.db.prepare(`
      SELECT * FROM call_history ORDER BY id DESC LIMIT ?
    `).all(limit) as any[];

    return rows.map((r) => ({
      id: r.id,
      timestamp: r.timestamp,
      shift: r.shift,
      storylet_id: r.storylet_id,
      choice_id: r.choice_id,
      initiator_id: r.initiator_id,
      state_snapshot_before: JSON.parse(r.state_snapshot_before),
      state_snapshot_after: JSON.parse(r.state_snapshot_after),
      deltas: JSON.parse(r.deltas),
      reverted: r.reverted
    }));
  }

  /**
   * Rolls back the last un-reverted call history transaction in a single frame.
   * Returns the restored snapshot state, or null if history stack is empty.
   */
  public rollbackLastCall(): Record<string, unknown> | null {
    const last = this.db.prepare(`
      SELECT * FROM call_history WHERE reverted = 0 ORDER BY id DESC LIMIT 1
    `).get() as any;

    if (!last) return null;

    // Mark as reverted
    this.db.prepare("UPDATE call_history SET reverted = 1 WHERE id = ?").run(last.id);

    // Return the prior state snapshot to be re-applied
    return JSON.parse(last.state_snapshot_before);
  }

}
