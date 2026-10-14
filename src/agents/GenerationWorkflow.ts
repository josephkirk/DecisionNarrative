export const GENERATION_STAGES = [
  { id: "canon_and_voice", title: "Gather canon and voice constraints" },
  { id: "physical_seed", title: "Establish a physical scene seed" },
  { id: "premise_options", title: "Compare three distinct premises" },
  { id: "scene_plan", title: "Plan wants, conflict, subtext, and beats" },
  { id: "storylet_contract", title: "Shape the executable storylet contract" },
  { id: "scene_variants", title: "Draft three contract-bound variants" },
  { id: "craft_audit", title: "Audit continuity, voice, and choices" },
  { id: "revision", title: "Revise the selected candidate" },
  { id: "review_package", title: "Prepare the approval package" },
] as const;

export type GenerationStageId = typeof GENERATION_STAGES[number]["id"];
export type GenerationJobStatus = "ready" | "running" | "failed" | "complete";

export interface GenerationJobRequest {
  title?: string;
  location: string;
  clusterType: "linear" | "player_controlled_passage" | "wheel" | "aftermath";
  characterRoles: string[];
  triggerPreconditions: Array<{ variable: string; op: string; value: unknown }>;
  desiredOutcomes: Record<string, unknown>;
  seedDetail?: string;
  urgencyWeight?: number;
  projectContext?: Record<string, unknown>;
}

export interface GenerationStageRecord {
  stage: GenerationStageId;
  input: { request: GenerationJobRequest; previous: Partial<Record<GenerationStageId, unknown>> };
  artifact: unknown;
  source: string;
  model: string;
  promptVersion: string;
  craftResources?: import("./StoryCraftResources.ts").StoryCraftResourceReference[];
  completedAt: number;
}

export interface GenerationJob {
  id: string;
  revision: number;
  status: GenerationJobStatus;
  nextStageIndex: number;
  request: GenerationJobRequest;
  stages: GenerationStageRecord[];
  attempts: Partial<Record<GenerationStageId, number>>;
  draftStoryletId?: string;
  draftRevision?: number;
  failure?: { stage: GenerationStageId; message: string };
  createdAt: number;
  updatedAt: number;
}

export interface GenerationStageResult {
  artifact: unknown;
  source: string;
  model: string;
  promptVersion: string;
  craftResources?: import("./StoryCraftResources.ts").StoryCraftResourceReference[];
}

export type GenerationStageRunner = (
  stage: GenerationStageId,
  request: GenerationJobRequest,
  previous: Partial<Record<GenerationStageId, unknown>>,
) => Promise<GenerationStageResult>;

export interface GenerationJobStore {
  get(id: string): GenerationJob | null;
  list(): GenerationJob[];
  put(job: GenerationJob): void;
  delete(id: string): boolean;
}

export class GenerationWorkflowError extends Error {
  constructor(message: string, public readonly statusCode = 400) {
    super(message);
    this.name = "GenerationWorkflowError";
  }
}

/** Resumable stage runner. The injected store makes each test independently clean. */
export class GenerationWorkflow {
  private readonly listeners = new Map<string, Set<(job: GenerationJob) => void>>();
  private readonly locks = new Map<string, Promise<GenerationJob>>();

  constructor(private readonly store: GenerationJobStore, private readonly runStage: GenerationStageRunner) {}

  recoverInterruptedJobs(): void {
    for (const job of this.store.list()) {
      if (job.status !== "running") continue;
      job.status = "ready";
      job.revision++;
      job.updatedAt = Date.now();
      this.store.put(job);
    }
  }

  create(request: GenerationJobRequest): GenerationJob {
    if (!request.location?.trim()) throw new GenerationWorkflowError("Choose a location for this storylet.");
    if (!request.clusterType || !Array.isArray(request.characterRoles) || !Array.isArray(request.triggerPreconditions)
      || request.desiredOutcomes === null || typeof request.desiredOutcomes !== "object" || Array.isArray(request.desiredOutcomes)) {
      throw new GenerationWorkflowError("The generation brief is incomplete or invalid.");
    }
    const now = Date.now();
    const job: GenerationJob = {
      id: crypto.randomUUID(), revision: 0, status: "ready", nextStageIndex: 0,
      request: structuredClone(request), stages: [], attempts: {}, createdAt: now, updatedAt: now,
    };
    this.store.put(job);
    return structuredClone(job);
  }

  get(id: string): GenerationJob {
    const job = this.store.get(id);
    if (!job) throw new GenerationWorkflowError(`Generation job '${id}' was not found.`, 404);
    return job;
  }

  delete(id: string): boolean {
    return this.store.delete(id);
  }

  markDraftSaved(id: string, storyletId: string, revision: number): GenerationJob {
    const job = this.get(id);
    if (job.status !== "complete") throw new GenerationWorkflowError("Only a complete job can produce a draft.", 409);
    if (job.draftStoryletId) return job;
    job.draftStoryletId = storyletId;
    job.draftRevision = revision;
    job.revision++;
    job.updatedAt = Date.now();
    this.store.put(job);
    this.publish(job);
    return structuredClone(job);
  }

  subscribe(id: string, listener: (job: GenerationJob) => void): () => void {
    const listeners = this.listeners.get(id) ?? new Set();
    listeners.add(listener);
    this.listeners.set(id, listeners);
    return () => {
      listeners.delete(listener);
      if (!listeners.size) this.listeners.delete(id);
    };
  }

  advance(id: string, expectedRevision: number): Promise<GenerationJob> {
    const previous = this.locks.get(id) ?? Promise.resolve(this.get(id));
    const result = previous.then(() => this.advanceUnlocked(id, expectedRevision));
    this.locks.set(id, result);
    void result.finally(() => { if (this.locks.get(id) === result) this.locks.delete(id); }).catch(() => undefined);
    return result;
  }

  editStage(id: string, stage: GenerationStageId, artifact: unknown, expectedRevision: number): GenerationJob {
    const job = this.get(id);
    this.assertRevision(job, expectedRevision);
    if (job.status === "running") throw new GenerationWorkflowError("Wait for the active generation stage before editing its inputs.", 409);
    if (artifact === undefined || artifact === null) throw new GenerationWorkflowError("A stage artifact cannot be empty.");
    const index = GENERATION_STAGES.findIndex((item) => item.id === stage);
    if (index < 0 || index >= job.nextStageIndex) throw new GenerationWorkflowError("Only completed stage outputs can be edited.");
    job.stages = job.stages.slice(0, index);
    job.stages.push({
      stage,
      input: { request: structuredClone(job.request), previous: this.previousArtifacts(job, index) },
      artifact: structuredClone(artifact), source: "designer", model: "human edit", promptVersion: "manual-v1", completedAt: Date.now(),
    });
    job.nextStageIndex = index + 1;
    for (const later of GENERATION_STAGES.slice(index + 1)) delete job.attempts[later.id];
    job.status = index === GENERATION_STAGES.length - 1 ? "complete" : "ready";
    job.failure = undefined;
    job.revision++;
    job.updatedAt = Date.now();
    this.store.put(job);
    this.publish(job);
    return structuredClone(job);
  }

  private async advanceUnlocked(id: string, expectedRevision: number): Promise<GenerationJob> {
    const job = this.get(id);
    this.assertRevision(job, expectedRevision);
    if (job.status === "complete") throw new GenerationWorkflowError("This generation job is complete.", 409);
    if (job.status === "running") throw new GenerationWorkflowError("A generation stage is already running.", 409);
    if (job.status === "failed") throw new GenerationWorkflowError("This job reached its retry limit. Edit an earlier stage to resume it.", 409);
    const descriptor = GENERATION_STAGES[job.nextStageIndex];
    if (!descriptor) throw new GenerationWorkflowError("No generation stages remain.", 409);

    const stage = descriptor.id;
    const previous = this.previousArtifacts(job, job.nextStageIndex);
    job.status = "running";
    job.failure = undefined;
    job.revision++;
    job.updatedAt = Date.now();
    this.store.put(job);
    this.publish(job);

    let failure: unknown;
    for (let attempt = (job.attempts[stage] ?? 0) + 1; attempt <= 3; attempt++) {
      job.attempts[stage] = attempt;
      this.store.put(job);
      try {
        const result = await this.runStage(stage, structuredClone(job.request), structuredClone(previous));
        if (result.artifact === undefined || result.artifact === null || result.artifact === "") {
          throw new Error("The generation service returned an empty stage artifact.");
        }
        job.stages.push({
          stage,
          input: { request: structuredClone(job.request), previous: structuredClone(previous) },
          ...structuredClone(result),
          completedAt: Date.now(),
        });
        job.nextStageIndex++;
        job.status = job.nextStageIndex === GENERATION_STAGES.length ? "complete" : "ready";
        job.revision++;
        job.updatedAt = Date.now();
        this.store.put(job);
        this.publish(job);
        return structuredClone(job);
      } catch (error) {
        failure = error;
      }
    }

    job.status = "failed";
    job.failure = { stage, message: failure instanceof Error ? failure.message : String(failure) };
    job.revision++;
    job.updatedAt = Date.now();
    this.store.put(job);
    this.publish(job);
    throw new GenerationWorkflowError(`Stage '${stage}' failed after three attempts: ${job.failure.message}`, 503);
  }

  private previousArtifacts(job: GenerationJob, beforeIndex: number): Partial<Record<GenerationStageId, unknown>> {
    return Object.fromEntries(job.stages.slice(0, beforeIndex).map((record) => [record.stage, record.artifact]));
  }

  private assertRevision(job: GenerationJob, expected: number): void {
    if (!Number.isInteger(expected) || job.revision !== expected) {
      throw new GenerationWorkflowError(`Generation job revision conflict: expected ${expected}, current ${job.revision}.`, 409);
    }
  }

  private publish(job: GenerationJob): void {
    for (const listener of this.listeners.get(job.id) ?? []) listener(structuredClone(job));
  }
}
