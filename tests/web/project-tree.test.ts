import { describe, expect, test } from "bun:test";
import { buildTreeItems, defaultExpandedKeys, flattenTree, folderContents, itemRoute } from "../../src/web/studio/projectTree.ts";
const source = buildTreeItems({
  projectName: "Demo",
  folders: [{ id: "f", name: "Templates", root: "entity_templates", parent_id: null }],
  entities: [{ id: "world", name: "World", tags: [], traits: [] }],
  entityTemplates: [{ id: "guard", object_id: "o", game_id: "guard", game_id_auto: false, name: "Guard", description: "", folder_id: "f", tags: [], properties: [], traits: [{ id: "t", template_id: "character-trait", values: {} }] }],
  traitTemplates: [{ id: "character-trait", key: "system.character", name: "Character", properties: [] }],
  assets: [],
  storylets: [],
});
describe("entity template project tree", () => {
 test("shows applied trait labels and supports entities without traits", () => { const rows = flattenTree({ ...source, expanded: [...defaultExpandedKeys(), "folder:f"] }); expect(rows.some((r) => r.key === "item:entity_templates:guard")).toBe(true); expect(rows.find((r) => r.key === "item:entity_templates:guard")?.detail).toBe("Character"); expect(source.items.find((item) => item.id === "world")?.detail).toBe("No traits"); });
 test("routes entity templates to editor", () => { expect(itemRoute("entity_templates", "guard")).toBe("/entity-templates/guard"); });
 test("places intrinsic World under Entities, not Entity Templates", () => { expect(source.items.find((item) => item.id === "world")?.root).toBe("entities"); expect(source.items.some((item) => item.root === "entity_templates" && item.id === "world")).toBe(false); expect(itemRoute("entities", "world")).toBe("/entities/world"); });
 test("sorts numbered names naturally", () => {
  const template = (id: string, name: string) => ({ id, object_id: id, game_id: id, game_id_auto: false, name, description: "", folder_id: null, tags: [], properties: [], traits: [] });
  const numbered = buildTreeItems({ projectName: "Demo", folders: [], entities: [], entityTemplates: [template("c10", "Clerk 10"), template("c2", "clerk 2"), template("c1", "Clerk 1")], traitTemplates: [], assets: [], storylets: [] });
  const labels = flattenTree({ ...numbered, expanded: defaultExpandedKeys() }).filter((r) => r.key.startsWith("item:entity_templates:")).map((r) => r.label);
  expect(labels).toEqual(["Clerk 1", "clerk 2", "Clerk 10"]);
 });
});
