import { createInterface } from "node:readline";

const SERVER_NAME = "decisionnarrative-strata-storylets";
const SERVER_VERSION = "1.1.0";
const PROTOCOL_VERSION = "2025-03-26";
const FEATURES = [
  "semantic_relevance", "urgency", "pacing_arc_alignment", "novelty", "waiting_time",
  "goal_progress", "role_binding_quality", "predicted_consequence_alignment", "no_action",
] as const;

export interface BatchArguments {
  scenario_family: string;
  count: number;
  batch_index: number;
  brief: string;
  constraints?: string[];
}

export interface ContentBatchArguments {
  scenario_family: string;
  count: number;
  batch_index: number;
  brief: string;
  constraints?: string[];
}

export function validateBatchPayload(value: unknown, args: BatchArguments): Record<string, unknown>[] {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Expected a JSON object");
  const rows = (value as { examples?: unknown }).examples;
  if (!Array.isArray(rows) || rows.length !== args.count) throw new Error(`Expected exactly ${args.count} examples`);
  const ids = new Set<string>();
  return rows.map((candidate, index) => {
    if (!candidate || typeof candidate !== "object" || Array.isArray(candidate)) throw new Error(`Example ${index + 1} must be an object`);
    const row = candidate as Record<string, unknown>;
    for (const key of ["query", "positive", "negative"]) {
      if (typeof row[key] !== "string" || !row[key].trim()) throw new Error(`Example ${index + 1} has empty ${key}`);
    }
    const features = (side: "positive_features" | "negative_features"): Record<string, number> => {
      const raw = row[side];
      if (!raw || typeof raw !== "object" || Array.isArray(raw)) throw new Error(`Example ${index + 1} has invalid ${side}`);
      const record = raw as Record<string, unknown>;
      if (Object.keys(record).length !== FEATURES.length || FEATURES.some((name) => !(name in record))) {
        throw new Error(`Example ${index + 1} must provide exactly the nine ranking features in ${side}`);
      }
      for (const name of FEATURES) {
        const score = record[name];
        if (typeof score !== "number" || !Number.isFinite(score) || score < 0 || score > 1) {
          throw new Error(`Example ${index + 1} has out-of-range ${side}.${name}`);
        }
      }
      return record as Record<string, number>;
    };
    const id = `strata-${args.scenario_family}-${String(args.batch_index).padStart(3, "0")}-${String(index + 1).padStart(4, "0")}`;
    if (ids.has(id)) throw new Error(`Duplicate example_id ${id}`);
    ids.add(id);
    return {
      example_id: id,
      scenario_family: args.scenario_family,
      split: "train",
      label_source: "strata_synthetic",
      provenance: `Strata-generated; model=${Bun.env.STRATA_MODEL ?? "strata-uncensored"}; review_required=true`,
      query: row.query,
      positive: row.positive,
      negative: row.negative,
      positive_features: features("positive_features"),
      negative_features: features("negative_features"),
    };
  });
}

const CLUSTER_TYPES = ["linear", "player_controlled_passage", "wheel", "aftermath"] as const;
const REPEATABILITY_TYPES = ["once_ever", "always", "cooldown", "persistent"] as const;

export function extractCanonScope(brief: string, constraints?: string[]): {
  locations?: Set<string>;
  qualities?: Set<string>;
  characters?: Set<string>;
  effects?: Set<string>;
  diversions?: Set<string>;
} {
  const combined = [brief, ...(constraints ?? [])].join("\n");
  const result: {
    locations?: Set<string>;
    qualities?: Set<string>;
    characters?: Set<string>;
    effects?: Set<string>;
    diversions?: Set<string>;
  } = {};

  const scopeItems = (match: RegExpMatchArray | null): Set<string> | undefined => {
    if (!match) return undefined;
    const raw = match[1].split(/[.;]/, 1)[0].replace(/[\[\]"']/g, "");
    const items = raw.split(/[,;\s]+/).map(s => s.trim()).filter(Boolean);
    return items.length ? new Set(items) : undefined;
  };
  result.locations = scopeItems(combined.match(/(?:allowed\s+locations|locations?|valid\s+locations)\s*[:=]\s*([^`\n\[\]]+|\[[^\]]+\])/i));
  result.qualities = scopeItems(combined.match(/(?:allowed\s+qualities|qualities|valid\s+qualities)\s*[:=]\s*([^`\n\[\]]+|\[[^\]]+\])/i));
  result.characters = scopeItems(combined.match(/(?:allowed\s+characters|characters|valid\s+characters|roles)\s*[:=]\s*([^`\n\[\]]+|\[[^\]]+\])/i));
  result.effects = scopeItems(combined.match(/(?:allowed\s+effects|effects|valid\s+effects)\s*[:=]\s*([^`\n\[\]]+|\[[^\]]+\])/i));
  result.diversions = scopeItems(combined.match(/(?:allowed\s+diversions|divert_targets|diversions)\s*[:=]\s*([^`\n\[\]]+|\[[^\]]+\])/i));

  return result;
}

export function validateContentBatchPayload(
  value: unknown,
  args: ContentBatchArguments,
  fixedCreatedAt = Date.now()
): Record<string, unknown>[] {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("Expected a JSON object");
  }
  const rows = (value as { storylets?: unknown }).storylets;
  if (!Array.isArray(rows) || rows.length !== args.count) {
    throw new Error(`Expected exactly ${args.count} storylets`);
  }

  const canon = extractCanonScope(args.brief, args.constraints);
  const ids = new Set<string>();

  return rows.map((candidate, index) => {
    if (!candidate || typeof candidate !== "object" || Array.isArray(candidate)) {
      throw new Error(`Storylet ${index + 1} must be an object`);
    }
    const row = candidate as Record<string, unknown>;

    if (typeof row.ending !== "boolean") {
      throw new Error(`Storylet ${index + 1} must include boolean ending`);
    }

    if (typeof row.title !== "string" || !row.title.trim()) {
      throw new Error(`Storylet ${index + 1} must have a non-empty title`);
    }

    if (row.location !== undefined && row.location !== null) {
      if (typeof row.location !== "string" || !row.location.trim()) {
        throw new Error(`Storylet ${index + 1} has invalid location`);
      }
      if (canon.locations && !canon.locations.has(row.location.trim())) {
        throw new Error(`Storylet ${index + 1} references unallowed location '${row.location}'`);
      }
    }

    if (row.group_tag !== undefined && row.group_tag !== null) {
      if (typeof row.group_tag !== "string" || !row.group_tag.trim()) {
        throw new Error(`Storylet ${index + 1} has invalid group_tag`);
      }
    }

    if (typeof row.cluster_type !== "string" || !CLUSTER_TYPES.includes(row.cluster_type as any)) {
      throw new Error(`Storylet ${index + 1} has invalid cluster_type '${String(row.cluster_type)}'`);
    }

    const urgency = typeof row.urgency_weight === "number" ? row.urgency_weight : 50;
    if (!Number.isFinite(urgency) || urgency < 0) {
      throw new Error(`Storylet ${index + 1} has invalid urgency_weight`);
    }

    if (typeof row.repeatability !== "string" || !REPEATABILITY_TYPES.includes(row.repeatability as any)) {
      throw new Error(`Storylet ${index + 1} has invalid repeatability '${String(row.repeatability)}'`);
    }

    const cooldown = typeof row.cooldown_turns === "number" ? row.cooldown_turns : 0;
    if (!Number.isInteger(cooldown) || cooldown < 0) {
      throw new Error(`Storylet ${index + 1} has invalid cooldown_turns`);
    }

    const rawParams = row.parameters;
    const parameters: Array<{ param_name: string; query: string }> = [];
    if (rawParams !== undefined && rawParams !== null) {
      if (!Array.isArray(rawParams)) {
        throw new Error(`Storylet ${index + 1} parameters must be an array`);
      }
      for (let pIdx = 0; pIdx < rawParams.length; pIdx++) {
        const p = rawParams[pIdx];
        if (!p || typeof p !== "object" || Array.isArray(p)) {
          throw new Error(`Storylet ${index + 1} parameter ${pIdx + 1} must be an object`);
        }
        if (typeof p.param_name !== "string" || !p.param_name.trim() || typeof p.query !== "string" || !p.query.trim()) {
          throw new Error(`Storylet ${index + 1} parameter ${pIdx + 1} must have param_name and query`);
        }
        if (canon.characters) {
          const match = p.query.trim().match(/^(?:character\.)?role\s*==\s*(['"])(.*?)\1$/i);
          if (match && !canon.characters.has(match[2].trim())) {
            throw new Error(`Storylet ${index + 1} parameter references unallowed character role '${match[2]}'`);
          }
        }
        parameters.push({ param_name: p.param_name.trim(), query: p.query.trim() });
      }
    }

    if (!row.preconditions || typeof row.preconditions !== "object" || Array.isArray(row.preconditions)) {
      throw new Error(`Storylet ${index + 1} must have a preconditions object`);
    }
    const preObj = row.preconditions as Record<string, unknown>;
    const validatePredicates = (list: unknown, kind: string) => {
      if (!Array.isArray(list)) throw new Error(`Storylet ${index + 1} preconditions.${kind} must be an array`);
      for (let i = 0; i < list.length; i++) {
        const pred = list[i];
        if (!pred || typeof pred !== "object" || Array.isArray(pred)) {
          throw new Error(`Storylet ${index + 1} preconditions.${kind}[${i}] must be an object`);
        }
        if (typeof pred.variable !== "string" || !pred.variable.trim() || typeof pred.op !== "string" || !pred.op.trim() || pred.value === undefined) {
          throw new Error(`Storylet ${index + 1} preconditions.${kind}[${i}] must have variable, op, and value`);
        }
        if (canon.qualities && pred.variable !== "location" && pred.variable !== "current_location") {
          if (!canon.qualities.has(pred.variable.trim())) {
            throw new Error(`Storylet ${index + 1} preconditions.${kind} references unallowed quality '${pred.variable}'`);
          }
        }
      }
    };
    if (preObj.all_of !== undefined) validatePredicates(preObj.all_of, "all_of");
    if (preObj.any_of !== undefined) validatePredicates(preObj.any_of, "any_of");

    if (!row.content || typeof row.content !== "object" || Array.isArray(row.content)) {
      throw new Error(`Storylet ${index + 1} must have a content object`);
    }
    const contentObj = row.content as Record<string, unknown>;
    if (typeof contentObj.text_template !== "string" || !contentObj.text_template.trim()) {
      throw new Error(`Storylet ${index + 1} content.text_template must be non-empty`);
    }

    if (!Array.isArray(row.choices) || row.choices.length === 0) {
      throw new Error(`Storylet ${index + 1} choices must be a non-empty array`);
    }
    const choices = row.choices.map((c: any, cIdx: number) => {
      if (!c || typeof c !== "object" || Array.isArray(c)) {
        throw new Error(`Storylet ${index + 1} choice ${cIdx + 1} must be an object`);
      }
      if (typeof c.label !== "string" || !c.label.trim()) {
        throw new Error(`Storylet ${index + 1} choice ${cIdx + 1} label must be non-empty`);
      }
      if (!c.effects || typeof c.effects !== "object" || Array.isArray(c.effects)) {
        throw new Error(`Storylet ${index + 1} choice ${cIdx + 1} must have an effects object`);
      }
      if (canon.effects || canon.qualities) {
        for (const effKey of Object.keys(c.effects)) {
          if (canon.effects && !canon.effects.has(effKey)) {
            throw new Error(`Storylet ${index + 1} choice ${cIdx + 1} writes unallowed effect key '${effKey}'`);
          }
          if (canon.qualities && !canon.qualities.has(effKey)) {
            throw new Error(`Storylet ${index + 1} choice ${cIdx + 1} writes unallowed quality effect '${effKey}'`);
          }
        }
      }
      if (c.divert_target !== undefined && c.divert_target !== null) {
        if (typeof c.divert_target !== "string" || !c.divert_target.trim()) {
          throw new Error(`Storylet ${index + 1} choice ${cIdx + 1} has invalid divert_target`);
        }
        if (canon.diversions && !canon.diversions.has(c.divert_target.trim())) {
          throw new Error(`Storylet ${index + 1} choice ${cIdx + 1} references unallowed divert_target '${c.divert_target}'`);
        }
      }
      return {
        choice_id: typeof c.choice_id === "string" && c.choice_id.trim() ? c.choice_id.trim() : `choice_${cIdx + 1}`,
        label: c.label.trim(),
        systemVerb: typeof c.systemVerb === "string" && c.systemVerb.trim() ? c.systemVerb.trim() : "ACT",
        ...(c.conditions ? { conditions: c.conditions } : {}),
        effects: c.effects,
        ...(c.divert_target ? { divert_target: c.divert_target.trim() } : {}),
        ...(c.advisorPreview ? { advisorPreview: c.advisorPreview } : {}),
      };
    });

    if (!row.effects || typeof row.effects !== "object" || Array.isArray(row.effects)) {
      throw new Error(`Storylet ${index + 1} must have an effects object`);
    }
    if (canon.effects || canon.qualities) {
      for (const effKey of Object.keys(row.effects as Record<string, unknown>)) {
        if (canon.effects && !canon.effects.has(effKey)) {
          throw new Error(`Storylet ${index + 1} writes unallowed effect key '${effKey}'`);
        }
        if (canon.qualities && !canon.qualities.has(effKey)) {
          throw new Error(`Storylet ${index + 1} writes unallowed quality effect '${effKey}'`);
        }
      }
    }

    const id = `stl_${args.scenario_family}_${String(args.batch_index).padStart(3, "0")}_${String(index + 1).padStart(4, "0")}`;
    if (ids.has(id)) {
      throw new Error(`Duplicate storylet ID '${id}'`);
    }
    ids.add(id);

    return {
      id,
      revision: 1,
      approved_revision: null,
      ending: row.ending,
      title: row.title.trim(),
      ...(row.location ? { location: (row.location as string).trim() } : {}),
      ...(row.group_tag ? { group_tag: (row.group_tag as string).trim() } : {}),
      cluster_type: row.cluster_type,
      urgency_weight: urgency,
      repeatability: row.repeatability,
      cooldown_turns: cooldown,
      parameters,
      preconditions: preObj,
      content: {
        text_template: (contentObj.text_template as string).trim(),
        ...(typeof contentObj.sceneOpening === "string" && contentObj.sceneOpening.trim() ? { sceneOpening: contentObj.sceneOpening.trim() } : {}),
        ...(typeof contentObj.speaker_id === "string" && contentObj.speaker_id.trim() ? { speaker_id: contentObj.speaker_id.trim() } : {}),
        ...(Array.isArray(contentObj.dialogue_quips) ? { dialogue_quips: contentObj.dialogue_quips } : {}),
      },
      choices,
      effects: row.effects as Record<string, unknown>,
      created_at: fixedCreatedAt,
    };
  });
}

export async function generateContentBatch(input: unknown, fetcher: typeof fetch = fetch): Promise<Record<string, unknown>[]> {
  if (!input || typeof input !== "object" || Array.isArray(input)) throw new Error("Arguments must be an object");
  const raw = input as Record<string, unknown>;
  const scenario_family = typeof raw.scenario_family === "string" ? raw.scenario_family.trim() : "";
  const brief = typeof raw.brief === "string" ? raw.brief.trim() : "";
  const count = raw.count;
  if (!/^[a-zA-Z0-9][a-zA-Z0-9._-]{0,79}$/.test(scenario_family)) throw new Error("scenario_family must be a 1-80 character slug");
  if (!brief || brief.length > 12000) throw new Error("brief must contain 1-12000 characters");
  if (!Number.isInteger(count) || (count as number) < 1 || (count as number) > 24) throw new Error("count must be an integer from 1 to 24");
  const batch_index = raw.batch_index === undefined ? 1 : raw.batch_index;
  if (!Number.isInteger(batch_index) || (batch_index as number) < 1 || (batch_index as number) > 999999) throw new Error("batch_index must be an integer from 1 to 999999");
  if (raw.constraints !== undefined && (!Array.isArray(raw.constraints) || !raw.constraints.every((item) => typeof item === "string"))) throw new Error("constraints must be an array of strings");
  const constraints = (raw.constraints ?? []) as string[];
  if (constraints.length > 32 || constraints.some((item) => item.length > 1000)) throw new Error("constraints exceed the bounded input size");
  const args: ContentBatchArguments = { scenario_family, count: count as number, batch_index: batch_index as number, brief, constraints };
  const base = (Bun.env.STRATA_BASE_URL ?? "http://127.0.0.1:8080/v1").replace(/\/$/, "");
  const endpoint = new URL(base);
  const endpointHost = endpoint.hostname.replace(/^[\[\]]/g, "").replace(/[\[\]]$/g, "");
  if (endpoint.protocol !== "http:" || !["localhost", "127.0.0.1", "::1"].includes(endpointHost)) throw new Error("STRATA_BASE_URL must use HTTP on localhost, 127.0.0.1, or ::1");
  const model = Bun.env.STRATA_MODEL ?? "strata-uncensored";
  let response: Response;
  try {
    response = await fetcher(`${base}/chat/completions`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        model,
        temperature: 0.7,
        max_tokens: Math.min(12000, 750 * args.count),
        chat_template_kwargs: { enable_thinking: false },
        messages: [
          { role: "system", content: "You are a specialized narrative designer generating storylet content candidates matching the DecisionNarrative StoryletRecord schema. Do not invent unmentioned canon, locations, qualities, effects, or characters. Return only a single valid JSON object." },
          { role: "user", content: [
            `Create exactly ${args.count} diverse storylet candidates for scenario family ${args.scenario_family}.`,
            "Each candidate must match the StoryletRecord shape:",
            `{"storylets":[{"title":string,"location"?:string,"group_tag"?:string,"cluster_type":"linear"|"player_controlled_passage"|"wheel"|"aftermath","urgency_weight":number,"repeatability":"once_ever"|"always"|"cooldown"|"persistent","cooldown_turns":number,"parameters":[{"param_name":string,"query":string}],"preconditions":{"all_of"?:[{"variable":string,"op":string,"value":any}]},"content":{"text_template":string,"sceneOpening"?:string},"choices":[{"choice_id":string,"label":string,"systemVerb"?:string,"effects":object,"divert_target"?:string}],"effects":object}]}`,
            `Project brief: ${brief}`,
            `Additional constraints: ${JSON.stringify(constraints)}`,
            "Include ending as a required boolean; set true only for the designated terminal storylet. Do not include id, revision, approved_revision, or created_at; the server assigns these deterministically.",
            "All content must strictly reference only characters, locations, qualities, and effects declared or allowed in the brief.",
            "Return only JSON with the top-level 'storylets' key.",
          ].join("\n") },
        ],
      }),
      signal: AbortSignal.timeout(120_000),
    });
  } catch (error) {
    throw new Error(`Strata request failed: ${error instanceof Error ? error.message : String(error)}`);
  }
  if (!response.ok) throw new Error(`Strata returned HTTP ${response.status}`);
  let completion: { choices?: Array<{ message?: { content?: unknown } }> };
  try { completion = await response.json() as typeof completion; }
  catch { throw new Error("Strata returned invalid JSON; no partial batch was accepted"); }
  const content = completion.choices?.[0]?.message?.content;
  if (typeof content !== "string") throw new Error("Strata returned no completion text");
  let parsed: unknown;
  try {
    const jsonMatch = content.match(/\{[\s\S]*\}/);
    parsed = JSON.parse(jsonMatch ? jsonMatch[0] : content);
  } catch {
    throw new Error("Strata returned invalid JSON; no partial batch was accepted");
  }
  return validateContentBatchPayload(parsed, args, Date.now());
}

function toolError(message: string) {
  return { content: [{ type: "text", text: message }], isError: true };
}

export async function generateBatch(input: unknown, fetcher: typeof fetch = fetch) {
  if (!input || typeof input !== "object" || Array.isArray(input)) throw new Error("Arguments must be an object");
  const raw = input as Record<string, unknown>;
  const scenario_family = typeof raw.scenario_family === "string" ? raw.scenario_family.trim() : "";
  const brief = typeof raw.brief === "string" ? raw.brief.trim() : "";
  const count = raw.count;
  if (!/^[a-zA-Z0-9][a-zA-Z0-9._-]{0,79}$/.test(scenario_family)) throw new Error("scenario_family must be a 1-80 character slug");
  if (!brief || brief.length > 12000) throw new Error("brief must contain 1-12000 characters");
  if (!Number.isInteger(count) || (count as number) < 1 || (count as number) > 24) throw new Error("count must be an integer from 1 to 24");
  const batch_index = raw.batch_index === undefined ? 1 : raw.batch_index;
  if (!Number.isInteger(batch_index) || (batch_index as number) < 1 || (batch_index as number) > 999999) throw new Error("batch_index must be an integer from 1 to 999999");
  if (raw.constraints !== undefined && (!Array.isArray(raw.constraints) || !raw.constraints.every((item) => typeof item === "string"))) throw new Error("constraints must be an array of strings");
  const constraints = (raw.constraints ?? []) as string[];
  if (constraints.length > 32 || constraints.some((item) => item.length > 1000)) throw new Error("constraints exceed the bounded input size");
  const args: BatchArguments = { scenario_family, count: count as number, batch_index: batch_index as number, brief, constraints };
  const base = (Bun.env.STRATA_BASE_URL ?? "http://127.0.0.1:8080/v1").replace(/\/$/, "");
  const endpoint = new URL(base);
  const endpointHost = endpoint.hostname.replace(/^\[|\]$/g, "");
  if (endpoint.protocol !== "http:" || !["localhost", "127.0.0.1", "::1"].includes(endpointHost)) throw new Error("STRATA_BASE_URL must use HTTP on localhost, 127.0.0.1, or ::1");
  const model = Bun.env.STRATA_MODEL ?? "strata-uncensored";
  let response: Response;
  try {
    response = await fetcher(`${base}/chat/completions`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        model,
        temperature: 0.7,
        max_tokens: Math.min(12000, 550 * args.count),
        chat_template_kwargs: { enable_thinking: false },
        messages: [
          { role: "system", content: "Generate synthetic narrative-ranking TRAINING examples only. Never claim a synthetic label is human-reviewed. Follow the exact project JSONL contract. Do not invent known canon or executable effects." },
          { role: "user", content: [
            `Create exactly ${args.count} diverse ranking examples for scenario family ${args.scenario_family}.`,
            "Each query describes narrative context and eligible storylet candidates. `positive` and `negative` are concrete, distinct storylet content summaries. The positive must be preferable for the query for explicit feature reasons; the negative must be plausible and eligible, not absurd.",
            `Project brief: ${brief}`,
            `Additional constraints: ${JSON.stringify(constraints)}`,
            `Return only JSON: {"examples":[{"query":string,"positive":string,"negative":string,"positive_features":{${FEATURES.map((key) => `"${key}":number`).join(",")} },"negative_features":{...}}]}.`,
            "Every numeric feature is a finite number from 0 to 1. No split, label_source, provenance, or example_id fields; the server assigns these. Never produce validation or held_out examples.",
          ].join("\n") },
        ],
      }),
      signal: AbortSignal.timeout(120_000),
    });
  } catch (error) {
    throw new Error(`Strata request failed: ${error instanceof Error ? error.message : String(error)}`);
  }
  if (!response.ok) throw new Error(`Strata returned HTTP ${response.status}`);
  const completion = await response.json() as { choices?: Array<{ message?: { content?: unknown } }> };
  const content = completion.choices?.[0]?.message?.content;
  if (typeof content !== "string") throw new Error("Strata returned no completion text");
  let parsed: unknown;
  try { parsed = JSON.parse(content); } catch { throw new Error("Strata returned invalid JSON; no partial batch was accepted"); }
  return validateBatchPayload(parsed, args);
}


type PlatformArgs = Record<string, unknown>;

const platformOperations = [
  "platform.health", "workspace.inspect", "workspace.validate", "workspace.replace",
  "content.list", "content.save", "content.delete", "storylet.approve",
  "scenarios.list", "scenarios.get", "scenarios.create", "scenarios.save", "scenarios.delete", "craft_resources.list",
  "generation.list", "generation.create", "generation.get", "generation.advance", "generation.edit_stage", "generation.save_draft", "generation.delete",
  "playtest.list", "playtest.create_from_workspace", "playtest.create", "playtest.snapshot", "playtest.director", "playtest.rank", "playtest.command", "playtest.choose", "playtest.fork", "playtest.events", "playtest.delete",
] as const;

function textArg(args: PlatformArgs, key: string): string {
  const value = args[key];
  if (typeof value !== "string" || !value.trim() || value.length > 500) throw new Error(key + " must be a non-empty string (maximum 500 characters)");
  return value;
}

function objectArg(args: PlatformArgs, key: string): Record<string, unknown> {
  const value = args[key];
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(key + " must be a JSON object");
  return value as Record<string, unknown>;
}

function revisionArg(args: PlatformArgs): number {
  const value = args.expectedRevision;
  if (!Number.isSafeInteger(value) || (value as number) < 0) throw new Error("expectedRevision must be a non-negative safe integer");
  return value as number;
}

export function buildPlatformRequest(input: unknown): { path: string; method: string; body?: unknown } {
  if (!input || typeof input !== "object" || Array.isArray(input)) throw new Error("Arguments must be an object");
  const args = input as PlatformArgs;
  const operation = textArg(args, "operation");
  const id = () => encodeURIComponent(textArg(args, "id"));
  const payload = (key = "payload") => objectArg(args, key);
  const revision = () => ({ expectedRevision: revisionArg(args) });

  switch (operation) {
    case "platform.health": return { path: "/api/health", method: "GET" };
    case "workspace.inspect": return { path: "/api/v1/project-bundle", method: "GET" };
    case "workspace.validate": return { path: "/api/v1/project-bundle/validate", method: "POST", body: { bundle: payload("bundle") } };
    case "workspace.replace":
      if (args.confirmReplace !== true) throw new Error("workspace.replace requires confirmReplace=true after successful validation");
      return { path: "/api/v1/project-bundle/import", method: "POST", body: { bundle: payload("bundle"), confirmReplace: true } };
    case "content.list":
    case "content.save":
    case "content.delete": {
      const entity = textArg(args, "entity");
      const routes: Record<string, string> = { qualities: "/api/qualities", characters: "/api/characters", locations: "/api/locations", storylets: "/api/storylets", npc_behaviors: "/api/npc-behaviors" };
      if (!routes[entity] || (operation === "content.delete" && entity === "qualities")) throw new Error("Invalid entity for content operation");
      const method = operation === "content.list" ? "GET" : operation === "content.save" ? "POST" : "DELETE";
      return { path: routes[entity] + (operation === "content.delete" ? "/" + id() : ""), method, ...(operation === "content.save" ? { body: payload() } : {}) };
    }
    case "storylet.approve": return { path: "/api/storylets/" + id() + "/approve", method: "POST", body: revision() };
    case "scenarios.list": return { path: "/api/v1/scenarios", method: "GET" };
    case "scenarios.get": return { path: "/api/v1/scenarios/" + id(), method: "GET" };
    case "scenarios.create": return { path: "/api/v1/scenarios", method: "POST", body: payload() };
    case "scenarios.save": return { path: "/api/v1/scenarios/" + id(), method: "PUT", body: payload() };
    case "scenarios.delete": return { path: "/api/v1/scenarios/" + id(), method: "DELETE" };
    case "craft_resources.list": return { path: "/api/v1/craft-resources", method: "GET" };
    case "generation.list": return { path: "/api/v1/generation-jobs", method: "GET" };
    case "generation.create": return { path: "/api/v1/generation-jobs", method: "POST", body: payload() };
    case "generation.get": return { path: "/api/v1/generation-jobs/" + id(), method: "GET" };
    case "generation.advance": return { path: "/api/v1/generation-jobs/" + id() + "/advance", method: "POST", body: revision() };
    case "generation.edit_stage": return { path: "/api/v1/generation-jobs/" + id() + "/stages/" + encodeURIComponent(textArg(args, "stage")), method: "PUT", body: { ...revision(), artifact: args.artifact } };
    case "generation.save_draft": return { path: "/api/v1/generation-jobs/" + id() + "/draft", method: "POST", body: {} };
    case "generation.delete": return { path: "/api/v1/generation-jobs/" + id(), method: "DELETE" };
    case "playtest.list": return { path: "/api/v1/playtests", method: "GET" };
    case "playtest.create_from_workspace": return { path: "/api/v1/playtests/from-workspace", method: "POST", body: { startLocationId: args.startLocationId, scenarioId: args.scenarioId } };
    case "playtest.create": return { path: "/api/v1/playtests", method: "POST", body: payload() };
    case "playtest.snapshot": return { path: "/api/v1/playtests/" + id(), method: "GET" };
    case "playtest.director": {
      const slope = args.targetTensionSlope ?? 0.15;
      if (typeof slope !== "number" || !Number.isFinite(slope)) throw new Error("targetTensionSlope must be finite");
      return { path: "/api/v1/playtests/" + id() + "/director?targetTensionSlope=" + encodeURIComponent(String(slope)), method: "GET" };
    }
    case "playtest.rank": return { path: "/api/v1/playtests/" + id() + "/director/rank", method: "POST", body: {} };
    case "playtest.command": return { path: "/api/v1/playtests/" + id() + "/commands", method: "POST", body: { command: payload("command") } };
    case "playtest.choose": return { path: "/api/v1/playtests/" + id() + "/choices", method: "POST", body: { storyletId: textArg(args, "storyletId"), choiceId: textArg(args, "choiceId"), idempotencyKey: typeof args.idempotencyKey === "string" ? args.idempotencyKey : crypto.randomUUID(), expectedRevision: revisionArg(args) } };
    case "playtest.fork": return { path: "/api/v1/playtests/" + id() + "/fork", method: "POST", body: { eventId: textArg(args, "eventId"), ...(typeof args.branchSessionId === "string" ? { branchSessionId: args.branchSessionId } : {}) } };
    case "playtest.events": return { path: "/api/v1/playtests/" + id() + "/events", method: "GET" };
    case "playtest.delete": return { path: "/api/v1/playtests/" + id(), method: "DELETE" };
    default: throw new Error("Unsupported narrative operation: " + operation);
  }
}

export async function callNarrativePlatform(input: unknown, fetcher: typeof fetch = fetch, configuredBase = Bun.env.NARRATIVE_API_BASE_URL ?? "http://127.0.0.1:3000"): Promise<unknown> {
  const request = buildPlatformRequest(input);
  const base = new URL(configuredBase);
  const hostname = base.hostname.replace(/^\[|\]$/g, "");
  if (base.protocol !== "http:" || !["localhost", "127.0.0.1", "::1"].includes(hostname) || base.username || base.password || base.pathname !== "/" || base.search || base.hash) {
    throw new Error("NARRATIVE_API_BASE_URL must be an HTTP loopback origin without credentials, path, query, or fragment");
  }
  let response: Response;
  try {
    response = await fetcher(new URL(request.path, base), {
      method: request.method,
      headers: request.body === undefined ? undefined : { "Content-Type": "application/json" },
      body: request.body === undefined ? undefined : JSON.stringify(request.body),
      signal: AbortSignal.timeout(120_000),
    });
  } catch (error) {
    throw new Error("Narrative platform unavailable at " + base.origin + ": " + (error instanceof Error ? error.message : String(error)));
  }
  const text = await response.text();
  if (text.length > 2_000_000) throw new Error("Narrative platform response exceeded 2 MB");
  let parsed: unknown = null;
  if (text) { try { parsed = JSON.parse(text); } catch { parsed = text; } }
  if (!response.ok) throw new Error("Narrative platform returned HTTP " + response.status + ": " + (typeof parsed === "string" ? parsed : JSON.stringify(parsed)));
  return parsed;
}

const platformTool = {
  name: "narrative_platform",
  description: "Operate the local DecisionNarrative platform: inspect and validate workspace content, manage project entities, run staged Strata-backed generation, and exercise deterministic Rust playtests. Bundle replacement requires explicit confirmation; storylet approval is revision-checked. Synthetic ranking labels remain training-only.",
  inputSchema: {
    type: "object", additionalProperties: false, required: ["operation"],
    properties: {
      operation: { type: "string", enum: platformOperations },
      entity: { type: "string", enum: ["qualities", "characters", "locations", "storylets", "npc_behaviors"] },
      id: { type: "string", minLength: 1, maxLength: 500 }, payload: { type: "object" }, bundle: { type: "object" }, confirmReplace: { type: "boolean" },
      expectedRevision: { type: "integer", minimum: 0 }, stage: { type: "string" }, artifact: {}, startLocationId: { type: "string" }, scenarioId: { type: "string" },
      command: { type: "object" }, storyletId: { type: "string" }, choiceId: { type: "string" }, idempotencyKey: { type: "string" },
      eventId: { type: "string" }, branchSessionId: { type: "string" }, targetTensionSlope: { type: "number" },
    },
  },
};

const tools = [platformTool, {
    name: "generate_storylet_content_batch",
    description: "Generate 1-24 valid DecisionNarrative StoryletRecord draft candidates using local Strata. Returns candidate records with deterministic IDs and approved_revision=null. This tool only generates candidates; it does not approve or save content. The caller must review candidates and persist them via narrative_platform content.save.",
    inputSchema: {
      type: "object",
      additionalProperties: false,
      required: ["scenario_family", "count", "brief"],
      properties: {
        scenario_family: { type: "string", minLength: 1, maxLength: 80, pattern: "^[a-zA-Z0-9][a-zA-Z0-9._-]*$" },
        count: { type: "integer", minimum: 1, maximum: 24 },
        batch_index: { type: "integer", minimum: 1, maximum: 999999, default: 1 },
        brief: { type: "string", minLength: 1, maxLength: 12000 },
        constraints: { type: "array", maxItems: 32, items: { type: "string", maxLength: 1000 } },
      },
    },
  },
  {
  name: "generate_storylet_ranking_batch",
  description: "Generate 1-24 Strata-synthetic, training-only storylet ranking examples matching model/ranking-example.schema.json. Each example contains a query and a positive/negative storylet content pair with the nine bounded ranking features. Never use generated rows for validation or held-out evaluation.",
  inputSchema: {
    type: "object",
    additionalProperties: false,
    required: ["scenario_family", "count", "brief"],
    properties: {
      scenario_family: { type: "string", minLength: 1, maxLength: 80, pattern: "^[a-zA-Z0-9][a-zA-Z0-9._-]*$" },
      count: { type: "integer", minimum: 1, maximum: 24 },
      batch_index: { type: "integer", minimum: 1, maximum: 999999, default: 1 },
      brief: { type: "string", minLength: 1, maxLength: 12000 },
      constraints: { type: "array", maxItems: 32, items: { type: "string", maxLength: 1000 } },
    },
  },
}];

function send(message: unknown) { process.stdout.write(JSON.stringify(message) + "\n"); }

export async function handleMessage(message: any): Promise<void> {
  const id = message?.id;
  const method = message?.method;
  if (typeof method !== "string") return;
  if (method === "notifications/initialized" || method.startsWith("notifications/")) return;
  if (method === "initialize") {
    send({ jsonrpc: "2.0", id, result: { protocolVersion: PROTOCOL_VERSION, capabilities: { tools: { listChanged: false } }, serverInfo: { name: SERVER_NAME, version: SERVER_VERSION } } });
    return;
  }
  if (method === "ping") { send({ jsonrpc: "2.0", id, result: {} }); return; }
  if (method === "tools/list") { send({ jsonrpc: "2.0", id, result: { tools } }); return; }
  if (method === "tools/call") {
    const name = message.params?.name;
    if (name !== "generate_storylet_ranking_batch" && name !== "generate_storylet_content_batch" && name !== "narrative_platform") {
      send({ jsonrpc: "2.0", id, result: toolError(`Unknown tool: ${String(name)}`) });
      return;
    }
    try {
      if (name === "narrative_platform") {
        const result = await callNarrativePlatform(message.params?.arguments);
        send({ jsonrpc: "2.0", id, result: { content: [{ type: "text", text: JSON.stringify(result) }] } });
        return;
      }
      if (name === "generate_storylet_content_batch") {
        const storylets = await generateContentBatch(message.params?.arguments);
        send({ jsonrpc: "2.0", id, result: { content: [{ type: "text", text: JSON.stringify(storylets, null, 2) }] } });
        return;
      }
      const rows = await generateBatch(message.params?.arguments);
      send({ jsonrpc: "2.0", id, result: { content: [{ type: "text", text: rows.map((row) => JSON.stringify(row)).join("\n") }] } });
    } catch (error) { send({ jsonrpc: "2.0", id, result: toolError(error instanceof Error ? error.message : String(error)) }); }
    return;
  }
  send({ jsonrpc: "2.0", id, error: { code: -32601, message: `Method not found: ${method}` } });
}

if (import.meta.main) {
  const lines = createInterface({ input: process.stdin, crlfDelay: Infinity });
  for await (const line of lines) {
    if (!line.trim()) continue;
    let message: unknown;
    try { message = JSON.parse(line); }
    catch { send({ jsonrpc: "2.0", id: null, error: { code: -32700, message: "Parse error" } }); continue; }
    await handleMessage(message);
  }
}
