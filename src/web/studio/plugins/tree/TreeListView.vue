<script setup lang="ts">
import { computed, ref, shallowRef, watch } from "vue";
import { useRoute } from "vue-router";
import PropertyValueInput from "../../components/PropertyValueInput.vue";
import type { PropertyValue } from "../../contracts.ts";
import { studioState } from "../../state.ts";
import { thumbnailUrl } from "../../thumbnails.ts";
import type { AnyEntity } from "../runtime.ts";
import { TREE_ITEM_KEY, type TreeGraph } from "./model.ts";
import { treeListRows } from "./listModel.ts";
import { setTreeColumnValue, treeColumnValue, treeItemColumns, type TreeListColumn } from "./listProperties.ts";
import { discardFailedTreeItem, editTreeItem, flushTreeItem, itemEditState, retryTreeItem, treeCellDrafts, treeCellErrors, treeItemEdits } from "./listEdits.ts";

const props = defineProps<{ graph: TreeGraph; owner: AnyEntity; readonly?: boolean }>();
const emit = defineEmits<{ (event: "patch", patch: Partial<AnyEntity>): void }>();
const route = useRoute();
const ownerKind = computed(() => route.name === "entity" ? "entities" : "entity_templates");
type ItemKind = "entities" | "entity_templates";
type Item = { record: AnyEntity; kind: ItemKind };
const ownerDraft = shallowRef<AnyEntity>(props.owner);
watch(() => props.owner, (owner) => { ownerDraft.value = owner; });
const collapsed = ref(new Set<string>());
const drafts = treeCellDrafts;
const errors = treeCellErrors;
const failedItems = computed(() => [...treeItemEdits.values()].filter((state) => !!state.error));
const invalidCount = computed(() => Object.keys(errors.value).length);
function discardInvalidInputs(): void {
  for (const key of Object.keys(errors.value)) {
    delete errors.value[key];
    delete drafts.value[key];
  }
}
function retryFailedItems(): void {
  for (const state of failedItems.value) retryTreeItem(state.kind, state.id);
}
function discardFailedItems(): void {
  for (const state of failedItems.value) discardFailedTreeItem(state.kind, state.id);
}
const itemTemplate = computed(() => studioState.traitTemplates.find((template) => template.key === TREE_ITEM_KEY));
const columns = computed(() => treeItemColumns(itemTemplate.value));
const items = computed(() => {
  const result = new Map<string, Item>();
  for (const record of studioState.entities) result.set(record.id, { record, kind: "entities" });
  for (const record of studioState.entityTemplates) result.set(record.id, { record, kind: "entity_templates" });
  const owner = ownerDraft.value;
  const existing = result.get(owner.id);
  if (!existing || existing.kind === ownerKind.value) result.set(owner.id, { record: owner, kind: ownerKind.value });
  return result;
});
const rows = computed(() => treeListRows(props.graph, collapsed.value));
const nameOf = (id: string) => items.value.get(id)?.record.name || id;
const draftKey = (id: string, column: TreeListColumn) => JSON.stringify([id, column.traitTemplateId, column.id]);
function isOwner(item: Item): boolean { return item.record.id === props.owner.id && item.kind === ownerKind.value; }
function hasItemTrait(item: Item): boolean {
  return !!itemTemplate.value && item.record.traits?.some((trait) => trait.template_id === itemTemplate.value!.id);
}
function cellValue(item: Item, column: TreeListColumn): PropertyValue {
  if (!isOwner(item)) {
    const state = itemEditState(item.kind, item.record.id);
    if (state && Object.prototype.hasOwnProperty.call(state.values, column.id)) return state.values[column.id]!;
  }
  return treeColumnValue(item.record, column);
}
function updateValue(item: Item, column: TreeListColumn, value: PropertyValue): void {
  if (props.readonly || !hasItemTrait(item)) return;
  if (isOwner(item)) {
    ownerDraft.value = setTreeColumnValue(ownerDraft.value, column, value);
    emit("patch", { traits: ownerDraft.value.traits });
  } else {
    editTreeItem(item.kind, item.record.id, column, value);
  }
}
function commitCell(item: Item): void {
  if (!isOwner(item)) flushTreeItem(item.kind, item.record.id);
}
function focusOut(event: FocusEvent, item: Item): void {
  if (event.relatedTarget instanceof Node && (event.currentTarget as Element).contains(event.relatedTarget)) return;
  commitCell(item);
}
function toggle(id: string): void {
  const next = new Set(collapsed.value);
  if (next.has(id)) next.delete(id); else next.add(id);
  collapsed.value = next;
}
function itemRoute(item: Item): string {
  return `/${item.kind === "entities" ? "entities" : "entity-templates"}/${encodeURIComponent(item.record.id)}`;
}
function isUnsigned(column: TreeListColumn): boolean {
  return column.property.type === "int" && !column.property.is_array && (column.property.key === "level" || column.property.key === "cost");
}
function inputUnsigned(item: Item, column: TreeListColumn, event: Event): void {
  const key = draftKey(item.record.id, column);
  const input = event.target as HTMLInputElement;
  const raw = input.value;
  const number = input.valueAsNumber;
  if (!raw || !Number.isSafeInteger(number) || number < 0) {
    drafts.value[key] = raw;
    errors.value[key] = "Enter a non-negative whole number.";
    return;
  }
  delete drafts.value[key];
  delete errors.value[key];
  updateValue(item, column, number);
}
function unsignedValue(item: Item, column: TreeListColumn): string | number {
  const key = draftKey(item.record.id, column);
  return Object.prototype.hasOwnProperty.call(drafts.value, key) ? drafts.value[key]! : Number(cellValue(item, column));
}
function jsonValue(item: Item, column: TreeListColumn): string {
  const key = draftKey(item.record.id, column);
  return Object.prototype.hasOwnProperty.call(drafts.value, key) ? drafts.value[key]! : JSON.stringify(cellValue(item, column));
}
function inputJson(item: Item, column: TreeListColumn, event: Event): void {
  drafts.value[draftKey(item.record.id, column)] = (event.target as HTMLTextAreaElement).value;
}
function commitJson(item: Item, column: TreeListColumn): void {
  const key = draftKey(item.record.id, column);
  if (!Object.prototype.hasOwnProperty.call(drafts.value, key)) return;
  try {
    const parsed: unknown = JSON.parse(drafts.value[key]!);
    const object = (value: unknown) => !!value && typeof value === "object" && !Array.isArray(value);
    if (column.property.is_array ? !Array.isArray(parsed) || !parsed.every(object) : !object(parsed)) throw new Error("Invalid shape");
    delete drafts.value[key];
    delete errors.value[key];
    updateValue(item, column, parsed as PropertyValue);
    commitCell(item);
  } catch {
    errors.value[key] = column.property.is_array ? "Enter an array of JSON objects." : "Enter a JSON object.";
  }
}
</script>

<template>
  <section class="tree-list-view" aria-label="Tree list">
    <p class="tree-list-note">TreeItem fields on other entities save automatically. Owner changes save with the owner. Scroll horizontally for more fields.</p>
    <p v-if="invalidCount" class="tree-list-error" role="alert">Some fields contain invalid values. <button type="button" @click="discardInvalidInputs">Discard invalid input</button></p>
    <p v-if="failedItems.length" class="tree-list-error" role="alert">Some TreeItem changes could not be saved. <button type="button" @click="retryFailedItems">Retry failed saves</button> <button type="button" @click="discardFailedItems">Discard failed edits</button></p>
    <p v-if="!graph.nodes.length" class="tree-list-empty">No items in this tree. Add TreeItems from the canvas.</p>
    <template v-else>
      <p v-if="!itemTemplate" class="tree-list-error" role="alert">The TreeItem trait is unavailable.</p>
      <p v-else-if="!columns.length" class="tree-list-empty">Add properties to the TreeItem trait to edit them here.</p>
      <div class="tree-table-scroll" tabindex="0" aria-label="Tree list table; scroll horizontally for more properties">
        <table>
          <thead>
            <tr>
              <th scope="col" class="tree-name-column">Tree item</th>
              <th scope="col" class="tree-relationship-column">Relationship</th>
              <th v-for="column in columns" :key="column.id" scope="col">{{ column.label }}</th>
              <th scope="col" class="tree-status-column">Status</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="row in rows" :key="row.key" :data-node-id="row.nodeId" :data-depth="row.depth" :class="{ 'is-reference': row.reference }">
              <th scope="row" class="tree-name-column">
                <div class="tree-item-name" :style="{ paddingLeft: `${row.depth * 20 + 8}px` }">
                  <button v-if="row.hasChildren && !row.reference" type="button" class="tree-disclosure" :aria-expanded="!collapsed.has(row.nodeId)" :aria-label="`${collapsed.has(row.nodeId) ? 'Expand' : 'Collapse'} ${nameOf(row.nodeId)}`" @click="toggle(row.nodeId)">{{ collapsed.has(row.nodeId) ? '▸' : '▾' }}</button>
                  <span v-else class="tree-disclosure-spacer" aria-hidden="true"></span>
                  <img v-if="items.get(row.nodeId) && thumbnailUrl(items.get(row.nodeId)!.record.thumbnail)" class="tree-item-thumbnail" :src="thumbnailUrl(items.get(row.nodeId)!.record.thumbnail) ?? ''" width="24" height="24" alt="" />
                  <router-link v-if="items.get(row.nodeId)" :to="itemRoute(items.get(row.nodeId)!)" :title="nameOf(row.nodeId)">{{ nameOf(row.nodeId) }}</router-link>
                  <span v-else class="tree-missing">{{ row.nodeId }} (missing)</span>
                  <span v-if="row.reference" class="tree-reference" title="This item also appears elsewhere in the tree">Reference</span>
                </div>
              </th>
              <td class="tree-relationship-column">
                <template v-if="row.edge"><span class="tree-relationship-name">{{ row.edge.relationship || 'Connection' }}</span><router-link v-if="items.get(row.edge.from)" class="tree-relationship-source" :to="itemRoute(items.get(row.edge.from)!)" :title="`From ${nameOf(row.edge.from)}`">from {{ nameOf(row.edge.from) }}</router-link><span v-else class="tree-relationship-source">from {{ nameOf(row.edge.from) }}</span></template>
                <span v-else class="tree-root-label">Root</span>
              </td>
              <td v-for="column in columns" :key="column.id" :data-property="column.property.key">
                <template v-if="items.get(row.nodeId) && hasItemTrait(items.get(row.nodeId)!)">
                  <div class="tree-cell-editor" role="group" :aria-label="`${column.label} for ${nameOf(row.nodeId)}`" @focusout="focusOut($event, items.get(row.nodeId)!)">
                    <input v-if="isUnsigned(column)" type="number" min="0" step="1" :aria-label="`${column.label} for ${nameOf(row.nodeId)}`" :aria-invalid="!!errors[draftKey(row.nodeId, column)]" :value="unsignedValue(items.get(row.nodeId)!, column)" :disabled="readonly" @input="inputUnsigned(items.get(row.nodeId)!, column, $event)" />
                    <textarea v-else-if="column.property.type === 'json'" rows="1" :aria-label="`${column.label} for ${nameOf(row.nodeId)}`" :aria-invalid="!!errors[draftKey(row.nodeId, column)]" :value="jsonValue(items.get(row.nodeId)!, column)" :disabled="readonly" @input="inputJson(items.get(row.nodeId)!, column, $event)" @change="commitJson(items.get(row.nodeId)!, column)" />
                    <PropertyValueInput v-else :model-value="cellValue(items.get(row.nodeId)!, column)" :type="column.property.type" :is-array="column.property.is_array" :multiline="column.property.multiline" :disabled="readonly" @update:model-value="updateValue(items.get(row.nodeId)!, column, $event)" />
                    <small v-if="errors[draftKey(row.nodeId, column)]" class="tree-cell-error" role="alert">{{ errors[draftKey(row.nodeId, column)] }}</small>
                  </div>
                </template>
                <span v-else class="tree-missing">Unavailable</span>
              </td>
              <td class="tree-status-column">
                <template v-if="items.get(row.nodeId) && !isOwner(items.get(row.nodeId)!) && itemEditState(items.get(row.nodeId)!.kind, row.nodeId)">
                  <span v-if="itemEditState(items.get(row.nodeId)!.kind, row.nodeId)!.error" class="tree-cell-error" role="alert" :title="itemEditState(items.get(row.nodeId)!.kind, row.nodeId)!.error">Not saved <button type="button" @click="retryTreeItem(items.get(row.nodeId)!.kind, row.nodeId)">Retry</button></span>
                  <span v-else>Saving…</span>
                </template>
                <span v-else-if="items.get(row.nodeId) && isOwner(items.get(row.nodeId)!)">Owner draft</span>
                <span v-else-if="items.get(row.nodeId) && !hasItemTrait(items.get(row.nodeId)!)" class="tree-cell-error">Missing TreeItem trait</span>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </template>
  </section>
</template>

<style scoped>
.tree-list-view { min-width: 0; display: grid; gap: 8px; }
.tree-list-note, .tree-list-empty { margin: 0; color: var(--text-muted); font-size: 12px; }
.tree-list-error, .tree-cell-error { color: var(--status-danger); }
.tree-list-error button { color: var(--accent); border: 0; background: transparent; cursor: pointer; text-decoration: underline; }
.tree-table-scroll { max-width: 100%; max-height: min(65vh, 620px); overflow: auto; border: 1px solid var(--border-subtle); background: var(--bg-card); }
table { border-collapse: separate; border-spacing: 0; width: 100%; min-width: max-content; color: var(--text-main); font-size: 13px; }
th, td { box-sizing: border-box; padding: 4px 8px; border-right: 1px solid var(--border-subtle); border-bottom: 1px solid var(--border-subtle); text-align: left; vertical-align: middle; }
thead th { position: sticky; top: 0; z-index: 2; min-width: 150px; background: var(--bg-card-hover, #212735); font-size: 12px; font-weight: 600; }
tbody tr:nth-child(even) { background: var(--bg-card-hover, #212735); }
tbody tr:hover { background: var(--bg-hover, #282a2e); }
tbody th { font-weight: 500; }
.tree-name-column { position: sticky; left: 0; z-index: 1; min-width: 230px; max-width: 420px; background: var(--bg-card); }
tbody tr:nth-child(even) .tree-name-column { background: var(--bg-card-hover, #212735); }
thead .tree-name-column { z-index: 3; background: var(--bg-card-hover, #212735); }
.tree-item-name { display: flex; align-items: center; gap: 6px; min-height: 32px; white-space: nowrap; }
.tree-item-name a { color: var(--text-main); overflow: hidden; text-overflow: ellipsis; }
.tree-item-name a:hover { color: var(--accent); }
.tree-item-thumbnail { width: 24px; height: 24px; object-fit: cover; }
.tree-disclosure, .tree-disclosure-spacer { width: 24px; height: 28px; flex: 0 0 24px; }
.tree-disclosure { border: 0; color: var(--text-main); background: transparent; cursor: pointer; }
.tree-disclosure:focus-visible, .tree-table-scroll:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
.tree-reference, .tree-root-label, .tree-relationship-source, .tree-missing, .tree-status-column { color: var(--text-muted); font-size: 12px; }
.tree-relationship-column { min-width: 160px; }
.tree-relationship-name, .tree-relationship-source { display: block; }
.tree-status-column { min-width: 100px; }
.tree-cell-editor { min-width: 130px; }
.tree-cell-editor > input, .tree-cell-editor > textarea { box-sizing: border-box; width: 100%; min-height: 32px; padding: 5px 8px; border: 1px solid transparent; background: transparent; color: var(--text-main); font: inherit; }
.tree-cell-editor > input:hover, .tree-cell-editor > textarea:hover { border-color: var(--border-subtle); }
.tree-cell-editor > input:focus, .tree-cell-editor > textarea:focus { border-color: var(--accent); outline: 1px solid var(--accent); }
.tree-cell-editor :deep(.form-input), .tree-cell-editor :deep(.form-select), .tree-cell-editor :deep(.num-stepper__input), .tree-cell-editor :deep(.form-textarea) { min-height: 32px; min-width: 0; max-width: 100%; }
.tree-cell-editor :deep(.form-textarea) { height: 36px; min-height: 36px !important; padding: 6px 8px; resize: vertical; }
.tree-cell-editor :deep(.tag-editor) { min-height: 34px; padding: 4px 6px; }
.tree-cell-editor :deep(.form-input) { border-color: transparent; background: transparent; }
.tree-cell-editor :deep(.form-input:hover) { border-color: var(--border-subtle); }
.tree-cell-editor :deep(.form-input:focus) { border-color: var(--accent); outline: 1px solid var(--accent); }
.tree-cell-editor :deep(.property-array) { min-width: 140px; }
.tree-cell-error { display: block; font-size: 11px; }
.tree-status-column button { color: var(--accent); background: transparent; border: 0; cursor: pointer; }
</style>
