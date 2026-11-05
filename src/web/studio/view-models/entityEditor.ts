import { computed, ref } from "vue";
import { useRoute } from "vue-router";
import { StudioApi } from "../api.ts";
import type { ContentReference, EntityRecord, EntityTemplateRecord, PropertyValue, TraitInstance, TraitPropertyDefinition, TraitTemplateRecord } from "../contracts.ts";
import { newTraitInstance } from "../entityModel.ts";
import { notify } from "../state.ts";
import { refreshWorkspace, currentFolderId } from "./workspace.ts";
export type EntityTab = "properties" | "traits" | "references";
export function useEntityEditor(entityId: string) {
  const isEntityRoute = useRoute().name === "entity";
  const isWorld = computed(() => isEntityRoute && entityId === "world");
  const draft = ref<EntityTemplateRecord | EntityRecord | null>(null); const savedJson = ref(""); const loading = ref(true); const saving = ref(false);
  const activeTab = ref<EntityTab>("properties"); const references = ref<ContentReference[]>([]); const referencesLoading = ref(false);
  const dirty = computed(() => !!draft.value && JSON.stringify(draft.value) !== savedJson.value);
  async function load() { loading.value = true; try { if (isEntityRoute) await refreshWorkspace(["entity_templates"]); draft.value = await StudioApi.getContent<EntityTemplateRecord | EntityRecord>(isEntityRoute ? "entities" : "entity_templates", entityId); savedJson.value = JSON.stringify(draft.value); await loadReferences(); } catch (error) { notify(error instanceof Error ? error.message : "Unable to load entity", "error"); } finally { loading.value = false; } }
  function patch(changes: Partial<EntityTemplateRecord & EntityRecord>) { if (draft.value) draft.value = { ...draft.value, ...changes } as EntityTemplateRecord | EntityRecord; }
  function addTrait(template: TraitTemplateRecord) {
    if (!draft.value || (!template.allow_multiple && draft.value.traits.some((trait) => trait.template_id === template.id))) return;
    const existingIds = new Set(draft.value.traits.map((trait) => trait.id));
    let instance = newTraitInstance(template);
    while (existingIds.has(instance.id)) instance = newTraitInstance(template);
    draft.value = { ...draft.value, traits: [...draft.value.traits, instance] };
  }
  function removeTrait(instanceId: string) { if (draft.value) draft.value = { ...draft.value, traits: draft.value.traits.filter((t) => t.id !== instanceId) }; }
  function setTraitValue(instanceId: string, key: string, value: PropertyValue) { if (!draft.value) return; draft.value = { ...draft.value, traits: draft.value.traits.map((t) => t.id === instanceId ? { ...t, values: { ...t.values, [key]: value } } : t) }; }
  function traitPropertyDisplayValue(instance: TraitInstance, property: TraitPropertyDefinition): PropertyValue | "" { return instance.values[property.key] ?? property.default ?? ""; }
  function onTraitPropertyInput(instanceId: string, property: TraitPropertyDefinition, value: PropertyValue) { setTraitValue(instanceId, property.key, value); }
  async function save(): Promise<boolean> {
    if (isWorld.value || !draft.value) return false;
    saving.value = true;
    try {
      const kind = isEntityRoute ? "entities" : "entity_templates";
      const folder = currentFolderId(kind, entityId);
      if (folder !== undefined) draft.value.folder_id = folder;
      draft.value = await StudioApi.saveContent(kind, draft.value);
      savedJson.value = JSON.stringify(draft.value);
      await refreshWorkspace([kind]);
      notify(isEntityRoute ? "Entity saved" : "Entity template saved", "success");
      return true;
    } catch (error) {
      notify(error instanceof Error ? error.message : "Unable to save entity", "error");
      return false;
    } finally {
      saving.value = false;
    }
  }
  async function loadReferences() { referencesLoading.value = true; try { references.value = (await StudioApi.getReferences(isEntityRoute ? "entities" : "entity_templates", entityId)).references; } catch { references.value = []; } finally { referencesLoading.value = false; } }
  async function remove(): Promise<boolean> {
    if (isWorld.value) return false;
    try {
      const kind = isEntityRoute ? "entities" : "entity_templates";
      await StudioApi.deleteContent(kind, entityId);
      await refreshWorkspace([kind]);
      notify(isEntityRoute ? "Entity deleted" : "Entity template deleted", "success");
      return true;
    } catch (error) {
      notify(error instanceof Error ? error.message : "Unable to delete entity", "error");
      return false;
    }
  }
  void load();
  return { draft, savedJson, loading, saving, activeTab, references, referencesLoading, dirty, isWorld, isEntityRoute, load, patch, addTrait, removeTrait, setTraitValue, traitPropertyDisplayValue, onTraitPropertyInput, save, loadReferences, remove };
}
