<script setup lang="ts">
import { onMounted, computed, ref, watch } from "vue";
import { useRoute } from "vue-router";
import TagEditor from "../components/TagEditor.vue";

import { useCodexViewModel } from "../view-models/codex.ts";
import { CODEX_CATEGORIES } from "../definitions.ts";
import { studioState } from "../state.ts";
import { StudioApi } from "../api.ts";
import type { StoryletRecord } from "../contracts.ts";
import { buildTagIndex, uniqueTagNames } from "../tagData.ts";

const {
  activeCategory,
  searchQuery,
  selectedId,
  loading,
  loadAll,
  saveItem,
  deleteItem,
} = useCodexViewModel();
const editingItem = ref<any>(null);
const route = useRoute();
const availableStorylets = ref<StoryletRecord[]>([]);
const tagIndex = computed(() => buildTagIndex({ characters: studioState.characters, locations: studioState.locations, storylets: availableStorylets.value }));
const characterRoleOptions = computed(() => uniqueTagNames(tagIndex.value.filter((tag) => tag.kind === "role").map((tag) => tag.name)));
const locationTagOptions = computed(() => uniqueTagNames(tagIndex.value.filter((tag) => tag.kind === "location").map((tag) => tag.name)));

function openRequestedEntity() {
  const category = route.query.category;
  const id = route.query.id;
  if (typeof category !== "string" || typeof id !== "string") return;
  const codexCategory = CODEX_CATEGORIES.find((item) => item.id === category);
  if (!codexCategory) return;
  activeCategory.value = codexCategory.id as any;
  const source = category === "characters" ? studioState.characters : category === "locations" ? studioState.locations : [];
  const requested = source.find((item) => (item.id || item.name) === id);
  if (requested) selectItem(requested);
}

onMounted(async () => {
  const [_, storylets] = await Promise.all([
    loadAll(),
    StudioApi.listContent<StoryletRecord>("storylets").catch(() => []),
  ]);
  availableStorylets.value = storylets;
  openRequestedEntity();
});
watch(() => [route.query.category, route.query.id], openRequestedEntity);

const currentList = computed(() => {
  const cat = activeCategory.value;
  let items: any[] = [];
  if (cat === "qualities") items = studioState.qualities;
  else if (cat === "characters") items = studioState.characters;
  else if (cat === "locations") items = studioState.locations;
  else if (cat === "npc_behaviors") items = studioState.npcBehaviors;
  else if (cat === "events") items = studioState.events;

  if (!searchQuery.value.trim()) return items;
  const q = searchQuery.value.toLowerCase();
  return items.filter((item) => {
    const text = (item.name || item.id || item.title || "").toLowerCase();
    return text.includes(q);
  });
});

function selectItem(item: any) {
  selectedId.value = item.id || item.name;
  editingItem.value = JSON.parse(JSON.stringify(item));
}

function saveEditingItem(category: string, item: any) {
  if (category === "characters") item.role = String(item.role ?? "").trim();
  if (category === "locations") item.tags = uniqueTagNames(item.tags ?? []);
  saveItem(category, item);
}

function createNew() {
  const cat = activeCategory.value;
  if (cat === "qualities") {
    editingItem.value = { name: "new_quality", type: "metric", value: 0, description: "" };
  } else if (cat === "characters") {
    editingItem.value = { id: "char_new", name: "New Character", role: "witness", home_location: "", current_location: "", agenda: "", red_line: "" };
  } else if (cat === "locations") {
    editingItem.value = { id: "loc_new", name: "New Location", description: "", tags: [], connected_zones: [] };
  } else if (cat === "npc_behaviors") {
    editingItem.value = { id: "npc_new", actor_id: "", priority: 10, action: "wait", conditions: {}, effects: [] };
  } else if (cat === "events") {
    editingItem.value = { id: "event_new", title: "New Event", description: "", event_type: "world", trigger: {}, effects: {} };
  }
}
</script>

<template>
  <div style="display:flex; height:100%;">
    <!-- Master list -->
    <div style="width:320px; border-right:1px solid var(--border-subtle); display:flex; flex-direction:column; background:var(--bg-card);">
      <div style="padding:14px 16px; border-bottom:1px solid var(--border-subtle);">
        <div style="display:flex; gap:4px; flex-wrap:wrap; margin-bottom:10px;">
          <button
            v-for="cat in CODEX_CATEGORIES"
            :key="cat.id"
            :class="['btn', activeCategory === cat.id ? 'btn-primary' : 'btn-secondary']"
            style="font-size:11px; padding:3px 8px;"
            @click="activeCategory = cat.id as any; editingItem = null;"
          >
            {{ cat.label }}
          </button>
        </div>

        <div style="display:flex; gap:8px;">
          <input v-model="searchQuery" class="form-input" placeholder="Filter..." style="flex:1;" />
          <button class="btn btn-secondary" @click="createNew">+</button>
        </div>
      </div>

      <div style="flex:1; overflow-y:auto; padding:8px;">
        <div
          v-for="item in currentList"
          :key="item.id || item.name"
          :style="{
            padding: '8px 12px',
            borderRadius: '8px',
            marginBottom: '4px',
            cursor: 'pointer',
            background: (item.id || item.name) === selectedId ? 'var(--accent-subtle)' : 'transparent',
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center'
          }"
          @click="selectItem(item)"
        >
          <div>
            <div style="font-weight:600; font-size:13px;">{{ item.name || item.title || item.id }}</div>
            <div style="font-size:11px; color:var(--text-muted);">{{ item.role || item.type || item.event_type || item.id }}</div>
          </div>
          <button class="btn" style="padding:2px 6px; color:var(--status-danger);" @click.stop="deleteItem(activeCategory, item.id || item.name)">✕</button>
        </div>
      </div>
    </div>

    <!-- Detail edit canvas -->
    <div style="flex:1; padding:24px 32px; overflow-y:auto;">
      <div v-if="!editingItem" style="text-align:center; padding:60px; color:var(--text-muted);">
        Select an item from the Codex list or create a new one.
      </div>

      <div v-else style="max-width:640px;">
        <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:20px;">
          <h2 style="font-size:18px; font-weight:700;">Edit {{ activeCategory }}</h2>
          <button class="btn btn-primary" @click="saveEditingItem(activeCategory, editingItem)">Save Record</button>
        </div>

        <div v-if="activeCategory === 'qualities'">
          <div class="form-group">
            <label class="form-label">Quality Name / Key</label>
            <input v-model="editingItem.name" class="form-input" />
          </div>
          <div class="form-group">
            <label class="form-label">Type</label>
            <select v-model="editingItem.type" class="form-select">
              <option value="currency">Currency</option>
              <option value="metric">Metric</option>
              <option value="tool">Tool</option>
              <option value="territory">Territory</option>
              <option value="menace">Menace</option>
            </select>
          </div>
          <div class="form-group">
            <label class="form-label">Default Numeric Value</label>
            <input v-model.number="editingItem.value" type="number" class="form-input" />
          </div>
        </div>

        <div v-else-if="activeCategory === 'characters'">
          <div class="form-group">
            <label class="form-label">Character ID</label>
            <input v-model="editingItem.id" class="form-input" />
          </div>
          <div class="form-group">
            <label class="form-label">Display Name</label>
            <input v-model="editingItem.name" class="form-input" />
          </div>
          <div class="form-group">
            <label class="form-label">Role Tag</label>
            <TagEditor v-model="editingItem.role" :options="characterRoleOptions" :multiple="false" label="Character role tag" placeholder="Choose or create a role tag" />
          </div>
          <div class="form-group">
            <label class="form-label">Home Location</label>
            <input v-model="editingItem.home_location" class="form-input" />
          </div>
        </div>

        <div v-else-if="activeCategory === 'locations'">
          <div class="form-group">
            <label class="form-label">Location ID</label>
            <input v-model="editingItem.id" class="form-input" />
          </div>
          <div class="form-group">
            <label class="form-label">Name</label>
            <input v-model="editingItem.name" class="form-input" />
          </div>
          <div class="form-group">
            <label class="form-label">Description</label>
            <textarea v-model="editingItem.description" class="form-textarea" style="min-height:80px;"></textarea>
          </div>
          <div class="form-group">
            <label class="form-label">Location Tags</label>
            <TagEditor v-model="editingItem.tags" :options="locationTagOptions" label="Location tags" placeholder="Choose or create location tags" />
          </div>
        </div>

        <div v-else-if="activeCategory === 'events'">
          <div class="form-group">
            <label class="form-label">Event ID</label>
            <input v-model="editingItem.id" class="form-input" />
          </div>
          <div class="form-group">
            <label class="form-label">Title</label>
            <input v-model="editingItem.title" class="form-input" />
          </div>
          <div class="form-group">
            <label class="form-label">Event Type</label>
            <select v-model="editingItem.event_type" class="form-select">
              <option value="world">World Event</option>
              <option value="canon">Canon Event</option>
            </select>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
