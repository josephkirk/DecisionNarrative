<script setup lang="ts">
import { computed, useId } from "vue";
import type { RunAction, RunView } from "../../contracts.ts";
import { actionsLeftText, hasClock, humanParticipants, moveBlockText, showParticipantTabs } from "../../runModel.ts";

const props = defineProps<{ view: RunView; active: number; busy?: boolean }>();
const emit = defineEmits<{
  (e: "select-participant", id: number): void;
  (e: "act", action: RunAction): void;
}>();

const uid = useId();
const tabs = computed(() => showParticipantTabs(props.view));
const seats = computed(() => humanParticipants(props.view));
const clock = computed(() => hasClock(props.view));
const moves = computed(() => props.view.place?.moves ?? []);
const left = computed(() => props.view.place?.actionsLeft);
const outOfActions = computed(() => left.value === 0);

function go(to: number, enabled: boolean) {
  if (enabled && !props.busy) emit("act", { kind: "move", to });
}
</script>

<template>
  <section class="play-strip" aria-label="Where and when">
    <div v-if="tabs" class="play-tabs" role="tablist" aria-label="Participants">
      <button
        v-for="p in seats"
        :key="p.id"
        type="button"
        role="tab"
        class="play-tab"
        :aria-selected="p.id === active"
        :class="{ 'is-active': p.id === active }"
        @click="emit('select-participant', p.id)"
      >{{ p.name }}</button>
    </div>

    <div class="play-place">
      <span class="play-place__label">Place</span>
      <span class="play-place__name">{{ view.place?.name ?? "Nowhere" }}</span>
      <span v-if="clock" class="play-place__clock">· {{ view.place?.clockLabel }}</span>
      <span v-if="clock" class="play-place__actions" :class="{ 'is-out': outOfActions }">· {{ actionsLeftText(left) }}</span>
    </div>

    <div v-if="moves.length" class="play-moves" role="group" aria-label="Go to">
      <span class="play-place__label">Go to</span>
      <span v-for="(m, i) in moves" :key="m.to" class="play-move">
        <button
          type="button"
          class="play-chip"
          :class="{ 'is-blocked': !m.enabled }"
          :aria-disabled="!m.enabled || busy ? 'true' : undefined"
          :aria-describedby="!m.enabled && m.block ? `${uid}-why-${m.to}` : undefined"
          @click="go(m.to, m.enabled)"
        >
          <kbd v-if="m.enabled" class="play-kbd">G {{ moves.filter((x) => x.enabled).indexOf(m) + 1 }}</kbd>
          {{ m.name }}
        </button>
        <span v-if="!m.enabled && m.block" :id="`${uid}-why-${m.to}`" class="play-move__why" role="tooltip">{{ moveBlockText(m.block) }}</span>
      </span>
    </div>

    <button
      v-if="clock"
      type="button"
      class="btn btn-secondary play-btn play-end-turn"
      :disabled="busy || outOfActions"
      aria-keyshortcuts="E"
      @click="emit('act', { kind: 'wait' })"
    >End turn <kbd class="play-kbd">E</kbd></button>
  </section>
</template>
