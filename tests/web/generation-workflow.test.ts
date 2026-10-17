import { expect, test } from "bun:test";
import {
  GENERATION_STAGES,
  MAX_PIPELINE_RETRIES,
  GenerationWorkflow,
  type GenerationJob,
  type GenerationJobStore,
} from "../../src/agents/GenerationWorkflow.ts";
import { UnslopStoryletPipeline } from "../../src/agents/UnslopStoryletPipeline.ts";
import { StoryletRepository } from "../../src/db/repository.ts";

class MemoryJobStore implements GenerationJobStore {
  private jobs = new Map<string, GenerationJob>();
  get(id: string) { const job = this.jobs.get(id); return job ? structuredClone(job) : null; }
  list() { return [...this.jobs.values()].map((job) => structuredClone(job)); }
  put(job: GenerationJob) { this.jobs.set(job.id, structuredClone(job)); }
  delete(id: string) { return this.jobs.delete(id); }
}

const request = {
  locationTags: ["archive"],
  clusterType: "linear" as const,
  characterRoles: ["archivist"],
  triggerPreconditions: [{ variable: "clue.open", op: "eq", value: true }],
  desiredOutcomes: { "clue.read": true },
  projectContext: { genre: "detective fiction" },
};

test("generation jobs resume stage by stage and require the current revision", async () => {
  const store = new MemoryJobStore();
  const calls: string[] = [];
  const workflow = new GenerationWorkflow(store, async (stage, _request, previous) => {
    calls.push(stage);
    return { artifact: { stage, previousCount: Object.keys(previous).length }, source: "test", model: "fixture", promptVersion: "test-v1" };
  });
  let job = workflow.create(request);
  for (const stage of GENERATION_STAGES) {
    job = await workflow.advance(job.id, job.revision);
    expect(job.stages.at(-1)?.stage).toBe(stage.id);
  }

  expect(job.status).toBe("complete");
  expect(job.stages).toHaveLength(9);
  expect(calls).toEqual(GENERATION_STAGES.map((stage) => stage.id));
  expect(job.stages[4].input.previous).toHaveProperty("scene_plan");
  await expect(workflow.advance(job.id, job.revision - 1)).rejects.toThrow("revision conflict");
});

test("generated stage records retain craft source hashes and versions", async () => {
  const reference = {
    id: "scene-craft" as const,
    version: "1.0.0" as const,
    source: ".agents/skills/scene-craft/SKILL.md",
    sourceHash: "a".repeat(64),
    promptHash: "b".repeat(64),
  };
  const workflow = new GenerationWorkflow(new MemoryJobStore(), async () => ({
    artifact: { seed: "A receipt folded around a key" },
    source: "strata",
    model: "fixture",
    promptVersion: `storylet-craft-v2+scene-craft@${reference.sourceHash}.${reference.promptHash}`,
    craftResources: [reference],
  }));
  const created = workflow.create(request);
  const advanced = await workflow.advance(created.id, created.revision);

  expect(advanced.stages[0].craftResources).toEqual([reference]);
});

test("editing a prior stage invalidates all later outputs and allows a clean resume", async () => {
  const workflow = new GenerationWorkflow(new MemoryJobStore(), async (stage) => ({
    artifact: { stage, edit: false }, source: "test", model: "fixture", promptVersion: "test-v1",
  }));
  let job = workflow.create(request);
  for (let i = 0; i < 4; i++) job = await workflow.advance(job.id, job.revision);
  job = workflow.editStage(job.id, "physical_seed", { seed: "A torn map under a glass weight" }, job.revision);

  expect(job.status).toBe("ready");
  expect(job.nextStageIndex).toBe(2);
  expect(job.stages.map((stage) => stage.stage)).toEqual(["canon_and_voice", "physical_seed"]);
  const resumed = await workflow.advance(job.id, job.revision);
  expect(resumed.stages.at(-1)?.stage).toBe("premise_options");
});

test("a failed provider gets two automatic retries and persists its failure state", async () => {
  const store = new MemoryJobStore();
  let attempts = 0;
  const workflow = new GenerationWorkflow(store, async () => {
    attempts++;
    throw new Error("model unavailable");
  });
  const job = workflow.create(request);

  await expect(workflow.advance(job.id, job.revision)).rejects.toThrow("stage attempt limit (3 attempts) reached");
  const failed = workflow.get(job.id);
  expect(attempts).toBe(3);
  expect(failed.status).toBe("failed");
  expect(failed.attempts.canon_and_voice).toBe(3);
  expect(failed.failure?.message).toBe("model unavailable");
});

test("generation enforces at most ten retries across all stages in one job", async () => {
  const perStageCalls = new Map<string, number>();
  let totalCalls = 0;
  const workflow = new GenerationWorkflow(new MemoryJobStore(), async (stage) => {
    totalCalls++;
    const calls = (perStageCalls.get(stage) ?? 0) + 1;
    perStageCalls.set(stage, calls);
    const stageIndex = GENERATION_STAGES.findIndex((item) => item.id === stage);
    if (stageIndex < 5 && calls === 3) return { artifact: { stage }, source: "test", model: "fixture", promptVersion: "test-v1" };
    throw new Error("fixture failure");
  });
  let job = workflow.create(request);
  for (let stage = 0; stage < 5; stage++) job = await workflow.advance(job.id, job.revision);

  await expect(workflow.advance(job.id, job.revision)).rejects.toThrow("pipeline retry limit (" + MAX_PIPELINE_RETRIES + ") reached");
  const failed = workflow.get(job.id);
  expect(failed.totalRetries).toBe(MAX_PIPELINE_RETRIES);
  expect(totalCalls).toBe(16);
  expect(failed.attempts[GENERATION_STAGES[5].id]).toBe(1);
  expect(failed.status).toBe("failed");
});

test("editing an earlier stage preserves the retry budget across workflow reload", async () => {
  const repository = new StoryletRepository(":memory:");
  try {
    const store: GenerationJobStore = {
      get: (id) => repository.getGenerationJob<GenerationJob>(id),
      list: () => repository.listGenerationJobs<GenerationJob>(),
      put: (job) => repository.saveGenerationJob(job),
      delete: (id) => repository.deleteGenerationJob(id),
    };
    const perStageCalls = new Map<string, number>();
    let totalCalls = 0;
    const initial = new GenerationWorkflow(store, async (stage) => {
      totalCalls++;
      const calls = (perStageCalls.get(stage) ?? 0) + 1;
      perStageCalls.set(stage, calls);
      const stageIndex = GENERATION_STAGES.findIndex((item) => item.id === stage);
      if (stageIndex < 5 && calls === 3) return { artifact: { stage }, source: "test", model: "fixture", promptVersion: "test-v1" };
      throw new Error("fixture failure");
    });
    let job = initial.create(request);
    for (let stage = 0; stage < 5; stage++) job = await initial.advance(job.id, job.revision);
    expect(job.totalRetries).toBe(MAX_PIPELINE_RETRIES);

    job = initial.editStage(job.id, "premise_options", { premise: "A changed premise" }, job.revision);
    expect(job.totalRetries).toBe(MAX_PIPELINE_RETRIES);
    expect(job.attempts.scene_plan).toBeUndefined();

    const resumed = new GenerationWorkflow(store, async () => {
      totalCalls++;
      throw new Error("edited stage failure");
    });
    await expect(resumed.advance(job.id, job.revision)).rejects.toThrow("pipeline retry limit (" + MAX_PIPELINE_RETRIES + ") reached");
    const failed = resumed.get(job.id);
    expect(failed.totalRetries).toBe(MAX_PIPELINE_RETRIES);
    expect(failed.attempts.scene_plan).toBe(1);
    expect(totalCalls).toBe(16);
    expect(failed.status).toBe("failed");
  } finally {
    repository.close();
  }
});

test("saving a completed job as a draft is idempotent", async () => {
  const workflow = new GenerationWorkflow(new MemoryJobStore(), async (stage) => ({
    artifact: { stage }, source: "test", model: "fixture", promptVersion: "test-v1",
  }));
  let job = workflow.create(request);
  for (const _stage of GENERATION_STAGES) job = await workflow.advance(job.id, job.revision);
  const saved = workflow.markDraftSaved(job.id, "storylet-1", 2);
  const repeated = workflow.markDraftSaved(job.id, "storylet-2", 3);

  expect(saved.draftStoryletId).toBe("storylet-1");
  expect(saved.draftRevision).toBe(2);
  expect(repeated.draftStoryletId).toBe("storylet-1");
  expect(repeated.revision).toBe(saved.revision);
});

test("final review artifact becomes an unapproved, executable-shape storylet draft", () => {
  const job: GenerationJob = {
    id: "12345678-0000-0000-0000-000000000000", revision: 1, status: "complete", nextStageIndex: 9,
    request, stages: [{
      stage: "review_package", input: { request, previous: {} }, source: "test", model: "fixture",
      promptVersion: "test-v1", completedAt: 1,
      artifact: { storylet: { title: "The Ledger", sceneOpening: "A blue thread marks the page.", choices: [{ label: "Follow the thread", effects: { "clue.read": true } }] } },
    }], attempts: {}, totalRetries: 0, createdAt: 1, updatedAt: 1,
  };
  const result = new UnslopStoryletPipeline().storyletFromJob(job);

  expect(result.storylet.title).toBe("The Ledger");
  expect(result.storylet.choices[0].effects).toEqual({ "clue.read": true });
  expect(result.storylet.revision).toBeUndefined();
  expect(result.storylet.approved_revision).toBeUndefined();
  expect(result.audit.passed).toBe(true);
});

test("final review rejects choice effects outside the designer's declared outcomes", () => {
  const job: GenerationJob = {
    id: "12345678-0000-0000-0000-000000000000", revision: 1, status: "complete", nextStageIndex: 9,
    request: { ...request, desiredOutcomes: { "clue.read": true } },
    stages: [{
      stage: "review_package", input: { request, previous: {} }, source: "test", model: "fixture",
      promptVersion: "test-v1", completedAt: 1,
      artifact: { storylet: { title: "The Ledger", sceneOpening: "A blue thread marks the page.", choices: [
        { label: "Invent a clue", effects: { "world.secret": true } },
      ] } },
    }], attempts: {}, totalRetries: 0, createdAt: 1, updatedAt: 1,
  };

  expect(() => new UnslopStoryletPipeline().storyletFromJob(job))
    .toThrow("choice 1 writes undeclared outcome 'world.secret'");
});

test("final review rejects choice values that differ from the designer's outcome contract", () => {
  const job: GenerationJob = {
    id: "12345678-0000-0000-0000-000000000000", revision: 1, status: "complete", nextStageIndex: 9,
    request: { ...request, desiredOutcomes: { "clue.read": true } },
    stages: [{
      stage: "review_package", input: { request, previous: {} }, source: "test", model: "fixture",
      promptVersion: "test-v1", completedAt: 1,
      artifact: { storylet: { title: "The Ledger", sceneOpening: "A blue thread marks the page.", choices: [
        { label: "Erase the clue", effects: { "clue.read": false } },
      ] } },
    }], attempts: {}, totalRetries: 0, createdAt: 1, updatedAt: 1,
  };

  expect(() => new UnslopStoryletPipeline().storyletFromJob(job))
    .toThrow("choice 1 changes 'clue.read' to a value outside the designer's contract");
});

test("style audit includes generated choice labels in its review text", () => {
  const job: GenerationJob = {
    id: "12345678-0000-0000-0000-000000000000", revision: 1, status: "complete", nextStageIndex: 9,
    request: { ...request, desiredOutcomes: { "clue.read": true } },
    stages: [{
      stage: "review_package", input: { request, previous: {} }, source: "test", model: "fixture",
      promptVersion: "test-v1", completedAt: 1,
      artifact: { storylet: { title: "Quietly, a Tapestry", sceneOpening: "A blue thread marks the page.", choices: [
        { label: "Follow the clue", effects: { "clue.read": true } },
      ] } },
    }], attempts: {}, totalRetries: 0, createdAt: 1, updatedAt: 1,
  };

  const result = new UnslopStoryletPipeline().storyletFromJob(job);
  expect(result.audit.text).toBe("Quietly, a Tapestry\nA blue thread marks the page.\nFollow the clue");
  expect(result.audit.violations).toContain("banned_word: 'quietly'");
  expect(result.audit.violations).toContain("banned_word: 'tapestry'");
});

test("generation stage outputs survive a workflow reload and interrupted stages resume", async () => {
  const repository = new StoryletRepository(":memory:");
  try {
    const store: GenerationJobStore = {
      get: (id) => repository.getGenerationJob<GenerationJob>(id),
      list: () => repository.listGenerationJobs<GenerationJob>(),
      put: (job) => repository.saveGenerationJob(job),
      delete: (id) => repository.deleteGenerationJob(id),
    };
    const initial = new GenerationWorkflow(store, async (stage) => ({ artifact: { stage }, source: "test", model: "fixture", promptVersion: "test-v1" }));
    let job = initial.create(request);
    job = await initial.advance(job.id, job.revision);
    job.status = "running";
    repository.saveGenerationJob(job);
    const resumed = new GenerationWorkflow(store, async (stage, _request, previous) => ({
      artifact: { stage, previous: Object.keys(previous) }, source: "test", model: "fixture", promptVersion: "test-v1",
    }));
    resumed.recoverInterruptedJobs();
    const restored = resumed.get(job.id);
    expect(restored.status).toBe("ready");
    expect(restored.revision).toBe(job.revision + 1);
    expect(restored.stages).toHaveLength(1);
    const next = await resumed.advance(restored.id, restored.revision);
    expect(next.stages[1].input.previous).toHaveProperty("canon_and_voice");
  } finally {
    repository.close();
  }
});
