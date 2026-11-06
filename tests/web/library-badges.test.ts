import { expect, test } from "bun:test";
import { originBadge } from "../../src/web/studio/storyletModel.ts";

test("agent-origin drafts are marked until the current revision is approved", () => {
  expect(originBadge({ origin: "agent", revision: 2, approved_revision: null })).toBe("Agent draft");
  expect(originBadge({ origin: "agent", revision: 2, approved_revision: 1 })).toBe("Agent draft");
  expect(originBadge({ origin: "agent", revision: 2, approved_revision: 2 })).toBeNull();
  expect(originBadge({ origin: "human", revision: 1, approved_revision: null })).toBeNull();
});

test("window badges resolve slots to their labels", async () => {
  const { windowBadge } = await import("../../src/web/studio/storyletModel.ts");
  const labels = ["D1 morning", "D1 evening", "D2 morning", "D2 evening", "D3 morning", "D3 evening"];
  expect(windowBadge([4, 5], labels)).toBe("D3 morning, D3 evening");
  expect(windowBadge([], labels)).toBeNull();
  expect(windowBadge([9], labels)).toBe("Slot 10");
});

test("learn defaults honor the storylet visibility setting", async () => {
  const { learnNodeDefaults } = await import("../../src/web/studio/narrativeGraphTypes.ts");
  expect(learnNodeDefaults(undefined)).toEqual({ kind: "evidence", visibility: "private" });
  expect(learnNodeDefaults("co_located")).toEqual({ kind: "evidence", visibility: "co_located" });
});
