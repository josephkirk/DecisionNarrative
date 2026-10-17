import { expect, test } from "bun:test";
import { StoryletRepository, type ScenarioRecord } from "../../src/db/repository.ts";

test("scenario records persist independently from disposable playtest sessions", () => {
  const repository = new StoryletRepository(":memory:");
  try {
    const scenario: ScenarioRecord = {
      id: "first-interview", name: "Before the interview", player_start_location: "archive",
      initial_qualities: { "clue.receipt": 1 }, actor_locations: { clerk: "market" },
      storylet_ids: ["find-receipt"], seed: 44,
    };
    repository.saveScenario(scenario);
    expect(repository.getScenario(scenario.id)).toEqual(scenario);
    expect(repository.listScenarios()).toEqual([scenario]);

    const revised = { ...scenario, seed: 45, initial_qualities: { "clue.receipt": 2 } };
    repository.saveScenario(revised);
    expect(repository.listScenarios()).toEqual([revised]);
    expect(repository.deleteScenario(scenario.id)).toBe(true);
    expect(repository.getScenario(scenario.id)).toBeNull();
  } finally {
    repository.close();
  }
});


test("scenario records persist and round-trip optional selection policies", () => {
  const repository = new StoryletRepository(":memory:");
  try {
    const scenario: ScenarioRecord = {
      id: "policy-test",
      name: "Policy Test",
      player_start_location: "archive",
      initial_qualities: {},
      actor_locations: {},
      storylet_ids: [],
      seed: 99,
      start_location_policy: {
        mode: "by_attribute",
        attribute_key: "security",
        attribute_value: "high",
      },
      actor_population_policy: {
        mode: "random",
        count: 3,
        location_policy: {
          mode: "random",
        },
      },
    };
    repository.saveScenario(scenario);
    expect(repository.getScenario(scenario.id)).toEqual(scenario);
    expect(repository.listScenarios()).toEqual([scenario]);
  } finally {
    repository.close();
  }
});
