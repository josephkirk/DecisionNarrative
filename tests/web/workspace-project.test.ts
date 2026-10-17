import { expect, test } from "bun:test";
import { buildWorkspaceProject } from "../../src/web/workspaceProject.ts";

test("Codex records map to a generic validated project and a fresh scenario", () => {
  const result = buildWorkspaceProject({
    qualities: [{ name: "clue.receipt", type: "tool", value: 0, description: "Receipt found" }],
    locations: [
      { id: "market", name: "Market", description: "A covered market", connected_zones: ["archive"] },
      { id: "archive", name: "Archive", description: "A night archive", connected_zones: ["market"] },
    ],
    characters: [{
      id: "nora", name: "Nora", role: "Clerk", home_location: "archive", current_location: "archive",
      status_axes: {}, needs: {}, agenda: "Protect the audit copy", red_line: "Never alter a signed receipt",
    }],
    storylets: [{
      revision: 2, approved_revision: 2,
      id: "find-receipt", title: "The Receipt", location: "archive", cluster_type: "player_controlled_passage",
      urgency_weight: 65, repeatability: "always", cooldown_turns: 0,
      parameters: [{ param_name: "clerk", query: "character.role == 'Clerk'" }],
      preconditions: { all_of: [{ variable: "clue.receipt", op: "eq", value: 0 }] },
      content: { text_template: "Nora unfolds a receipt." },
      choices: [{ choice_id: "read", label: "Read it", effects: { "clue.receipt": 1 } }],
      effects: {}, created_at: 1,
    }],
  }, "market");

  expect(result.issues).toEqual([]);
  expect(result.project.name).toBe("Workspace");
  expect(result.project.scenarios[result.scenarioId].player_start_location).toBe("market");
  expect(result.project.storylets["find-receipt"].required_roles).toEqual([{ role: "Clerk" }]);
  expect(result.project.storylets["find-receipt"].approved).toBe(true);
  expect(result.project.storylets["find-receipt"].choices[0].effects).toEqual([
    { kind: "add_quality", key: "clue.receipt", amount: 1 },
  ]);
});

test("unsupported role query and unknown references are surfaced as conversion errors", () => {
  const result = buildWorkspaceProject({
    qualities: [],
    locations: [{ id: "room", name: "Room", description: "", connected_zones: ["missing-place"] }],
    characters: [],
    storylets: [{
      revision: 1, approved_revision: 1,
      id: "unsupported", title: "Unsupported", location: "room", cluster_type: "linear", urgency_weight: 50,
      repeatability: "always", cooldown_turns: 0,
      parameters: [{ param_name: "role", query: "role matches a designer expression" }],
      preconditions: { all_of: [{ variable: "unknown.quality", op: "eq", value: true }] },
      content: { text_template: "Draft" }, choices: [], effects: {}, created_at: 1,
    }],
  });

  expect(result.issues.some((issue) => issue.includes("unsupported role query"))).toBe(true);
  expect(result.issues.some((issue) => issue.includes("unknown quality 'unknown.quality'"))).toBe(true);
  expect(result.issues.some((issue) => issue.includes("unknown connected location 'missing-place'"))).toBe(true);
});

test("unapproved storylet revisions are kept out of runtime bundles", () => {
  const result = buildWorkspaceProject({
    qualities: [],
    locations: [{ id: "room", name: "Room", description: "", connected_zones: [] }],
    characters: [],
    storylets: [{
      id: "draft", revision: 3, approved_revision: 2, title: "Draft", location: "room", cluster_type: "linear",
      urgency_weight: 50, repeatability: "always", cooldown_turns: 0, parameters: [], preconditions: {},
      content: { text_template: "Not approved yet." }, choices: [], effects: {}, created_at: 1,
    }],
  }, "room");

  expect(result.issues).toEqual([]);
  expect(result.project.storylets.draft).toBeUndefined();
  expect(result.project.scenarios[result.scenarioId].storylet_ids).not.toContain("draft");
});

test("authored NPC rules reach the Rust scenario with validated triggers and effects", () => {
  const result = buildWorkspaceProject({
    qualities: [{ name: "clue.read", type: "tool", value: 0 }],
    locations: [{ id: "hall", name: "Hall", description: "", connected_zones: [] }],
    characters: [{
      id: "mira", name: "Mira", role: "archivist", home_location: "hall", current_location: "hall",
      status_axes: {}, needs: {}, agenda: "Protect the record", red_line: "Never falsify it",
    }],
    storylets: [],
    npcBehaviors: [{
      id: "mira-secures-record", actor_id: "mira", priority: 7, action: "secure record",
      conditions: { all_of: [{ variable: "clue.read", op: "eq", value: 1 }] },
      effects: [{ kind: "set_quality", key: "clue.read", value: 2 }],
    }],
  }, "hall");

  expect(result.issues).toEqual([]);
  expect(result.project.npc_behaviors?.["mira-secures-record"]).toEqual({
    id: "mira-secures-record",
    actor_id: "mira",
    priority: 7,
    action: "secure record",
    conditions: { all_of: [{ fact: { kind: "quality", key: "clue.read" }, op: "eq", value: 1 }], any_of: [] },
    effects: [{ kind: "set_quality", key: "clue.read", value: 2 }],
  });
});

test("a saved scenario preserves its start state, cast positions, seed, and selected storylets", () => {
  const scenario = {
    id: "before-interview", name: "Before the interview", player_start_location: "archive",
    initial_qualities: { "clue.receipt": 1 }, actor_locations: { nora: "market" },
    storylet_ids: ["find-receipt"], seed: 321,
  };
  const result = buildWorkspaceProject({
    qualities: [{ name: "clue.receipt", type: "tool", value: 0 }],
    locations: [
      { id: "market", name: "Market", description: "", connected_zones: ["archive"] },
      { id: "archive", name: "Archive", description: "", connected_zones: ["market"] },
    ],
    characters: [{ id: "nora", name: "Nora", role: "Clerk", home_location: "archive", current_location: "archive", status_axes: {}, needs: {}, agenda: "", red_line: "" }],
    storylets: [{
      id: "find-receipt", revision: 1, approved_revision: 1, title: "The Receipt", location: "archive",
      cluster_type: "player_controlled_passage", urgency_weight: 50, repeatability: "always", cooldown_turns: 0,
      parameters: [{ param_name: "clerk", query: "role == 'Clerk'" }], preconditions: {},
      content: { text_template: "Nora finds a receipt." }, choices: [], effects: {}, created_at: 1,
    }],
  }, { scenario });

  expect(result.issues).toEqual([]);
  expect(result.scenarioId).toBe(scenario.id);
  expect(result.project.scenarios[scenario.id]).toMatchObject({
    name: scenario.name, player_start_location: "archive", initial_qualities: scenario.initial_qualities,
    actor_locations: scenario.actor_locations, storylet_ids: scenario.storylet_ids, seed: 321,
  });
});

test("invalid saved scenario references are reported before runtime creation", () => {
  const result = buildWorkspaceProject({
    qualities: [], locations: [{ id: "room", name: "Room", description: "", connected_zones: [] }],
    characters: [], storylets: [],
  }, { scenario: {
    id: "broken", name: "Broken", player_start_location: "missing", initial_qualities: { missing: 1 },
    actor_locations: { absent: "missing" }, storylet_ids: ["draft-storylet"], seed: -1,
  } });

  expect(result.issues.some((issue) => issue.includes("Unknown playtest start location"))).toBe(true);
  expect(result.issues.some((issue) => issue.includes("unknown quality"))).toBe(true);
  expect(result.issues.some((issue) => issue.includes("unknown character"))).toBe(true);
  expect(result.issues.some((issue) => issue.includes("unknown location 'missing'"))).toBe(true);
  expect(result.issues.some((issue) => issue.includes("missing, invalid, or unapproved"))).toBe(true);
  expect(result.issues.some((issue) => issue.includes("nonnegative safe integer"))).toBe(true);
});


test("deterministic repeatability: identical seed produces identical resolved start location and actors", () => {
  const records = {
    qualities: [],
    locations: [
      { id: "loc-a", name: "Location A", description: "", connected_zones: ["loc-b", "loc-c"], environmental_params: { zone: "indoor", access: 1 } },
      { id: "loc-b", name: "Location B", description: "", connected_zones: ["loc-a"], environmental_params: { zone: "outdoor", access: 2 } },
      { id: "loc-c", name: "Location C", description: "", connected_zones: ["loc-a"], environmental_params: { zone: "indoor", access: 3 } },
      { id: "loc-d", name: "Location D", description: "", connected_zones: [], environmental_params: { zone: "outdoor", access: 1 } },
    ],
    characters: [
      { id: "c1", name: "Char 1", role: "Clerk", home_location: "loc-a", current_location: "loc-a", status_axes: { faction: "guild" }, needs: {}, agenda: "", red_line: "" },
      { id: "c2", name: "Char 2", role: "Guard", home_location: "loc-b", current_location: "loc-b", status_axes: { faction: "city" }, needs: {}, agenda: "", red_line: "" },
      { id: "c3", name: "Char 3", role: "Merchant", home_location: "loc-c", current_location: "loc-c", status_axes: { faction: "guild" }, needs: {}, agenda: "", red_line: "" },
      { id: "c4", name: "Char 4", role: "Clerk", home_location: "loc-d", current_location: "loc-d", status_axes: { faction: "city" }, needs: {}, agenda: "", red_line: "" },
    ],
    storylets: [],
  };

  const scenario1 = {
    id: "random-test",
    name: "Random Test",
    player_start_location: "loc-a",
    initial_qualities: {},
    actor_locations: {},
    storylet_ids: [],
    seed: 98765,
    start_location_policy: { mode: "random" as const },
    actor_population_policy: {
      mode: "random" as const,
      count: 2,
      location_policy: { mode: "random" as const },
    },
  };

  const res1 = buildWorkspaceProject(records, { scenario: scenario1 });
  const res2 = buildWorkspaceProject(records, { scenario: { ...scenario1 } });

  expect(res1.issues).toEqual([]);
  expect(res2.issues).toEqual([]);
  expect(res1.project.scenarios["random-test"].player_start_location)
    .toBe(res2.project.scenarios["random-test"].player_start_location);
  expect(res1.project.scenarios["random-test"].actor_locations)
    .toEqual(res2.project.scenarios["random-test"].actor_locations);
  expect(Object.keys(res1.project.scenarios["random-test"].actor_locations).length).toBe(2);
});

test("attribute matching: start location and actor population resolve matching entities", () => {
  const records = {
    qualities: [],
    locations: [
      { id: "loc-a", name: "Location A", description: "", connected_zones: [], environmental_params: { security_level: "High", is_open: true } },
      { id: "loc-b", name: "Location B", description: "", connected_zones: [], environmental_params: { security_level: "Low", is_open: false } },
    ],
    characters: [
      { id: "c1", name: "Alice", role: "Detective", home_location: "loc-a", current_location: "loc-a", status_axes: { rank: "Senior" }, needs: {}, agenda: "", red_line: "" },
      { id: "c2", name: "Bob", role: "Patrol", home_location: "loc-b", current_location: "loc-b", status_axes: { rank: "Junior" }, needs: {}, agenda: "", red_line: "" },
      { id: "c3", name: "Charlie", role: "Detective", home_location: "loc-a", current_location: "loc-a", status_axes: { rank: "Senior" }, needs: {}, agenda: "", red_line: "" },
    ],
    storylets: [],
  };

  const scenario = {
    id: "attr-test",
    name: "Attr Test",
    player_start_location: "loc-b",
    initial_qualities: {},
    actor_locations: {},
    storylet_ids: [],
    seed: 42,
    start_location_policy: {
      mode: "by_attribute" as const,
      attribute_key: "security_level",
      attribute_value: "High",
    },
    actor_population_policy: {
      mode: "by_attribute" as const,
      attribute_key: "rank",
      attribute_value: "Senior",
      location_policy: {
        mode: "by_attribute" as const,
        attribute_key: "security_level",
        attribute_value: "Low",
      },
    },
  };

  const res = buildWorkspaceProject(records, { scenario });
  expect(res.issues).toEqual([]);
  // Start location matches security_level == High -> loc-a
  expect(res.project.scenarios["attr-test"].player_start_location).toBe("loc-a");
  // Actors matching rank == Senior -> c1, c3 placed in security_level == Low -> loc-b
  expect(res.project.scenarios["attr-test"].actor_locations).toEqual({
    c1: "loc-b",
    c3: "loc-b",
  });
});

test("missing attribute matches and invalid selector config report validation errors", () => {
  const records = {
    qualities: [],
    locations: [{ id: "loc-a", name: "Loc A", description: "", connected_zones: [] }],
    characters: [{ id: "c1", name: "C1", role: "Role", home_location: "loc-a", current_location: "loc-a", status_axes: {}, needs: {}, agenda: "", red_line: "" }],
    storylets: [],
  };

  const brokenScenario = {
    id: "broken-attr",
    name: "Broken Attr",
    player_start_location: "loc-a",
    initial_qualities: {},
    actor_locations: {},
    storylet_ids: [],
    seed: 1,
    start_location_policy: {
      mode: "by_attribute" as const,
      attribute_key: "nonexistent",
      attribute_value: "val",
    },
    actor_population_policy: {
      mode: "random" as const,
      count: -5,
    },
  };

  const res = buildWorkspaceProject(records, { scenario: brokenScenario });
  expect(res.issues.some((issue) => issue.includes("no location matches attribute"))).toBe(true);
  expect(res.issues.some((issue) => issue.includes("non-negative integer count"))).toBe(true);
});

test("old scenario records without policies remain 100% backwards compatible", () => {
  const records = {
    qualities: [{ name: "gold", type: "currency", value: 10 }],
    locations: [{ id: "market", name: "Market", description: "", connected_zones: [] }],
    characters: [{ id: "vendor", name: "Vendor", role: "Merchant", home_location: "market", current_location: "market", status_axes: {}, needs: {}, agenda: "", red_line: "" }],
    storylets: [],
  };

  const legacyScenario = {
    id: "legacy",
    name: "Legacy Scenario",
    player_start_location: "market",
    initial_qualities: { gold: 50 },
    actor_locations: { vendor: "market" },
    storylet_ids: [],
    seed: 77,
  };

  const res = buildWorkspaceProject(records, { scenario: legacyScenario });
  expect(res.issues).toEqual([]);
  expect(res.project.scenarios["legacy"].player_start_location).toBe("market");
  expect(res.project.scenarios["legacy"].actor_locations).toEqual({ vendor: "market" });
  expect(res.project.scenarios["legacy"].initial_qualities).toEqual({ gold: 50 });
  expect(res.project.scenarios["legacy"].seed).toBe(77);
});
