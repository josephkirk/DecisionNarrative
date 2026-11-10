import { describe, expect, test } from "bun:test";
import { groupTraitProperties, reorderProperties } from "../../src/web/studio/traitPropertyGroups.ts";

describe("trait property groups", () => {
  test("groups matching categories in first-seen order and keeps ungrouped properties together", () => {
    const rows = [{ key: "a", category: "Stats" }, { key: "b" }, { key: "c", category: "Combat" }, { key: "d", category: " Stats " }];
    expect(groupTraitProperties(rows).map(g => [g.category, g.properties.map(p => p.key)])).toEqual([["Stats", ["a", "d"]], ["", ["b"]], ["Combat", ["c"]]]);
  });
  test("moves a property without changing the others", () => {
    expect(reorderProperties(["a", "b", "c"], 0, 2)).toEqual(["b", "c", "a"]);
    expect(reorderProperties(["a", "b"], -1, 1)).toEqual(["a", "b"]);
  });
});
