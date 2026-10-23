import { computed, ref } from "vue";
import { StudioApi } from "../api.ts";
import type { ContentReference, EntityRecord, EntityType, TraitInstance } from "../contracts.ts";
import { entityTypeLabel, withEntityType } from "../entityModel.ts";
import { notify } from "../state.ts";
import { refreshWorkspace, currentFolderId } from "./workspace.ts";

export type EntityTab = "properties" | "traits" | "references";

export function useEntityEditor(entityId: string) {
  const draft = ref<EntityRecord | null>(null);
  const savedJson = ref("");
  const loading = ref(true);
  const saving = ref(false);
  const activeTab = ref<EntityTab>("properties");
  const references = ref<ContentReference[]>([]);
  const referencesLoading = ref(false);

  const dirty = computed(
    () => draft.value !== null && JSON.stringify(draft.value) !== savedJson.value,
  );

  async function load() {
    loading.value = true;
    try {
      const entity = await StudioApi.getContent<EntityRecord>("entities", entityId);
      draft.value = entity;
      savedJson.value = JSON.stringify(entity);
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error), "error");
    } finally {
      loading.value = false;
    }
  }

  function patch(changes: Partial<EntityRecord>) {
    if (!draft.value) return;
    draft.value = { ...draft.value, ...changes };
  }

  function changeType(type: EntityType, confirm: boolean) {
    if (!draft.value) return;
    if (confirm && (draft.value.type === "character" || draft.value.type === "location")) {
      const label = entityTypeLabel(draft.value.type);
      if (!window.confirm(`Changing the type removes the ${label} settings.`)) return;
    }
    draft.value = withEntityType(draft.value, type);
  }

  function addTrait(templateId: string) {
    if (!draft.value || !templateId) return;
    const traits: TraitInstance[] = [...draft.value.traits, { template_id: templateId, values: {} }];
    patch({ traits });
  }

  function removeTrait(templateId: string) {
    if (!draft.value) return;
    patch({ traits: draft.value.traits.filter((trait) => trait.template_id !== templateId) });
  }

  function setTraitValue(templateId: string, definitionId: string, value: unknown) {
    if (!draft.value) return;
    const traits = draft.value.traits.map((trait) => {
      if (trait.template_id !== templateId) return trait;
      const values = { ...trait.values };
      values[definitionId] = value as never;
      return { ...trait, values };
    });
    patch({ traits });
  }

  function resetTraitValue(templateId: string, definitionId: string) {
    if (!draft.value) return;
    const traits = draft.value.traits.map((trait) => {
      if (trait.template_id !== templateId) return trait;
      const values = { ...trait.values };
      delete values[definitionId];
      return { ...trait, values };
    });
    patch({ traits });
  }

  async function save(): Promise<boolean> {
    if (!draft.value) return false;
    saving.value = true;
    try {
      const storedFolder = currentFolderId("entities", entityId);
      if (storedFolder !== undefined && storedFolder !== draft.value.folder_id) {
        draft.value = { ...draft.value, folder_id: storedFolder };
      }
      const saved = await StudioApi.saveContent<EntityRecord>("entities", draft.value);
      draft.value = saved;
      savedJson.value = JSON.stringify(saved);
      await refreshWorkspace(["entities"]);
      await loadReferences();
      notify("Entity saved", "success");
      return true;
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error), "error");
      return false;
    } finally {
      saving.value = false;
    }
  }

  async function loadReferences() {
    referencesLoading.value = true;
    try {
      const result = await StudioApi.getReferences("entities", entityId);
      references.value = result.references;
    } catch (error) {
      references.value = [];
      notify(error instanceof Error ? error.message : String(error), "error");
    } finally {
      referencesLoading.value = false;
    }
  }

  async function remove(): Promise<boolean> {
    if (!draft.value) return false;
    if (!window.confirm(`Delete '${draft.value.name}'?`)) return false;
    try {
      await StudioApi.deleteContent("entities", entityId);
      await refreshWorkspace(["entities"]);
      notify(`Entity '${draft.value.name}' deleted`, "success");
      return true;
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error), "error");
      return false;
    }
  }

  return {
    draft,
    dirty,
    loading,
    saving,
    activeTab,
    references,
    referencesLoading,
    load,
    patch,
    changeType,
    addTrait,
    removeTrait,
    setTraitValue,
    resetTraitValue,
    save,
    remove,
    loadReferences,
  };
}
