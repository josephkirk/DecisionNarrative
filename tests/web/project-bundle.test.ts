import { expect, test } from "bun:test";
import { inspectProjectBundle, makeProjectBundle, type ProjectBundleContent } from "../../src/web/projectBundle.ts";
import { StoryletRepository } from "../../src/db/repository.ts";

const content: ProjectBundleContent = {
  qualities: [{ name: "clock.pressure", type: "metric", value: 0, description: "Time until departure" }],
  locations: [{ id: "platform", name: "Platform", description: "An empty platform.", connected_zones: [] }],
  characters: [{
    id: "porter", name: "Porter", role: "Witness", home_location: "platform", current_location: "platform",
    status_axes: {}, needs: {}, agenda: "Keep the timetable", red_line: "Never abandon a passenger", schedule: { evening: "platform" },
  }],
  storylets: [{
    id: "arrival", revision: 3, approved_revision: 3, title: "Late arrival", location: "platform", group_tag: "arrival",
    cluster_type: "linear", urgency_weight: 40, repeatability: "always", cooldown_turns: 0, parameters: [],
    preconditions: { all_of: [] }, content: { text_template: "The porter checks the clock.", sceneOpening: "A train is late." },
    choices: [{ choice_id: "ask", label: "Ask about the delay", effects: {} }], effects: {}, created_at: 100,
  }],
  npcBehaviors: [],
  scenarios: [{ id: "before-train", name: "Before the train", player_start_location: "platform", initial_qualities: { "clock.pressure": 0 }, actor_locations: { porter: "platform" }, storylet_ids: ["arrival"], seed: 11 }],
  generationJobs: [{
    id: "generation-1", revision: 2, status: "ready", nextStageIndex: 1,
    request: { location: "platform", clusterType: "linear", characterRoles: [], triggerPreconditions: [], desiredOutcomes: {} },
    stages: [], attempts: {}, createdAt: 100, updatedAt: 101,
  }],
};

test("project bundle preserves authored records and reports a complete preview", () => {
  const bundle = makeProjectBundle({ id: "night-platform", name: "Night Platform", revision: 7 }, content);
  const inspected = inspectProjectBundle(JSON.parse(JSON.stringify(bundle)));
  expect(inspected.valid).toBe(true);
  expect(inspected.bundle).toEqual(bundle);
  expect(inspected.counts).toEqual({ qualities: 1, locations: 1, characters: 1, storylets: 1, npcBehaviors: 0, scenarios: 1, generationJobs: 1 });
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
  const empty: ProjectBundleContent = { qualities: [], locations: [], characters: [], storylets: [], npcBehaviors: [], scenarios: [], generationJobs: [] };
  const bundle = makeProjectBundle({ id: "new-project", name: "New Project", revision: 0 }, empty);
  expect(inspectProjectBundle(bundle)).toMatchObject({ valid: true, counts: { qualities: 0, locations: 0, characters: 0, storylets: 0, npcBehaviors: 0, scenarios: 0, generationJobs: 0 } });
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
    expect(repository.listGenerationJobs()).toEqual(content.generationJobs);

    expect(() => repository.replaceProjectContent(
      { id: "broken", name: "Broken", revision: 8 },
      { ...content, npcBehaviors: [{ id: "orphan-rule", actor_id: "missing-actor", priority: 0, action: "Wait", conditions: {}, effects: [] }] },
    )).toThrow();
    expect(repository.getProjectMetadata()).toEqual(metadata);
    expect(repository.listLocations().map((location) => location.id)).toEqual(["platform"]);
    expect(repository.getStorylet("arrival")).toMatchObject({ revision: 3, approved_revision: 3 });
    expect(repository.listGenerationJobs()).toEqual(content.generationJobs);
  } finally {
    repository.close();
  }
});

test("imported generation jobs cannot remain stuck in a running state", () => {
  const repository = new StoryletRepository(":memory:");
  try {
    const activeJob = { ...content.generationJobs[0]!, status: "running" as const, revision: 8 };
    repository.replaceProjectContent(
      { id: "night-platform", name: "Night Platform", revision: 1 },
      { ...content, generationJobs: [activeJob] },
    );
    expect(repository.getGenerationJob("generation-1")).toMatchObject({ status: "ready", revision: 9 });
  } finally {
    repository.close();
  }
});
