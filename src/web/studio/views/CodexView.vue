<script setup lang="ts">
import { computed, inject, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { useCodexViewModel, type CodexCategory, type CodexRecord } from "../view-models/codex.ts";
import { CODEX_CATEGORIES } from "../definitions.ts";
import { studioState } from "../state.ts";
import { interactionContextKey } from "../interactionContext.ts";

const {
  activeCategory,
  searchQuery,
  selectedId,
  loadAll,
  saveItem,
  deleteItem,
} = useCodexViewModel();
const editingItem = ref<CodexRecord | null>(null);
const route = useRoute();
const interactionContext = inject(interactionContextKey);
const contextEntityTypes: Record<string, string> = {
  npc_behaviors: "npc_behavior",
  events: "event",
};
watch(() => [activeCategory.value, selectedId.value, editingItem.value], () => {
  const item = editingItem.value;
  if (!item) {
    interactionContext?.setFocus({ entity: null, selection: null });
    return;
  }
  const label = "title" in item ? item.title : item.action;
  const id = item.id || label;
  interactionContext?.setFocus({
    entity: { type: contextEntityTypes[activeCategory.value] ?? activeCategory.value, id, label, data: JSON.parse(JSON.stringify(item)) },
    selection: { type: "codex_record", id, label },
  });
}, { deep: true });

function openRequestedEntity() {
  const category = route.query.category;
  const id = route.query.id;
  if (typeof category !== "string" || typeof id !== "string") return;
  const codexCategory = CODEX_CATEGORIES.find((item) => item.id === category);
  if (!codexCategory) return;
  activeCategory.value = codexCategory.id;
  const source = codexCategory.id === "npc_behaviors" ? studioState.npcBehaviors : studioState.events;
  const requested = source.find((item) => item.id === id);
  if (requested) selectItem(requested);
}

onMounted(async () => {
  await loadAll();
  openRequestedEntity();
});
watch(() => [route.query.category, route.query.id], openRequestedEntity);

const currentList = computed(() => {
  const items: CodexRecord[] =
    activeCategory.value === "npc_behaviors" ? studioState.npcBehaviors : studioState.events;
  if (!searchQuery.value.trim()) return items;
  const q = searchQuery.value.toLowerCase();
  return items.filter((item) => {
    const text = ("title" in item ? item.title : item.action).toLowerCase();
    return text.includes(q) || item.id.toLowerCase().includes(q);
  });
});

function selectItem(item: CodexRecord) {
  selectedId.value = item.id;
  editingItem.value = JSON.parse(JSON.stringify(item)) as CodexRecord;
}

function saveEditingItem(category: CodexCategory, item: CodexRecord | null) {
  if (!item) return;
  void saveItem(category, item);
}

function createNew() {
  const category = activeCategory.value;
  if (category === "npc_behaviors") {
    editingItem.value = { id: "npc_new", actor_id: "", priority: 10, conditions: {}, action: "wait", effects: [] };
  } else {
    editingItem.value = { id: "event_new", title: "New Event", description: "", event_type: "world", trigger: {}, effects: {} };
  }
}

function itemLabel(item: CodexRecord): string {
  return "title" in item ? item.title : item.action;
}

function itemDetail(item: CodexRecord): string {
  return "event_type" in item ? item.event_type : item.id;
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
            @click="activeCategory = cat.id; editingItem = null;"
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
          :key="item.id"
          :style="{
            padding: '8px 12px',
            borderRadius: '8px',
            marginBottom: '4px',
            cursor: 'pointer',
            background: item.id === selectedId ? 'var(--accent-subtle)' : 'transparent',
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center'
          }"
          @click="selectItem(item)"
        >
          <div>
            <div style="font-weight:600; font-size:13px;">{{ itemLabel(item) }}</div>
            <div style="font-size:11px; color:var(--text-muted);">{{ itemDetail(item) }}</div>
          </div>
          <button class="btn" style="padding:2px 6px; color:var(--status-danger);" @click.stop="deleteItem(activeCategory, item.id)">✕</button>
        </div>
      </div>
    </div>

    <!-- Detail edit canvas -->
    <div style="flex:1; padding:24px 32px; overflow-y:auto;">
      <div v-if="!editingItem" style="text-align:center; padding:60px; color:var(--text-muted);">
        Select an item from the list or create a new one.
      </div>

      <div v-else style="max-width:640px;">
        <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:20px;">
          <h2 style="font-size:18px; font-weight:700;">Edit {{ activeCategory === "npc_behaviors" ? "NPC Rule" : "Event" }}</h2>
          <button class="btn btn-primary" @click="saveEditingItem(activeCategory, editingItem)">Save Record</button>
        </div>

        <div v-if="activeCategory === 'events' && 'title' in editingItem">
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

        <div v-else-if="'action' in editingItem">
          <div class="form-group">
            <label class="form-label">Rule ID</label>
            <input v-model="editingItem.id" class="form-input" />
          </div>
          <div class="form-group">
            <label class="form-label">Actor</label>
            <select v-model="editingItem.actor_id" class="form-select">
              <option value="">Choose a character</option>
              <option v-for="character in studioState.characters" :key="character.id" :value="character.id">{{ character.name }}</option>
            </select>
          </div>
          <div class="form-group">
            <label class="form-label">Priority</label>
            <input v-model.number="editingItem.priority" type="number" class="form-input" />
          </div>
          <div class="form-group">
            <label class="form-label">Action</label>
            <input v-model="editingItem.action" class="form-input" />
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
