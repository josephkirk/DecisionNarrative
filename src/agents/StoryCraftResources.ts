import { createHash } from "node:crypto";
import * as path from "node:path";
import type { GenerationStageId } from "./GenerationWorkflow.ts";

const resources = {
  "character-management": {
    source: ".agents/skills/character-management/SKILL.md",
    guidance: "For each role, preserve the designer's supplied goals, boundaries, relationships, agency, and knowledge. Give each character a motive and tactic; do not invent profile facts.",
  },
  worldbuilding: {
    source: ".agents/skills/worldbuilding/SKILL.md",
    guidance: "Treat setting facts as authored canon. Ground action in available place, objects, social systems, and constraints. Mark unknown details instead of filling gaps as fact.",
  },
  "scene-craft": {
    source: ".agents/skills/scene-craft/SKILL.md",
    guidance: "Work at scene grain: concrete goal, resistance, tactic, subtext, beat progression, and an outcome that changes the next playable situation. Keep physical details causally relevant.",
  },
  "interactive-fiction": {
    source: ".agents/skills/interactive-fiction/SKILL.md",
    guidance: "Make each choice understandable before selection and distinct in intent and consequence. Preserve branch continuity and ensure branch consequences follow the declared storylet effects.",
  },
  "voice-style": {
    source: ".agents/skills/voice-style/SKILL.md",
    guidance: "Keep dialogue and narration consistent with project-owned voice constraints. Differentiate speakers through word choice, rhythm, attention, and tactics; do not infer a house style when none is supplied.",
  },
  "line-editing": {
    source: ".agents/skills/line-editing/SKILL.md",
    guidance: "Review prose for clarity, specificity, rhythm, point of view, and redundant explanation. Report edits and reasons; do not silently alter authored text or executable effects.",
  },
  "revision-continuity": {
    source: ".agents/skills/revision-continuity/SKILL.md",
    guidance: "Check event order, character knowledge, relationships, location facts, and prior branch consequences against supplied canon and earlier stages. Distinguish contradictions from unresolved unknowns.",
  },
} as const;

export type StoryCraftResourceId = keyof typeof resources;

export interface StoryCraftResourceReference {
  id: StoryCraftResourceId;
  version: "1.0.0";
  source: string;
  sourceHash: string;
  promptHash: string;
}

export interface LoadedStoryCraftResources {
  prompt: string;
  references: StoryCraftResourceReference[];
}

const stageResources: Record<GenerationStageId, StoryCraftResourceId[]> = {
  canon_and_voice: ["character-management", "worldbuilding", "voice-style", "revision-continuity"],
  physical_seed: ["worldbuilding", "scene-craft"],
  premise_options: ["scene-craft", "interactive-fiction"],
  scene_plan: ["character-management", "worldbuilding", "scene-craft", "voice-style"],
  storylet_contract: ["interactive-fiction", "revision-continuity"],
  scene_variants: ["scene-craft", "interactive-fiction", "voice-style"],
  craft_audit: ["character-management", "worldbuilding", "scene-craft", "interactive-fiction", "voice-style", "line-editing", "revision-continuity"],
  revision: ["scene-craft", "interactive-fiction", "voice-style", "line-editing", "revision-continuity"],
  review_package: ["interactive-fiction", "voice-style", "line-editing", "revision-continuity"],
};

function sha256(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}

/** Loads project-independent craft guidance and records exact source provenance. */
export class StoryCraftResourceRegistry {
  constructor(private readonly root = process.cwd()) {}

  async catalog(): Promise<StoryCraftResourceReference[]> {
    return Promise.all((Object.keys(resources) as StoryCraftResourceId[]).map((id) => this.reference(id)));
  }

  async forStage(stage: GenerationStageId): Promise<LoadedStoryCraftResources> {
    const selected = stageResources[stage];
    const references = await Promise.all(selected.map((id) => this.reference(id)));
    const prompt = selected.map((id) => `CRAFT RESOURCE ${id}/1.0.0:\n${resources[id].guidance}`).join("\n\n");
    return { prompt, references };
  }

  private async reference(id: StoryCraftResourceId): Promise<StoryCraftResourceReference> {
    const definition = resources[id];
    const absolutePath = path.resolve(this.root, definition.source);
    let sourceText: string;
    try {
      sourceText = await Bun.file(absolutePath).text();
    } catch (error) {
      throw new Error(`Craft resource source '${definition.source}' is unavailable: ${error instanceof Error ? error.message : String(error)}`);
    }
    const normalizedSource = sourceText.replace(/\r\n/g, "\n");
    return {
      id,
      version: "1.0.0",
      source: definition.source.replace(/\\/g, "/"),
      sourceHash: sha256(normalizedSource),
      promptHash: sha256(definition.guidance),
    };
  }
}
