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
