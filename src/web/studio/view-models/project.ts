import { ref, reactive } from "vue";
import { StudioApi } from "../api.ts";
import type { ProjectMetadata } from "../contracts.ts";
import { notify, studioState } from "../state.ts";

export function useProjectViewModel() {
  const metadata = reactive<ProjectMetadata>({
    id: "workspace-project",
    name: "Workspace",
    revision: 0,
  });
  const loading = ref(false);

  async function loadProject() {
    loading.value = true;
    try {
      const meta = await StudioApi.getProject();
      Object.assign(metadata, meta);
      studioState.project = meta;
    } catch (e) {
      notify(e instanceof Error ? e.message : String(e), "error");
    } finally {
      loading.value = false;
    }
  }

  async function saveName(name: string) {
    try {
      const updated = await StudioApi.patchProjectName(name);
      Object.assign(metadata, updated);
      studioState.project = updated;
      notify(`Saved project name '${updated.name}'`, "success");
    } catch (e) {
      notify(e instanceof Error ? e.message : String(e), "error");
    }
  }

  return {
    metadata,
    loading,
    loadProject,
    saveName,
  };
}
