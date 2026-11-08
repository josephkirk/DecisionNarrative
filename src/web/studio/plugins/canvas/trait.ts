/**
 * The "Canvas" trait: entities with it get the Canvas tab. The plugin creates
 * the trait on first load when the project does not have it yet.
 */
import type { TraitTemplateRecord } from "../../contracts.ts";
import type { AnyEntity } from "../runtime.ts";

export const CANVAS_TRAIT_KEY = "canvas";

export function canvasTraitTemplate(): TraitTemplateRecord {
  return {
    id: CANVAS_TRAIT_KEY, key: CANVAS_TRAIT_KEY, name: "Canvas",
    description: "Adds a vector Canvas tab: layers, markers, sticky notes and entity/storylet references. Added by the vector-canvas plugin.",
    properties: [], allow_multiple: false,
  };
}

export function findCanvasTrait(templates: TraitTemplateRecord[]): TraitTemplateRecord | undefined {
  return templates.find((template) => template.key === CANVAS_TRAIT_KEY);
}

export function hasCanvasTrait(entity: AnyEntity, templates: TraitTemplateRecord[]): boolean {
  const trait = findCanvasTrait(templates);
  return !!trait && entity.traits.some((instance) => instance.template_id === trait.id);
}
