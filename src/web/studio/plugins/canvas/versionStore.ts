import { StudioApi } from "../../api.ts";
import { refreshWorkspace } from "../../view-models/workspace.ts";
import type { EntityRecord } from "../../contracts.ts";
import { emptyCanvas, serializeCanvas, syncCanvasProperties, type CanvasDoc } from "./model.ts";
import { canvasOf } from "./modifyCanvasNode.ts";
import { allEntities } from "./refs.ts";

let queue: Promise<unknown> = Promise.resolve();
function serial<T>(task: () => Promise<T>): Promise<T> {
  const run = queue.then(task, task);
  queue = run.catch(() => undefined);
  return run;
}

async function writeCanvas(entityId: string, mutate: (doc: CanvasDoc) => CanvasDoc): Promise<boolean> {
  const fresh = await StudioApi.getContent<EntityRecord>("entities", entityId);
  const doc = canvasOf(fresh) ?? emptyCanvas();
  const next = mutate(doc);
  if (serializeCanvas(next) === serializeCanvas(doc)) return false;
  await StudioApi.saveContent<EntityRecord>("entities", { ...fresh, properties: syncCanvasProperties(fresh.properties, next) });
  return true;
}

export function updateEntityCanvas(entityId: string, mutate: (doc: CanvasDoc) => CanvasDoc): Promise<void> {
  return serial(async () => {
    if (await writeCanvas(entityId, mutate)) await refreshWorkspace(["entities"]);
  });
}

export function updateCanvases(touches: (doc: CanvasDoc) => boolean, mutate: (doc: CanvasDoc) => CanvasDoc): Promise<void> {
  return serial(async () => {
    const targets = allEntities().filter((entity) => { const doc = canvasOf(entity); return !!doc && touches(doc); });
    let changed = false;
    for (const entity of targets) changed = (await writeCanvas(entity.id, mutate)) || changed;
    if (changed) await refreshWorkspace(["entities"]);
  });
}
