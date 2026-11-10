import { expect, test } from "bun:test";
import type { EntityRecord, TraitTemplateRecord } from "../../src/web/studio/contracts.ts";
import { treeItemTemplate } from "../../src/web/studio/plugins/tree/model.ts";
import { setTreeColumnValue, treeColumnValue, treeItemColumns } from "../../src/web/studio/plugins/tree/listProperties.ts";

const template: TraitTemplateRecord = {
  id: "item-template", key: "tree_item", name: "TreeItem", properties: [
    { id: "level", key: "level", name: "Level", type: "int", default: 3 },
    { id: "entity", key: "entity", name: "Gameplay entity", type: "entity", default: null },
    { id: "json", key: "data", name: "Data", type: "json" },
  ],
};
const item: EntityRecord = {
  id: "item", name: "Item", tags: [],
  properties: [{ id: "p", key: "secret", name: "Secret", type: "string", value: "not a column" }],
  traits: [
    { id: "tree", template_id: template.id, values: { level: 0, entity: null } },
    { id: "other", template_id: "unrelated", values: { level: 9 } },
    { id: "graph", template_id: "tree_graph", values: { edges: ['["item","other","requires"]'] } },
  ],
};

test("built-in TreeItem fields provide gameplay data defaults", () => {
  const definitions = new Map(treeItemTemplate().properties.map((property) => [property.key, property]));
  expect(definitions.get("level")).toMatchObject({ type: "int", default: 0 });
  expect(definitions.get("cost")).toMatchObject({ type: "int", default: 0 });
  expect(definitions.get("cost_entity")).toMatchObject({ type: "entity", default: null });
  expect(definitions.get("gameplay_tags_add")).toMatchObject({ type: "tag", is_array: true, default: [] });
  expect(definitions.get("gameplay_tags_remove")).toMatchObject({ type: "tag", is_array: true, default: [] });
  expect(definitions.get("description")).toMatchObject({ type: "string", multiline: true });
  expect(definitions.get("asset")).toMatchObject({ type: "asset", default: null });
  expect(definitions.get("entity")).toMatchObject({ type: "entity", default: null });
});

test("columns use only TreeItem definitions and preserve explicit values and defaults", () => {
  expect(treeItemColumns(undefined)).toEqual([]);
  const columns = treeItemColumns(template);
  expect(columns.map((column) => column.property.key)).toEqual(["level", "entity", "data"]);
  expect(treeColumnValue(item, columns[0]!)).toBe(0);
  expect(treeColumnValue(item, columns[1]!)).toBeNull();
  expect(treeColumnValue(item, columns[2]!)).toEqual({});
  const missingValue = { ...item, traits: [{ id: "tree", template_id: template.id, values: {} }] };
  expect(treeColumnValue(missingValue, columns[0]!)).toBe(3);
});

test("editing a gameplay reference updates only the TreeItem instance", () => {
  const column = treeItemColumns(template)[1]!;
  const updated = setTreeColumnValue(item, column, "actual-skill");
  expect(updated.traits[0]!.values).toEqual({ level: 0, entity: "actual-skill" });
  expect(updated.traits[1]).toEqual(item.traits[1]);
  expect(updated.traits[2]!.values.edges).toEqual(['["item","other","requires"]']);
  expect(updated.properties).toEqual(item.properties);
  expect(item.traits[0]!.values.entity).toBeNull();
  expect(() => setTreeColumnValue({ ...item, traits: [] }, column, "actual-skill")).toThrow();
});
