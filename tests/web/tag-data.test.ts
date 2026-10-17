import { expect, test } from "bun:test";
import { buildTagIndex, uniqueTagNames } from "../../src/web/studio/tagData.ts";

test("tag names trim and deduplicate case-insensitively while preserving first spelling", () => {
  expect(uniqueTagNames(["  Guard ", "guard", "", "Market", " market ", 1])).toEqual(["Guard", "Market"]);
});

test("tag index counts distinct entities and retains precise storylet jump anchors", () => {
  const tags = buildTagIndex({
    characters: [{ id: "char-1", name: "Ari", role: "Guard" }, { id: "char-2", name: "Bea", role: "guard" }],
    locations: [{ id: "loc-1", name: "Market", tags: ["Public", "public"] }],
    storylets: [
      { id: "scene-1", title: "At the market", location_tags: ["PUBLIC"], parameters: [{ param_name: "GUARD" }], graph: { nodes: [
        { id: "dialog-1", type: "dialog", title: "Arrival", speaker_role: "Guard", sections: [{ id: "sec-1", title: "Greeting", items: [
          { id: "line-1", kind: "line", speaker_role: "Guard", target_role: "player", emotion_tag: "Wary", action_tag: "Points" },
          { id: "idle-1", kind: "idle_action", actor_role: "Guard", target: "player", emote: "points" },
        ] }] },
      ] } },
      { id: "scene-2", title: "Second visit", location_tags: ["public"], parameters: [], graph: null },
    ],
  });
  const get = (kind: string, name: string) => tags.find((tag) => tag.kind === kind && tag.name.toLowerCase() === name.toLowerCase());
  expect(get("role", "guard")?.references).toHaveLength(3);
  expect(get("location", "public")?.references).toHaveLength(3);
  expect(get("emotion", "wary")?.references[0]).toMatchObject({ entityId: "scene-1", nodeId: "dialog-1", itemId: "line-1" });
  expect(get("action", "points")?.references).toHaveLength(1);
  expect(get("action", "points")?.references[0]).toMatchObject({ entityId: "scene-1", itemId: "line-1" });
});
