<script setup lang="ts">
import { computed, inject, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { studioState } from "../state.ts";
import { buildTagIndex, TAG_KINDS, type TagKind } from "../tagData.ts";
import { interactionContextKey } from "../interactionContext.ts";
import { refreshWorkspace } from "../view-models/workspace.ts";

const router = useRouter();
const activeKind = ref<TagKind>("role");
const query = ref("");
const loading = ref(true);
const error = ref("");

onMounted(async () => {
  try {
    await refreshWorkspace(["entities", "storylets"]);
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : "Unable to load tag references.";
  } finally {
    loading.value = false;
  }
});

const allTags = computed(() => buildTagIndex({
  characters: studioState.characters,
  locations: studioState.locations,
  storylets: studioState.storylets,
}));
const visibleTags = computed(() => allTags.value.filter((tag) => tag.kind === activeKind.value && tag.name.toLocaleLowerCase().includes(query.value.trim().toLocaleLowerCase())));
const kindCount = (kind: TagKind) => allTags.value.filter((tag) => tag.kind === kind).length;
const interactionContext = inject(interactionContextKey);
watch(() => [activeKind.value, query.value, visibleTags.value], () => {
  const search = query.value.trim();
  interactionContext?.setFocus({
    entity: { type: "tag_manager", id: activeKind.value, label: "Tag Manager", data: { category: activeKind.value, total: kindCount(activeKind.value) } },
    selection: search ? {
      type: "tag_filter",
      id: `${activeKind.value}:${search}`,
      label: search,
      data: visibleTags.value.map((tag) => ({ name: tag.name, references: tag.references })),
    } : null,
  });
}, { deep: true, immediate: true });
function openReference(reference: { entityType: string; entityId: string; nodeId?: string; itemId?: string }) {
  if (reference.entityType === "character" || reference.entityType === "location") {
    router.push(`/entities/${encodeURIComponent(reference.entityId)}`);
  } else {
    router.push({ path: `/storylets/${encodeURIComponent(reference.entityId)}/edit`, query: { ...(reference.nodeId ? { node: reference.nodeId } : {}), ...(reference.itemId ? { item: reference.itemId } : {}) } });
  }
}
</script>

<template>
  <main class="tag-manager">
    <header class="tag-manager__header">
      <div>
        <h1>Tag Manager</h1>
        <p>Review every role, location, emotion, and action tag with its entity references.</p>
      </div>
      <input v-model="query" class="form-input" placeholder="Search tags…" aria-label="Search tags" />
    </header>
    <nav class="tag-manager__tabs" aria-label="Tag categories">
      <button v-for="kind in TAG_KINDS" :key="kind.id" :class="['btn', activeKind === kind.id ? 'btn-primary' : 'btn-secondary']" @click="activeKind = kind.id">
        {{ kind.label }} <span class="tag-manager__count">{{ kindCount(kind.id) }}</span>
      </button>
    </nav>
    <div v-if="loading" class="loading-state">Loading tag references…</div>
    <p v-else-if="error" class="tag-manager__error">{{ error }}</p>
    <div v-else-if="!visibleTags.length" class="tag-manager__empty">No {{ TAG_KINDS.find((kind) => kind.id === activeKind)?.label.toLocaleLowerCase() }} found.</div>
    <section v-else class="tag-manager__list" :aria-label="TAG_KINDS.find((kind) => kind.id === activeKind)?.label">
      <article v-for="tag in visibleTags" :key="`${tag.kind}:${tag.name.toLocaleLowerCase()}`" class="tag-manager__tag">
        <header class="tag-manager__tag-header">
          <h2>{{ tag.name }}</h2>
          <span>{{ tag.references.length }} {{ tag.references.length === 1 ? "entity" : "entities" }}</span>
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
.tag-manager { height: 100%; overflow: auto; padding: 24px 32px; }
.tag-manager__header { display: flex; justify-content: space-between; align-items: center; gap: 20px; margin-bottom: 20px; }
.tag-manager__header h1 { margin: 0; font-size: 22px; }
.tag-manager__header p { margin: 6px 0 0; color: var(--text-muted); font-size: 13px; }
.tag-manager__header .form-input { width: min(280px, 40%); }
.tag-manager__tabs { display: flex; flex-wrap: wrap; gap: 8px; margin-bottom: 20px; }
.tag-manager__count { margin-left: 5px; opacity: .75; }
.tag-manager__list { display: grid; gap: 12px; max-width: 900px; }
.tag-manager__tag { border: 1px solid var(--border-subtle); border-radius: 10px; background: var(--bg-card); padding: 14px 16px; }
.tag-manager__tag-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px; }
.tag-manager__tag-header h2 { margin: 0; font-size: 15px; }
.tag-manager__tag-header span, .tag-manager__tag li small { color: var(--text-muted); font-size: 11px; }
.tag-manager__tag ul { list-style: none; padding: 0; margin: 0; }
.tag-manager__tag li { display: flex; align-items: center; justify-content: space-between; gap: 12px; border-top: 1px solid var(--border-subtle); padding: 9px 0; }
.tag-manager__tag li div { display: grid; gap: 3px; }
.tag-manager__empty, .tag-manager__error { padding: 24px; color: var(--text-muted); }
.tag-manager__error { color: var(--status-danger); }
@media (max-width: 680px) { .tag-manager { padding: 18px; } .tag-manager__header { align-items: stretch; flex-direction: column; } .tag-manager__header .form-input { width: auto; } }
</style>
