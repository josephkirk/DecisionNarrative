import { describe, expect, test } from "bun:test";
import { appliedTemplates, hasTraitKey, buildEntityDraft, newTraitInstance, traitEditProblem, traitLabels, uniqueTraitProblem } from "../../src/web/studio/entityModel.ts";
import { itemTemplate } from "../../src/web/studio/plugins/inventory/model.ts";
import type { EntityRecord, TraitTemplateRecord } from "../../src/web/studio/contracts.ts";
const template: TraitTemplateRecord = { id: "t", key: "system.character", name: "Character", properties: [{ id: "p", key: "payload", name: "Payload", label: "Payload", type: "json", is_array: false, default: { value: 1 } }], allow_multiple: true };
describe("entity model", () => {
 test("item and unique entity types cannot coexist", () => {
  const core = ["character", "location", "fact"].map(key => ({ id: `system.${key}`, key, name: key, system: true, unique: true, properties: [] }));
  const item = itemTemplate();
  expect(item.unique).toBe(true);
  const traits = core.slice(0, 2).map((trait, i) => ({ id: `${i}`, template_id: trait.id, values: {} }));
  expect(uniqueTraitProblem(traits, [...core, item])).not.toBeNull();
 });
 test("an entity without a template starts empty with no template reference", () => {
  const draft = buildEntityDraft(null, "folder");
  expect(draft).toMatchObject({ id: "", folder_id: "folder", game_id: "", game_id_auto: true, template_id: null, is_template: false, name: "New Entity", description: "", thumbnail: null, tags: [], properties: [], traits: [] });
 });
 test("an entity created from a template copies its content, gets fresh trait ids and records template_id", () => {
  const source: EntityRecord = { id: "guard", object_id: "o", game_id: "guard", template_id: null, is_template: true, name: "Guard", description: "Watches the gate", thumbnail: "guard.png", folder_id: "elsewhere", tags: ["role.guard"], properties: [{ id: "p", key: "hp", name: "HP", type: "int", default: 5 }, { id: "q", key: "mood", name: "Mood", type: "string", value: "calm" }], traits: [{ id: "ti-1", template_id: "t", values: { payload: { value: 1 } } }] };
  const draft = buildEntityDraft(source, "folder");
  expect(draft).toMatchObject({ id: "", object_id: "", game_id: "", game_id_auto: true, template_id: "guard", is_template: false, name: "New Guard", description: "Watches the gate", thumbnail: "guard.png", folder_id: "folder", tags: ["role.guard"] });
  expect(draft.properties.map((property) => [property.key, property.value])).toEqual([["hp", 5], ["mood", "calm"]]);
  expect(draft.traits).toHaveLength(1);
  expect(draft.traits[0]!.id).not.toBe("ti-1");
  expect(draft.traits[0]).toMatchObject({ template_id: "t", values: { payload: { value: 1 } } });
  expect(buildEntityDraft(source, null).traits[0]!.id).not.toBe(draft.traits[0]!.id);
  draft.tags.push("x"); (draft.traits[0]!.values.payload as { value: number }).value = 2;
  expect(source.tags).toEqual(["role.guard"]); expect(source.traits[0]!.values.payload).toEqual({ value: 1 });
 });
 test("stable repeated trait instances retain distinct IDs and JSON values", () => { const a = newTraitInstance(template); const b = newTraitInstance(template); expect(a.id).not.toBe(b.id); expect(a.values.payload).toEqual({ value: 1 }); const entity = { ...buildEntityDraft(null, null), traits: [a, b] }; expect(appliedTemplates(entity, [template])).toHaveLength(2); expect(traitLabels(entity, [template])).toEqual(["Character"]); expect(hasTraitKey(entity, [template], "system.character")).toBe(true); });
 test("zero and multiple traits are supported and keys or IDs resolve", () => { const empty = buildEntityDraft(null, null); expect(traitLabels(empty, [template])).toEqual([]); expect(hasTraitKey(empty, [template], "system.character")).toBe(false); expect(hasTraitKey({ traits: [{ id: "ti", template_id: "t", values: {} }] }, [template], "t")).toBe(true); });
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
