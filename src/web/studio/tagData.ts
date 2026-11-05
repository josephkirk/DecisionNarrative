import type { EntityRecord, EntityTemplateRecord, GameplayTagRecord, StoryletRecord, TraitTemplateRecord } from "./contracts.ts";

export const TAG_KINDS = [{ id: "gameplay", label: "Gameplay tags" }] as const;
export type TagKind = "gameplay";
export type TagEntityType = "entity" | "entity_template" | "storylet" | "trait_template";
export interface TagReference { entityId: string; entityType: TagEntityType; entityLabel: string; detail: string; nodeId?: string; itemId?: string; }
export interface TagEntry { kind: "gameplay"; name: string; references: TagReference[]; registered: boolean; protected: boolean; }
export interface TagIndexInput { gameplayTags: GameplayTagRecord[]; entities: EntityRecord[]; entityTemplates: EntityTemplateRecord[]; storylets: StoryletRecord[]; traitTemplates: TraitTemplateRecord[]; }
export function normalizeTagName(value: unknown): string { return typeof value === "string" ? value.trim().toLowerCase() : ""; }
export function uniqueTagNames(values: readonly unknown[] | undefined): string[] { const seen = new Set<string>(); const result: string[] = []; for (const value of values ?? []) { if (typeof value !== "string") continue; const name = value.trim(); const key = normalizeTagName(name); if (key && !seen.has(key)) { seen.add(key); result.push(name); } } return result; }
function object(value: unknown): Record<string, unknown> | null { return value && typeof value === "object" ? value as Record<string, unknown> : null; }
export function buildTagIndex(input: TagIndexInput): TagEntry[] {
  const map = new Map<string, TagEntry>();
  for (const tag of input.gameplayTags) { const name = tag.name.trim(); if (name && !name.includes("..") && !name.startsWith(".") && !name.endsWith(".")) map.set(normalizeTagName(name), { kind: "gameplay", name, references: [], registered: true, protected: tag.protected }); }
  const add = (raw: unknown, ref: TagReference) => { if (typeof raw !== "string") return; const name = raw.trim(); if (!name || name.includes("..") || name.startsWith(".") || name.endsWith(".")) return; const key = normalizeTagName(name); const entry = map.get(key) ?? { kind: "gameplay" as const, name, references: [], registered: false, protected: false }; map.set(key, entry); const rk = `${ref.entityType}:${ref.entityId}`; if (!entry.references.some((r) => `${r.entityType}:${r.entityId}` === rk)) entry.references.push(ref); };
  for (const entity of input.entities) { const ref = { entityId: entity.id, entityType: "entity" as const, entityLabel: entity.name || entity.id, detail: "Intrinsic entity" }; add(entity.root_tag, { ...ref, detail: "Root tag" }); for (const tag of uniqueTagNames(entity.tags)) add(tag, { ...ref, detail: "Entity tag" }); }
  for (const entity of input.entityTemplates) {
    const ref = { entityId: entity.id, entityType: "entity_template" as const, entityLabel: entity.name || entity.id, detail: "Entity Template" };
    add(entity.root_tag, { ...ref, detail: "Root tag" });
    for (const tag of uniqueTagNames(entity.tags)) add(tag, { ...ref, detail: "Entity tag" });
    for (const trait of entity.traits ?? []) {
      const value = object(trait.values)?.role;
      if (typeof value === "string") add(value, { ...ref, detail: "Character role" });
    }
  }
  for (const storylet of input.storylets) { const ref = { entityId: storylet.id, entityType: "storylet" as const, entityLabel: storylet.title || storylet.id, detail: "Storylet" }; for (const tag of uniqueTagNames((storylet as unknown as { location_tags?: string[] }).location_tags)) add(tag, { ...ref, detail: "Location requirement" }); const walk = (value: unknown, nodeId?: string) => { if (Array.isArray(value)) return value.forEach((v) => walk(v, nodeId)); const item = object(value); if (!item) return; const id = typeof item.id === "string" ? item.id : nodeId; for (const key of ["tag", "tag_name", "gameplay_tag", "emotion_tag", "action_tag", "speaker_role", "target_role"]) add(item[key], { ...ref, detail: key, ...(id ? { nodeId: id } : {}) }); Object.values(item).forEach((v) => { if (typeof v === "object") walk(v, id); }); }; walk((storylet as unknown as { graph?: unknown }).graph); }
  for (const trait of input.traitTemplates) for (const property of trait.properties ?? []) { const value = (property as unknown as { tag?: unknown }).tag; add(value, { entityId: trait.id, entityType: "trait_template", entityLabel: trait.name || trait.id, detail: "Trait tag" }); }
  return [...map.values()].map((e) => ({ ...e, references: e.references.sort((a, b) => a.entityLabel.localeCompare(b.entityLabel)) })).sort((a, b) => a.name.localeCompare(b.name));
}
export interface HierarchicalTagNode { segment: string; fullPath: string; children: HierarchicalTagNode[]; entry?: TagEntry; directCount: number; totalCount: number; }
export function buildHierarchicalTagTree(entries: TagEntry[]): HierarchicalTagNode[] { const roots: HierarchicalTagNode[] = []; const valid = entries.filter((e) => e.name.split(".").every((p) => p.trim())); const get = (list: HierarchicalTagNode[], segment: string, path: string) => list.find((n) => n.segment.toLowerCase() === segment.toLowerCase()) ?? (list[list.push({ segment, fullPath: path, children: [], directCount: 0, totalCount: 0 }) - 1]!); for (const entry of valid) { let list = roots; let path = ""; entry.name.split(".").forEach((part, i, parts) => { path = i ? `${path}.${part.trim()}` : part.trim(); const node = get(list, part.trim(), path); if (i === parts.length - 1) { node.entry = entry; node.directCount = entry.references.length; node.totalCount = node.directCount; } list = node.children; }); } const sort = (list: HierarchicalTagNode[]) => { list.sort((a, b) => a.segment.localeCompare(b.segment)); list.forEach((n) => sort(n.children)); }; sort(roots); return roots; }
export function isValidGameplayTagSegment(value: string): boolean {
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(value.trim());
}
export function renamedGameplayTagName(currentName: string, nextSegment: string): string | null {
  const segment = nextSegment.trim();
  if (!isValidGameplayTagSegment(segment)) return null;
  const lastDot = currentName.lastIndexOf(".");
  const parentPath = lastDot < 0 ? "" : currentName.slice(0, lastDot);
  return parentPath ? `${parentPath}.${segment}` : segment;
}
