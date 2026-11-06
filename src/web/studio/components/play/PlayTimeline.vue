<script setup lang="ts">
import { computed } from "vue";
import type { RunView } from "../../contracts.ts";
import { activeBranchId, stepText, timelineDots, type TimelineDot } from "../../runModel.ts";

const props = defineProps<{ view: RunView; busy: boolean }>();
const emit = defineEmits<{ (e: "rewind", step: number): void; (e: "undo"): void }>();

const dots = computed(() => timelineDots(props.view));
const names = computed(() => new Map(props.view.participants.map((p) => [p.id, p.name])));
const active = computed(() => activeBranchId(props.view));
const letter = (id: number) => String.fromCharCode(97 + (id % 26));
function label(d: TimelineDot): string {
  if (!d.step) return "Step 0: start";
  const who = names.value.get(d.step.participant) ?? "Someone";
  return `Step ${d.index}: ${who} · ${stepText(d.step)}${d.state === "ghost" ? " (undone)" : ""}`;
}
</script>

<template>
  <footer class="play-timeline" aria-label="Step timeline">
    <span class="play-eyebrow">Steps</span>
    <ol class="play-dots">
      <li v-for="d in dots" :key="d.index">
        <button
          type="button"
          class="play-dot"
          :class="`is-${d.state}`"
          :aria-label="label(d)"
          :title="label(d)"
          :aria-current="d.state === 'current' ? 'step' : undefined"
          :disabled="busy || d.state === 'current'"
          @click="emit('rewind', d.index)"
        ></button>
      </li>
    </ol>
    <button type="button" class="btn btn-secondary play-btn" :disabled="busy || view.currentStep === 0" @click="emit('undo')">
      ↶ Undo <kbd class="play-kbd">Ctrl Z</kbd>
    </button>
    <span v-if="view.branches.length > 1" class="play-branches" aria-label="Branches">
      branch:
      <span v-for="b in view.branches" :key="b.id" class="play-branch" :class="{ 'is-active': b.id === active }">{{ letter(b.id) }}</span>
    </span>
  </footer>
</template>
