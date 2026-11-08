<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";

const props = defineProps<{ modelValue: string; label: string; disabled?: boolean }>();
const emit = defineEmits<{ (event: "update:modelValue", value: string): void }>();
const opened = ref(false);
const trigger = ref<HTMLButtonElement | null>(null);
const popup = ref<HTMLElement | null>(null);
const position = ref({ left: 0, top: 0 });
const hue = ref(0), saturation = ref(100), value = ref(100);
const hex = ref("#000000");

function fromHex(raw: string) {
  if (!/^#[0-9a-f]{6}$/i.test(raw)) return;
  hex.value = raw.toLowerCase();
  const r = parseInt(raw.slice(1, 3), 16) / 255, g = parseInt(raw.slice(3, 5), 16) / 255, b = parseInt(raw.slice(5, 7), 16) / 255;
  const max = Math.max(r, g, b), min = Math.min(r, g, b), delta = max - min;
  if (delta) hue.value = Math.round(((max === r ? (g - b) / delta + (g < b ? 6 : 0) : max === g ? (b - r) / delta + 2 : (r - g) / delta + 4) * 60) % 360);
  saturation.value = max ? Math.round(delta / max * 100) : 0;
  value.value = Math.round(max * 100);
}
watch(() => props.modelValue, fromHex, { immediate: true });
const huePoint = computed(() => ({ left: `${50 + 46 * Math.sin(hue.value * Math.PI / 180)}%`, top: `${50 - 46 * Math.cos(hue.value * Math.PI / 180)}%` }));
function apply() {
  const h = hue.value / 60, s = saturation.value / 100, v = value.value / 100;
  const c = v * s, x = c * (1 - Math.abs(h % 2 - 1)), m = v - c;
  const rgb = h < 1 ? [c, x, 0] : h < 2 ? [x, c, 0] : h < 3 ? [0, c, x] : h < 4 ? [0, x, c] : h < 5 ? [x, 0, c] : [c, 0, x];
  hex.value = `#${rgb.map(n => Math.round((n + m) * 255).toString(16).padStart(2, "0")).join("")}`;
  emit("update:modelValue", hex.value);
}
function updateHue(e: PointerEvent) {
  const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
  const degrees = Math.atan2(e.clientX - rect.left - rect.width / 2, -(e.clientY - rect.top - rect.height / 2)) * 180 / Math.PI;
  hue.value = Math.round((degrees + 360) % 360); apply();
}
function updateSquare(e: PointerEvent) {
  const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
  saturation.value = Math.round(Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width)) * 100);
  value.value = Math.round((1 - Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height))) * 100); apply();
}
function start(e: PointerEvent, update: (e: PointerEvent) => void) {
  if (e.button !== 0) return;
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId); update(e); e.preventDefault();
}
function toggle() {
  if (props.disabled) return;
  opened.value = !opened.value;
  if (!opened.value) return;
  const rect = trigger.value!.getBoundingClientRect();
  position.value = { left: Math.max(8, Math.min(rect.left, window.innerWidth - 292)), top: Math.max(8, Math.min(rect.bottom + 6, window.innerHeight - 368)) };
  void nextTick(() => popup.value?.querySelector<HTMLInputElement>("input")?.focus());
}
function closeOutside(e: PointerEvent) { if (opened.value && !popup.value?.contains(e.target as Node) && !trigger.value?.contains(e.target as Node)) opened.value = false; }
function closeEscape(e: KeyboardEvent) { if (opened.value && e.key === "Escape") { e.stopPropagation(); opened.value = false; trigger.value?.focus(); } }
window.addEventListener("pointerdown", closeOutside);
window.addEventListener("keydown", closeEscape, true);
onBeforeUnmount(() => { window.removeEventListener("pointerdown", closeOutside); window.removeEventListener("keydown", closeEscape, true); });
function applyHex() { if (/^#[0-9a-f]{6}$/i.test(hex.value)) { fromHex(hex.value); emit("update:modelValue", hex.value); } else fromHex(props.modelValue); }
</script>

<template>
  <button ref="trigger" type="button" class="color-trigger" :disabled="disabled" :aria-label="label" :aria-expanded="opened" aria-haspopup="dialog" :style="{ backgroundColor: modelValue }" @click.stop="toggle" />
  <Teleport to="body">
    <div v-if="opened" ref="popup" class="color-popover tree-context" role="dialog" :aria-label="`${label} picker`" :style="{ left: `${position.left}px`, top: `${position.top}px` }" @pointerdown.stop @click.stop>
      <div class="color-wheel" aria-hidden="true" @pointerdown="start($event, updateHue)" @pointermove="$event.buttons && updateHue($event)">
        <span class="color-hue-point" :style="huePoint" />
        <div class="color-square" :style="{ backgroundColor: `hsl(${hue}, 100%, 50%)` }" @pointerdown.stop="start($event, updateSquare)" @pointermove.stop="$event.buttons && updateSquare($event)">
          <span class="color-square-point" :style="{ left: `${saturation}%`, top: `${100 - value}%` }" />
        </div>
      </div>
      <label>Hue <input v-model.number="hue" type="range" min="0" max="359" aria-label="Hue" @input="apply" /></label>
      <label>Saturation <input v-model.number="saturation" type="range" min="0" max="100" aria-label="Saturation" @input="apply" /></label>
      <label>Brightness <input v-model.number="value" type="range" min="0" max="100" aria-label="Brightness" @input="apply" /></label>
      <label>Hex <input v-model="hex" aria-label="Hex color" maxlength="7" @change="applyHex" @keydown.enter.prevent="applyHex" /></label>
    </div>
  </Teleport>
</template>

<style scoped>
.color-trigger { width: 36px; height: 26px; border: 1px solid var(--border-subtle); cursor: pointer; flex: none; }
.color-trigger:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.color-popover { position: fixed; z-index: 10000; width: 276px; padding: 12px; display: grid; gap: 6px; background: var(--bg-card); border: 1px solid var(--border-subtle); box-shadow: 0 8px 24px rgb(0 0 0 / 35%); }
.color-popover label { display: flex; justify-content: space-between; align-items: center; gap: 8px; font-size: 12px; color: var(--text-primary); }
.color-popover input[type=range] { width: 150px; }
.color-popover input:not([type=range]) { width: 90px; }
.color-wheel { position: relative; width: 220px; height: 220px; margin: 0 auto; border-radius: 50%; background: conic-gradient(red, yellow, lime, cyan, blue, magenta, red); touch-action: none; cursor: crosshair; }
.color-wheel::before { content: ""; position: absolute; inset: 24px; border-radius: 50%; background: var(--bg-card); pointer-events: none; }
.color-square { position: absolute; inset: 49px; background-image: linear-gradient(to top, black, transparent), linear-gradient(to right, white, transparent); touch-action: none; }
.color-hue-point, .color-square-point { position: absolute; width: 14px; height: 14px; border: 2px solid white; box-shadow: 0 0 0 1px black; border-radius: 50%; transform: translate(-50%, -50%); pointer-events: none; }
</style>
