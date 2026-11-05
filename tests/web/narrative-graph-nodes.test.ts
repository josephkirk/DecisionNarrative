import { expect, test } from "bun:test";
import { graphNodeTypes } from "../../src/web/studio/narrativeGraphTypes";

test("knowledge nodes are available in the appropriate graph modes", () => {
  expect(graphNodeTypes("story")).toContain("learn");
  expect(graphNodeTypes("story")).toContain("knows");
  expect(graphNodeTypes("preconditions")).toContain("knows");
  expect(graphNodeTypes("preconditions")).not.toContain("learn");
});
