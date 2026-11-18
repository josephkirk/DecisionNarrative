import { describe, expect, test } from "bun:test";
import type { EntityRecord } from "../../src/web/studio/contracts.ts";
import { buildTreeItems, CONTENT_ROOTS, defaultExpandedKeys, flattenTree, folderContents, itemRoute } from "../../src/web/studio/projectTree.ts";
import { withTemplateFlag } from "../../src/web/studio/entityModel.ts";
const entity = (id: string, name: string, extra: Partial<EntityRecord> = {}): EntityRecord => ({ id, object_id: id, game_id: id, game_id_auto: false, template_id: null, is_template: false, name, description: "", folder_id: null, tags: [], properties: [], traits: [], ...extra });
const source = buildTreeItems({
  projectName: "Demo",
  folders: [{ id: "f", name: "Guards", root: "entities", parent_id: null }],
  entities: [
    entity("world", "World"),
    entity("guard", "Guard", { folder_id: "f", is_template: true, traits: [{ id: "t", template_id: "character-trait", values: {} }] }),
  ],
  traitTemplates: [{ id: "character-trait", key: "system.character", name: "Character", properties: [] }],
  assets: [],
  storylets: [],
});
describe("entity project tree", () => {
 test("shows applied trait labels and supports entities without traits", () => { const rows = flattenTree({ ...source, expanded: [...defaultExpandedKeys(), "folder:f"] }); expect(rows.some((r) => r.key === "item:entities:guard")).toBe(true); expect(rows.find((r) => r.key === "item:entities:guard")?.detail).toBe("Character"); expect(source.items.find((item) => item.id === "world")?.detail).toBe("No traits"); });
 test("has no separate template root and routes every entity to the entity editor", () => {
  expect(CONTENT_ROOTS.map((definition) => definition.root)).toEqual(["storylets", "entities", "trait_templates", "assets"]);
  const rows = flattenTree({ ...source, expanded: [...defaultExpandedKeys(), "folder:f"] });
  expect(rows.filter((row) => row.kind === "root").map((row) => row.label)).toEqual(["Storylets", "Entities", "Traits", "Assets"]);
  expect(itemRoute("entities", "guard")).toBe("/entities/guard");
 });
 test("marks template entities in their rows", () => {
  const rows = flattenTree({ ...source, expanded: [...defaultExpandedKeys(), "folder:f"] });
  expect(rows.find((row) => row.key === "item:entities:guard")?.isTemplate).toBe(true);
  expect(rows.find((row) => row.key === "item:entities:world")?.isTemplate).toBe(false);
 });
 test("toggling the template flag changes only is_template", () => {
  const guard = entity("guard", "Guard", { tags: ["a"] });
  expect(withTemplateFlag(guard, true)).toEqual({ ...guard, is_template: true });
  expect(withTemplateFlag(withTemplateFlag(guard, true), false)).toEqual(guard);
  expect(guard.is_template).toBe(false);
 });
 test("places intrinsic World under Entities", () => { expect(source.items.find((item) => item.id === "world")?.root).toBe("entities"); expect(itemRoute("entities", "world")).toBe("/entities/world"); });
 test("sorts numbered names naturally", () => {
  const numbered = buildTreeItems({ projectName: "Demo", folders: [], entities: [entity("c10", "Clerk 10"), entity("c2", "clerk 2"), entity("c1", "Clerk 1")], traitTemplates: [], assets: [], storylets: [] });
  const labels = flattenTree({ ...numbered, expanded: defaultExpandedKeys() }).filter((r) => r.key.startsWith("item:entities:")).map((r) => r.label);
  expect(labels).toEqual(["Clerk 1", "clerk 2", "Clerk 10"]);
 });
});
