import { describe, expect, test } from "vitest";
import { buildTreeItems, defaultExpandedKeys, flattenTree, itemRoute } from "../../src/web/studio/projectTree.ts";
const source = buildTreeItems({ projectName: "Demo", folders: [{ id: "f", name: "Templates", root: "entity_templates", parent_id: null }], entities: [{ id: "world", name: "World", root_tag: "world", tags: [] }], entityTemplates: [{ id: "guard", object_id: "o", game_id: "guard", game_id_auto: false, name: "Guard", root_tag: "character", description: "", folder_id: "f", tags: [], properties: [], traits: [] }], traitTemplates: [], assets: [], storylets: [] });
describe("entity template project tree", () => {
 test("uses entity_templates root and root tag detail", () => { const rows = flattenTree({ ...source, expanded: [...defaultExpandedKeys(), "folder:f"] }); expect(rows.some((r) => r.key === "item:entity_templates:guard")).toBe(true); expect(rows.find((r) => r.key === "item:entity_templates:guard")?.detail).toBe("Character"); });
 test("routes entity templates to editor", () => { expect(itemRoute("entity_templates", "guard")).toBe("/entity-templates/guard"); });
 test("places intrinsic World under Entities, not Entity Templates", () => { expect(source.items.find((item) => item.id === "world")?.root).toBe("entities"); expect(source.items.some((item) => item.root === "entity_templates" && item.id === "world")).toBe(false); expect(itemRoute("entities", "world")).toBe("/entities/world"); });
});
