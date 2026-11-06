// Pure helpers for storylet authoring fields.
import type { EntityTemplateRecord } from "./contracts.ts";

export interface ParticipantOption { id: string; name: string }

/** Entities that carry the participant trait, sorted by name. */
export function participantOptions(templates: EntityTemplateRecord[]): ParticipantOption[] {
  return templates
    .filter((entity) => entity.traits?.some((t) => t.template_id === "system.participant"))
    .map((entity) => ({ id: entity.id, name: entity.name }))
    .sort((a, b) => a.name.localeCompare(b.name));
}

/** Adds or removes `id` from a played-by list without duplicating it. */
export function togglePlayedBy(current: string[], id: string, checked: boolean): string[] {
  if (!checked) return current.filter((existing) => existing !== id);
  return current.includes(id) ? current : [...current, id];
}
