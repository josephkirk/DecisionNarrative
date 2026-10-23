<script setup lang="ts">
import { computed } from "vue";
import type { CharacterSection, MicroPassionRecord, MacroPassionThresholdRecord, ZeroSumPassionLinkRecord } from "../contracts.ts";
import { studioState } from "../state.ts";
import TagEditor from "./TagEditor.vue";

const props = withDefaults(defineProps<{ modelValue: CharacterSection; entityId?: string | null }>(), {
  entityId: null,
});
const emit = defineEmits<{ "update:modelValue": [value: CharacterSection] }>();

const section = computed(() => props.modelValue);
const passions = computed(() => section.value.passion_pool ?? []);
const thresholds = computed(() => section.value.thresholds ?? []);
const links = computed(() => section.value.zero_sum_passion_links ?? []);
const activePassionOptions = computed(() =>
  passions.value.map((passion) => String(passion.passion_id ?? "")).filter(Boolean),
);
const rivalCharacters = computed(() =>
  studioState.characters.filter((character) => character.id !== props.entityId),
);

function patch(changes: Partial<CharacterSection>) {
  emit("update:modelValue", { ...section.value, ...changes });
}

function patchPassion(index: number, changes: Partial<MicroPassionRecord>) {
  patch({
    passion_pool: passions.value.map((passion, position) =>
      position === index ? { ...passion, ...changes } : passion,
    ),
  });
}

function patchThreshold(index: number, changes: Partial<MacroPassionThresholdRecord>) {
  patch({
    thresholds: thresholds.value.map((threshold, position) =>
      position === index ? { ...threshold, ...changes } : threshold,
    ),
  });
}

function patchLink(index: number, changes: Partial<ZeroSumPassionLinkRecord>) {
  patch({
    zero_sum_passion_links: links.value.map((link, position) =>
      position === index ? { ...link, ...changes } : link,
    ),
  });
}

function addPassion() {
  patch({
    passion_pool: [...passions.value, { passion_id: "", current_value: 0, weight: 1, is_hidden: false }],
  });
}

function removePassion(index: number) {
  const next = passions.value.filter((_passion, position) => position !== index);
  const ids = new Set(next.map((passion) => passion.passion_id));
  patch({
    passion_pool: next,
    active_passions: (section.value.active_passions ?? []).filter((id) => ids.has(id)),
  });
}

function addThreshold() {
  patch({
    thresholds: [
      ...thresholds.value,
      {
        threshold_id: `threshold_${crypto.randomUUID()}`,
        trigger_value: 0,
        direction: "positive",
        linked_graph_asset_id: "",
      },
    ],
  });
}

function addLink() {
  patch({
    zero_sum_passion_links: [
      ...links.value,
      { target_character_id: "", source_passion_id: "", target_passion_id: "", weight: -1 },
    ],
  });
}
</script>

<template>
  <div class="settings-section">
    <div class="form-group">
      <label class="form-label">Role Tag</label>
      <TagEditor
        :model-value="section.role"
        :multiple="false"
        label="Character role tag"
        placeholder="Choose or create a role tag"
        @update:model-value="(value) => patch({ role: Array.isArray(value) ? value[0] ?? '' : value })"
      />
    </div>
    <div class="form-group">
      <label class="form-label">Faction ID</label>
      <input
        class="form-input"
        placeholder="Optional; used for drone disposition groups"
        :value="section.faction_id ?? ''"
        @input="patch({ faction_id: ($event.target as HTMLInputElement).value })"
      />
    </div>
    <div class="form-group">
      <label class="form-label">Home Location</label>
      <select
        class="form-select"
        :value="section.home_location"
        @change="patch({ home_location: ($event.target as HTMLSelectElement).value })"
      >
        <option value="">Choose a location</option>
        <option v-for="location in studioState.locations" :key="location.id" :value="location.id">{{ location.name }}</option>
      </select>
    </div>
    <div class="form-group">
      <label class="form-label">Current Location</label>
      <select
        class="form-select"
        :value="section.current_location"
        @change="patch({ current_location: ($event.target as HTMLSelectElement).value })"
      >
        <option value="">Use Home Location</option>
        <option v-for="location in studioState.locations" :key="location.id" :value="location.id">{{ location.name }}</option>
      </select>
    </div>
    <section class="settings-block">
      <h3 class="form-label">Micro-Passions</h3>
      <div class="form-group">
        <label class="form-label">Trust Inertia</label>
        <input
          type="number"
          min="-100"
          max="100"
          step="any"
          class="form-input"
          :value="section.trust_inertia ?? 0"
          @input="patch({ trust_inertia: Number(($event.target as HTMLInputElement).value) })"
        />
      </div>
      <div
        v-for="(passion, index) in passions"
        :key="index"
        style="display:grid; grid-template-columns:1.3fr 1fr 1fr auto; gap:8px; align-items:end; margin-bottom:8px;"
      >
        <label class="form-group">
          <span class="form-label">Passion ID</span>
          <input class="form-input" :value="passion.passion_id" @input="patchPassion(Number(index), { passion_id: ($event.target as HTMLInputElement).value })" />
        </label>
        <label class="form-group">
          <span class="form-label">Value</span>
          <input
            type="number"
            min="-100"
            max="100"
            step="any"
            class="form-input"
            :value="passion.current_value"
            @input="patchPassion(Number(index), { current_value: Number(($event.target as HTMLInputElement).value) })"
          />
        </label>
        <label class="form-group">
          <span class="form-label">Weight</span>
          <input
            type="number"
            min="0"
            step="any"
            class="form-input"
            :value="passion.weight"
            @input="patchPassion(Number(index), { weight: Number(($event.target as HTMLInputElement).value) })"
          />
        </label>
        <div style="display:flex; align-items:center; gap:6px;">
          <input
            type="checkbox"
            :checked="passion.is_hidden"
            :aria-label="`Hide ${passion.passion_id || 'passion'}`"
            @change="patchPassion(Number(index), { is_hidden: ($event.target as HTMLInputElement).checked })"
          />
          <button class="btn btn-danger" type="button" @click="removePassion(Number(index))">Remove</button>
        </div>
      </div>
      <button class="btn btn-secondary" type="button" @click="addPassion">Add Micro-Passion</button>
      <div class="form-group" style="margin-top:12px;">
        <label class="form-label">Active Passions</label>
        <TagEditor
          :model-value="section.active_passions ?? []"
          :options="activePassionOptions"
          label="Active passions"
          placeholder="Choose active passions"
          @update:model-value="(value) => patch({ active_passions: Array.isArray(value) ? value : [value] })"
        />
      </div>
    </section>

    <section class="settings-block">
      <h3 class="form-label">Macro-Passion Thresholds</h3>
      <div
        v-for="(threshold, index) in thresholds"
        :key="index"
        style="display:grid; grid-template-columns:1fr 1fr 1fr 1.5fr auto; gap:8px; align-items:end; margin-bottom:8px;"
      >
        <label class="form-group">
          <span class="form-label">ID</span>
          <input class="form-input" :value="threshold.threshold_id" @input="patchThreshold(Number(index), { threshold_id: ($event.target as HTMLInputElement).value })" />
        </label>
        <label class="form-group">
          <span class="form-label">Threshold</span>
          <input
            type="number"
            min="-100"
            max="100"
            step="any"
            class="form-input"
            :value="threshold.trigger_value"
            @input="patchThreshold(Number(index), { trigger_value: Number(($event.target as HTMLInputElement).value) })"
          />
        </label>
        <label class="form-group">
          <span class="form-label">Direction</span>
          <select class="form-select" :value="threshold.direction" @change="patchThreshold(Number(index), { direction: ($event.target as HTMLSelectElement).value as 'positive' | 'negative' })">
            <option value="positive">Positive</option>
            <option value="negative">Negative</option>
          </select>
        </label>
        <label class="form-group">
          <span class="form-label">Linked Storylet</span>
          <select class="form-select" :value="threshold.linked_graph_asset_id" @change="patchThreshold(Number(index), { linked_graph_asset_id: ($event.target as HTMLSelectElement).value })">
            <option value="">Choose a crisis storylet</option>
            <option v-for="storylet in studioState.storylets" :key="storylet.id" :value="storylet.id">{{ storylet.title }} ({{ storylet.id }})</option>
          </select>
        </label>
        <button class="btn btn-danger" type="button" @click="patch({ thresholds: thresholds.filter((_item, position) => position !== Number(index)) })">Remove</button>
      </div>
      <button class="btn btn-secondary" type="button" @click="addThreshold">Add Threshold</button>
    </section>

    <section class="settings-block">
      <h3 class="form-label">Zero-Sum Passion Links</h3>
      <div
        v-for="(link, index) in links"
        :key="index"
        style="display:grid; grid-template-columns:1.2fr 1fr 1fr 1fr auto; gap:8px; align-items:end; margin-bottom:8px;"
      >
        <label class="form-group">
          <span class="form-label">Rival Character</span>
          <select class="form-select" :value="link.target_character_id" @change="patchLink(Number(index), { target_character_id: ($event.target as HTMLSelectElement).value })">
            <option value="">Choose character</option>
            <option v-for="character in rivalCharacters" :key="character.id" :value="character.id">{{ character.name }}</option>
          </select>
        </label>
        <label class="form-group">
          <span class="form-label">Source Passion</span>
          <select class="form-select" :value="link.source_passion_id" @change="patchLink(Number(index), { source_passion_id: ($event.target as HTMLSelectElement).value })">
            <option value="">Choose passion</option>
            <option v-for="passion in passions" :key="passion.passion_id" :value="passion.passion_id">{{ passion.passion_id }}</option>
          </select>
        </label>
        <label class="form-group">
          <span class="form-label">Rival Passion</span>
          <select class="form-select" :value="link.target_passion_id" @change="patchLink(Number(index), { target_passion_id: ($event.target as HTMLSelectElement).value })">
            <option value="">Choose rival passion</option>
            <option
              v-for="passion in (studioState.characters.find((item) => item.id === link.target_character_id)?.passion_pool ?? [])"
              :key="passion.passion_id"
              :value="passion.passion_id"
            >{{ passion.passion_id }}</option>
          </select>
        </label>
        <label class="form-group">
          <span class="form-label">Multiplier</span>
          <input
            type="number"
            step="any"
            class="form-input"
            :value="link.weight"
            @input="patchLink(Number(index), { weight: Number(($event.target as HTMLInputElement).value) })"
          />
        </label>
        <button class="btn btn-danger" type="button" @click="patch({ zero_sum_passion_links: links.filter((_item, position) => position !== Number(index)) })">Remove</button>
      </div>
      <button class="btn btn-secondary" type="button" @click="addLink">Add Rival Link</button>
    </section>
  </div>
</template>
