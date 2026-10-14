import { expect, test } from "bun:test";
import { auditUnslopCompliance } from "../../src/agents/UnslopStoryletPipeline.ts";
import { summarizeUnslopViolations } from "../../src/agents/UnslopAudit.ts";

test("prose audit reports style violations without changing the designer's text", () => {
  const source = "The clerk quietly unfolds a tapestry of receipts.";
  const result = auditUnslopCompliance(source);

  expect(result.violations).toContain("banned_word: 'quietly'");
  expect(result.text).toBe(source);
});

test("style findings summary counts pattern families and banned words", () => {
  expect(summarizeUnslopViolations([
    "banned_word: 'quietly'",
    "banned_word: 'tapestry'",
    "banned_pattern: negative_parallelism",
    "banned_pattern: dramatic_countdown",
  ])).toEqual({ negativeParallelism: 1, countdowns: 1, selfAnsweredQuestions: 0, buzzwords: 2 });
});
