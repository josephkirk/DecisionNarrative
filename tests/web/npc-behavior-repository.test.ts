import { expect, test } from "bun:test";
import { StoryletRepository } from "../../src/db/repository.ts";

test("NPC behavior rules are project data and are removed with their character", () => {
  const repository = new StoryletRepository(":memory:");
  try {
    repository.saveCharacter({
      id: "keeper", name: "Keeper", role: "Archivist", home_location: "archive", current_location: "archive",
      status_axes: {}, needs: {}, agenda: "Protect the records", red_line: "", schedule: {}, qualities: {},
    });
    const rule = {
      id: "keeper-reacts", actor_id: "keeper", priority: 3, action: "secure the file",
      conditions: { all_of: [{ variable: "clue.found", op: "eq", value: true }] },
      effects: [{ kind: "set_quality", key: "record.secured", value: true }],
    };
    repository.saveNpcBehavior(rule);

    expect(repository.listNpcBehaviors()).toEqual([rule]);
    expect(repository.deleteNpcBehavior(rule.id)).toBe(true);
    repository.saveNpcBehavior(rule);
    repository.deleteCharacter("keeper");
    expect(repository.listNpcBehaviors()).toEqual([]);
  } finally {
    repository.close();
  }
});
