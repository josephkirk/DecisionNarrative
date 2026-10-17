import { ref, reactive } from "vue";
import { StudioApi } from "../api.ts";
import type { LibraryPageResult, LibrarySummaryItem } from "../contracts.ts";
import { notify } from "../state.ts";

export function useLibraryViewModel() {
  const loading = ref(false);
  const result = ref<LibraryPageResult>({
    items: [],
    total: 0,
    page: 1,
    page_size: 60,
    total_pages: 1,
    available_locations: [],
  });

  const filters = reactive({
    q: "",
    status: "all",
    location_id: "",
    ending: "all",
    sort: "title",
    page: 1,
  });

  async function loadLibrary() {
    loading.value = true;
    try {
      result.value = await StudioApi.queryLibrary(filters);
    } catch (e) {
      notify(e instanceof Error ? e.message : String(e), "error");
    } finally {
      loading.value = false;
    }
  }

  function setPage(p: number) {
    filters.page = Math.max(1, Math.min(p, result.value.total_pages || 1));
    loadLibrary();
  }

  function resetFilters() {
    filters.q = "";
    filters.status = "all";
    filters.location_id = "";
    filters.ending = "all";
    filters.sort = "title";
    filters.page = 1;
    loadLibrary();
  }

  async function deleteStorylet(id: string) {
    if (!window.confirm(`Delete storylet '${id}'?`)) return;
    try {
      await StudioApi.deleteContent("storylets", id);
      notify(`Deleted storylet '${id}'`, "info");
      loadLibrary();
    } catch (e) {
      notify(e instanceof Error ? e.message : String(e), "error");
    }
  }

  return {
    loading,
    result,
    filters,
    loadLibrary,
    setPage,
    resetFilters,
    deleteStorylet,
  };
}
