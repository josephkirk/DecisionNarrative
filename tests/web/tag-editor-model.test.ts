import { expect, test } from "bun:test";
import { addTagSelection, commitSingleTagEdit, nextTagSuggestionIndex, tagSuggestionQuery, tagSuggestions } from "../../src/web/studio/tagEditorModel.ts";

test("tag suggestions filter case-insensitively and omit already selected tags", () => {
  expect(tagSuggestions(["Market", "Market", "Harbor", "Forest"], ["market"], "AR")).toEqual(["Harbor"]);
});

test("focused single-tag fields show replacement options without matching the current value", () => {
  const query = tagSuggestionQuery("Guard", ["Guard"], false);
  expect(query).toBe("");
  expect(tagSuggestions(["Guard", "Scout", "Healer"], ["Guard"], query)).toEqual(["Scout", "Healer"]);
  expect(tagSuggestionQuery("sc", ["Guard"], false)).toBe("sc");
  expect(tagSuggestionQuery("Guard", ["Guard"], true)).toBe("Guard");
});

test("tag selection preserves canonical option spelling and uniqueness", () => {
  expect(addTagSelection(["Guard"], " guard ", ["GUARD", "Scout"], true)).toEqual(["Guard"]);
  expect(addTagSelection(["Guard"], " sCoUt ", ["Scout"], true)).toEqual(["Guard", "Scout"]);
  expect(addTagSelection(["Guard"], " scout ", ["Scout"], false)).toEqual(["Scout"]);
  expect(addTagSelection(["Guard"], "  ", [], true)).toEqual(["Guard"]);
});

test("single-tag blur preserves a committed choice and commits active edits", () => {
  expect(commitSingleTagEdit(["Scout"], "", false, ["Scout"])).toEqual(["Scout"]);
  expect(commitSingleTagEdit(["Scout"], "", true, ["Scout"])).toEqual([]);
  expect(commitSingleTagEdit(["Scout"], "Guard", true, ["Guard", "Scout"])).toEqual(["Guard"]);
});

test("tag suggestion navigation starts at either edge and wraps", () => {
  expect(nextTagSuggestionIndex(-1, 1, 3)).toBe(0);
  expect(nextTagSuggestionIndex(-1, -1, 3)).toBe(2);
  expect(nextTagSuggestionIndex(2, 1, 3)).toBe(0);
  expect(nextTagSuggestionIndex(0, -1, 3)).toBe(2);
  expect(nextTagSuggestionIndex(0, 1, 0)).toBe(-1);
});
