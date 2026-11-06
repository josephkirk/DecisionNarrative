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

/** Mark agent-authored material until its current revision is approved. */
export function originBadge(storylet: { origin?: string; revision: number; approved_revision?: number | null }): string | null {
  return storylet.origin === "agent" && storylet.approved_revision !== storylet.revision ? "Agent draft" : null;
}

/** Human-readable slot list, preserving unknown slots instead of hiding them. */
export function windowBadge(window: number[] | undefined, labels: string[]): string | null {
  if (!window?.length) return null;
  return window.map((slot) => labels[slot] ?? `Slot ${slot + 1}`).join(", ");
}

export function clockLabels(entities: Array<{ id: string; traits?: Array<{ template_id: string; values: Record<string, unknown> }> }>): string[] {
  const value = entities.find((entity) => entity.id === "world")?.traits?.find((trait) => trait.template_id === "system.clock")?.values.slot_labels;
  return Array.isArray(value) ? value.filter((label): label is string => typeof label === "string") : [];
}
