import { describe, expect, test } from "bun:test";
import { appliedTemplates, hasTraitKey, newEntityTemplateDraft, newTraitInstance, traitEditProblem, traitLabels, uniqueTraitProblem } from "../../src/web/studio/entityModel.ts";
import { itemTemplate } from "../../src/web/studio/plugins/inventory/model.ts";
import type { TraitTemplateRecord } from "../../src/web/studio/contracts.ts";
const template: TraitTemplateRecord = { id: "t", key: "system.character", name: "Character", properties: [{ id: "p", key: "payload", name: "Payload", label: "Payload", type: "json", is_array: false, default: { value: 1 } }], allow_multiple: true };
describe("entity template model", () => {
 test("item and unique entity types cannot coexist", () => {
  const core = ["character", "location", "fact"].map(key => ({ id: `system.${key}`, key, name: key, system: true, unique: true, properties: [] }));
  const item = itemTemplate();
  expect(item.unique).toBe(true);
  const traits = core.slice(0, 2).map((trait, i) => ({ id: `${i}`, template_id: trait.id, values: {} }));
  expect(uniqueTraitProblem(traits, [...core, item])).not.toBeNull();
 });
 test("new entity templates start without traits and preserve identity fields", () => { const draft = newEntityTemplateDraft("folder"); expect(draft).toMatchObject({ folder_id: "folder", game_id: "", game_id_auto: true, traits: [] }); });
 test("stable repeated trait instances retain distinct IDs and JSON values", () => { const a = newTraitInstance(template); const b = newTraitInstance(template); expect(a.id).not.toBe(b.id); expect(a.values.payload).toEqual({ value: 1 }); const entity = { ...newEntityTemplateDraft(null), traits: [a, b] }; expect(appliedTemplates(entity, [template])).toHaveLength(2); expect(traitLabels(entity, [template])).toEqual(["Character"]); expect(hasTraitKey(entity, [template], "system.character")).toBe(true); });
 test("zero and multiple traits are supported and keys or IDs resolve", () => { const empty = newEntityTemplateDraft(null); expect(traitLabels(empty, [template])).toEqual([]); expect(hasTraitKey(empty, [template], "system.character")).toBe(false); expect(hasTraitKey({ traits: [{ id: "ti", template_id: "t", values: {} }] }, [template], "t")).toBe(true); });
 test("unique traits allow only one applied instance, including repeats of the same trait", () => {
  const unique: TraitTemplateRecord = { id: "unique", key: "unique", name: "Unique", unique: true, properties: [] };
  const one = [{ id: "one", template_id: unique.id, values: {} }];
  expect(uniqueTraitProblem(one, [unique])).toBeNull();
  expect(uniqueTraitProblem([...one, { id: "two", template_id: unique.id, values: {} }], [unique])).toBe("An entity can have only one unique trait.");
  expect(traitEditProblem([...one, { id: "other", template_id: "another-unique", values: {} }], [unique, { ...unique, id: "another-unique" }])).toBe("An entity can have only one unique trait.");
 });
 test("non-unique traits coexist while non-multiple traits still reject repeated instances", () => {
  const nonUnique: TraitTemplateRecord = { ...template, unique: false };
  const instances = [{ id: "one", template_id: "t", values: {} }, { id: "two", template_id: "t", values: {} }];
  expect(traitEditProblem(instances, [nonUnique])).toBeNull();
  expect(traitEditProblem(instances, [{ ...nonUnique, allow_multiple: false }])).toBe("The Character trait does not allow multiple instances.");
 });
});
