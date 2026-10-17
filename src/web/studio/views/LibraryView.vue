<script setup lang="ts">
import { onMounted } from "vue";
import { useRouter } from "vue-router";
import { useLibraryViewModel } from "../view-models/library.ts";

const router = useRouter();
const {
  loading,
  result,
  filters,
  loadLibrary,
  setPage,
  resetFilters,
  deleteStorylet,
} = useLibraryViewModel();

onMounted(() => {
  loadLibrary();
});

function openStorylet(id: string) {
  router.push(`/storylets/${id}/edit`);
}

function createStorylet() {
  router.push("/storylets/new");
}
</script>

<template>
  <div style="display:flex; flex-direction:column; height:100%;">
    <header class="view-header">
      <div style="display:flex; align-items:center; gap:16px;">
        <h1 class="view-title">Storylets</h1>
        <span class="badge">{{ result.total }} Total</span>
      </div>

      <div style="display:flex; align-items:center; gap:12px;">
        <button class="btn btn-primary" @click="createStorylet">
          + New Storylet
        </button>
      </div>
    </header>

    <div style="padding:12px 20px; border-bottom:1px solid var(--border-subtle); display:flex; gap:12px; align-items:center; background:var(--bg-card); flex-wrap:wrap;">
      <input
        v-model="filters.q"
        class="form-input"
        placeholder="Search title, text, or group tag..."
        style="width: 240px;"
        @input="loadLibrary"
      />

      <select v-model="filters.status" class="form-select" style="width: 130px;" @change="loadLibrary">
        <option value="all">All Status</option>
        <option value="approved">Approved</option>
        <option value="draft">Drafts</option>
      </select>

      <select v-model="filters.location_id" class="form-select" style="width: 140px;" @change="loadLibrary">
        <option value="">All Locations</option>
        <option v-for="loc in result.available_locations" :key="loc" :value="loc">
          {{ loc }}
        </option>
      </select>

      <select v-model="filters.ending" class="form-select" style="width: 130px;" @change="loadLibrary">
        <option value="all">All Scenes</option>
        <option value="ending">Endings Only</option>
        <option value="non-ending">Non-Endings</option>
      </select>

      <select v-model="filters.sort" class="form-select" style="width: 130px;" @change="loadLibrary">
        <option value="title">Sort: Title</option>
        <option value="created_desc">Sort: Newest</option>
      </select>

      <button class="btn btn-secondary" style="margin-left:auto;" @click="resetFilters">
        Reset
      </button>
    </div>

    <div class="view-content">
      <div v-if="loading" style="padding: 40px; text-align:center; color:var(--text-muted);">
        Loading storylet deck...
      </div>

      <div v-else-if="result.items.length === 0" style="padding: 60px; text-align:center; color:var(--text-muted);">
        <p style="font-size:15px; font-weight:500;">No storylets found</p>
        <p style="font-size:12px; margin-top:6px;">Try adjusting your search query or filters.</p>
      </div>

      <div v-else class="card-grid">
        <div
          v-for="item in result.items"
          :key="item.id"
          class="storylet-card"
          @click="openStorylet(item.id)"
        >
          <div>
            <div class="card-header">
              <h3 class="card-title">{{ item.title }}</h3>
              <div style="display:flex; gap:4px;">
                <span v-if="item.ending" class="badge ending">Ending</span>
                <span :class="['badge', item.approved_revision === item.revision ? 'approved' : '']">
                  {{ item.approved_revision === item.revision ? 'Approved' : 'Draft' }} · v{{ item.revision }}
                </span>
              </div>
            </div>

            <p class="card-excerpt">{{ item.excerpt || "(No scene text yet)" }}</p>
          </div>

          <div class="card-footer">
            <span>📍 {{ item.location ?? "Any location" }}</span>
            <div style="display:flex; align-items:center; gap:8px;">
              <span>{{ item.choice_count }} choices</span>
              <button
                class="btn"
                style="padding:2px 6px; font-size:11px; color:var(--status-danger);"
                title="Delete storylet"
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
</template>
