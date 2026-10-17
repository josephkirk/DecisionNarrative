import { ref } from "vue";
import { StudioApi } from "../api.ts";
import type { GenerationJobRecord } from "../contracts.ts";
import { notify } from "../state.ts";

export function useGenerationViewModel() {
  const jobs = ref<GenerationJobRecord[]>([]);
  const selectedJobId = ref<string | null>(null);
  const loading = ref(false);

  async function loadJobs() {
    loading.value = true;
    try {
      jobs.value = await StudioApi.listGenerationJobs();
      if (jobs.value.length > 0 && !selectedJobId.value) {
        selectedJobId.value = jobs.value[0].id;
      }
    } catch (e) {
      notify(e instanceof Error ? e.message : String(e), "error");
    } finally {
      loading.value = false;
    }
  }

  async function createJob(req: any) {
    loading.value = true;
    try {
      const job = await StudioApi.createGenerationJob(req);
      selectedJobId.value = job.id;
      notify(`Created generation job '${job.id.slice(0, 8)}'`, "success");
      await loadJobs();
    } catch (e) {
      notify(e instanceof Error ? e.message : String(e), "error");
    } finally {
      loading.value = false;
    }
  }

  async function advanceStage(id: string, rev: number) {
    try {
      await StudioApi.advanceGenerationJob(id, rev);
      notify("Advanced generation stage", "success");
      await loadJobs();
    } catch (e) {
      notify(e instanceof Error ? e.message : String(e), "error");
    }
  }

  async function saveDraft(id: string) {
    try {
      const st = await StudioApi.saveGenerationDraft(id);
      notify(`Saved draft '${st.title}' to Storylets`, "success");
      await loadJobs();
      return st.id;
    } catch (e) {
      notify(e instanceof Error ? e.message : String(e), "error");
      return null;
    }
  }

  async function editStage(id: string, stage: string, rev: number, artifact: unknown) {
    try {
      await StudioApi.editGenerationStage(id, stage, rev, artifact);
      notify(`Saved edit for stage '${stage}'`, "success");
      await loadJobs();
    } catch (e) {
      notify(e instanceof Error ? e.message : String(e), "error");
    }
  }

  return {
    jobs,
    selectedJobId,
    loading,
    loadJobs,
    createJob,
    advanceStage,
    editStage,
    saveDraft,
  };
}
