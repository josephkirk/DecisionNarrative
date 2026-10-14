import { expect, test } from "bun:test";
import { StoryletRepository, type StoryletRecord } from "../../src/db/repository.ts";

const draft: StoryletRecord = {
  id: "receipt-scene", title: "The Receipt", location: "archive", cluster_type: "linear",
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
    expect(repository.approveStorylet(first.id, first.revision!)).toBe(true);
    expect(repository.getStorylet(first.id)?.approved_revision).toBe(1);
    expect(repository.approveStorylet(first.id, 0)).toBe(false);

    repository.saveStorylet({ ...draft, content: { text_template: "A damp receipt lies on the desk." } });
    const edited = repository.getStorylet(first.id)!;
    expect(edited.revision).toBe(2);
    expect(edited.approved_revision).toBeNull();
  } finally {
    repository.close();
  }
});
