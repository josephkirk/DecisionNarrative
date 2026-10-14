export function summarizeUnslopViolations(violations: string[]): {
  negativeParallelism: number;
  countdowns: number;
  selfAnsweredQuestions: number;
  buzzwords: number;
} {
  return {
    negativeParallelism: violations.filter((item) => item.includes("negative_parallelism")).length,
    countdowns: violations.filter((item) => item.includes("dramatic_countdown")).length,
    selfAnsweredQuestions: violations.filter((item) => item.includes("self_answered_question")).length,
    buzzwords: violations.filter((item) => item.startsWith("banned_word:")).length,
  };
}
