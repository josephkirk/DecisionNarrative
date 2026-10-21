import { ref } from "vue";
import { StudioApi } from "../api.ts";
import type { EventRecord, NpcBehaviorRecord } from "../contracts.ts";
import { notify } from "../state.ts";
import { refreshWorkspace } from "./workspace.ts";

export type CodexCategory = "npc_behaviors" | "events";
export type CodexRecord = NpcBehaviorRecord | EventRecord;

export function useCodexViewModel() {
  const activeCategory = ref<CodexCategory>("npc_behaviors");
  const searchQuery = ref("");
  const selectedId = ref<string | null>(null);
  const loading = ref(false);

  async function loadAll() {
    loading.value = true;
    try {
      await refreshWorkspace(["npc_behaviors", "events"]);
    } finally {
      loading.value = false;
    }
  }

  async function saveItem(category: CodexCategory, item: CodexRecord) {
    try {
      await StudioApi.saveContent(category, item);
      notify(`Saved ${category} item`, "success");
      await refreshWorkspace([category]);
    } catch (err) {
      notify(err instanceof Error ? err.message : String(err), "error");
    }
  }

  async function deleteItem(category: CodexCategory, id: string) {
    if (!window.confirm(`Delete '${id}'?`)) return;
    try {
      await StudioApi.deleteContent(category, id);
      notify(`Deleted ${id}`, "info");
      selectedId.value = null;
      await refreshWorkspace([category]);
    } catch (err) {
      notify(err instanceof Error ? err.message : String(err), "error");
    }
  }

  return {
    activeCategory,
    searchQuery,
    selectedId,
    loading,
    loadAll,
    saveItem,
    deleteItem,
  };
}
