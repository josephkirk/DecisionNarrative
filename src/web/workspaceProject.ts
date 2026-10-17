import type {
  CharacterRecord,
  LocationRecord,
  NpcBehaviorRecord,
  QualityRecord,
  ScenarioRecord,
  StartLocationPolicy,
  ActorPopulationPolicy,
  ActorLocationPolicy,
  StoryletRecord,
  ScenarioEventRecord,
} from "../db/repository.ts";
import type { RustProjectBundle } from "../engine/RustRuntimeClient.ts";
import { buildGraphFromStorylet, validateNarrativeGraph } from "./studio/narrativeGraphTypes.ts";

interface WorkspaceRecords {
  qualities: QualityRecord[];
  locations: LocationRecord[];
  characters: CharacterRecord[];
  storylets: StoryletRecord[];
  npcBehaviors?: NpcBehaviorRecord[];
  scenarios?: ScenarioRecord[];
}

export interface WorkspaceProjectOptions {
  playerStartLocation?: string;
  scenario?: ScenarioRecord;
  projectId?: string;
  projectName?: string;
  projectRevision?: number;
}

export interface WorkspaceProjectResult {
  project: RustProjectBundle;
  scenarioId: string;
  issues: string[];
}

type RustPredicate = {
  fact: Record<string, unknown>;
  op: "eq" | "neq" | "gt" | "gte" | "lt" | "lte" | "contains";
  value: unknown;
};

function uniqueId(base: string, existing: Set<string>): string {
  let candidate = base;
  let suffix = 1;
  while (existing.has(candidate)) candidate = `${base}-${suffix++}`;
  existing.add(candidate);
  return candidate;
}

function normalizeOp(op: string): RustPredicate["op"] | null {
  const normalized = op.trim().toLowerCase();
  const aliases: Record<string, RustPredicate["op"]> = {
    "=": "eq", "==": "eq", eq: "eq",
    "!=": "neq", "<>": "neq", neq: "neq",
    ">": "gt", gt: "gt", ">=": "gte", gte: "gte",
    "<": "lt", lt: "lt", "<=": "lte", lte: "lte",
    contains: "contains",
  };
  return aliases[normalized] ?? null;
}

function mapPredicate(
  source: { variable: string; op: string; value: unknown },
  qualityIds: Set<string>,
  issues: string[],
  path: string,
): RustPredicate | null {
  const op = normalizeOp(source.op);
  if (!op) {
    issues.push(`${path}: unsupported predicate operator '${source.op}'`);
    return null;
  }
  const variable = source.variable.trim();
  let fact: Record<string, unknown>;
  if (variable === "location" || variable === "current_location") {
    fact = { kind: "current_location" };
  } else if (qualityIds.has(variable)) {
    fact = { kind: "quality", key: variable };
  } else {
    issues.push(`${path}: unknown quality '${variable}'`);
    return null;
  }
  return { fact, op, value: source.value };
}

function mapPredicates(
  predicates: Array<{ variable: string; op: string; value: unknown }> | undefined,
  qualityIds: Set<string>,
  issues: string[],
  path: string,
): RustPredicate[] {
  return (predicates ?? []).flatMap((predicate, index) => {
    const mapped = mapPredicate(predicate, qualityIds, issues, `${path}[${index}]`);
    return mapped ? [mapped] : [];
  });
}

function mapEffects(
  effects: Record<string, unknown>,
  qualityIds: Set<string>,
  issues: string[],
  path: string,
): Array<Record<string, unknown>> {
  const mapped: Array<Record<string, unknown>> = [];
  for (const [key, value] of Object.entries(effects ?? {})) {
    if (!qualityIds.has(key)) {
      issues.push(`${path}.${key}: unknown quality '${key}'`);
      continue;
    }
    if (typeof value === "number" && Number.isFinite(value)) {
      mapped.push({ kind: "add_quality", key, amount: value });
    } else if (typeof value === "boolean" || typeof value === "string") {
      mapped.push({ kind: "set_quality", key, value });
    } else {
      issues.push(`${path}.${key}: only numeric deltas and boolean/string sets can be converted`);
    }
  }
  return mapped;
}

function mapRoleQuery(query: string): string | null {
  const match = query.trim().match(/^(?:(?:character\.)?role|character_role)\s*==\s*(['"])(.*?)\1$/i);
  return match?.[2]?.trim() || null;
}


/** PRNG created deterministically from seed (xorshift32) */
export function createDeterministicRng(seed: number): () => number {
  let s = (Math.floor(seed) >>> 0) || 1;
  return function next(): number {
    s = (s ^ (s << 13)) >>> 0;
    s = (s ^ (s >>> 17)) >>> 0;
    s = (s ^ (s << 5)) >>> 0;
    return (s >>> 0) / 4294967296;
  };
}

export function sampleDeterministic<T>(items: T[], count: number, rng: () => number): T[] {
  if (count >= items.length) return [...items];
  if (count <= 0) return [];
  const pool = [...items];
  const result: T[] = [];
  for (let i = 0; i < count; i++) {
    const idx = Math.floor(rng() * pool.length);
    result.push(pool[idx]);
    pool.splice(idx, 1);
  }
  return result;
}

export function matchAttributeValue(actual: unknown, expected: unknown): boolean {
  if (actual === undefined && expected === undefined) return true;
  if (actual === undefined || expected === undefined) return false;
  if (typeof actual === "number" && typeof expected === "number") {
    return actual === expected;
  }
  if (typeof actual === "boolean" && typeof expected === "boolean") {
    return actual === expected;
  }
  const actualStr = String(actual).trim().toLowerCase();
  const expectedStr = String(expected).trim().toLowerCase();
  return actualStr === expectedStr;
}

export function getLocationAttribute(loc: LocationRecord, key: string): unknown {
  const trimmedKey = key.trim();
  if (trimmedKey === "id") return loc.id;
  if (trimmedKey === "name") return loc.name;
  if (trimmedKey === "description") return loc.description;
  if (loc.environmental_params && Object.prototype.hasOwnProperty.call(loc.environmental_params, trimmedKey)) {
    return loc.environmental_params[trimmedKey];
  }
  if (loc.devices && Object.prototype.hasOwnProperty.call(loc.devices, trimmedKey)) {
    return loc.devices[trimmedKey];
  }
  return undefined;
}

export function matchLocation(loc: LocationRecord, key: string, expectedValue: unknown): boolean {
  const actual = getLocationAttribute(loc, key);
  return matchAttributeValue(actual, expectedValue);
}

export function getCharacterAttribute(char: CharacterRecord, key: string): unknown {
  const trimmedKey = key.trim();
  if (trimmedKey === "id") return char.id;
  if (trimmedKey === "name") return char.name;
  if (trimmedKey === "role") return char.role;
  if (trimmedKey === "home_location") return char.home_location;
  if (trimmedKey === "current_location") return char.current_location;
  if (trimmedKey === "agenda") return char.agenda;
  if (trimmedKey === "red_line") return char.red_line;
  if (char.status_axes && Object.prototype.hasOwnProperty.call(char.status_axes, trimmedKey)) {
    return char.status_axes[trimmedKey];
  }
  if (char.qualities && Object.prototype.hasOwnProperty.call(char.qualities, trimmedKey)) {
    return char.qualities[trimmedKey];
  }
  if (char.needs && Object.prototype.hasOwnProperty.call(char.needs, trimmedKey)) {
    return char.needs[trimmedKey];
  }
  return undefined;
}

export function matchCharacter(char: CharacterRecord, key: string, expectedValue: unknown): boolean {
  const actual = getCharacterAttribute(char, key);
  return matchAttributeValue(actual, expectedValue);
}

export function resolveLocationFromPolicy(
  policy: StartLocationPolicy | ActorLocationPolicy | undefined,
  fallbackLocationId: string,
  locations: LocationRecord[],
  rng: () => number,
  issues: string[],
  pathPrefix: string,
): string {
  if (!policy || policy.mode === "fixed") {
    const locId = policy?.location_id || fallbackLocationId;
    if (!locId) {
      issues.push(`${pathPrefix}: fixed location mode requires a location`);
      return "";
    }
    const found = locations.find((l) => l.id === locId);
    if (!found) {
      issues.push(`${pathPrefix}: unknown fixed location '${locId}'`);
      return "";
    }
    return found.id;
  }

  if (policy.mode === "random") {
    if (!locations.length) {
      issues.push(`${pathPrefix}: cannot select random location from empty locations list`);
      return "";
    }
    const idx = Math.floor(rng() * locations.length);
    return locations[idx].id;
  }

  if (policy.mode === "by_attribute") {
    const attrKey = policy.attribute_key?.trim();
    if (!attrKey) {
      issues.push(`${pathPrefix}: by_attribute mode requires an attribute key`);
      return "";
    }
    const matches = locations.filter((l) => matchLocation(l, attrKey, policy.attribute_value));
    if (!matches.length) {
      issues.push(`${pathPrefix}: no location matches attribute '${attrKey}' = ${JSON.stringify(policy.attribute_value)}`);
      return "";
    }
    const idx = Math.floor(rng() * matches.length);
    return matches[idx].id;
  }

  issues.push(`${pathPrefix}: unsupported location policy mode '${(policy as any).mode}'`);
  return fallbackLocationId;
}

export function resolveActorPopulation(
  scenario: ScenarioRecord | undefined,
  characters: CharacterRecord[],
  locations: LocationRecord[],
  seed: number,
  issues: string[],
  pathPrefix: string,
): Record<string, string> {
  const rng = createDeterministicRng(seed);
  const locationIds = new Set(locations.map((l) => l.id));
  const policy = scenario?.actor_population_policy;

  if (!policy) {
    const result: Record<string, string> = {};
    if (scenario?.actor_locations) {
      for (const [actorId, locId] of Object.entries(scenario.actor_locations)) {
        result[actorId] = locId;
      }
    } else {
      for (const character of characters) {
        if (locationIds.has(character.current_location)) {
          result[character.id] = character.current_location;
        }
      }
    }
    return result;
  }

  let selectedCharacters: CharacterRecord[] = [];
  if (policy.mode === "all") {
    selectedCharacters = [...characters];
  } else if (policy.mode === "random") {
    if (policy.count === undefined || !Number.isInteger(policy.count) || policy.count < 0) {
      issues.push(`${pathPrefix}.count: random actor population requires a non-negative integer count`);
      return {};
    }
    selectedCharacters = sampleDeterministic(characters, policy.count, rng);
  } else if (policy.mode === "by_attribute") {
    const attrKey = policy.attribute_key?.trim();
    if (!attrKey) {
      issues.push(`${pathPrefix}.attribute_key: by_attribute actor population requires an attribute key`);
      return {};
    }
    const matches = characters.filter((c) => matchCharacter(c, attrKey, policy.attribute_value));
    if (!matches.length) {
      issues.push(`${pathPrefix}: no character matches attribute '${attrKey}' = ${JSON.stringify(policy.attribute_value)}`);
      return {};
    }
    selectedCharacters = matches;
  } else {
    issues.push(`${pathPrefix}.mode: unsupported actor population mode '${(policy as any).mode}'`);
    return {};
  }

  const result: Record<string, string> = {};
  for (const character of selectedCharacters) {
    const locPolicy = policy.location_policy;
    if (!locPolicy || locPolicy.mode === "fixed") {
      const fixedLoc = locPolicy?.location_id || scenario?.actor_locations?.[character.id] || character.current_location || character.home_location;
      if (!locationIds.has(fixedLoc)) {
        issues.push(`${pathPrefix}.${character.id}: unknown location '${fixedLoc}'`);
      } else {
        result[character.id] = fixedLoc;
      }
    } else {
      const resolvedLoc = resolveLocationFromPolicy(
        locPolicy,
        character.current_location || character.home_location,
        locations,
        rng,
        issues,
        `${pathPrefix}.${character.id}.location`,
      );
      if (resolvedLoc) {
        result[character.id] = resolvedLoc;
      }
    }
  }

  return result;
}

/** Build a disposable Rust project bundle from the current designer Codex. */
export function buildWorkspaceProject(
  records: WorkspaceRecords,
  playerStartLocationOrOptions?: string | WorkspaceProjectOptions,
): WorkspaceProjectResult {
  const options = typeof playerStartLocationOrOptions === "string"
    ? { playerStartLocation: playerStartLocationOrOptions }
    : playerStartLocationOrOptions ?? {};
  const issues: string[] = [];
  const locations = [...records.locations].sort((a, b) => a.name.localeCompare(b.name));
  const characters = [...records.characters].sort((a, b) => a.id.localeCompare(b.id));
  const storylets = [...records.storylets].sort((a, b) => a.id.localeCompare(b.id));
  const npcBehaviors = [...(records.npcBehaviors ?? [])].sort((a, b) => b.priority - a.priority || a.id.localeCompare(b.id));
  const qualityIds = new Set(records.qualities.map((quality) => quality.name));
  const usedIds = new Set([
    ...qualityIds,
    ...locations.map((location) => location.id),
    ...characters.map((character) => character.id),
    ...storylets.map((storylet) => storylet.id),
    ...npcBehaviors.map((rule) => rule.id),
  ]);
  const projectId = options.projectId?.trim();
  if (projectId) {
    if (usedIds.has(projectId)) issues.push(`project.id: '${projectId}' is already used by another project record`);
    usedIds.add(projectId);
  }
  const resolvedProjectId = projectId || uniqueId("workspace-project", usedIds);
  const authoredScenario = options.scenario;
  let scenarioId: string;
  if (authoredScenario) {
    scenarioId = authoredScenario.id;
    if (usedIds.has(scenarioId)) issues.push(`scenarios.${scenarioId}: ID is already used by another project record`);
    usedIds.add(scenarioId);
  } else {
    scenarioId = uniqueId("workspace-playtest", usedIds);
  }

  const qualities = Object.fromEntries(records.qualities.map((quality) => [quality.name, {
    key: quality.name,
    kind: quality.type,
    default_value: quality.value,
    description: quality.description ?? "",
  }]));
  const locationIds = new Set(locations.map((location) => location.id));
  for (const location of locations) {
    for (const connectedId of location.connected_zones ?? []) {
      if (!locationIds.has(connectedId)) {
        issues.push(`locations.${location.id}: unknown connected location '${connectedId}'`);
      }
    }
  }
  const locationDefinitions = Object.fromEntries(locations.map((location, index) => {
    const configuredPosition = location.environmental_params?.map_position;
    const position = Array.isArray(configuredPosition) && configuredPosition.length === 2
      ? [Number(configuredPosition[0]), Number(configuredPosition[1])]
      : [(index % 5) * 3, Math.floor(index / 5) * 2];
    return [location.id, {
      id: location.id,
      name: location.name,
      description: location.description,
      tags: location.tags ?? [],
      connected_locations: (location.connected_zones ?? []).filter((id) => locationIds.has(id)),
      map_position: position,
    }];
  }));

  const characterDefinitions = Object.fromEntries(characters.map((character) => [character.id, {
    id: character.id,
    name: character.name,
    description: character.agenda || "",
    role_tags: character.role.trim() ? [character.role.trim()] : [],
    home_location: character.home_location,
    goals: character.agenda ? [character.agenda] : [],
    red_lines: character.red_line ? [character.red_line] : [],
    voice_profile: "",
    schedule: character.schedule ?? {},
  }]));

  const storyletDefinitions: Record<string, unknown> = {};
  for (const storylet of storylets) {
    if (storylet.revision == null || storylet.approved_revision !== storylet.revision) continue;
    const path = `storylets.${storylet.id}`;
    for (const issue of validateNarrativeGraph(buildGraphFromStorylet(storylet))) {
      issues.push(`${path}.graph: ${issue}`);
    }
    const roles: Array<{ role: string }> = [];
    for (const parameter of storylet.parameters ?? []) {
      const role = mapRoleQuery(parameter.query);
      if (!role) issues.push(`${path}.parameters.${parameter.param_name}: unsupported role query '${parameter.query}'`);
      else roles.push({ role });
    }
    const trigger = storylet.preconditions ?? {};
    const choices = (storylet.choices ?? []).map((choice, index) => {
      const choicePath = `${path}.choices[${index}]`;
      const requirements = mapPredicates(choice.conditions, qualityIds, issues, `${choicePath}.conditions`);
      return {
        id: choice.choice_id,
        label: choice.label,
        requirements: { all_of: requirements, any_of: [] },
        effects: mapEffects(choice.effects as Record<string, unknown>, qualityIds, issues, `${choicePath}.effects`),
        continues_scene: false,
        divert_target: choice.divert_target ?? null,
      };
    });
    const repeatability = storylet.repeatability === "cooldown"
      ? { kind: "cooldown", turns: Math.max(0, storylet.cooldown_turns ?? 0) }
      : storylet.repeatability === "once_ever"
        ? { kind: "once_ever" }
        : storylet.repeatability === "persistent"
          ? { kind: "persistent" }
          : { kind: "always" };
    storyletDefinitions[storylet.id] = {
      id: storylet.id,
      revision: storylet.revision ?? 1,
      title: storylet.title,
      summary: storylet.content.text_template,
      approved: true,
      ending: storylet.ending === true,
      location_tags: storylet.location_tags ?? [],
      urgency_weight: storylet.urgency_weight,
      cluster_type: storylet.cluster_type,
      repeatability,
      required_roles: roles,
      trigger: {
        all_of: mapPredicates(trigger.all_of, qualityIds, issues, `${path}.preconditions.all_of`),
        any_of: mapPredicates(trigger.any_of, qualityIds, issues, `${path}.preconditions.any_of`),
      },
      content: { opening: storylet.content.sceneOpening ?? storylet.content.text_template },
      choices,
    };
  }

  const npcBehaviorDefinitions: Record<string, unknown> = {};
  const characterIds = new Set(characters.map((character) => character.id));
  for (const rule of npcBehaviors) {
    const path = `npc_behaviors.${rule.id}`;
    if (!Number.isInteger(rule.priority) || rule.priority < -2147483648 || rule.priority > 2147483647) {
      issues.push(`${path}.priority: must be a 32-bit integer`);
    }
    if (!rule.action.trim()) issues.push(`${path}.action: action text is required`);
    if (!characterIds.has(rule.actor_id)) {
      issues.push(`${path}: unknown character '${rule.actor_id}'`);
    }
    const conditions = rule.conditions ?? {};
    const effects: Array<Record<string, unknown>> = [];
    for (const [index, effect] of (rule.effects ?? []).entries()) {
      const kind = effect.kind;
      if ((kind === "set_quality" || kind === "add_quality") && typeof effect.key === "string") {
        if (!qualityIds.has(effect.key)) {
          issues.push(`${path}.effects[${index}]: unknown quality '${effect.key}'`);
        } else if (kind === "add_quality" && (typeof effect.amount !== "number" || !Number.isFinite(effect.amount))) {
          issues.push(`${path}.effects[${index}]: add_quality amount must be a finite number`);
        } else if (kind === "set_quality" && (effect.value === undefined || typeof effect.value === "number" && !Number.isFinite(effect.value))) {
          issues.push(`${path}.effects[${index}]: set_quality value must be a finite JSON value`);
        } else {
          effects.push(effect);
        }
      } else if (kind === "transfer_quality" && typeof effect.from === "string" && typeof effect.to === "string") {
        for (const key of [effect.from, effect.to]) {
          if (!qualityIds.has(key)) issues.push(`${path}.effects[${index}]: unknown quality '${key}'`);
        }
        if (qualityIds.has(effect.from) && qualityIds.has(effect.to)) {
          if (typeof effect.amount !== "number" || !Number.isFinite(effect.amount) || effect.amount < 0) {
            issues.push(`${path}.effects[${index}]: transfer amount must be a finite nonnegative number`);
          } else effects.push(effect);
        }
      } else {
        issues.push(`${path}.effects[${index}]: unsupported or malformed NPC effect`);
      }
    }
    npcBehaviorDefinitions[rule.id] = {
      id: rule.id,
      actor_id: rule.actor_id,
      priority: rule.priority,
      action: rule.action,
      conditions: {
        all_of: mapPredicates(conditions.all_of, qualityIds, issues, `${path}.conditions.all_of`),
        any_of: mapPredicates(conditions.any_of, qualityIds, issues, `${path}.conditions.any_of`),
      },
      effects,
    };
  }

  const scenarioSeed = authoredScenario?.seed ?? 1;
  const startLocationRng = createDeterministicRng(scenarioSeed);

  let resolvedStartLocation = authoredScenario?.start_location_policy
    ? resolveLocationFromPolicy(
        authoredScenario.start_location_policy,
        authoredScenario.player_start_location || options.playerStartLocation || locations[0]?.id || "",
        locations,
        startLocationRng,
        issues,
        `scenarios.${scenarioId}.start_location_policy`,
      )
    : (authoredScenario?.player_start_location || options.playerStartLocation || locations[0]?.id || "");

  if (!resolvedStartLocation) issues.push("Choose or create a location before starting a playtest");
  else if (!locationIds.has(resolvedStartLocation)) issues.push(`Unknown playtest start location '${resolvedStartLocation}'`);

  let resolvedActorLocations: Record<string, string> = {};
  if (authoredScenario?.actor_population_policy) {
    resolvedActorLocations = resolveActorPopulation(
      authoredScenario,
      characters,
      locations,
      scenarioSeed,
      issues,
      `scenarios.${scenarioId}.actor_population_policy`,
    );
  } else if (authoredScenario?.actor_locations) {
    resolvedActorLocations = { ...authoredScenario.actor_locations };
  } else {
    resolvedActorLocations = Object.fromEntries(
      characters
        .filter((character) => locationIds.has(character.current_location))
        .map((character) => [character.id, character.current_location]),
    );
  }

  if (authoredScenario) {
    if (!authoredScenario.name.trim()) issues.push(`scenarios.${scenarioId}.name: a name is required`);
    if (!Number.isSafeInteger(authoredScenario.seed) || authoredScenario.seed < 0) {
      issues.push(`scenarios.${scenarioId}.seed: use a nonnegative safe integer`);
    }
    for (const [key, value] of Object.entries(authoredScenario.initial_qualities)) {
      if (!qualityIds.has(key)) issues.push(`scenarios.${scenarioId}.initial_qualities.${key}: unknown quality`);
      if (value === null || value === undefined) issues.push(`scenarios.${scenarioId}.initial_qualities.${key}: value cannot be null`);
    }
    for (const [actorId, locationId] of Object.entries(authoredScenario.actor_locations ?? {})) {
      if (!characters.some((character) => character.id === actorId)) issues.push(`scenarios.${scenarioId}.actor_locations.${actorId}: unknown character`);
      if (!locationIds.has(locationId)) issues.push(`scenarios.${scenarioId}.actor_locations.${actorId}: unknown location '${locationId}'`);
    }
    for (const storyletId of authoredScenario.storylet_ids) {
      if (!Object.hasOwn(storyletDefinitions, storyletId)) issues.push(`scenarios.${scenarioId}.storylet_ids: '${storyletId}' is missing, invalid, or unapproved`);
    }
  }
  for (const character of characters) {
    if (!locationIds.has(character.home_location)) {
      issues.push(`characters.${character.id}: unknown home location '${character.home_location}'`);
    }
  }

  const scenarioEvents: Record<string, unknown> = {};
  const scenarioEventIds: string[] = [];
  for (const event of authoredScenario?.event_definitions ?? []) {
    scenarioEventIds.push(event.id);
    const trigger = {
      all_of: mapPredicates(event.trigger.all_of, qualityIds, issues, `events.${event.id}.trigger.all_of`),
      any_of: mapPredicates(event.trigger.any_of, qualityIds, issues, `events.${event.id}.trigger.any_of`),
    };
    scenarioEvents[event.id] = {
      id: event.id, title: event.title, description: event.description, event_type: event.event_type, trigger,
      effects: mapEffects(event.effects ?? {}, qualityIds, issues, `events.${event.id}.effects`),
      delay_turns: event.delay_turns ?? 0, repeatable: event.repeatable ?? false,
    };
  }

  const project: RustProjectBundle = {
    schema_version: 1,
    id: resolvedProjectId,
    name: options.projectName?.trim() || "Workspace",
    revision: options.projectRevision ?? 0,
    qualities,
    locations: locationDefinitions,
    characters: characterDefinitions,
    events: scenarioEvents,
    storylets: storyletDefinitions,
    npc_behaviors: npcBehaviorDefinitions,
    scenarios: resolvedStartLocation ? {
      [scenarioId]: {
        id: scenarioId,
        name: authoredScenario?.name ?? "Playtest",
        player_start_location: resolvedStartLocation,
        initial_qualities: authoredScenario?.initial_qualities ?? {},
        actor_locations: resolvedActorLocations,
        storylet_ids: authoredScenario?.storylet_ids ?? Object.keys(storyletDefinitions),
        seed: scenarioSeed,
        event_ids: scenarioEventIds,
      },
    } : {},
  };

  return { project, scenarioId, issues };
}
