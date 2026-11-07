<script setup lang="ts">
import { computed, ref } from "vue";
import { thumbnailUrl } from "../../thumbnails.ts";
import { studioState } from "../../state.ts";
import type { AnyEntity } from "../runtime.ts";
import { INVENTORY_OWNER_KEY, ITEM_KEY, MAX_GRID_DIMENSION, inventoryErrors, inventoryOf, setInventory, traitInstance } from "./model.ts";

const props = defineProps<{ entity: AnyEntity; readonly?: boolean }>();
const emit = defineEmits<{ (event: "patch", patch: Partial<AnyEntity>): void }>();
const selected = ref("");
const error = ref("");
const owner = computed(() => traitInstance(props.entity, studioState.traitTemplates, INVENTORY_OWNER_KEY));
const inventory = computed(() => owner.value ? inventoryOf(owner.value) : null);
const entities = computed(() => [...studioState.entities, ...studioState.entityTemplates]);
const itemEntities = computed(() => entities.value.filter((entity) => !!traitInstance(entity, studioState.traitTemplates, ITEM_KEY)));
const available = computed(() => itemEntities.value.filter((entity) => !inventory.value?.items.includes(entity.id)));
const byId = computed(() => new Map(entities.value.map((entity) => [entity.id, entity])));
const capacity = computed(() => inventory.value ? inventory.value.rows * inventory.value.columns : 0);

function resize(axis: "rows" | "columns", event: Event) {
  const input = event.target as HTMLInputElement;
  if (!inventory.value || !owner.value || props.readonly) return;
  const next = { ...inventory.value, [axis]: input.valueAsNumber };
  const issues = inventoryErrors(next);
  if (issues.length) { error.value = issues.join(" "); input.value = String(inventory.value[axis]); return; }
  error.value = "";
  emit("patch", setInventory(props.entity, owner.value, next));
}
function add() {
  if (!inventory.value || !owner.value || props.readonly) return;
  const item = available.value.find((entity) => entity.id === selected.value);
  if (!item) { error.value = "Choose an entity with the Item trait."; return; }
  if (inventory.value.items.length >= capacity.value) { error.value = "Inventory is full. Increase the grid size first."; return; }
  emit("patch", setInventory(props.entity, owner.value, { ...inventory.value, items: [...inventory.value.items, item.id] }));
  selected.value = ""; error.value = "";
}
function remove(index: number) {
  if (!inventory.value || !owner.value || props.readonly) return;
  emit("patch", setInventory(props.entity, owner.value, { ...inventory.value, items: inventory.value.items.filter((_, i) => i !== index) }));
  error.value = "";
}
</script>

<template>
  <section v-if="inventory" class="inventory-panel" aria-label="Inventory">
    <div class="inventory-toolbar">
      <div>
        <h2>Inventory</h2>
        <p class="field-sublabel">{{ inventory.items.length }} / {{ capacity }} slots · Save the entity to keep changes.</p>
      </div>
      <div class="inventory-dimensions">
        <label>Columns <input class="form-input" type="number" min="1" :max="MAX_GRID_DIMENSION" :value="inventory.columns" :disabled="readonly" @change="resize('columns', $event)" /></label>
        <label>Rows <input class="form-input" type="number" min="1" :max="MAX_GRID_DIMENSION" :value="inventory.rows" :disabled="readonly" @change="resize('rows', $event)" /></label>
      </div>
    </div>
    <div v-if="!readonly" class="inventory-add">
      <label for="inventory-item">Item</label>
      <select id="inventory-item" v-model="selected" class="form-select">
        <option value="">Choose an Item entity…</option>
        <option v-for="item in available" :key="item.id" :value="item.id">{{ item.name }} ({{ item.id }})</option>
      </select>
      <button type="button" class="btn btn-primary" :disabled="!selected || inventory.items.length >= capacity" @click="add">Add item</button>
    </div>
    <p v-if="error" class="inventory-error" role="alert">{{ error }}</p>
    <div class="inventory-scroll">
      <ol class="inventory-grid" :style="{ gridTemplateColumns: `repeat(${inventory.columns}, minmax(90px, 1fr))` }" aria-label="Inventory slots">
        <li v-for="slot in capacity" :key="slot" class="inventory-slot">
          <template v-if="inventory.items[slot - 1]">
            <img v-if="thumbnailUrl(byId.get(inventory.items[slot - 1]!)?.thumbnail)" :src="thumbnailUrl(byId.get(inventory.items[slot - 1]!)?.thumbnail) ?? ''" alt="" width="40" height="40" />
            <span class="inventory-name">{{ byId.get(inventory.items[slot - 1]!)?.name ?? inventory.items[slot - 1] }}</span>
            <button v-if="!readonly" type="button" class="btn btn-secondary" :aria-label="`Remove ${byId.get(inventory.items[slot - 1]!)?.name ?? inventory.items[slot - 1]}`" @click="remove(slot - 1)">Remove</button>
          </template>
          <span v-else class="inventory-empty">Empty</span>
        </li>
      </ol>
    </div>
  </section>
</template>

<style scoped>
.inventory-panel { display: grid; gap: 1rem; }
.inventory-toolbar, .inventory-dimensions, .inventory-add { display: flex; align-items: end; gap: 0.75rem; flex-wrap: wrap; }
.inventory-toolbar { justify-content: space-between; }
.inventory-toolbar h2 { margin: 0 0 0.25rem; }
.inventory-dimensions label { display: grid; gap: 0.25rem; }
.inventory-dimensions input { width: 5rem; }
.inventory-add select { max-width: 24rem; }
.inventory-error { color: var(--status-danger); margin: 0; }
.inventory-scroll { overflow-x: auto; }
.inventory-grid { display: grid; gap: 0.35rem; list-style: none; margin: 0; padding: 0; min-width: min-content; }
.inventory-slot { min-height: 100px; min-width: 90px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 0.35rem; padding: 0.5rem; border: 1px solid var(--border-subtle); background: var(--bg-card); }
.inventory-slot img { object-fit: contain; }
.inventory-name { max-width: 100%; overflow-wrap: anywhere; text-align: center; }
.inventory-empty { color: var(--text-muted); }
</style>
