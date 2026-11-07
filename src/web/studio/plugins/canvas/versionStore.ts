/**
 * Read-modify-write of an entity's canvas document on the service, used by
 * the Modify Canvas node to stage, commit, discard and remove its version.
 * Writes run one at a time so a commit and a discard never interleave.
 */
import { StudioApi } from "../../api.ts";
import { studioState } from "../../state.ts";
import { refreshWorkspace } from "../../view-models/workspace.ts";
import type { AnyEntity } from "../runtime.ts";
import { emptyCanvas, serializeCanvas, syncCanvasProperties, type CanvasDoc } from "./model.ts";
import { canvasOf } from "./modifyCanvasNode.ts";
import { allEntities } from "./refs.ts";

let queue: Promise<unknown> = Promise.resolve();
function serial<T>(task: () => Promise<T>): Promise<T> {
  const run = queue.then(task, task);
  queue = run.catch(() => undefined);
  return run;
}

function kindOf(entityId: string) {
  return studioState.entities.some((row) => row.id === entityId) ? "entities" : "entity_templates";
}

async function writeCanvas(entityId: string, mutate: (doc: CanvasDoc) => CanvasDoc): Promise<boolean> {
  const kind = kindOf(entityId);
  const fresh = await StudioApi.getContent<AnyEntity>(kind, entityId);
  const doc = canvasOf(fresh) ?? emptyCanvas();
  const next = mutate(doc);
  if (serializeCanvas(next) === serializeCanvas(doc)) return false;
  await StudioApi.saveContent<AnyEntity>(kind, { ...fresh, properties: syncCanvasProperties(fresh.properties, next) });
  return true;
}

export function updateEntityCanvas(entityId: string, mutate: (doc: CanvasDoc) => CanvasDoc): Promise<void> {
  return serial(async () => {
    if (await writeCanvas(entityId, mutate)) await refreshWorkspace([kindOf(entityId)]);
  });
}

/** Apply `mutate` to every canvas that `touches` says is affected. */
export function updateCanvases(touches: (doc: CanvasDoc) => boolean, mutate: (doc: CanvasDoc) => CanvasDoc): Promise<void> {
  return serial(async () => {
    const targets = allEntities().filter((entity) => { const doc = canvasOf(entity); return !!doc && touches(doc); });
    let changed = false;
    for (const entity of targets) changed = (await writeCanvas(entity.id, mutate)) || changed;
    if (changed) await refreshWorkspace(["entities", "entity_templates"]);
  });
}
