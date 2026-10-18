import { expect, test } from "bun:test";
import { inspectProjectBundle, makeProjectBundle, type ProjectBundleContent } from "../../src/web/projectBundle.ts";
import { StoryletRepository } from "../../src/db/repository.ts";
import { Database } from "bun:sqlite";
import { migrateLocationTagColumns } from "../../src/db/schema.ts";

const content: ProjectBundleContent = {
  qualities: [{ name: "clock.pressure", type: "metric", value: 0, description: "Time until departure" }],
  locations: [{ id: "platform", name: "Platform", description: "An empty platform.", tags: ["platform"], connected_zones: [] }],
  characters: [{
    id: "porter", name: "Porter", role: "Witness", home_location: "platform", current_location: "platform",
    status_axes: {}, needs: {}, agenda: "Keep the timetable", red_line: "Never abandon a passenger", schedule: { evening: "platform" },
  }],
  storylets: [{
    id: "arrival", revision: 3, approved_revision: 3, title: "Late arrival", location_tags: ["platform"], group_tag: "arrival",
    cluster_type: "linear", urgency_weight: 40, repeatability: "always", cooldown_turns: 0, parameters: [],
    preconditions: { all_of: [] }, content: { text_template: "The porter checks the clock.", sceneOpening: "A train is late." },
    choices: [{ choice_id: "ask", label: "Ask about the delay", effects: {} }], effects: {}, created_at: 100,
  }],
  npcBehaviors: [],
  scenarios: [{ id: "before-train", name: "Before the train", player_start_location: "platform", initial_qualities: { "clock.pressure": 0 }, actor_locations: { porter: "platform" }, storylet_ids: ["arrival"], seed: 11 }],
};

test("project bundle preserves authored records and reports a complete preview", () => {
  const bundle = makeProjectBundle({ id: "night-platform", name: "Night Platform", revision: 7 }, content);
  const inspected = inspectProjectBundle(JSON.parse(JSON.stringify(bundle)));
  expect(inspected.valid).toBe(true);
  expect(inspected.bundle).toEqual(bundle);
  expect(inspected.counts).toEqual({ qualities: 1, locations: 1, characters: 1, storylets: 1, npcBehaviors: 0, scenarios: 1 });
});

test("project bundle rejects unsupported versions and dangling references before import", () => {
  const bundle = makeProjectBundle({ id: "night-platform", name: "Night Platform", revision: 1 }, content);
  expect(inspectProjectBundle({ ...bundle, schema_version: 900 }).issues.join(" ").toLowerCase()).toContain("unsupported schema version");

  const invalid = structuredClone(bundle);
  invalid.content.scenarios[0]!.player_start_location = "missing-location";
  expect(inspectProjectBundle(invalid).issues.join(" ")).toContain("Unknown playtest start location");

  const malformed = structuredClone(bundle) as any;
  malformed.content.qualities = [null];
  expect(inspectProjectBundle(malformed).issues.join(" ")).toContain("has no stable ID");

  const invalidRevision = structuredClone(bundle);
  invalidRevision.content.storylets[0]!.approved_revision = 2;
  expect(inspectProjectBundle(invalidRevision).issues.join(" ")).toContain("invalid required fields or approval revision");
});

test("a new empty project is a valid portable bundle", () => {
  const empty: ProjectBundleContent = { qualities: [], locations: [], characters: [], storylets: [], npcBehaviors: [], scenarios: [] };
  const bundle = makeProjectBundle({ id: "new-project", name: "New Project", revision: 0 }, empty);
  expect(inspectProjectBundle(bundle)).toMatchObject({ valid: true, counts: { qualities: 0, locations: 0, characters: 0, storylets: 0, npcBehaviors: 0, scenarios: 0 } });
});

test("project replacement is atomic and preserves valid storylet approval revisions", () => {
  const repository = new StoryletRepository(":memory:");
  try {
    const metadata = { id: "night-platform", name: "Night Platform", revision: 7 };
    repository.saveLocation({ id: "old", name: "Old", description: "Keep until commit." });
    repository.replaceProjectContent(metadata, content);
    expect(repository.getProjectMetadata()).toEqual(metadata);
    expect(repository.listLocations().map((location) => location.id)).toEqual(["platform"]);
    expect(repository.getStorylet("arrival")).toMatchObject({ revision: 3, approved_revision: 3 });

    expect(() => repository.replaceProjectContent(
      { id: "broken", name: "Broken", revision: 8 },
      { ...content, npcBehaviors: [{ id: "orphan-rule", actor_id: "missing-actor", priority: 0, action: "Wait", conditions: {}, effects: [] }] },
    )).toThrow();
    expect(repository.getProjectMetadata()).toEqual(metadata);
    expect(repository.listLocations().map((location) => location.id)).toEqual(["platform"]);
    expect(repository.getStorylet("arrival")).toMatchObject({ revision: 3, approved_revision: 3 });
  } finally {
    repository.close();
  }
});



test("legacy location-bound storylets migrate to matching location tags", () => {
  const db = new Database(":memory:");
  try {
    db.run("CREATE TABLE locations (id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT NOT NULL, connected_zones TEXT NOT NULL, environmental_params TEXT NOT NULL, devices TEXT NOT NULL, created_at INTEGER NOT NULL)");
    db.run("CREATE TABLE storylets (id TEXT PRIMARY KEY, title TEXT NOT NULL, location TEXT, group_tag TEXT, cluster_type TEXT NOT NULL, urgency_weight REAL NOT NULL, repeatability TEXT NOT NULL, cooldown_turns INTEGER NOT NULL, parameters TEXT NOT NULL, preconditions TEXT NOT NULL, content TEXT NOT NULL, choices TEXT NOT NULL, effects TEXT NOT NULL, created_at INTEGER NOT NULL)");
    db.run("INSERT INTO locations VALUES (?, ?, ?, ?, ?, ?, ?)", ["archive", "Archive", "", "[]", "{}", "{}", 1]);
    db.run("INSERT INTO storylets VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)", ["legacy", "Legacy scene", "archive", null, "linear", 50, "always", 0, "[]", "{}", "{}", "[]", "{}", 1]);
    migrateLocationTagColumns(db);
    expect(JSON.parse((db.query("SELECT tags FROM locations WHERE id = ?").get("archive") as { tags: string }).tags)).toEqual(["archive"]);
    expect(JSON.parse((db.query("SELECT location_tags FROM storylets WHERE id = ?").get("legacy") as { location_tags: string }).location_tags)).toEqual(["archive"]);
  } finally {
    db.close();
  }
});
