import { ref, reactive, computed } from "vue";
import { StudioApi } from "../api.ts";
import type { StoryletRecord, StoryletChoiceRecord } from "../contracts.ts";
import { notify, studioState } from "../state.ts";
import { refreshWorkspace, currentFolderId } from "./workspace.ts";

export function useStoryletEditorViewModel(storyletId?: string, folderId?: string | null) {
  const activeTab = ref<"content" | "logic" | "review">("content");
  const loading = ref(false);
  const isNew = ref(!storyletId);

  const draft = reactive<StoryletRecord>({
    id: storyletId || `stl_${Date.now().toString(36)}`,
    revision: 1,
    approved_revision: null,
    title: "",
    ending: false,
    location_tags: [],
    group_tag: null,
    cluster_type: "player_passage",
    urgency_weight: 50,
    repeatability: "always",
    cooldown_turns: 0,
    parameters: [],
    preconditions: { all_of: [], any_of: [], visited_locations_any_of: [], met_characters_any_of: [] },
    content: {
      text_template: "",
      sceneOpening: "",
      dialogue_quips: [],
    },
    choices: [],
    effects: {},
    folder_id: folderId ?? null,
    created_at: Date.now(),
  });

  const isApproved = computed(() => {
    return draft.revision > 0 && draft.approved_revision === draft.revision;
  });

  async function loadStorylet(id: string) {
    loading.value = true;
    try {
      const record = await StudioApi.getContent<StoryletRecord>("storylets", id);
      Object.assign(draft, record);
      isNew.value = false;
    } catch (e) {
      notify(e instanceof Error ? e.message : String(e), "error");
    } finally {
      loading.value = false;
    }
  }

  async function saveDraft(): Promise<boolean> {
    if (!draft.title.trim()) {
      notify("Storylet title is required", "error");
      return false;
    }

    const storedFolder = currentFolderId("storylets", draft.id);
    if (storedFolder !== undefined && storedFolder !== draft.folder_id) {
      draft.folder_id = storedFolder;
    }

    try {
      if (isNew.value) {
        await StudioApi.saveContent("storylets", draft);
        notify(`Created storylet '${draft.title}'`, "success");
        isNew.value = false;
      } else {
        draft.revision += 1;
        draft.approved_revision = null; // Unapproved until explicit approval
        await StudioApi.updateContent("storylets", draft.id, draft);
        notify(`Saved draft revision ${draft.revision}`, "success");
      }
      await refreshWorkspace(["storylets"]);
      return true;
    } catch (e) {
      notify(e instanceof Error ? e.message : String(e), "error");
      return false;
    }
  }

  async function approveStorylet(): Promise<boolean> {
    if (isApproved.value) {
      notify("Storylet is already approved at this revision", "info");
      return true;
    }
    try {
      const updated = await StudioApi.approveStorylet(draft.id, draft.revision);
      draft.approved_revision = updated.approved_revision;
      await refreshWorkspace(["storylets"]);
      notify(`Approved storylet revision ${draft.revision}`, "success");
      return true;
    } catch (e) {
      notify(e instanceof Error ? e.message : String(e), "error");
      return false;
    }
  }

  function addChoice() {
    const nextIdx = draft.choices.length + 1;
    draft.choices.push({
      choice_id: `choice_${nextIdx}`,
      label: `Choice ${nextIdx}`,
      effects: {},
      conditions: [],
    });
  }

  function removeChoice(index: number) {
    draft.choices.splice(index, 1);
  }

  function addRoleParameter() {
    draft.parameters.push({
      param_name: "participant",
      query: 'role == "witness"',
    });
  }

  function removeRoleParameter(index: number) {
    draft.parameters.splice(index, 1);
  }

  return {
    activeTab,
    loading,
    isNew,
    draft,
    isApproved,
    loadStorylet,
    saveDraft,
    approveStorylet,
    addChoice,
    removeChoice,
    addRoleParameter,
    removeRoleParameter,
  };
}
