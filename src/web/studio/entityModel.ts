import type { EntityTemplateRecord, PropertyValue, TraitTemplateRecord, TraitInstance } from "./contracts.ts";
export interface RootTagDefinition { value: string; label: string; color: string; }
export const ROOT_TAGS: RootTagDefinition[] = [
  { value: "character", label: "Character", color: "var(--type-character)" },
  { value: "location", label: "Location", color: "var(--type-location)" },
  { value: "item", label: "Item", color: "var(--type-item)" },
  { value: "fact", label: "Fact", color: "var(--type-fact)" },
  { value: "other", label: "Other", color: "var(--type-other)" },
  { value: "world", label: "World", color: "var(--type-world)" },
];
export function rootTagLabel(tag: string): string { return tag.split(".").pop()?.replace(/(^|_)(.)/g, (_, __, c) => c.toUpperCase()) ?? "Other"; }
export function rootTagColor(tag: string): string { return ROOT_TAGS.find((entry) => entry.value === tag)?.color ?? "var(--type-other)"; }
export function newEntityTemplateDraft(folderId: string | null, rootTag = "character"): EntityTemplateRecord {
  return { id: "", object_id: "", game_id: "", game_id_auto: true, name: "New Entity Template", root_tag: rootTag, description: "", folder_id: folderId, tags: [], properties: [], traits: [] };
}
export function newTraitInstance(template: TraitTemplateRecord): TraitInstance {
  const values: Record<string, PropertyValue> = {};
  for (const property of template.properties) {
    if (property.default !== undefined) values[property.key] = property.default;
  }
  return { id: crypto.randomUUID(), template_id: template.id, values };
}
export function appliedTemplates(entity: EntityTemplateRecord, templates: TraitTemplateRecord[]): TraitTemplateRecord[] {
  return entity.traits.map((instance) => templates.find((template) => template.id === instance.template_id)).filter((template): template is TraitTemplateRecord => !!template);
}
