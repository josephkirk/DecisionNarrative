import { expect, test } from "bun:test";
import type { EntityRecord } from "../../src/web/studio/contracts.ts";
import { chooseTemplate, NO_TEMPLATE_ID, settleTemplatePicker, templatePicker, templatePickerItems } from "../../src/web/studio/view-models/templatePicker.ts";

const guard: EntityRecord = { id: "guard", object_id: "o", game_id: "g", template_id: null, is_template: true, name: "Guard", thumbnail: "guard.png", tags: [], properties: [], traits: [{ id: "i", template_id: "character", values: {} }] };
const traits = [{ id: "character", key: "system.character", name: "Character", properties: [] }];

test("picker lists None first, then each template with its trait labels", () => {
  expect(templatePickerItems([guard], traits)).toEqual([
    { id: NO_TEMPLATE_ID, title: "None — empty entity", subtitle: "Start without description, properties or traits" },
    { id: "guard", title: "Guard", subtitle: "Character", thumbnail: "guard.png" },
  ]);
});

test("picker resolves with the chosen id, None as the empty id and cancel as null", async () => {
  const items = templatePickerItems([guard], traits);
  const chosen = chooseTemplate(items, null);
  expect(templatePicker.open).toBe(true);
  settleTemplatePicker("guard");
  expect(await chosen).toBe("guard");
  expect(templatePicker.open).toBe(false);
  const none = chooseTemplate(items, null);
  settleTemplatePicker(NO_TEMPLATE_ID);
  expect(await none).toBe("");
  const cancelled = chooseTemplate(items, null);
  settleTemplatePicker(null);
  expect(await cancelled).toBeNull();
});

test("opening a second picker cancels the first", async () => {
  const first = chooseTemplate([], null);
  const second = chooseTemplate([], null);
  expect(await first).toBeNull();
  settleTemplatePicker("x");
  expect(await second).toBe("x");
});
