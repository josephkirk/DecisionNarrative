<script setup lang="ts">
import { computed, inject, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { useCodexViewModel, type CodexCategory, type CodexRecord } from "../view-models/codex.ts";
import { CODEX_CATEGORIES } from "../definitions.ts";
import TagEditor from "../components/TagEditor.vue";
import StudioIcon from "../components/StudioIcon.vue";
import EmptyState from "../components/ui/EmptyState.vue";
import IconButton from "../components/ui/IconButton.vue";
import SearchField from "../components/ui/SearchField.vue";
import SegmentedControl from "../components/ui/SegmentedControl.vue";
import { isChronicleEntry } from "../chronicleModel.ts";
import { studioState } from "../state.ts";
import { hasTraitKey } from "../entityModel.ts";
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

const locationOptions = computed(() => studioState.entityTemplates.filter((entity) => hasTraitKey(entity, studioState.traitTemplates, "location")));
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
  <div class="codex">
    <!-- Master list -->
    <aside class="codex__list" aria-label="Events">
      <div class="codex__list-head">
        <SegmentedControl
          v-if="CODEX_CATEGORIES.length > 1"
          class="codex__categories"
          label="Rule categories"
          :model-value="activeCategory"
          :options="CODEX_CATEGORIES.map((cat) => ({ value: cat.id, label: cat.label }))"
          @update:model-value="activeCategory = $event; editingItem = null"
        />

        <div class="codex__filter">
          <SearchField v-model="searchQuery" class="codex__search" label="Filter events" placeholder="Filter…" />
          <button type="button" class="btn btn-secondary codex__new" title="New event" aria-label="New event" @click="createNew"><StudioIcon name="plus" /></button>
        </div>
      </div>

      <div class="codex__items">
        <p v-if="!currentList.length" class="codex__none">No events yet.</p>
        <div
          v-for="item in currentList"
          :key="item.id"
          class="codex__item ui-reveal-host"
          :class="{ 'is-selected': item.id === selectedId }"
          tabindex="0"
          :aria-current="item.id === selectedId ? 'true' : undefined"
          @click="selectItem(item)"
          @keydown.enter.self="selectItem(item)"
        >
          <div class="codex__item-text">
            <div class="codex__item-title">{{ itemLabel(item) }}</div>
            <div class="codex__item-detail">{{ itemDetail(item) }}</div>
          </div>
          <IconButton icon="trash" :label="'Delete ' + itemLabel(item)" tone="danger" reveal @click.stop="deleteItem(activeCategory, item.id)" @keydown.enter.stop />
        </div>
      </div>
    </aside>

    <!-- Detail edit canvas -->
    <section class="codex__detail">
      <EmptyState v-if="!editingItem" icon="scroll" title="No event selected" hint="Select an event from the list or create a new one.">
        <button class="btn btn-primary" type="button" @click="createNew"><StudioIcon name="plus" /> New event</button>
      </EmptyState>

      <div v-else class="codex__form">
        <div class="codex__form-head">
          <h2>Edit event</h2>
          <button type="button" class="btn btn-primary" @click="saveEditingItem(activeCategory, editingItem)">Save record</button>
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
          <div class="codex__pair">
            <div class="form-group">
              <label class="form-label" for="event-delay">Delay (turns)</label>
              <input id="event-delay" v-model.number="editingItem.delay_turns" type="number" min="0" class="form-input" />
            </div>
            <label class="form-group codex__check" for="event-repeatable">
              <input id="event-repeatable" v-model="editingItem.repeatable" type="checkbox" />
              <span>Repeatable</span>
            </label>
          </div>
          <template v-if="isCanon">
            <fieldset class="canon-fields">
              <legend>Chronicle entry</legend>
              <div class="codex__pair codex__pair--wide">
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
    </section>
  </div>
</template>

<style scoped>
.codex { display: flex; height: 100%; min-height: 0; }
.codex__list { flex: 0 0 300px; min-width: 0; display: flex; flex-direction: column; border-right: 1px solid var(--border-faint); }
.codex__list-head { display: flex; flex-direction: column; gap: 10px; padding: 14px; border-bottom: 1px solid var(--border-faint); }
.codex__categories { align-self: flex-start; flex-wrap: wrap; }
.codex__filter { display: flex; gap: 8px; }
.codex__search { flex: 1; }
.codex__new { width: 34px; min-height: 34px; padding: 0; font-size: 15px; }
.codex__items { flex: 1; overflow-y: auto; padding: 8px; display: flex; flex-direction: column; gap: 2px; }
.codex__none { padding: 16px 10px; color: var(--text-dim); font-size: 12px; }
.codex__item { display: flex; justify-content: space-between; align-items: center; gap: 8px; padding: 8px 8px 8px 12px; border-radius: var(--radius-md); cursor: pointer; transition: background-color 0.12s ease; }
.codex__item:hover { background: var(--bg-hover); }
.codex__item.is-selected { background: rgba(255, 255, 255, 0.08); box-shadow: inset 2px 0 0 var(--accent); }
.codex__item-text { min-width: 0; }
.codex__item-title { font-weight: 600; font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.codex__item-detail { font-size: 11px; color: var(--text-muted); }
.codex__detail { flex: 1; min-width: 0; padding: 24px 28px; overflow-y: auto; }
.codex__form { max-width: 640px; }
.codex__form-head { display: flex; justify-content: space-between; align-items: center; gap: 12px; margin-bottom: 20px; }
.codex__form-head h2 { margin: 0; }
.codex__pair { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; align-items: end; }
.codex__pair--wide { grid-template-columns: 1fr 2fr; }
.codex__check { display: flex; align-items: center; gap: 8px; min-height: 34px; font-size: 13px; font-weight: 500; color: var(--text-main); }
.canon-fields { border: 1px solid var(--border-subtle); border-radius: var(--radius-lg); padding: 12px 16px; margin: 0 0 16px; }
.canon-fields legend { font-size: 11px; letter-spacing: 0.06em; text-transform: uppercase; color: var(--text-muted); padding: 0 4px; }
.canon-text { font-family: var(--font-serif); font-size: 16px; line-height: 24px; }
@container (max-width: 640px) {
  .codex { flex-direction: column; overflow-y: auto; }
  .codex__list { flex: none; max-height: 40%; border-right: 0; border-bottom: 1px solid var(--border-faint); }
  .codex__detail { overflow: visible; padding: 20px 16px; }
  .codex__pair, .codex__pair--wide { grid-template-columns: 1fr; }
}
</style>
