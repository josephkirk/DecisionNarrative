<script setup lang="ts">
import { inject, onMounted, computed, ref, watch } from "vue";
import { useRoute } from "vue-router";
import TagEditor from "../components/TagEditor.vue";

import { useCodexViewModel } from "../view-models/codex.ts";
import { CODEX_CATEGORIES } from "../definitions.ts";
import { studioState } from "../state.ts";
import { StudioApi } from "../api.ts";
import type { StoryletRecord } from "../contracts.ts";
import { buildTagIndex, uniqueTagNames } from "../tagData.ts";
import { interactionContextKey } from "../interactionContext.ts";

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
const activePassionOptions = computed(() => (editingItem.value?.passion_pool ?? []).map((passion: any) => String(passion.passion_id ?? "")).filter(Boolean));
const interactionContext = inject(interactionContextKey);
const contextEntityTypes: Record<string, string> = {
  qualities: "quality",
  characters: "character",
  locations: "location",
  npc_behaviors: "npc_behavior",
  events: "event",
};
watch(() => [activeCategory.value, selectedId.value, editingItem.value], () => {
  const item = editingItem.value;
  if (!item) {
    interactionContext?.setFocus({ entity: null, selection: null });
    return;
  }
  const id = String(item.id ?? item.name ?? selectedId.value ?? "");
  const label = String(item.name ?? item.title ?? id);
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
  if (activeCategory.value === "characters") {
    editingItem.value.passion_pool ??= [];
    editingItem.value.active_passions ??= [];
    editingItem.value.trust_inertia ??= 0;
    editingItem.value.thresholds ??= [];
    editingItem.value.zero_sum_passion_links ??= [];
  }
}

function saveEditingItem(category: string, item: any) {
  if (category === "characters") item.role = String(item.role ?? "").trim();
  if (category === "locations") item.tags = uniqueTagNames(item.tags ?? []);
  saveItem(category, item);
}

function addPassion() {
  editingItem.value.passion_pool ??= [];
  editingItem.value.passion_pool.push({ passion_id: "", current_value: 0, weight: 1, is_hidden: false });
}

function removePassion(index: number) {
  editingItem.value.passion_pool.splice(index, 1);
  const ids = new Set(editingItem.value.passion_pool.map((passion: any) => passion.passion_id));
  editingItem.value.active_passions = (editingItem.value.active_passions ?? []).filter((id: string) => ids.has(id));
}

function addPassionThreshold() {
  editingItem.value.thresholds ??= [];
  editingItem.value.thresholds.push({ threshold_id: `threshold_${crypto.randomUUID()}`, trigger_value: 0, direction: "positive", linked_graph_asset_id: "" });
}

function addZeroSumPassionLink() {
  editingItem.value.zero_sum_passion_links ??= [];
  editingItem.value.zero_sum_passion_links.push({
    target_character_id: "",
    source_passion_id: "",
    target_passion_id: "",
    weight: -1,
  });
}

function createNew() {
  const cat = activeCategory.value;
  if (cat === "qualities") {
    editingItem.value = {
      name: "new_quality",
      type: "metric",
      value: 0,
      min_value: 0,
      max_value: 100,
      is_hidden: false,
      description: "",
    };
  } else if (cat === "characters") {
    editingItem.value = {
      id: "char_new",
      name: "New Character",
      role: "witness",
      faction_id: "",
      home_location: "",
      current_location: "",
      agenda: "",
      red_line: "",
      passion_pool: [],
      active_passions: [],
      trust_inertia: 0,
      thresholds: [],
      zero_sum_passion_links: [],
    };
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
            <label class="form-label">Category</label>
            <select v-model="editingItem.type" class="form-select">
              <option value="currency">Currency</option>
              <option value="metric">Metric</option>
              <option value="tool">Tool</option>
              <option value="progress">Progress</option>
              <option value="menace">Menace</option>
              <option value="territory">Territory (legacy)</option>
            </select>
          </div>
          <div class="form-group">
            <label class="form-label">Default Value</label>
            <input v-model.number="editingItem.value" type="number" class="form-input" />
          </div>
          <div style="display:grid; grid-template-columns:1fr 1fr; gap:10px;">
            <div class="form-group">
              <label class="form-label">Minimum</label>
              <input v-model.number="editingItem.min_value" type="number" class="form-input" />
            </div>
            <div class="form-group">
              <label class="form-label">Maximum</label>
              <input v-model.number="editingItem.max_value" type="number" class="form-input" />
            </div>
          </div>
          <label class="form-group" style="display:flex; align-items:center; gap:8px;">
            <input v-model="editingItem.is_hidden" type="checkbox" />
            Hide this quality from the player-facing quality list
          </label>
          <div v-if="editingItem.type === 'menace'" class="form-group">
            <label class="form-label">Crisis Storylet ID (optional)</label>
            <input v-model="editingItem.menace_crisis_storylet_id" class="form-input" placeholder="storylet-id" />
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
            <label class="form-label">Faction ID</label>
            <input v-model="editingItem.faction_id" class="form-input" placeholder="Optional; used for drone disposition groups" />
          </div>
          <div class="form-group">
            <label class="form-label">Home Location</label>
            <input v-model="editingItem.home_location" class="form-input" />
          </div>
          <div class="form-group">
            <label class="form-label">Current Location</label>
            <select v-model="editingItem.current_location" class="form-select">
              <option value="">Use Home Location</option>
              <option v-for="location in studioState.locations" :key="location.id" :value="location.id">{{ location.name }}</option>
            </select>
          </div>
          <section class="form-group" style="padding:12px; border:1px solid var(--border-subtle); border-radius:8px;">
            <h3 class="form-label">Micro-Passions</h3>
            <div class="form-group">
              <label class="form-label">Trust Inertia</label>
              <input v-model.number="editingItem.trust_inertia" type="number" min="-100" max="100" step="any" class="form-input" />
            </div>
            <div v-for="(passion, index) in (editingItem.passion_pool || [])" :key="index" style="display:grid; grid-template-columns:1.3fr 1fr 1fr auto; gap:8px; align-items:end; margin-bottom:8px;">
              <label class="form-group">
                <span class="form-label">Passion ID</span>
                <input v-model="passion.passion_id" class="form-input" />
              </label>
              <label class="form-group">
                <span class="form-label">Value</span>
                <input v-model.number="passion.current_value" type="number" min="-100" max="100" step="any" class="form-input" />
              </label>
              <label class="form-group">
                <span class="form-label">Weight</span>
                <input v-model.number="passion.weight" type="number" min="0" step="any" class="form-input" />
              </label>
              <div style="display:flex; align-items:center; gap:6px;">
                <input v-model="passion.is_hidden" type="checkbox" :aria-label="`Hide ${passion.passion_id || 'passion'}`" />
                <button class="btn btn-danger" type="button" @click="removePassion(Number(index))">Remove</button>
              </div>
            </div>
            <button class="btn btn-secondary" type="button" @click="addPassion">Add Micro-Passion</button>
            <div class="form-group" style="margin-top:12px;">
              <label class="form-label">Active Passions</label>
              <TagEditor v-model="editingItem.active_passions" :options="activePassionOptions" label="Active passions" placeholder="Choose active passions" />
            </div>
          </section>
          <section class="form-group" style="padding:12px; border:1px solid var(--border-subtle); border-radius:8px;">
            <h3 class="form-label">Macro-Passion Thresholds</h3>
            <div v-for="(threshold, index) in (editingItem.thresholds || [])" :key="index" style="display:grid; grid-template-columns:1fr 1fr 1fr 1.5fr auto; gap:8px; align-items:end; margin-bottom:8px;">
              <label class="form-group">
                <span class="form-label">ID</span>
                <input v-model="threshold.threshold_id" class="form-input" />
              </label>
              <label class="form-group">
                <span class="form-label">Threshold</span>
                <input v-model.number="threshold.trigger_value" type="number" min="-100" max="100" step="any" class="form-input" />
              </label>
              <label class="form-group">
                <span class="form-label">Direction</span>
                <select v-model="threshold.direction" class="form-select">
                  <option value="positive">Positive</option>
                  <option value="negative">Negative</option>
                </select>
              </label>
              <label class="form-group">
                <span class="form-label">Linked Storylet</span>
                <select v-model="threshold.linked_graph_asset_id" class="form-select">
                  <option value="">Choose a crisis storylet</option>
                  <option v-for="storylet in availableStorylets" :key="storylet.id" :value="storylet.id">{{ storylet.title }} ({{ storylet.id }})</option>
                </select>
              </label>
              <button class="btn btn-danger" type="button" @click="editingItem.thresholds.splice(index, 1)">Remove</button>
            </div>
            <button class="btn btn-secondary" type="button" @click="addPassionThreshold">Add Threshold</button>
          </section>
          <section class="form-group" style="padding:12px; border:1px solid var(--border-subtle); border-radius:8px;">
            <h3 class="form-label">Zero-Sum Passion Links</h3>
            <div v-for="(link, index) in (editingItem.zero_sum_passion_links || [])" :key="index" style="display:grid; grid-template-columns:1.2fr 1fr 1fr 1fr auto; gap:8px; align-items:end; margin-bottom:8px;">
              <label class="form-group">
                <span class="form-label">Rival Character</span>
                <select v-model="link.target_character_id" class="form-select">
                  <option value="">Choose character</option>
                  <option v-for="character in studioState.characters.filter((item) => item.id !== editingItem.id)" :key="character.id" :value="character.id">{{ character.name }}</option>
                </select>
              </label>
              <label class="form-group">
                <span class="form-label">Source Passion</span>
                <select v-model="link.source_passion_id" class="form-select">
                  <option value="">Choose passion</option>
                  <option v-for="passion in (editingItem.passion_pool || [])" :key="passion.passion_id" :value="passion.passion_id">{{ passion.passion_id }}</option>
                </select>
              </label>
              <label class="form-group">
                <span class="form-label">Rival Passion</span>
                <select v-model="link.target_passion_id" class="form-select">
                  <option value="">Choose rival passion</option>
                  <option v-for="passion in (studioState.characters.find((item) => item.id === link.target_character_id)?.passion_pool ?? [])" :key="passion.passion_id" :value="passion.passion_id">{{ passion.passion_id }}</option>
                </select>
              </label>
              <label class="form-group">
                <span class="form-label">Multiplier</span>
                <input v-model.number="link.weight" type="number" step="any" class="form-input" />
              </label>
              <button class="btn btn-danger" type="button" @click="editingItem.zero_sum_passion_links.splice(index, 1)">Remove</button>
            </div>
            <button class="btn btn-secondary" type="button" @click="addZeroSumPassionLink">Add Rival Link</button>
          </section>
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
