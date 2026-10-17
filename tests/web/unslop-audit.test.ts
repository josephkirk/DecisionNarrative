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


test("prose audit detects sentence-final participial clauses without flagging ordinary -ing words", () => {
  expect(auditUnslopCompliance("She took the ring.").violations).not.toContain("banned_pattern: participial_closer");
  expect(auditUnslopCompliance("The lock gave way, highlighting the danger.").violations).toContain("banned_pattern: participial_closer");
});

test("prose audit enforces em dash and sentence burstiness gates", () => {
  expect(auditUnslopCompliance("He waited—then left.").violations).toContain("banned_pattern: em_dash");
  expect(auditUnslopCompliance("The door opened. The door closed. The door opened. The door closed.").violations.some((item) => item.startsWith("sentence_length_cv_below_0.35:"))).toBe(true);
  expect(auditUnslopCompliance("Run. The cracked window leaked rain into the room all night.").violations.some((item) => item.startsWith("sentence_length_cv_below_0.35:"))).toBe(false);
});
