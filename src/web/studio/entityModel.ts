import type { EntityRecord, PropertyValue, TraitTemplateRecord, TraitInstance } from "./contracts.ts";

function clone<T>(value: T): T {
  return value === undefined ? value : (JSON.parse(JSON.stringify(value)) as T);
}

export function buildEntityDraft(template: EntityRecord | null, folderId: string | null): EntityRecord {
  const base = { id: "", object_id: "", game_id: "", game_id_auto: true, is_template: false, folder_id: folderId };
  if (!template) return { ...base, template_id: null, name: "New Entity", description: "", thumbnail: null, tags: [], properties: [], traits: [] };
  return {
    ...base,
    template_id: template.id,
    name: `New ${template.name}`,
    description: template.description ?? "",
    thumbnail: template.thumbnail ?? null,
    tags: [...template.tags],
    properties: template.properties.map((property) => ({
      ...clone(property),
      value: clone(property.value !== undefined && property.value !== null ? property.value : property.default),
    })),
    traits: template.traits.map((trait) => ({ ...clone(trait), id: crypto.randomUUID() })),
  };
}

export function withTemplateFlag(entity: EntityRecord, isTemplate: boolean): EntityRecord {
  return { ...entity, is_template: isTemplate };
}

export function newTraitInstance(template: TraitTemplateRecord): TraitInstance {
  const values: Record<string, PropertyValue> = {};
  for (const property of template.properties) {
    if (property.default !== undefined) values[property.key] = property.default;
  }
  return { id: crypto.randomUUID(), template_id: template.id, values };
}

export function appliedTemplates(entity: Pick<EntityRecord, "traits">, templates: TraitTemplateRecord[]): TraitTemplateRecord[] {
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
