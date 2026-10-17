import { ref, reactive } from "vue";
import { StudioApi } from "../api.ts";
import { notify, studioState } from "../state.ts";

export function useCodexViewModel() {
  const activeCategory = ref<"qualities" | "characters" | "locations" | "npc_behaviors" | "events">("qualities");
  const searchQuery = ref("");
  const selectedId = ref<string | null>(null);
  const loading = ref(false);

  async function loadAll() {
    loading.value = true;
    try {
      const [q, c, l, n, e] = await Promise.all([
        StudioApi.listContent<any>("qualities"),
        StudioApi.listContent<any>("characters"),
        StudioApi.listContent<any>("locations"),
        StudioApi.listContent<any>("npc_behaviors"),
        StudioApi.listContent<any>("events"),
      ]);
      studioState.qualities = q;
      studioState.characters = c;
      studioState.locations = l;
      studioState.npcBehaviors = n;
      studioState.events = e;
    } catch (err) {
      notify(err instanceof Error ? err.message : String(err), "error");
    } finally {
      loading.value = false;
    }
  }

  async function saveItem(entity: string, item: any) {
    try {
      await StudioApi.saveContent(entity, item);
      notify(`Saved ${entity} item`, "success");
      await loadAll();
    } catch (err) {
      notify(err instanceof Error ? err.message : String(err), "error");
    }
  }

  async function deleteItem(entity: string, id: string) {
    if (!window.confirm(`Delete '${id}'?`)) return;
    try {
      await StudioApi.deleteContent(entity, id);
      notify(`Deleted ${id}`, "info");
      selectedId.value = null;
      await loadAll();
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
