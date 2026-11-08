import { expect, test } from "bun:test";
import { buildHierarchicalTagTree, buildTagIndex, isValidGameplayTagSegment, renamedGameplayTagName, uniqueTagNames } from "../../src/web/studio/tagData.ts";

test("tag names deduplicate case-insensitively", () => {
  expect(uniqueTagNames([" Guard ", "guard", "", "Market", " market "])).toEqual(["Guard", "Market"]);
});

test("registered zero-reference tags remain in the catalog", () => {
  const tags = buildTagIndex({ gameplayTags: [{ id: "1", name: "world.unused", protected: false }], entities: [], entityTemplates: [], storylets: [], traitTemplates: [] });
  expect(tags).toContainEqual({ kind: "gameplay", name: "world.unused", references: [], registered: true, protected: false });
});

test("references use canonical names without entity prefixes and are unique per record", () => {
  const tags = buildTagIndex({
    gameplayTags: [],
    entities: [],
    entityTemplates: [{ id: "e1", object_id: "o", game_id: "game", name: "Ari", tags: ["world.garden"], properties: [], traits: [{ id: "t", template_id: "role", values: { role: "guard" } }] }],
    storylets: [{ id: "s1", title: "Scene", location_tags: ["world.garden"], graph: { nodes: [{ id: "n1", tag: "world.garden" }] } } as never],
    traitTemplates: [],
  });
  const garden = tags.find((tag) => tag.name === "world.garden");
  expect(garden?.references).toHaveLength(2);
  expect(garden?.references.every((ref) => ref.entityId !== "game.world.garden")).toBe(true);
  expect(tags.find((tag) => tag.name === "guard")?.references).toHaveLength(1);
  expect(tags.find((tag) => tag.name === "character")).toBeUndefined();
});

test("malformed paths are rejected and tree counts direct references only", () => {
  const tags = buildTagIndex({ gameplayTags: [{ id: "1", name: "world", protected: false }, { id: "2", name: "world.garden", protected: false }, { id: "3", name: "world..bad", protected: false }], entities: [], entityTemplates: [{ id: "e", object_id: "o", game_id: "g", name: "E", tags: ["world.garden"], properties: [], traits: [] }], storylets: [], traitTemplates: [] });
  expect(tags.some((tag) => tag.name.includes(".."))).toBe(false);
  const tree = buildHierarchicalTagTree(tags);
  expect(tree[0]?.directCount).toBe(0);
  expect(tree[0]?.children[0]?.directCount).toBe(1);
});

test("intrinsic entity gameplay tags are counted as references", () => {
  const tags = buildTagIndex({
    gameplayTags: [
      { id: "world", name: "world", protected: true },
      { id: "weather", name: "world.weather", protected: false },
    ],
    entities: [{ id: "world", name: "World", tags: ["world.weather"], properties: [], traits: [] }],
    entityTemplates: [],
    storylets: [],
    traitTemplates: [],
  });
  expect(tags.find((tag) => tag.name === "world")?.references).toEqual([]);
  expect(tags.find((tag) => tag.name === "world.weather")?.references[0]?.entityType).toBe("entity");
});

test("entity trait roles appear in tag references", () => {
  const tags = buildTagIndex({
    gameplayTags: [],
    entities: [{ id: "guard", name: "Guard", tags: [], properties: [], traits: [
      { id: "role", template_id: "system.character_settings", values: { role: "character.guard" } },
    ] }],
    entityTemplates: [], storylets: [], traitTemplates: [],
  });
  expect(tags.find((tag) => tag.name === "character.guard")?.references).toEqual([
    { entityId: "guard", entityType: "entity", entityLabel: "Guard", detail: "Character role" },
  ]);
});

test("tag segments have the same validation for create and rename", () => {
  expect(isValidGameplayTagSegment("custom_root")).toBe(true);
  expect(isValidGameplayTagSegment("custom-root")).toBe(false);
  expect(isValidGameplayTagSegment("bad name")).toBe(false);
  expect(renamedGameplayTagName("mystery.sample", "active")).toBe("mystery.active");
  expect(renamedGameplayTagName("mystery.sample", "a.b")).toBeNull();
  expect(renamedGameplayTagName("mystery.sample", "1bad")).toBeNull();
  expect(renamedGameplayTagName("mystery.sample", "")).toBeNull();
});
