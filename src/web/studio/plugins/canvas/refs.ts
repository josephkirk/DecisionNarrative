/** Resolves what a canvas reference shape points at: name, thumbnail, colour and route. */
import { studioState } from "../../state.ts";
import { rootTagColor } from "../../entityModel.ts";
import { thumbnailUrl } from "../../thumbnails.ts";
import type { AnyEntity } from "../runtime.ts";
import type { CanvasShape } from "./model.ts";

export interface RefInfo {
  name: string;
  thumb: string | null;
  color: string;
  description?: string;
  route?: string;
}

export const STORYLET_COLOR = "#bf5af2";

export function allEntities(): AnyEntity[] {
  return [...studioState.entities, ...studioState.entityTemplates] as AnyEntity[];
}

export function refInfo(shape: CanvasShape): RefInfo | null {
  if (shape.kind === "entity" && shape.entity_id) {
    const entity = allEntities().find((entry) => entry.id === shape.entity_id);
    if (!entity) return { name: shape.entity_id, thumb: null, color: rootTagColor("other") };
    const isTemplate = studioState.entityTemplates.some((template) => template.id === entity.id);
    return {
      name: entity.name, thumb: thumbnailUrl(entity.thumbnail), color: rootTagColor(entity.root_tag),
      description: entity.description, route: isTemplate ? `/entity-templates/${entity.id}` : `/entities/${entity.id}`,
    };
  }
  if (shape.kind === "storylet" && shape.storylet_id) {
    const storylet = studioState.storylets.find((entry) => entry.id === shape.storylet_id);
    if (!storylet) return { name: shape.storylet_id, thumb: null, color: STORYLET_COLOR };
    return {
      name: storylet.title, thumb: thumbnailUrl(storylet.thumbnail), color: STORYLET_COLOR,
      description: storylet.content?.text_template, route: `/storylets/${storylet.id}/edit`,
    };
  }
  return null;
}

export const refName = (shape: CanvasShape) => refInfo(shape)?.name;
