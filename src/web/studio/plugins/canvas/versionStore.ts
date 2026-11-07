/**
 * Read-modify-write of an entity's canvas document on the service, used by
 * the Modify Canvas node to save or remove the version it owns.
 */
import { StudioApi } from "../../api.ts";
import { studioState } from "../../state.ts";
import { refreshWorkspace } from "../../view-models/workspace.ts";
import type { AnyEntity } from "../runtime.ts";
import { emptyCanvas, syncCanvasProperties, type CanvasDoc } from "./model.ts";
import { canvasOf } from "./modifyCanvasNode.ts";

export async function updateEntityCanvas(entityId: string, mutate: (doc: CanvasDoc) => CanvasDoc): Promise<AnyEntity> {
  const kind = studioState.entities.some((row) => row.id === entityId) ? "entities" : "entity_templates";
  const fresh = await StudioApi.getContent<AnyEntity>(kind, entityId);
  const next = mutate(canvasOf(fresh) ?? emptyCanvas());
  const saved = await StudioApi.saveContent<AnyEntity>(kind, { ...fresh, properties: syncCanvasProperties(fresh.properties, next) });
  await refreshWorkspace([kind]);
  return saved;
}
