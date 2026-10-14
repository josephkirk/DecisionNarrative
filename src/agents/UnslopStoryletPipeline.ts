/**
 * Generic Storylet Generative Pipeline
 * Adapts dbohdan/unslop pipeline principles and story-skills (.agents/skills/*):
 * - Scene Craft & Character Management from story-skills
 * - 9-phase prompt synthesis (Style Anchor -> SEED -> Plot Premise -> Scene -> Choices -> Anti-Slop Audit)
 * - Anti-slop kill-list audit enforcing unslop-style-guide-sf.md & tropes.md
 * - Strata Qwen 3.8 Flash live integration
 */

import type { StoryletRecord } from "../db/repository.ts";
import { StoryCraftResourceRegistry } from "./StoryCraftResources.ts";
import {
  GENERATION_STAGES,
  type GenerationJob,
  type GenerationJobRequest,
  type GenerationStageId,
  type GenerationStageResult,
} from "./GenerationWorkflow.ts";

export type StoryletGenerationRequest = GenerationJobRequest;

export class StoryletGenerationError extends Error {
  constructor(message: string, public readonly stage: string, public readonly statusCode = 503) { super(message); this.name = "StoryletGenerationError"; }
}

export interface UnslopAuditResult {
  passed: boolean;
  violations: string[];
  text: string;
}

const BANNED_WORDS = [
  "delve", "tapestry", "landscape", "pivotal", "foster", "underscore", "testament",
  "enhance", "crucial", "intricate", "leverage", "robust", "streamline", "harness",
  "utilize", "multifaceted", "resonate", "catalyze", "quietly", "deeply", "fundamentally",
  "remarkably", "arguably", "subtly", "serves as", "stands as", "crucible", "nexus",
  "bruised silence", "think of it as", "imagine a world where", "here's the kicker",
  "let's break this down", "let's unpack"
];

const BANNED_PATTERNS = [
  { name: "negative_parallelism", regex: /not\s+[^—\-\.]+[-—]\s*it's/i },
  { name: "dramatic_countdown", regex: /not\s+[^—\-\.]+\.\s*not\s+[^—\-\.]+\.\s*just\s+/i },
  { name: "self_answered_question", regex: /\?\s+[A-Z][a-z0-9\s]+\./i }
];

function stableJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stableJson).join(",")}]`;
  if (value && typeof value === "object") {
    return `{${Object.entries(value as Record<string, unknown>).sort(([a], [b]) => a.localeCompare(b))
      .map(([key, item]) => `${JSON.stringify(key)}:${stableJson(item)}`).join(",")}}`;
  }
  return JSON.stringify(value);
}

function validateChoiceEffects(choices: unknown[], declaredOutcomes: Record<string, unknown>): string | null {
  if (!choices.length) return "The final storylet must contain at least one choice.";
  for (let index = 0; index < choices.length; index++) {
    const choice = choices[index] as { effects?: unknown } | null;
    const effects = choice?.effects;
    if (!effects || typeof effects !== "object" || Array.isArray(effects)) {
      return `Choice ${index + 1} must contain an effects object.`;
    }
    for (const [key, value] of Object.entries(effects as Record<string, unknown>)) {
      if (!Object.hasOwn(declaredOutcomes, key)) {
        return `choice ${index + 1} writes undeclared outcome '${key}'`;
      }
      if (stableJson(value) !== stableJson(declaredOutcomes[key])) {
        return `choice ${index + 1} changes '${key}' to a value outside the designer's contract`;
      }
    }
  }
  return null;
}

export function auditUnslopCompliance(text: string): UnslopAuditResult {
  const violations: string[] = [];

  const lower = text.toLowerCase();
  for (const w of BANNED_WORDS) {
    if (lower.includes(w)) {
      violations.push(`banned_word: '${w}'`);
    }
  }

  for (const pat of BANNED_PATTERNS) {
    if (pat.regex.test(text)) {
      violations.push(`banned_pattern: ${pat.name}`);
    }
  }

  return {
    passed: violations.length === 0,
    violations,
    text,
  };
}

export class UnslopStoryletPipeline {
  private readonly craftResources = new StoryCraftResourceRegistry();
  private readonly promptVersion = "storylet-craft-v2";

  public async generateWorkflowStage(
    stage: GenerationStageId,
    request: StoryletGenerationRequest,
    previous: Partial<Record<GenerationStageId, unknown>>,
  ): Promise<GenerationStageResult> {
    const descriptor = GENERATION_STAGES.find((item) => item.id === stage);
    if (!descriptor) throw new StoryletGenerationError(`Unknown generation stage '${stage}'`, stage);
    const craft = await this.craftResources.forStage(stage);
    const endpoint = `${(Bun.env.STRATA_BASE_URL ?? "http://127.0.0.1:8080/v1").replace(/\/$/, "")}/chat/completions`;
    const model = Bun.env.STRATA_MODEL ?? "strata-uncensored";
    const stageGuidance: Record<GenerationStageId, string> = {
      canon_and_voice: "Extract project canon, genre promise, voice constraints, character goals, boundaries, knowledge and location facts. Return {artifact:{canonSummary, styleRules, characterConstraints, unknowns}}. Preserve unknowns; invent no canon.",
      physical_seed: "Propose one specific, ordinary physical mechanism or situational detail that can produce story pressure. Return {artifact:{seed, mechanism, sensoryEvidence}}. Avoid generic alarms and arbitrary danger.",
      premise_options: "Return exactly three distinct scene premises. Each needs a title, conflict, dramatic opportunity, strength, and concern. Return {artifact:{premises:[...]}}.",
      scene_plan: "Plan each needed character's want, obstacle, tactic, subtext, concrete beat sequence, and outcome shape. Keep facts and knowledge within the supplied canon. Return {artifact:{wants, conflict, subtext, beats, outcomeShapes}}.",
      storylet_contract: "Restate the designer's required roles, trigger predicates, desired outcomes, choice requirements, and effects as a structured storylet contract. Do not weaken or add executable effects. Return {artifact:{roles, triggers, choices, effects, lifetime, pacing}}.",
      scene_variants: "Draft exactly three scene variants with dialogue, action, and interaction. Each must preserve the storylet contract and give choices legible, distinct consequences. Return {artifact:{variants:[{title,sceneOpening,choices:[{label,systemVerb,effects,narration}]}]}}. Every effects object must use only the designer's declared outcome keys and values.",
      craft_audit: "Audit all variants for canon continuity, point of view, character voice, knowledge access, choice legibility, effect-contract fidelity, branch consequences, and generic prose patterns. Report findings; do not silently rewrite. Return {artifact:{findings:[...], selectedVariant, rationale}}.",
      revision: "Revise the selected scene using the audit findings while preserving its executable contract. Return {artifact:{title,sceneOpening,choices:[{label,systemVerb,effects,narration}]}}.",
      review_package: "Prepare the final approval package from the revised scene. Compare it briefly to the other premises and variants. Return {artifact:{comparison, auditFindings, storylet:{title,sceneOpening,choices:[{label,systemVerb,effects,narration}]}}}. Do not invent effects or alter the designer contract.",
    };
    const prompt = `${descriptor.title}\n${stageGuidance[stage]}\n\n${craft.prompt}\n\nDESIGNER BRIEF:\n${JSON.stringify(request)}\n\nPROJECT FACTS AND STYLE:\n${JSON.stringify(request.projectContext ?? {})}\n\nPRIOR STAGE OUTPUTS:\n${JSON.stringify(previous)}\n\nReturn only one JSON object. Put the stage result under the 'artifact' key.`;
    let response: Response;
    try {
      response = await fetch(endpoint, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          model,
          messages: [
            { role: "system", content: "You are a careful narrative design partner. Follow project-owned facts and the designer's executable contract. Produce specific, character-led scenes; preserve uncertainty; never invent system effects." },
            { role: "user", content: prompt },
          ],
          temperature: 0.7,
          max_tokens: stage === "scene_variants" || stage === "review_package" ? 2800 : 1600,
          reasoning_effort: "none",
        }),
        signal: AbortSignal.timeout(120_000),
      });
    } catch (error) {
      throw new StoryletGenerationError(`Strata request failed: ${error instanceof Error ? error.message : String(error)}`, stage);
    }
    if (!response.ok) throw new StoryletGenerationError(`Strata returned HTTP ${response.status}`, stage);
    const data = await response.json() as any;
    const text = data.choices?.[0]?.message?.content ?? data.choices?.[0]?.message?.reasoning_content ?? "";
    const parsed = this.extractJson(text);
    if (!parsed || parsed.artifact === undefined || parsed.artifact === null) {
      throw new StoryletGenerationError("Strata returned invalid stage JSON", stage);
    }
    if (stage === "review_package") {
      const final = parsed.artifact.storylet;
      if (!final || typeof final.title !== "string" || typeof final.sceneOpening !== "string" || !Array.isArray(final.choices)) {
        throw new StoryletGenerationError("Final review package lacks a valid storylet", stage);
      }
      const violation = validateChoiceEffects(final.choices, request.desiredOutcomes);
      if (violation) throw new StoryletGenerationError(violation, stage, 422);
    }
    const resourceVersion = craft.references.map((resource) => `${resource.id}@${resource.sourceHash}.${resource.promptHash}`).join("+");
    return {
      artifact: parsed.artifact,
      source: "strata",
      model,
      promptVersion: `${this.promptVersion}+${resourceVersion}`,
      craftResources: craft.references,
    };
  }

  public storyletFromJob(job: GenerationJob): { storylet: StoryletRecord; audit: UnslopAuditResult } {
    if (job.status !== "complete") throw new StoryletGenerationError("Complete all generation stages before saving a draft", "review_package", 409);
    const finalOutput = job.stages.find((record) => record.stage === "review_package")?.artifact as any;
    const final = finalOutput?.storylet;
    if (!final || typeof final.title !== "string" || typeof final.sceneOpening !== "string" || !Array.isArray(final.choices)) {
      throw new StoryletGenerationError("Final review package does not contain a storylet", "review_package", 422);
    }
    const contractViolation = validateChoiceEffects(final.choices, job.request.desiredOutcomes);
    if (contractViolation) throw new StoryletGenerationError(contractViolation, "review_package", 422);
    const choices = final.choices.map((choice: any, index: number) => {
      if (typeof choice.label !== "string" || !choice.label.trim()
        || choice.effects === null || typeof choice.effects !== "object" || Array.isArray(choice.effects)) {
        throw new StoryletGenerationError(`Final choice ${index + 1} lacks a label or effects object`, "review_package", 422);
      }
      return {
        choice_id: `choice_${index + 1}`,
        label: choice.label,
        systemVerb: typeof choice.systemVerb === "string" ? choice.systemVerb : "ACT",
        effects: choice.effects,
      };
    });
    const locationSlug = job.request.location.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "scene";
    const storylet: StoryletRecord = {
      id: `stl_${locationSlug}_${job.id.slice(0, 8)}`,
      title: job.request.title?.trim() || final.title,
      location: job.request.location,
      group_tag: job.request.location,
      cluster_type: job.request.clusterType,
      urgency_weight: job.request.urgencyWeight ?? 50,
      repeatability: job.request.clusterType === "aftermath" ? "once_ever" : "always",
      cooldown_turns: 0,
      parameters: job.request.characterRoles.map((role, index) => ({
        param_name: `role_${index + 1}`,
        query: `character.role == '${role.replace(/\\/g, "\\\\").replace(/'/g, "\\'")}'`,
      })),
      preconditions: { all_of: job.request.triggerPreconditions },
      content: { text_template: final.sceneOpening, sceneOpening: final.sceneOpening },
      choices,
      effects: job.request.desiredOutcomes,
      created_at: Date.now(),
    };
    const auditedText = [final.title, final.sceneOpening, ...choices.map((choice: { label: string }) => choice.label)].join("\n");
    return { storylet, audit: auditUnslopCompliance(auditedText) };
  }

  private extractJson(raw: string): any {
    try {
      const match = raw.match(/\{[\s\S]*\}/);
      if (match) return JSON.parse(match[0]);
    } catch {
      return null;
    }
  }


}
