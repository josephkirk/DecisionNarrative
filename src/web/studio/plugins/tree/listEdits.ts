import { reactive, ref } from "vue";
import { StudioApi } from "../../api.ts";
import type { EntityRecord, EntityTemplateRecord, PropertyValue } from "../../contracts.ts";
import { notify, studioState } from "../../state.ts";
import { router } from "../../router.ts";
import { refreshWorkspace, type WorkspaceKind } from "../../view-models/workspace.ts";
import type { AnyEntity } from "../runtime.ts";
import { setTreeColumnValue, type TreeListColumn } from "./listProperties.ts";

type ItemKind = Extract<WorkspaceKind, "entities" | "entity_templates">;
type Change = { column: TreeListColumn; value: PropertyValue };
interface ItemEdits {
  kind: ItemKind;
  id: string;
  values: Record<string, PropertyValue>;
  pending: Record<string, Change>;
  saving: boolean;
  scheduled: boolean;
  error: string;
  timer?: ReturnType<typeof setTimeout>;
}

// Edits outlive the panel so changing entity tabs cannot discard a failed save.
export const treeItemEdits = reactive(new Map<string, ItemEdits>());
export const treeCellDrafts = ref<Record<string, string>>(Object.create(null));
export const treeCellErrors = ref<Record<string, string>>(Object.create(null));
let saveQueue: Promise<void> = Promise.resolve();
const itemKey = (kind: ItemKind, id: string) => JSON.stringify([kind, id]);

function hasChanges(state: ItemEdits): boolean {
  return state.saving || state.scheduled || Object.keys(state.pending).length > 0;
}

if (typeof window !== "undefined") {
  window.addEventListener("beforeunload", (event) => {
    if (Object.keys(treeCellDrafts.value).length || [...treeItemEdits.values()].some(hasChanges)) {
      event.preventDefault();
      event.returnValue = "";
    }
  });
}

export function itemEditState(kind: ItemKind, id: string): ItemEdits | undefined {
  return treeItemEdits.get(itemKey(kind, id));
}

export function editTreeItem(kind: ItemKind, id: string, column: TreeListColumn, value: PropertyValue): void {
  const key = itemKey(kind, id);
  let state = treeItemEdits.get(key);
  if (!state) {
    treeItemEdits.set(key, { kind, id, values: Object.create(null), pending: Object.create(null), saving: false, scheduled: false, error: "" });
    state = treeItemEdits.get(key)!;
  }
  state.values[column.id] = value;
  state.pending[column.id] = { column, value };
  state.error = "";
  clearTimeout(state.timer);
  state.timer = setTimeout(() => scheduleSave(state!), 600);
}

function scheduleSave(state: ItemEdits): void {
  clearTimeout(state.timer);
  state.timer = undefined;
  if (state.scheduled || !Object.keys(state.pending).length) return;
  state.scheduled = true;
  // Serialize workspace refreshes as well as writes to prevent stale cross-item refreshes.
  saveQueue = saveQueue.then(async () => {
    state.scheduled = false;
    const changes = state.pending;
    state.pending = Object.create(null);
    if (!Object.keys(changes).length) return;
    state.saving = true;
    state.error = "";
    try {
      let item = await StudioApi.getContent<AnyEntity>(state.kind, state.id);
      for (const change of Object.values(changes)) {
        item = setTreeColumnValue(item, change.column, change.value);
      }
      const saved = await StudioApi.saveContent<AnyEntity>(state.kind, item);
      if (state.kind === "entities") {
        studioState.entities = studioState.entities.map((record) => record.id === state.id ? saved as EntityRecord : record);
      } else {
        studioState.entityTemplates = studioState.entityTemplates.map((record) => record.id === state.id ? saved as EntityTemplateRecord : record);
      }
      await refreshWorkspace([state.kind]);
      for (const key of Object.keys(changes)) {
        if (!Object.hasOwn(state.pending, key)) delete state.values[key];
      }
    } catch (error) {
      state.pending = Object.assign(Object.create(null), changes, state.pending);
      state.error = error instanceof Error ? error.message : String(error);
      notify(`TreeItem changes for ${state.id} could not be saved: ${state.error}`, "error");
    } finally {
      state.saving = false;
      if (!state.error && Object.keys(state.pending).length) scheduleSave(state);
      if (!hasChanges(state) && !state.error) treeItemEdits.delete(itemKey(state.kind, state.id));
    }
  });
}

export function flushTreeItem(kind: ItemKind, id: string): void {
  const state = itemEditState(kind, id);
  if (state && !state.error) scheduleSave(state);
}

export function retryTreeItem(kind: ItemKind, id: string): void {
  const state = itemEditState(kind, id);
  if (!state) return;
  state.error = "";
  scheduleSave(state);
}
export function discardFailedTreeItem(kind: ItemKind, id: string): void {
  const state = itemEditState(kind, id);
  if (!state?.error || state.saving || state.scheduled) return;
  clearTimeout(state.timer);
  treeItemEdits.delete(itemKey(kind, id));
}


export async function flushAllTreeItems(): Promise<boolean> {
  for (const state of treeItemEdits.values()) {
    if (!state.error) scheduleSave(state);
  }
  let observed: Promise<void>;
  do {
    observed = saveQueue;
    await observed;
  } while (observed !== saveQueue);
  return ![...treeItemEdits.values()].some((state) => hasChanges(state) || !!state.error);
}

// Item edits can outlive the panel when the author switches entity tabs.
router.beforeEach(async () => {
  if (Object.keys(treeCellDrafts.value).length) {
    notify("Finish or discard the TreeItem input before leaving.", "error");
    return false;
  }
  const saved = await flushAllTreeItems();
  if (!saved) notify("TreeItem changes could not be saved. Retry before leaving.", "error");
  return saved;
});
