import type { EntityTemplateRecord, PropertyValue, TraitTemplateRecord, TraitInstance } from "./contracts.ts";

export function newEntityTemplateDraft(folderId: string | null): EntityTemplateRecord {
  return { id: "", object_id: "", game_id: "", game_id_auto: true, name: "New Entity Template", description: "", folder_id: folderId, tags: [], properties: [], traits: [] };
}

export function newTraitInstance(template: TraitTemplateRecord): TraitInstance {
  const values: Record<string, PropertyValue> = {};
  for (const property of template.properties) {
    if (property.default !== undefined) values[property.key] = property.default;
  }
  return { id: crypto.randomUUID(), template_id: template.id, values };
}

export function appliedTemplates(entity: Pick<EntityTemplateRecord, "traits">, templates: TraitTemplateRecord[]): TraitTemplateRecord[] {
  return entity.traits.map((instance) => templates.find((template) => template.id === instance.template_id)).filter((template): template is TraitTemplateRecord => !!template);
}

export function hasTraitKey(entity: { traits?: TraitInstance[] }, templates: TraitTemplateRecord[], key: string): boolean {
  const templateIds = new Set(templates.filter((template) => template.key === key || template.id === key).map((template) => template.id));
  return (entity.traits ?? []).some((instance) => templateIds.has(instance.template_id));
}

export function traitLabels(entity: { traits?: TraitInstance[] }, templates: TraitTemplateRecord[]): string[] {
  const ids = new Set((entity.traits ?? []).map((instance) => instance.template_id));
  return templates.filter((template) => ids.has(template.id)).map((template) => template.name);
}

export function uniqueTraitProblem(traits: TraitInstance[], templates: TraitTemplateRecord[]): string | null {
  const uniqueIds = new Set(templates.filter((template) => template.unique).map((template) => template.id));
  const uniqueInstances = traits.filter((instance) => uniqueIds.has(instance.template_id));
  return uniqueInstances.length > 1 ? "An entity can have only one unique trait." : null;
}

export function traitEditProblem(traits: TraitInstance[], templates: TraitTemplateRecord[]): string | null {
  const uniquenessProblem = uniqueTraitProblem(traits, templates);
  if (uniquenessProblem) return uniquenessProblem;
  for (const template of templates) {
    if (!template.allow_multiple && traits.filter((instance) => instance.template_id === template.id).length > 1) {
      return `The ${template.name} trait does not allow multiple instances.`;
    }
  }
  return null;
}
