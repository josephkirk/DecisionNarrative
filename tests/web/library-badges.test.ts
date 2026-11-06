import { expect, test } from "bun:test";
import { originBadge } from "../../src/web/studio/storyletModel.ts";

test("agent-origin drafts are marked until the current revision is approved", () => {
  expect(originBadge({ origin: "agent", revision: 2, approved_revision: null })).toBe("Agent draft");
  expect(originBadge({ origin: "agent", revision: 2, approved_revision: 1 })).toBe("Agent draft");
  expect(originBadge({ origin: "agent", revision: 2, approved_revision: 2 })).toBeNull();
  expect(originBadge({ origin: "human", revision: 1, approved_revision: null })).toBeNull();
});
