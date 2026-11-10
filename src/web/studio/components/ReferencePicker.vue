<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { thumbnailUrl, thumbnailInitial } from "../thumbnails.ts";
import StudioIcon from "./StudioIcon.vue";

export interface PickerItem {
  id: string;
  title: string;
  subtitle?: string;
  thumbnail?: string | null;
}

const props = defineProps<{
  open: boolean;
  title: string;
  items: PickerItem[];
  selectedId: string | null;
}>();
const emit = defineEmits<{
  (e: "select", id: string): void;
  (e: "close"): void;
}>();

const query = ref("");
const viewMode = ref<"list" | "grid">("list");
const searchInput = ref<HTMLInputElement | null>(null);

const filtered = computed(() => {
  const needle = query.value.trim().toLowerCase();
  if (!needle) return props.items;
  return props.items.filter((item) =>
    item.title.toLowerCase().includes(needle) ||
    (item.subtitle ?? "").toLowerCase().includes(needle) ||
    item.id.toLowerCase().includes(needle),
  );
});

function onKeyDown(event: KeyboardEvent) {
  if (event.key === "Escape") emit("close");
}

watch(
  () => props.open,
  (isOpen) => {
    if (isOpen) {
      query.value = "";
      document.addEventListener("keydown", onKeyDown);
      nextTick(() => searchInput.value?.focus());
    } else {
      document.removeEventListener("keydown", onKeyDown);
    }
  },
);
onBeforeUnmount(() => document.removeEventListener("keydown", onKeyDown));

function choose(id: string) {
  emit("select", id);
}
</script>

<template>
  <Teleport to="body"><div v-if="open" class="picker-overlay" @click.self="$emit('close')">
    <div
      class="picker-dialog"
      role="dialog"
      aria-modal="true"
      :aria-label="title"
      @click.stop
    >
      <div class="picker-header">
        <span class="picker-title">{{ title }}</span>
        <div class="picker-tools">
          <button
            type="button"
            class="picker-view-btn"
            :class="{ 'is-active': viewMode === 'list' }"
            title="List view"
            aria-label="List view"
            @click="viewMode = 'list'"
          >
            <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="M1.5 2.5h9M1.5 6h9M1.5 9.5h9"/></svg>
          </button>
          <button
            type="button"
            class="picker-view-btn"
            :class="{ 'is-active': viewMode === 'grid' }"
            title="Grid view"
            aria-label="Grid view"
            @click="viewMode = 'grid'"
          >
            <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><rect x="1.5" y="1.5" width="3.5" height="3.5" rx="0.5"/><rect x="7" y="1.5" width="3.5" height="3.5" rx="0.5"/><rect x="1.5" y="7" width="3.5" height="3.5" rx="0.5"/><rect x="7" y="7" width="3.5" height="3.5" rx="0.5"/></svg>
          </button>
          <button
            type="button"
            class="picker-close-btn"
            title="Close"
            aria-label="Close picker"
            @click="$emit('close')"
          ><StudioIcon name="close" /></button>
        </div>
      </div>

      <div class="picker-search">
        <input
          ref="searchInput"
          v-model="query"
          class="form-input"
          type="text"
          placeholder="Search…"
          aria-label="Search"
        />
      </div>

      <div v-if="!filtered.length" class="picker-empty">
        No matches found.
      </div>
      <div v-else-if="viewMode === 'list'" class="picker-list" role="listbox" aria-label="Results">
        <button
          v-for="item in filtered"
          :key="item.id"
          type="button"
          class="picker-row"
          :class="{ 'is-selected': item.id === selectedId }"
          role="option"
          :aria-selected="item.id === selectedId"
          :title="item.title"
          @click="choose(item.id)"
        >
          <span class="picker-thumb" aria-hidden="true">
            <img v-if="thumbnailUrl(item.thumbnail)" :src="thumbnailUrl(item.thumbnail) ?? ''" :alt="''" />
            <span v-else>{{ thumbnailInitial(item.title) }}</span>
          </span>
          <span class="picker-text">
            <span class="picker-name">{{ item.title }}</span>
            <span v-if="item.subtitle" class="picker-sub">{{ item.subtitle }}</span>
          </span>
          <span v-if="item.id === selectedId" class="picker-check" aria-hidden="true">✓</span>
        </button>
      </div>
      <div v-else class="picker-grid" role="listbox" aria-label="Results">
        <button
          v-for="item in filtered"
          :key="item.id"
          type="button"
          class="picker-tile"
          :class="{ 'is-selected': item.id === selectedId }"
          role="option"
          :aria-selected="item.id === selectedId"
          :title="item.subtitle ? `${item.title} — ${item.subtitle}` : item.title"
          @click="choose(item.id)"
        >
          <span class="picker-tile__thumb" aria-hidden="true">
            <img v-if="thumbnailUrl(item.thumbnail)" :src="thumbnailUrl(item.thumbnail) ?? ''" :alt="''" />
            <span v-else>{{ thumbnailInitial(item.title) }}</span>
          </span>
          <span class="picker-tile__name">{{ item.title }}</span>
          <span v-if="item.id === selectedId" class="picker-tile__check" aria-hidden="true">✓</span>
        </button>
      </div>
    </div>
  </div></Teleport>
</template>

<style scoped>
.picker-overlay {
  position: fixed;
  inset: 0;
  z-index: 200;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  background: rgba(10, 12, 16, 0.72);
}
.picker-dialog {
  width: min(560px, 100%);
  max-height: min(520px, 86vh);
  display: flex;
  flex-direction: column;
  background: var(--bg-card);
  border: 1px solid var(--border-muted);
  border-radius: var(--radius-lg, 10px);
  box-shadow: var(--shadow-lg);
  overflow: hidden;
}
.picker-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 14px;
  border-bottom: 1px solid var(--border-subtle);
}
.picker-title {
  font-size: 13px;
  font-weight: 700;
  color: var(--text-main);
}
.picker-tools {
  display: flex;
  align-items: center;
  gap: 4px;
}
.picker-view-btn,
.picker-close-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: 1px solid transparent;
  border-radius: var(--radius-xs, 3px);
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
  transition: background-color 0.12s ease-out, color 0.12s ease-out, border-color 0.12s ease-out;
}
.picker-view-btn svg {
  width: 14px;
  height: 14px;
}
.picker-view-btn:hover,
.picker-close-btn:hover {
  background: var(--bg-badge);
  color: var(--text-main);
}
.picker-view-btn.is-active {
  border-color: var(--accent);
  color: var(--accent);
  background: var(--accent-subtle);
}
.picker-search {
  padding: 10px 14px 4px;
}
.picker-empty {
  padding: 28px 14px;
  text-align: center;
  color: var(--text-dim);
  font-size: 12px;
}
.picker-list {
  overflow-y: auto;
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.picker-row {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 6px 8px;
  border: 1px solid transparent;
  border-radius: var(--radius-sm, 4px);
  background: transparent;
  color: var(--text-main);
  cursor: pointer;
  text-align: left;
  transition: background-color 0.12s ease-out, border-color 0.12s ease-out;
}
.picker-row:hover {
  background: var(--bg-card-hover);
}
.picker-row.is-selected {
  border-color: var(--accent);
  background: var(--accent-subtle);
}
.picker-thumb {
  flex: none;
  width: 32px;
  height: 32px;
  border-radius: var(--radius-sm, 4px);
  border: 1px solid var(--border-subtle);
  background: var(--bg-input);
  color: var(--text-muted);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
  font-weight: 700;
  overflow: hidden;
}
.picker-thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
  aspect-ratio: 1 / 1;
}
.picker-text {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 1px;
}
.picker-name {
  font-size: 13px;
  font-weight: 600;
  overflow-wrap: anywhere;
}
.picker-sub {
  font-size: 11px;
  font-family: var(--font-mono);
  color: var(--text-dim);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.picker-check {
  flex: none;
  color: var(--accent);
  font-size: 14px;
  font-weight: 700;
}
.picker-grid {
  overflow-y: auto;
  padding: 12px 14px 14px;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(96px, 1fr));
  gap: 8px;
}
.picker-tile {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 10px 6px 8px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md, 8px);
  background: var(--bg-input);
  color: var(--text-main);
  cursor: pointer;
  min-width: 0;
  transition: border-color 0.12s ease-out, background-color 0.12s ease-out;
}
.picker-tile:hover {
  border-color: var(--border-muted);
  background: var(--bg-card-hover);
}
.picker-tile.is-selected {
  border-color: var(--accent);
  background: var(--accent-subtle);
}
.picker-tile__thumb {
  width: 44px;
  height: 44px;
  border-radius: var(--radius-sm, 4px);
  border: 1px solid var(--border-subtle);
  background: var(--bg-badge);
  color: var(--text-muted);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 18px;
  font-weight: 700;
  overflow: hidden;
  aspect-ratio: 1 / 1;
}
.picker-tile__thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}
.picker-tile__name {
  font-size: 11px;
  font-weight: 600;
  text-align: center;
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow-wrap: anywhere;
}
.picker-tile__check {
  position: absolute;
  top: 4px;
  right: 6px;
  color: var(--accent);
  font-size: 12px;
  font-weight: 700;
}
</style>
