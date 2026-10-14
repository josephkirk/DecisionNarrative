import type {
  CharacterRecord,
  LocationRecord,
  NpcBehaviorRecord,
  QualityRecord,
  ScenarioRecord,
  StoryletRecord,
} from "../db/repository.ts";
import type { GenerationJob } from "../agents/GenerationWorkflow.ts";
import { buildWorkspaceProject } from "./workspaceProject.ts";

export const PROJECT_BUNDLE_FORMAT = "decision-narrative-project";
export const PROJECT_BUNDLE_SCHEMA_VERSION = 1;

export interface ProjectBundleContent {
  qualities: QualityRecord[];
  locations: LocationRecord[];
  characters: CharacterRecord[];
  storylets: StoryletRecord[];
  npcBehaviors: NpcBehaviorRecord[];
  scenarios: ScenarioRecord[];
  generationJobs: GenerationJob[];
}

export interface ProjectMetadata {
  id: string;
  name: string;
  revision: number;
}

export interface StoryletProjectBundle {
  format: typeof PROJECT_BUNDLE_FORMAT;
  schema_version: typeof PROJECT_BUNDLE_SCHEMA_VERSION;
  project: ProjectMetadata;
  content: ProjectBundleContent;
}

export interface ProjectBundleInspection {
  valid: boolean;
  bundle: StoryletProjectBundle | null;
  issues: string[];
  counts: Record<keyof ProjectBundleContent, number>;
}

const CONTENT_COLLECTIONS: Array<keyof ProjectBundleContent> = [
  "qualities", "locations", "characters", "storylets", "npcBehaviors", "scenarios", "generationJobs",
];

function emptyCounts(): ProjectBundleInspection["counts"] {
  return { qualities: 0, locations: 0, characters: 0, storylets: 0, npcBehaviors: 0, scenarios: 0, generationJobs: 0 };
}

export function makeProjectBundle(project: ProjectMetadata, content: ProjectBundleContent): StoryletProjectBundle {
  return {
    format: PROJECT_BUNDLE_FORMAT,
    schema_version: PROJECT_BUNDLE_SCHEMA_VERSION,
    project: structuredClone(project),
    content: structuredClone(content),
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

/** Inspect untrusted JSON without mutating the project database. */
export function inspectProjectBundle(input: unknown): ProjectBundleInspection {
  const issues: string[] = [];
  const counts = emptyCounts();
  if (!isRecord(input)) {
    return { valid: false, bundle: null, issues: ["Project bundle must be a JSON object"], counts };
  }
  if (input.format !== PROJECT_BUNDLE_FORMAT) issues.push(`Unsupported project bundle format '${String(input.format)}'`);
  if (input.schema_version !== PROJECT_BUNDLE_SCHEMA_VERSION) {
    issues.push(`Unsupported schema version '${String(input.schema_version)}'; expected ${PROJECT_BUNDLE_SCHEMA_VERSION}`);
  }
  if (!isRecord(input.project) || typeof input.project.id !== "string" || !input.project.id.trim()
    || typeof input.project.name !== "string" || !input.project.name.trim()
    || !Number.isSafeInteger(input.project.revision) || Number(input.project.revision) < 0) {
    issues.push("Project metadata needs a nonempty ID and name plus a nonnegative integer revision");
  }
  if (!isRecord(input.content)) {
    issues.push("Project bundle content must be an object");
    return { valid: false, bundle: null, issues, counts };
  }
  const content = input.content;
  for (const collection of CONTENT_COLLECTIONS) {
    const value = content[collection];
    if (!Array.isArray(value)) issues.push(`content.${collection} must be an array`);
    else counts[collection] = value.length;
  }
  if (CONTENT_COLLECTIONS.some((key) => !Array.isArray(content[key]))) {
    return { valid: false, bundle: null, issues, counts };
  }

  const bundle = input as unknown as StoryletProjectBundle;
  for (const [index, quality] of bundle.content.qualities.entries()) {
    if (!isRecord(quality) || typeof quality.name !== "string" || !quality.name.trim()
      || !["currency", "metric", "tool", "territory", "menace"].includes(String(quality.type))
      || typeof quality.value !== "number" || !Number.isFinite(quality.value)) {
      issues.push(`content.qualities[${index}] is not a valid quality record`);
    }
  }
  for (const [index, storylet] of bundle.content.storylets.entries()) {
    if (!isRecord(storylet) || typeof storylet.title !== "string" || !storylet.title.trim()
      || !Number.isSafeInteger(storylet.revision) || Number(storylet.revision) < 1
      || (storylet.approved_revision != null && storylet.approved_revision !== storylet.revision)) {
      issues.push(`content.storylets[${index}] has invalid required fields or approval revision`);
    }
  }
  for (const [index, job] of bundle.content.generationJobs.entries()) {
    if (!isRecord(job) || typeof job.id !== "string" || !job.id.trim() || !Number.isSafeInteger(job.revision)
      || !["ready", "running", "failed", "complete"].includes(String(job.status))
      || !isRecord(job.request) || !Array.isArray(job.stages) || !isRecord(job.attempts)
      || !Number.isSafeInteger(job.nextStageIndex) || !Number.isSafeInteger(job.createdAt) || !Number.isSafeInteger(job.updatedAt)) {
      issues.push(`content.generationJobs[${index}] is not a valid generation job record`);
    }
  }
  const globalIds = new Map<string, string>();
  const idCollections: Array<[keyof ProjectBundleContent, Array<unknown>]> = [];
  for (const collection of CONTENT_COLLECTIONS) {
    if (collection !== "generationJobs") idCollections.push([collection, bundle.content[collection] as unknown[]]);
  }
  for (const [collection, records] of idCollections) {
    records.forEach((value, index) => {
      const record = isRecord(value) ? value : {};
      const id = collection === "qualities" ? record.name : record.id;
      if (typeof id !== "string" || !id.trim()) {
        issues.push(`content.${collection}[${index}] has no stable ID`);
        return;
      }
      const previous = globalIds.get(id);
      if (previous) issues.push(`ID '${id}' is duplicated in ${previous} and ${collection}`);
      else globalIds.set(id, collection);
    });
  }

  try {
    const records = {
      qualities: bundle.content.qualities,
      locations: bundle.content.locations,
      characters: bundle.content.characters,
      storylets: bundle.content.storylets,
      npcBehaviors: bundle.content.npcBehaviors,
      scenarios: bundle.content.scenarios,
    };
    const projectOptions = {
      projectId: bundle.project.id,
      projectName: bundle.project.name,
      projectRevision: bundle.project.revision,
    };
    const converted = buildWorkspaceProject(records, projectOptions);
    // An in-progress project can validly have no location yet. All other
    // authoring and runtime-reference diagnostics remain import blockers.
    issues.push(...converted.issues.filter((issue) => issue !== "Choose or create a location before starting a playtest"));
    for (const scenario of bundle.content.scenarios) {
      const scenarioResult = buildWorkspaceProject(records, { ...projectOptions, scenario });
      issues.push(...scenarioResult.issues.map((issue) => `Scenario '${scenario.id}': ${issue}`));
    }
  } catch (error) {
    issues.push(`Project content could not be inspected: ${error instanceof Error ? error.message : String(error)}`);
  }
  return { valid: issues.length === 0, bundle: issues.length === 0 ? bundle : null, issues, counts };
}

