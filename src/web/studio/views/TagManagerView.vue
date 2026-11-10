<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { studioState } from "../state.ts";
import { StudioApi } from "../api.ts";
import { buildHierarchicalTagTree, buildTagIndex, isValidGameplayTagSegment, renamedGameplayTagName, TAG_KINDS, type HierarchicalTagNode, type TagKind, type TagReference } from "../tagData.ts";
import TagTreeNode from "../components/TagTreeNode.vue";
import StudioIcon from "../components/StudioIcon.vue";
import EmptyState from "../components/ui/EmptyState.vue";
import SearchField from "../components/ui/SearchField.vue";
import SegmentedControl from "../components/ui/SegmentedControl.vue";
import { refreshWorkspace } from "../view-models/workspace.ts";

const router = useRouter();
const activeKind = ref<TagKind>("gameplay");
const query = ref("");
const loading = ref(true);
const error = ref("");
const status = ref("");
const tagViewMode = ref<"tree" | "list">("tree");
const expandedPaths = ref<Set<string>>(new Set());
const selectedTag = ref("");

async function load() {
  loading.value = true;
  error.value = "";
  try {
    await refreshWorkspace(["entities", "entity_templates", "storylets", "trait_templates", "gameplay_tags"]);
  } catch (cause) { error.value = cause instanceof Error ? cause.message : "Unable to load tag references."; }
  finally { loading.value = false; }
}
onMounted(load);

const allTags = computed(() => buildTagIndex({
  gameplayTags: studioState.gameplayTags,
  entities: studioState.entities,
  entityTemplates: studioState.entityTemplates,
  storylets: studioState.storylets,
  traitTemplates: studioState.traitTemplates,
}));
const visibleTags = computed(() => {
  const needle = query.value.trim().toLowerCase();
  const matching = allTags.value.filter((tag) => tag.kind === activeKind.value && (!needle || tag.name.toLowerCase().includes(needle)));
  if (activeKind.value !== "gameplay" || !needle) return matching;
  const paths = new Set<string>();
  matching.forEach((tag) => tag.name.split(".").forEach((_, i, parts) => paths.add(parts.slice(0, i + 1).join("."))));
  return allTags.value.filter((tag) => tag.kind === "gameplay" && paths.has(tag.name));
});
watch([activeKind, tagViewMode, visibleTags], ([kind, mode, tags]) => {
  if (kind !== "gameplay" || mode !== "tree" || !tags.some((tag) => tag.name === selectedTag.value)) selectedTag.value = "";
});
const hierarchicalTree = computed<HierarchicalTagNode[]>(() => buildHierarchicalTagTree(visibleTags.value));
const selectedTagIsRegistered = computed(() => studioState.gameplayTags.some((tag) => tag.name.toLowerCase() === selectedTag.value.toLowerCase()));
function hasChildTags(name: string) { const prefix = `${name.toLowerCase()}.`; return allTags.value.some((item) => item.name.toLowerCase().startsWith(prefix)); }
const kindCount = (kind: TagKind) => allTags.value.filter((tag) => tag.kind === kind).length;
function toggleBranch(path: string) { const next = new Set(expandedPaths.value); next.has(path) ? next.delete(path) : next.add(path); expandedPaths.value = next; }

async function createTag(parent = "") {
  const segment = window.prompt(parent ? `Child name under ${parent}` : "Root gameplay tag name");
  if (segment === null || !segment.trim()) return;
  if (!isValidGameplayTagSegment(segment)) { error.value = "Use letters, digits, or underscores; start with a letter or underscore."; return; }
  if (parent && !studioState.gameplayTags.some((tag) => tag.name.toLowerCase() === parent.toLowerCase())) {
    error.value = "Register this parent tag before adding a child.";
    return;
  }
  const name = parent ? `${parent}.${segment.trim()}` : segment.trim();
  if (studioState.gameplayTags.some((tag) => tag.name.toLowerCase() === name.toLowerCase())) { error.value = "A tag with this name already exists."; return; }
  try { await StudioApi.saveContent("gameplay_tags", { id: "", name, description: "", protected: false }); await load(); status.value = `Created ${name}`; selectedTag.value = name; }
  catch (cause) { error.value = cause instanceof Error ? cause.message : "Unable to create tag."; }
}
async function mutateTag(tag: { name: string; references: TagReference[] }, action: "rename" | "delete") {
  const record = studioState.gameplayTags.find((item) => item.name.toLowerCase() === tag.name.toLowerCase());
  if (!record) { error.value = "Only registered gameplay tags can be changed."; return; }
  if (tag.references.length) {
    error.value = action === "rename"
      ? `Remove references before renaming '${tag.name}'.`
      : `Remove references before deleting '${tag.name}'.`;
    return;
  }
  if (action === "rename" && allTags.value.some((item) => item.kind === "gameplay" && item.name.toLowerCase().startsWith(`${tag.name.toLowerCase()}.`))) { error.value = "Rename or remove child tags before renaming this parent."; return; }
  if (action === "delete" && allTags.value.some((item) => item.kind === "gameplay" && item.name.toLowerCase().startsWith(`${tag.name.toLowerCase()}.`))) { error.value = "Delete child tags before deleting this parent."; return; }
  const nextSegment = action === "rename"
    ? window.prompt("New tag segment", record.name.split(".").at(-1))
    : null;
  if (action === "rename" && nextSegment === null) return;
  const nextName = nextSegment === null ? null : renamedGameplayTagName(record.name, nextSegment);
  if (action === "rename" && !nextName) {
    error.value = "Enter one valid tag segment (letters, digits, or underscores; starting with a letter or underscore).";
    return;
  }
  if (nextName === record.name) return;
  if (action === "delete" && !window.confirm(`Delete ${record.name}?`)) return;
  try {
    if (action === "rename") await StudioApi.updateContent("gameplay_tags", record.id, { ...record, name: nextName! });
    else await StudioApi.deleteContent("gameplay_tags", record.id);
    await load();
    if (selectedTag.value.toLowerCase() === record.name.toLowerCase()) selectedTag.value = action === "rename" ? nextName! : "";
    status.value = action === "rename" ? "Tag renamed." : "Tag deleted.";
  } catch (cause) { error.value = cause instanceof Error ? cause.message : "Tag mutation failed."; }
}
function openReference(reference: { entityType: string; entityId: string; nodeId?: string; itemId?: string }) {
  if (reference.entityType === "entity") router.push(`/entities/${encodeURIComponent(reference.entityId)}`);
  else if (reference.entityType === "storylet") router.push({ path: `/storylets/${encodeURIComponent(reference.entityId)}/edit`, query: reference.nodeId ? { node: reference.nodeId } : {} });
  else if (reference.entityType === "trait_template") router.push(`/trait-templates/${encodeURIComponent(reference.entityId)}`);
  else router.push(`/entity-templates/${encodeURIComponent(reference.entityId)}`);
}
</script>

<template>
  <main class="tag-manager">
    <header class="tag-manager__header">
      <div class="tag-manager__intro">
        <h1>Tag manager</h1>
        <p>Manage the canonical hierarchical gameplay tag catalog used by authored content.</p>
      </div>
      <div class="tag-manager__toolbar">
        <SearchField v-model="query" class="tag-manager__search" label="Search tags" placeholder="Search tags…" />
        <SegmentedControl v-if="activeKind === 'gameplay'" v-model="tagViewMode" label="Tag view" :options="[{ value: 'tree', label: 'Tree' }, { value: 'list', label: 'List' }]" />
        <div v-if="activeKind === 'gameplay'" class="tag-manager__actions">
          <button type="button" class="btn btn-secondary" :disabled="!selectedTag || !selectedTagIsRegistered" :aria-label="selectedTag ? `Add child tag under ${selectedTag}` : 'Select a registered tag to add a child'" @click="selectedTagIsRegistered && createTag(selectedTag)">Add child</button>
          <button type="button" class="btn btn-primary" aria-label="Add root gameplay tag" @click="createTag()"><StudioIcon name="plus" /> Add root</button>
        </div>
      </div>
    </header>
    <p v-if="status" class="tag-manager__status" role="status">{{ status }}</p>

    <SegmentedControl
      v-if="TAG_KINDS.length > 1"
      v-model="activeKind"
      class="tag-manager__tabs"
      label="Tag categories"
      :options="TAG_KINDS.map((kind) => ({ value: kind.id, label: kind.label, count: kindCount(kind.id) }))"
    />

    <div v-if="loading" class="loading-state">Loading tag references…</div>
    <p v-else-if="error" class="tag-manager__error" role="alert">{{ error }}</p>
    <EmptyState v-else-if="!visibleTags.length" icon="tag" :title="`No ${TAG_KINDS.find((kind) => kind.id === activeKind)?.label.toLocaleLowerCase()} found.`">
      <button v-if="activeKind === 'gameplay' && !query" type="button" class="btn btn-primary" @click="createTag()"><StudioIcon name="plus" /> Add a root tag</button>
    </EmptyState>

    <!-- HIERARCHICAL TREE VIEW FOR GAMEPLAY TAGS -->
    <div v-else-if="activeKind === 'gameplay' && tagViewMode === 'tree'" class="tag-tree-container" role="tree" aria-label="Gameplay tag hierarchy">
      <div class="tag-tree">
        <TagTreeNode
          v-for="rootNode in hierarchicalTree"
          :key="rootNode.fullPath"
          :node="rootNode"
          :depth="0"
          :expanded-paths="expandedPaths"
          :selected-path="selectedTag"
          @toggle="toggleBranch"
          @select="selectedTag = $event"
          @add="createTag"
          @rename="mutateTag($event, 'rename')"
          @delete="mutateTag($event, 'delete')"
          @open="openReference"
        />
      </div>
    </div>
    <!-- STANDARD LIST VIEW -->
    <section v-else class="tag-manager__list" :aria-label="TAG_KINDS.find((kind) => kind.id === activeKind)?.label">
      <article v-for="tag in visibleTags" :key="`${tag.kind}:${tag.name.toLocaleLowerCase()}`" class="tag-manager__tag">
        <header class="tag-manager__tag-header">
          <h2>{{ tag.name }}</h2>
          <span>{{ tag.references.length }} {{ tag.references.length === 1 ? "reference" : "references" }}</span>
          <button type="button" class="btn btn-secondary" :disabled="!tag.registered" :aria-label="`Add child tag under ${tag.name}`" @click="tag.registered && createTag(tag.name)">Add Child</button>
          <button v-if="tag.registered && !tag.protected && !hasChildTags(tag.name)" type="button" class="btn btn-secondary" :disabled="tag.references.length > 0" :title="tag.references.length ? 'Remove references before renaming this tag.' : undefined" :aria-label="`Rename ${tag.name}`" @click="mutateTag(tag, 'rename')">Rename</button>
          <button v-if="tag.registered && !tag.protected && !hasChildTags(tag.name)" type="button" class="btn btn-secondary" :disabled="tag.references.length > 0" :title="tag.references.length ? 'Remove references before deleting this tag.' : undefined" :aria-label="`Delete ${tag.name}`" @click="mutateTag(tag, 'delete')">Delete</button>
        </header>
        <ul>
          <li v-for="reference in tag.references" :key="`${reference.entityType}:${reference.entityId}`">
            <div><strong>{{ reference.entityLabel }}</strong><small>{{ reference.detail }}</small></div>
            <button type="button" class="btn btn-secondary" @click="openReference(reference)">Open</button>
          </li>
        </ul>
      </article>
    </section>
  </main>
</template>

<style scoped>
.tag-manager { height: 100%; overflow: auto; padding: 20px 24px 32px; container-type: inline-size; }
.tag-manager__header { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: flex-end; gap: 16px 24px; margin-bottom: 20px; }
.tag-manager__intro { flex: 1 1 280px; min-width: 0; }
.tag-manager__header h1 { margin: 0; }
.tag-manager__header p { margin: 6px 0 0; color: var(--text-muted); font-size: 13px; max-width: 60ch; }
.tag-manager__toolbar { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
.tag-manager__search { flex: 1 1 180px; min-width: 160px; }
.tag-manager__tabs { margin-bottom: 20px; }
.tag-manager__error { color: var(--status-danger); }
.loading-state { color: var(--text-muted); padding: 40px 0; text-align: center; }
.tag-manager__list { display: flex; flex-direction: column; gap: 12px; }
.tag-manager__tag { background: rgba(255, 255, 255, 0.025); border: 1px solid var(--border-subtle); border-radius: var(--radius-lg); padding: 16px; }
.tag-manager__tag-header { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin-bottom: 12px; }
.tag-manager__tag-header h2 { margin: 0 auto 0 0; font-size: 15px; font-weight: 600; font-family: var(--font-mono); letter-spacing: 0; }
.tag-manager__tag-header span { font-size: 12px; color: var(--text-muted); }
.tag-manager__tag ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
.tag-manager__tag li { display: flex; justify-content: space-between; align-items: center; gap: 12px; padding: 8px 12px; background: rgba(0, 0, 0, 0.2); border-radius: var(--radius-md); }
.tag-manager__tag li div { display: flex; flex-direction: column; }
.tag-manager__tag li small { color: var(--text-muted); font-size: 11px; }

/* Hierarchical Tree */
.tag-tree-container { background: var(--bg-card); border: 1px solid var(--border-subtle); border-radius: 8px; padding: 12px; }
.tag-tree { display: flex; flex-direction: column; gap: 4px; }
.tag-tree-row { display: flex; align-items: center; gap: 8px; padding: 8px 12px; border-radius: 6px; cursor: pointer; user-select: none; }
.tag-tree-row:hover { background: var(--bg-card-hover, rgba(255,255,255,0.05)); }
.tag-tree-row.root-level { font-weight: 700; font-size: 14px; background: var(--bg-surface, rgba(0,0,0,0.2)); margin-top: 4px; }
.tag-tree-twisty { width: 14px; text-align: center; color: var(--text-muted); font-size: 11px; }
.tag-tree-label { font-size: 13px; font-weight: 600; }
.tag-tree-label.root-name { color: var(--accent, #38bdf8); }
.tag-tree-label.leaf { color: var(--text-main); }
.tag-tree-badge { font-family: var(--font-mono, monospace); font-size: 11px; color: var(--text-muted); background: var(--bg-surface); padding: 2px 6px; border-radius: 4px; }
.tag-tree-count { margin-left: auto; font-size: 11px; color: var(--text-muted); }
.tag-tree-refs { display: flex; flex-direction: column; gap: 4px; margin: 4px 0 8px; }
.tag-tree-ref-item { display: flex; justify-content: space-between; align-items: center; padding: 6px 10px; background: var(--bg-surface); border-radius: 4px; font-size: 12px; }
</style>
