import { expect, test } from "bun:test";
import { StoryletRepository, type StoryletRecord } from "../../src/db/repository.ts";

const draft: StoryletRecord = {
  id: "receipt-scene", title: "The Receipt", graph: { nodes: [{ id: "start" }, { id: "end" }], edges: [{ id: "edge", from_node: "start", to_node: "end" }] }, location_tags: ["archive"], cluster_type: "linear",
  chrono: {
    display_priority: 12, urgency_tag: "Critical", is_repeatable: true, cooldown_ms: 500,
    preconditions: [], required_tags: ["arc.chapter_one"], required_any_tags: ["clue.receipt"],
    forbidden_tags: [], required_arc_phases: ["phase.investigation"], ideal_state_vector: { Trust: 0.75 },
    salience_bonus_tags: ["clue.receipt"], effects_on_complete: [],
  },
  urgency_weight: 50, repeatability: "always", cooldown_turns: 0, parameters: [],
  preconditions: {}, content: { text_template: "A receipt lies on the desk." }, choices: [], effects: {}, created_at: 1,
};

test("storylet approval is bound to a content revision and edits return it to draft", () => {
  const repository = new StoryletRepository(":memory:");
  try {
    repository.saveStorylet(draft);
    const first = repository.getStorylet(draft.id)!;
    expect(first.revision).toBe(1);
    expect(first.approved_revision).toBeNull();
    expect(first.graph).toEqual(draft.graph);
    expect(first.chrono).toEqual(draft.chrono);
    expect(repository.approveStorylet(first.id, first.revision!)).toBe(true);
    expect(repository.getStorylet(first.id)?.approved_revision).toBe(1);
    expect(repository.approveStorylet(first.id, 0)).toBe(false);

    repository.saveStorylet({ ...draft, content: { text_template: "A damp receipt lies on the desk." } });
    const edited = repository.getStorylet(first.id)!;
    expect(edited.revision).toBe(2);
    expect(edited.graph).toEqual(draft.graph);
    expect(edited.chrono).toEqual(draft.chrono);
    expect(edited.approved_revision).toBeNull();
  } finally {
    repository.close();
  }
});
