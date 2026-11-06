<script setup lang="ts">
import { computed, inject, onMounted, watch } from "vue";
import { useRouter } from "vue-router";
import { useLibraryViewModel } from "../view-models/library.ts";
import { interactionContextKey } from "../interactionContext.ts";
import { clockLabels, originBadge, windowBadge } from "../storyletModel.ts";
import { thumbnailUrl } from "../thumbnails.ts";
import { studioState } from "../state.ts";
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
  <div style="display:flex; flex-direction:column; height:100%;">
    <!-- Top Header -->
    <header class="view-header">
      <div style="display:flex; align-items:center; gap:16px;">
        <h1 class="view-title">Storylets</h1>
        <span class="badge">{{ result.total }} total</span>
      </div>

      <div style="display:flex; align-items:center; gap:10px;">
        <button class="btn btn-primary" @click="createStorylet">
          + New storylet
        </button>
      </div>
    </header>

    <div style="display:flex; flex-direction:column; flex:1; overflow:hidden;">
      <div class="library-filters" style="padding:12px 20px; border-bottom:1px solid var(--border-subtle); display:flex; gap:12px; align-items:flex-end; background:var(--bg-card); flex-wrap:wrap;">
        <div class="filter-field">
          <label class="filter-label" for="lib-search">Search</label>
          <input
            id="lib-search"
            v-model="filters.q"
            class="form-input"
            placeholder="Search title, text, or group tag…"
            aria-label="Search storylets"
            style="width: 240px;"
            @input="onSearchInput"
          />
        </div>

        <div class="filter-field">
          <label class="filter-label" for="lib-status">Status</label>
          <select id="lib-status" v-model="filters.status" class="form-select" aria-label="Filter by approval status" style="width: 130px;" @change="onFilterChange">
            <option value="all">All status</option>
            <option value="approved">Approved</option>
            <option value="draft">Drafts</option>
          </select>
        </div>
        <div class="filter-field">
          <label class="filter-label" for="lib-location">Location tag</label>
          <select id="lib-location" v-model="filters.location_tag" class="form-select" aria-label="Filter by location tag" style="width: 160px;" @change="onFilterChange">
            <option value="">All location tags</option>
            <option v-for="tag in result.available_location_tags" :key="tag" :value="tag">
              {{ tag }}
            </option>
          </select>
        </div>

        <div class="filter-field">
          <label class="filter-label" for="lib-ending">Scene type</label>
          <select id="lib-ending" v-model="filters.ending" class="form-select" aria-label="Filter by scene ending type" style="width: 130px;" @change="onFilterChange">
            <option value="all">All scenes</option>
            <option value="ending">Endings only</option>
            <option value="non-ending">Non-endings</option>
          </select>
        </div>
        <div class="filter-field">
          <label class="filter-label" for="lib-sort">Sort by</label>
          <select id="lib-sort" v-model="filters.sort" class="form-select" aria-label="Sort storylets by" style="width: 130px;" @change="onFilterChange">
            <option value="title">Sort: Title</option>
            <option value="created_desc">Sort: Newest</option>
          </select>
        </div>
        <button class="btn btn-secondary" style="margin-left:auto; align-self:flex-end;" @click="resetFilters">
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

        <div v-else-if="result.items.length === 0" class="empty-state" style="padding: 60px; text-align:center; color:var(--text-muted); display:flex; flex-direction:column; align-items:center; gap:12px;">
          <p style="font-size:15px; font-weight:500; color:var(--text-main);">No storylets found</p>
          <p style="font-size:12px; margin-top:0;">Try adjusting your search query or filters.</p>
          <div style="display:flex; gap:12px; margin-top:8px;">
            <button class="btn btn-primary" @click="createStorylet">+ New storylet</button>
            <button class="btn btn-secondary" @click="resetFilters">Reset filters</button>
          </div>
        </div>

        <div v-else class="card-grid">
          <div
            v-for="item in result.items"
            :key="item.id"
            class="storylet-card"
            @click="openStorylet(item.id)"
          >
            <div>
              <div v-if="thumbnailUrl(item.thumbnail)" style="margin-bottom:8px; border-radius:6px; overflow:hidden; max-height:120px; display:flex; align-items:center; justify-content:center; background:rgba(0,0,0,0.3);">
                <img :src="thumbnailUrl(item.thumbnail) ?? ''" style="width:100%; height:120px; object-fit:cover;" :alt="item.title" />
              </div>
              <div class="card-header">
                <h3 class="card-title">{{ item.title }}</h3>
                <div style="display:flex; gap:4px;">
                  <span v-if="item.ending" class="badge ending">Ending</span>
                  <span v-if="windowBadge(item.window, slots)" class="badge">{{ windowBadge(item.window, slots) }}</span>
                  <span v-for="id in item.played_by ?? []" :key="id" class="badge" :aria-label="`Played by ${participantName(id)}`" :title="`Played by ${participantName(id)}`">{{ participantName(id).slice(0, 1).toUpperCase() }}</span>
                  <span v-if="originBadge(item)" class="badge">{{ originBadge(item) }}</span>
                  <span :class="['badge', item.approved_revision === item.revision ? 'approved' : '']">
                    {{ item.approved_revision === item.revision ? 'Approved' : 'Draft' }} · v{{ item.revision }}
                  </span>
                </div>
              </div>

              <p class="card-excerpt">{{ item.excerpt || "(No scene text yet)" }}</p>
            </div>

            <div class="card-footer">
              <span>📍 {{ item.location_tags.length ? item.location_tags.join(", ") : "Any location" }}</span>
              <div style="display:flex; align-items:center; gap:8px;">
                <span>{{ item.choice_count }} choices</span>
                <button
                  class="btn"
                  style="padding:4px 8px; font-size:11px; color:var(--status-danger); min-height:28px;"
                  title="Delete storylet"
                  aria-label="Delete storylet"
                  @click.stop="deleteStorylet(item.id)"
                >
                  ✕
                </button>
              </div>
            </div>
          </div>
        </div>

        <!-- Pagination Footer -->
        <div v-if="result.total_pages > 1" style="display:flex; justify-content:center; align-items:center; gap:16px; margin-top:28px;">
          <button
            class="btn btn-secondary"
            :disabled="filters.page <= 1"
            @click="setPage(filters.page - 1)"
          >
            Previous
          </button>
          <span style="font-size:12px; color:var(--text-muted);">
            Page {{ filters.page }} of {{ result.total_pages }}
          </span>
          <button
            class="btn btn-secondary"
            :disabled="filters.page >= result.total_pages"
            @click="setPage(filters.page + 1)"
          >
            Next
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
