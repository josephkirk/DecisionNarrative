<script setup lang="ts">
import { computed, inject, onMounted, watch } from "vue";
import { useRouter } from "vue-router";
import { useLibraryViewModel } from "../view-models/library.ts";
import { interactionContextKey } from "../interactionContext.ts";
import { clockLabels, originBadge, windowBadge } from "../storyletModel.ts";
import { thumbnailUrl } from "../thumbnails.ts";
import { studioState } from "../state.ts";
import StudioIcon from "../components/StudioIcon.vue";
import EmptyState from "../components/ui/EmptyState.vue";
import IconButton from "../components/ui/IconButton.vue";
import SearchField from "../components/ui/SearchField.vue";
import { countLabel } from "../countLabel.ts";
const router = useRouter();
const slots = computed(() => clockLabels(studioState.entities));
const participantName = (id: string) => studioState.entityTemplates.find((entity) => entity.id === id)?.name ?? id;
const {
  loading,
  result,
  filters,
  loadLibrary,
  setPage,
  resetFilters,
  deleteStorylet,
} = useLibraryViewModel();

const interactionContext = inject(interactionContextKey);
watch(() => [filters, result.value], () => {
  const visibleStorylets = result.value.items.map(({ id, title, revision, approved_revision, location_tags, role_tags }) => ({
    id, title, revision, approved_revision, location_tags, role_tags,
  }));
  interactionContext?.setFocus({
    entity: {
      type: "storylet_library",
      id: "storylets",
      label: "Storylets",
      data: { filters: { ...filters }, total: result.value.total, visibleStorylets },
    },
    selection: filters.q.trim() ? {
      type: "storylet_filter",
      id: filters.q.trim(),
      label: filters.q.trim(),
      data: { filters: { ...filters }, visibleStorylets },
    } : null,
  });
}, { deep: true });

onMounted(async () => {
  await loadLibrary();
});

function openStorylet(id: string) {
  router.push(`/storylets/${id}/edit`);
}
function createStorylet() {
  router.push("/storylets/new");
}

let searchDebounce: ReturnType<typeof setTimeout> | undefined;
function onSearchInput() {
  filters.page = 1;
  if (searchDebounce) clearTimeout(searchDebounce);
  searchDebounce = setTimeout(() => {
    loadLibrary();
  }, 200);
}

function onFilterChange() {
  filters.page = 1;
  loadLibrary();
}
</script>

<template>
  <div class="library-view">
    <header class="view-header">
      <div class="library-view__heading">
        <h1 class="view-title">Storylets</h1>
        <span class="badge">{{ result.total }} total</span>
      </div>
      <button type="button" class="btn btn-primary" @click="createStorylet">
        <StudioIcon name="plus" /> New storylet
      </button>
    </header>

    <div class="library-view__body">
      <div class="library-filters">
        <div class="filter-field library-filters__search">
          <label class="filter-label" for="lib-search">Search</label>
          <SearchField v-model="filters.q" input-id="lib-search" label="Search storylets" placeholder="Title, text, or tag…" @update:model-value="onSearchInput" />
        </div>
        <div class="filter-field">
          <label class="filter-label" for="lib-status">Status</label>
          <select id="lib-status" v-model="filters.status" class="form-select" aria-label="Filter by approval status" @change="onFilterChange">
            <option value="all">All status</option>
            <option value="approved">Approved</option>
            <option value="draft">Drafts</option>
          </select>
        </div>
        <div class="filter-field">
          <label class="filter-label" for="lib-location">Location tag</label>
          <select id="lib-location" v-model="filters.location_tag" class="form-select" aria-label="Filter by location tag" @change="onFilterChange">
            <option value="">All location tags</option>
            <option v-for="tag in result.available_location_tags" :key="tag" :value="tag">
              {{ tag }}
            </option>
          </select>
        </div>
        <div class="filter-field">
          <label class="filter-label" for="lib-ending">Scene type</label>
          <select id="lib-ending" v-model="filters.ending" class="form-select" aria-label="Filter by scene ending type" @change="onFilterChange">
            <option value="all">All scenes</option>
            <option value="ending">Endings only</option>
            <option value="non-ending">Non-endings</option>
          </select>
        </div>
        <div class="filter-field">
          <label class="filter-label" for="lib-sort">Sort by</label>
          <select id="lib-sort" v-model="filters.sort" class="form-select" aria-label="Sort storylets by" @change="onFilterChange">
            <option value="title">Sort: Title</option>
            <option value="created_desc">Sort: Newest</option>
          </select>
        </div>
        <button type="button" class="btn btn-secondary library-filters__reset" @click="resetFilters">
          Reset
        </button>
      </div>

      <div class="view-content scroll-fade-mask">
        <div v-if="loading" class="card-grid" aria-label="Loading storylets">
          <div v-for="n in 6" :key="n" class="skeleton-card">
            <div class="skeleton-header">
              <div class="skeleton-line skeleton-title"></div>
              <div class="skeleton-badge"></div>
            </div>
            <div class="skeleton-line skeleton-excerpt"></div>
            <div class="skeleton-line skeleton-excerpt short"></div>
            <div class="skeleton-footer">
              <div class="skeleton-pill"></div>
              <div class="skeleton-pill short"></div>
            </div>
          </div>
        </div>

        <EmptyState v-else-if="result.items.length === 0" icon="storylets" title="No storylets found" hint="Try adjusting your search query or filters.">
          <button type="button" class="btn btn-primary" @click="createStorylet"><StudioIcon name="plus" /> New storylet</button>
          <button type="button" class="btn btn-secondary" @click="resetFilters">Reset filters</button>
        </EmptyState>

        <div v-else class="card-grid">
          <div
            v-for="item in result.items"
            :key="item.id"
            class="storylet-card ui-reveal-host"
            tabindex="0"
            :aria-label="`Open ${item.title}`"
            @click="openStorylet(item.id)"
            @keydown.enter.self="openStorylet(item.id)"
          >
            <div>
              <div v-if="thumbnailUrl(item.thumbnail)" class="storylet-card__thumb">
                <img :src="thumbnailUrl(item.thumbnail) ?? ''" :alt="item.title" />
              </div>
              <h3 class="card-title">{{ item.title }}</h3>
              <div class="storylet-card__badges">
                <span :class="['badge', item.approved_revision === item.revision ? 'approved' : '']">
                  {{ item.approved_revision === item.revision ? 'Approved' : 'Draft' }} · v{{ item.revision }}
                </span>
                <span v-if="item.ending" class="badge ending">Ending</span>
                <span v-if="windowBadge(item.window, slots)" class="badge">{{ windowBadge(item.window, slots) }}</span>
                <span v-for="id in item.played_by ?? []" :key="id" class="badge" :aria-label="`Played by ${participantName(id)}`" :title="`Played by ${participantName(id)}`"><img v-if="thumbnailUrl(studioState.entityTemplates.find(entity => entity.id === id)?.thumbnail)" :src="thumbnailUrl(studioState.entityTemplates.find(entity => entity.id === id)?.thumbnail) ?? undefined" alt="" width="20" height="20" style="object-fit:cover; border-radius:3px; vertical-align:middle;" /><template v-else>{{ participantName(id).slice(0, 1).toUpperCase() }}</template></span>
                <span v-if="originBadge(item)" class="badge">{{ originBadge(item) }}</span>
              </div>

              <p class="card-excerpt">{{ item.excerpt || "(No scene text yet)" }}</p>
            </div>

            <div class="card-footer">
              <span class="storylet-card__meta">
                <StudioIcon name="pin" />
                <span>{{ item.location_tags.length ? item.location_tags.join(", ") : "Any location" }}</span>
              </span>
              <span class="storylet-card__meta">{{ countLabel(item.choice_count, "choice") }}</span>
              <IconButton icon="trash" label="Delete storylet" tone="danger" reveal @click.stop="deleteStorylet(item.id)" @keydown.enter.stop />
            </div>
          </div>
        </div>

        <nav v-if="result.total_pages > 1" class="library-pager" aria-label="Storylet pages">
          <button
            type="button"
            class="btn btn-secondary"
            :disabled="filters.page <= 1"
            @click="setPage(filters.page - 1)"
          >
            Previous
          </button>
          <span class="library-pager__label">
            Page {{ filters.page }} of {{ result.total_pages }}
          </span>
          <button
            type="button"
            class="btn btn-secondary"
            :disabled="filters.page >= result.total_pages"
            @click="setPage(filters.page + 1)"
          >
            Next
          </button>
        </nav>
      </div>
    </div>
  </div>
</template>

<style scoped>
.library-view { display: flex; flex-direction: column; height: 100%; min-height: 0; }
.library-view__heading { display: flex; align-items: center; gap: 10px; min-width: 0; }
.library-view__body { display: flex; flex-direction: column; flex: 1; min-height: 0; overflow: hidden; }
.library-filters {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  gap: 12px;
  padding: 14px 20px;
  border-bottom: 1px solid var(--border-faint);
}
.library-filters .filter-field { flex: 1 1 140px; min-width: 0; max-width: 200px; }
.library-filters .library-filters__search { flex: 2 1 220px; max-width: 320px; }
.library-filters__reset { margin-left: auto; }
.storylet-card__thumb { margin: -6px -6px 12px; border-radius: var(--radius-md); overflow: hidden; background: rgba(0, 0, 0, 0.3); }
.storylet-card__thumb img { display: block; width: 100%; height: 120px; object-fit: cover; }
.storylet-card__badges { display: flex; flex-wrap: wrap; gap: 4px; margin: 8px 0 10px; }
.storylet-card__meta { display: inline-flex; align-items: center; gap: 6px; min-width: 0; }
.storylet-card__meta > span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.storylet-card__meta:first-child { flex: 1; }
.library-pager { display: flex; justify-content: center; align-items: center; gap: 16px; margin-top: 28px; }
.library-pager__label { font-size: 12px; color: var(--text-muted); font-variant-numeric: tabular-nums; }
</style>
