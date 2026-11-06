<script setup lang="ts">
import { computed, inject, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { useCodexViewModel, type CodexCategory, type CodexRecord } from "../view-models/codex.ts";
import { CODEX_CATEGORIES } from "../definitions.ts";
import TagEditor from "../components/TagEditor.vue";
import { isChronicleEntry } from "../chronicleModel.ts";
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
watch(() => [activeCategory.value, selectedId.value, editingItem.value], () => {
  const item = editingItem.value;
  if (!item) {
    interactionContext?.setFocus({ entity: null, selection: null });
    return;
  }
  const label = item.title;
  const id = item.id || label;
  interactionContext?.setFocus({
    entity: { type: "event", id, label, data: JSON.parse(JSON.stringify(item)) },
    selection: { type: "codex_record", id, label },
  });
}, { deep: true });

function openRequestedEntity() {
  const category = route.query.category;
  const id = route.query.id;
  if (category !== "events" || typeof id !== "string") return;
  const requested = studioState.events.find((item) => item.id === id);
  if (requested) selectItem(requested);
}

onMounted(async () => {
  await loadAll();
  openRequestedEntity();
});
watch(() => [route.query.category, route.query.id], openRequestedEntity);

const currentList = computed(() => {
  const items: CodexRecord[] = studioState.events;
  if (!searchQuery.value.trim()) return items;
  const q = searchQuery.value.toLowerCase();
  return items.filter((item) => item.title.toLowerCase().includes(q) || item.id.toLowerCase().includes(q));
});

function selectItem(item: CodexRecord) {
  selectedId.value = item.id;
  editingItem.value = JSON.parse(JSON.stringify(item)) as CodexRecord;
}

function saveEditingItem(category: CodexCategory, item: CodexRecord | null) {
  if (item) void saveItem(category, item);
}

function createNew() {
  const id = `event_${Date.now().toString(36)}`;
  editingItem.value = { id, title: "New Event", description: "", event_type: "world", trigger: {}, effects: {}, delay_turns: 0, repeatable: false };
  selectedId.value = id;
}

function updateJsonField(field: "trigger" | "effects", event: Event) {
  if (!editingItem.value) return;
  const raw = (event.target as HTMLTextAreaElement).value.trim();
  try {
    editingItem.value[field] = raw ? JSON.parse(raw) : {};
  } catch {
    // Invalid JSON preserved until valid
  }
}

function itemLabel(item: CodexRecord): string {
  return item.title;
}

function itemDetail(item: CodexRecord): string {
  return isChronicleEntry(item) ? `canon · time ${item.story_time}` : item.event_type;
}

const locationOptions = computed(() => studioState.entityTemplates.filter((entity) => entity.root_tag === "location"));
const factOptions = computed(() => studioState.gameplayTags.map((tag) => tag.name).filter((name) => name.startsWith("fact.")));
const isCanon = computed(() => editingItem.value?.event_type === "canon");
function setStoryTime(event: Event) {
  if (!editingItem.value) return;
  const raw = (event.target as HTMLInputElement).value.trim();
  editingItem.value.story_time = raw === "" ? undefined : Number(raw);
}
function setFacts(value: string | string[]) {
  if (editingItem.value) editingItem.value.facts = Array.isArray(value) ? value : value ? [value] : [];
}
</script>

<template>
  <div style="display:flex; height:100%;">
    <!-- Master list -->
    <div style="width:320px; border-right:1px solid var(--border-subtle); display:flex; flex-direction:column; background:var(--bg-card);">
      <div style="padding:14px 16px; border-bottom:1px solid var(--border-subtle);">
        <div v-if="CODEX_CATEGORIES.length > 1" style="display:flex; gap:4px; flex-wrap:wrap; margin-bottom:10px;">
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
          <input v-model="searchQuery" class="form-input" placeholder="Filter…" aria-label="Filter events" style="flex:1;" />
          <button class="btn btn-secondary" title="New event" aria-label="New event" @click="createNew">+</button>
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
          <button type="button" class="btn" style="padding:2px 6px; color:var(--status-danger);" :title="'Delete ' + itemLabel(item)" :aria-label="'Delete ' + itemLabel(item)" @click.stop="deleteItem(activeCategory, item.id)">Delete</button>
        </div>
      </div>
    </div>

    <!-- Detail edit canvas -->
    <div style="flex:1; padding:24px 32px; overflow-y:auto;">
      <div v-if="!editingItem" style="display:flex; flex-direction:column; align-items:center; justify-content:center; padding:60px; color:var(--text-muted); gap:12px;">
        <span style="font-size:14px;">Select an event from the list or create a new one.</span>
        <button class="btn btn-primary" type="button" @click="createNew">+ New event</button>
      </div>

      <div v-else style="max-width:640px;">
        <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:20px;">
          <h2 style="font-size:18px; font-weight:700;">Edit event</h2>
          <button class="btn btn-primary" @click="saveEditingItem(activeCategory, editingItem)">Save record</button>
        </div>

        <div v-if="activeCategory === 'events' && 'title' in editingItem">
          <div class="form-group">
            <label class="form-label" for="event-id">Event ID</label>
            <input id="event-id" v-model="editingItem.id" class="form-input" />
          </div>
          <div class="form-group">
            <label class="form-label" for="event-title">Title</label>
            <input id="event-title" v-model="editingItem.title" class="form-input" />
          </div>
          <div class="form-group">
            <label class="form-label" for="event-type">Event type</label>
            <select id="event-type" v-model="editingItem.event_type" class="form-select">
              <option value="world">World event</option>
              <option value="canon">Canon event</option>
            </select>
          </div>
          <div class="form-group">
            <label class="form-label" for="event-desc">Description</label>
            <textarea id="event-desc" v-model="editingItem.description" class="form-textarea" placeholder="Event description…"></textarea>
          </div>
          <div style="display:grid; grid-template-columns:1fr 1fr; gap:12px;">
            <div class="form-group">
              <label class="form-label">Delay (turns)</label>
              <input v-model.number="editingItem.delay_turns" type="number" min="0" class="form-input" />
            </div>
            <div class="form-group" style="display:flex; align-items:center; gap:8px; margin-top:24px;">
              <input id="event-repeatable" v-model="editingItem.repeatable" type="checkbox" />
              <label for="event-repeatable" class="form-label" style="margin:0; cursor:pointer;">Repeatable</label>
            </div>
          </div>
          <template v-if="isCanon">
            <fieldset class="canon-fields">
              <legend>Chronicle entry</legend>
              <div style="display:grid; grid-template-columns:1fr 2fr; gap:12px;">
                <div class="form-group">
                  <label class="form-label" for="event-story-time">Story time</label>
                  <input id="event-story-time" :value="editingItem.story_time ?? ''" type="number" class="form-input" @input="setStoryTime" />
                </div>
                <div class="form-group">
                  <label class="form-label" for="event-story-label">Story time label</label>
                  <input id="event-story-label" v-model="editingItem.story_time_label" class="form-input" aria-label="Story time label" placeholder="e.g. D1 evening" />
                </div>
              </div>
              <div class="form-group">
                <label class="form-label" for="event-place">Place</label>
                <select id="event-place" v-model="editingItem.location_id" class="form-select">
                  <option :value="undefined">Anywhere</option>
                  <option v-for="place in locationOptions" :key="place.id" :value="place.id">{{ place.name }}</option>
                </select>
              </div>
              <div class="form-group">
                <TagEditor :model-value="editingItem.facts ?? []" :options="factOptions" :allow-create="false" label="Facts learned" placeholder="Choose a fact" @update:model-value="setFacts" />
              </div>
              <div class="form-group">
                <label class="form-label" for="event-text">Text</label>
                <textarea id="event-text" v-model="editingItem.text" class="form-textarea canon-text" rows="5" placeholder="What happened, in the author's words…"></textarea>
              </div>
            </fieldset>
          </template>
          <template v-else>
          <div class="form-group">
            <label class="form-label">Trigger condition (JSON)</label>
            <textarea class="form-textarea" rows="3" :value="JSON.stringify(editingItem.trigger || {}, null, 2)" placeholder="{}" @change="updateJsonField('trigger', $event)"></textarea>
          </div>
          <div class="form-group">
            <label class="form-label">Effects (JSON)</label>
            <textarea class="form-textarea" rows="3" :value="JSON.stringify(editingItem.effects || {}, null, 2)" placeholder="{}" @change="updateJsonField('effects', $event)"></textarea>
          </div>
          </template>
        </div>

      </div>
    </div>
  </div>
</template>

<style scoped>
.canon-fields { border: 1px solid var(--border-subtle); padding: 12px 16px; margin: 0 0 16px; }
.canon-fields legend { font-size: 11px; letter-spacing: 0.06em; text-transform: uppercase; color: var(--text-muted); padding: 0 4px; }
.canon-text { font-family: var(--font-serif); font-size: 16px; line-height: 24px; }
</style>
