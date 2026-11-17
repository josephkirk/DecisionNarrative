import { computed, ref, watch } from "vue";
import { StudioApi } from "../api.ts";
import type { ContentReference, EntityRecord, PropertyValue, TraitInstance, TraitPropertyDefinition, TraitTemplateRecord } from "../contracts.ts";
import { newTraitInstance, uniqueTraitProblem } from "../entityModel.ts";
import { notify, studioState } from "../state.ts";
import { refreshWorkspace, currentFolderId } from "./workspace.ts";
export type EntityTab = "properties" | "traits" | "references" | `plugin:${string}`;
export function useEntityEditor(entityId: string) {
  const isWorld = computed(() => entityId === "world");
  const draft = ref<EntityRecord | null>(null); const savedJson = ref(""); const loading = ref(true); const saving = ref(false);
  const activeTab = ref<EntityTab>("properties"); const references = ref<ContentReference[]>([]); const referencesLoading = ref(false);
  const dirty = computed(() => !!draft.value && JSON.stringify(draft.value) !== savedJson.value);
  async function load() { loading.value = true; try { await refreshWorkspace(["entities"]); draft.value = await StudioApi.getContent<EntityRecord>("entities", entityId); savedJson.value = JSON.stringify(draft.value); await loadReferences(); } catch (error) { notify(error instanceof Error ? error.message : "Unable to load entity", "error"); } finally { loading.value = false; } }
  function patch(changes: Partial<EntityRecord>) { if (draft.value) draft.value = { ...draft.value, ...changes }; }
  function addTrait(template: TraitTemplateRecord) {
    if (!draft.value) return false;
    if (!template.allow_multiple && draft.value.traits.some((trait) => trait.template_id === template.id)) {
      notify(`The ${template.name} trait is already applied.`, "error");
      return false;
    }
    if (template.unique && uniqueTraitProblem([...draft.value.traits, { id: "", template_id: template.id, values: {} }], [...studioState.traitTemplates.filter((item) => item.id !== template.id), template])) {
      notify("An entity can have only one unique trait. Remove the existing unique trait before adding another.", "error");
      return false;
    }
    const existingIds = new Set(draft.value.traits.map((trait) => trait.id));
    let instance = newTraitInstance(template);
    while (existingIds.has(instance.id)) instance = newTraitInstance(template);
    draft.value = { ...draft.value, traits: [...draft.value.traits, instance] };
    return true;
  }
  function removeTrait(instanceId: string) { if (draft.value) draft.value = { ...draft.value, traits: draft.value.traits.filter((t) => t.id !== instanceId) }; }
  function setTraitValue(instanceId: string, key: string, value: PropertyValue) { if (!draft.value) return; draft.value = { ...draft.value, traits: draft.value.traits.map((t) => t.id === instanceId ? { ...t, values: { ...t.values, [key]: value } } : t) }; }
  function traitPropertyDisplayValue(instance: TraitInstance, property: TraitPropertyDefinition): PropertyValue | "" { return instance.values[property.key] ?? property.default ?? ""; }
  function onTraitPropertyInput(instanceId: string, property: TraitPropertyDefinition, value: PropertyValue) { setTraitValue(instanceId, property.key, value); }
  async function save(): Promise<boolean> {
    if (isWorld.value || !draft.value) return false;
    saving.value = true;
    try {
      const folder = currentFolderId("entities", entityId);
      if (folder !== undefined) draft.value.folder_id = folder;
      draft.value = await StudioApi.saveContent<EntityRecord>("entities", draft.value);
      savedJson.value = JSON.stringify(draft.value);
      await refreshWorkspace(["entities"]);
      notify("Entity saved", "success");
      return true;
    } catch (error) {
      notify(error instanceof Error ? error.message : "Unable to save entity", "error");
      return false;
    } finally {
      saving.value = false;
    }
  }
  async function loadReferences() { referencesLoading.value = true; try { references.value = (await StudioApi.getReferences("entities", entityId)).references; } catch { references.value = []; } finally { referencesLoading.value = false; } }
  async function remove(): Promise<boolean> {
    if (isWorld.value) return false;
    try {
      await StudioApi.deleteContent("entities", entityId);
      await refreshWorkspace(["entities"]);
      notify("Entity deleted", "success");
      return true;
    } catch (error) {
      notify(error instanceof Error ? error.message : "Unable to delete entity", "error");
      return false;
    }
  }
  watch(() => studioState.entities.find((entity) => entity.id === entityId)?.is_template, (isTemplate) => {
    if (isTemplate === undefined || !draft.value || saving.value || loading.value) return;
    const saved = JSON.parse(savedJson.value) as EntityRecord;
    if (saved.is_template !== isTemplate) savedJson.value = JSON.stringify({ ...saved, is_template: isTemplate });
    if (draft.value.is_template !== isTemplate) draft.value = { ...draft.value, is_template: isTemplate };
  });
  void load();
  return { draft, savedJson, loading, saving, activeTab, references, referencesLoading, dirty, isWorld, load, patch, addTrait, removeTrait, setTraitValue, traitPropertyDisplayValue, onTraitPropertyInput, save, loadReferences, remove };
}
