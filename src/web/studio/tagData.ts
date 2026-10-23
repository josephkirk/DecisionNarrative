export const TAG_KINDS = [
  { id: "role", label: "Character roles" },
  { id: "location", label: "Location tags" },
  { id: "emotion", label: "Emotion tags" },
  { id: "action", label: "Action tags" },
] as const;

export type TagKind = (typeof TAG_KINDS)[number]["id"];
export type TagEntityType = "character" | "location" | "storylet";

export interface TagReference {
  entityId: string;
  entityType: TagEntityType;
  entityLabel: string;
  detail: string;
  nodeId?: string;
  itemId?: string;
}

export interface TagEntry {
  kind: TagKind;
  name: string;
  references: TagReference[];
}

export interface TagIndexInput {
  characters: Array<{ id: string; name: string; role: string }>;
  locations: Array<{ id: string; name: string; tags?: string[] }>;
  storylets: Array<{
    id: string;
    title: string;
    location_tags?: string[];
    parameters?: Array<{ param_name: string }>;
    graph?: unknown;
  }>;
}

export function normalizeTagName(value: unknown): string {
  return typeof value === "string" ? value.trim().toLowerCase() : "";
}

export function uniqueTagNames(values: readonly unknown[]): string[] {
  const seen = new Set<string>();
  const names: string[] = [];
  for (const value of values) {
    if (typeof value !== "string") continue;
    const name = value.trim();
    const key = normalizeTagName(name);
    if (!key || seen.has(key)) continue;
    seen.add(key);
    names.push(name);
  }
  return names;
}

function asObject(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === "object" ? value as Record<string, unknown> : null;
}

function addGraphReferences(storylet: TagIndexInput["storylets"][number], add: (kind: TagKind, name: unknown, reference: TagReference) => void): void {
  const graph = asObject(storylet.graph);
  const nodes = Array.isArray(graph?.nodes) ? graph.nodes : [];
  for (const nodeValue of nodes) {
    const node = asObject(nodeValue);
    if (!node) continue;
    const nodeId = typeof node.id === "string" ? node.id : undefined;
    const nodeLabel = typeof node.title === "string" ? node.title : String(node.type ?? "Graph node");
    const base: TagReference = {
      entityId: storylet.id,
      entityType: "storylet",
      entityLabel: storylet.title || storylet.id,
      detail: nodeLabel,
      ...(nodeId ? { nodeId } : {}),
    };
    if (node.type === "dialog") add("role", node.speaker_role, base);
    for (const sectionValue of Array.isArray(node.sections) ? node.sections : []) {
      const section = asObject(sectionValue);
      for (const itemValue of Array.isArray(section?.items) ? section.items : []) {
        const item = asObject(itemValue);
        if (!item) continue;
        const itemId = typeof item.id === "string" ? item.id : undefined;
        const detail = `${nodeLabel} · ${typeof section?.title === "string" ? section.title : "Dialogue"}`;
        const reference = { ...base, detail, ...(itemId ? { itemId } : {}) };
        if (item.kind === "line") {
          add("role", item.speaker_role, reference);
          add("role", item.target_role === "player" ? "" : item.target_role, reference);
          add("role", item.action_target === "player" ? "" : item.action_target, reference);
          add("emotion", item.emotion_tag, reference);
          add("action", item.action_tag, reference);
        } else if (item.kind === "idle_action") {
          add("role", item.actor_role, reference);
          add("role", item.target === "player" ? "" : item.target, reference);
          add("action", item.emote, reference);
        }
      }
    }
  }
}

export function buildTagIndex(input: TagIndexInput): TagEntry[] {
  const entries = new Map<string, { kind: TagKind; name: string; references: Map<string, TagReference> }>();
  const add = (kind: TagKind, rawName: unknown, reference: TagReference) => {
    if (typeof rawName !== "string") return;
    const name = rawName.trim();
    const normalized = normalizeTagName(name);
    if (!normalized) return;
    const key = `${kind}\u0000${normalized}`;
    let entry = entries.get(key);
    if (!entry) {
      entry = { kind, name, references: new Map() };
      entries.set(key, entry);
    }
    const referenceKey = `${reference.entityType}:${reference.entityId}`;
    const previous = entry.references.get(referenceKey);
    if (!previous || (!previous.itemId && reference.itemId)) entry.references.set(referenceKey, reference);
  };

  for (const character of input.characters) {
    add("role", character.role, { entityId: character.id, entityType: "character", entityLabel: character.name || character.id, detail: "Character role" });
  }
  for (const location of input.locations) {
    for (const tag of uniqueTagNames(location.tags ?? [])) {
      add("location", tag, { entityId: location.id, entityType: "location", entityLabel: location.name || location.id, detail: "Location tag" });
    }
  }
  for (const storylet of input.storylets) {
    for (const tag of uniqueTagNames(storylet.location_tags ?? [])) {
      add("location", tag, { entityId: storylet.id, entityType: "storylet", entityLabel: storylet.title || storylet.id, detail: "Storylet location requirement" });
    }
    for (const parameter of storylet.parameters ?? []) {
      add("role", parameter.param_name, { entityId: storylet.id, entityType: "storylet", entityLabel: storylet.title || storylet.id, detail: "Required character role" });
    }
    addGraphReferences(storylet, add);
  }

  return [...entries.values()]
    .map((entry) => ({ ...entry, references: [...entry.references.values()].sort((a, b) => a.entityLabel.localeCompare(b.entityLabel)) }))
    .sort((a, b) => {
      const order = TAG_KINDS.findIndex((kind) => kind.id === a.kind) - TAG_KINDS.findIndex((kind) => kind.id === b.kind);
      return order || a.name.localeCompare(b.name);
    });
}
