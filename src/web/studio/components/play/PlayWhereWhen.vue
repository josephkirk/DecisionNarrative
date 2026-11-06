<script setup lang="ts">
import { computed } from "vue";
import type { RunView } from "../../contracts.ts";
import { hasClock, showParticipantTabs } from "../../runModel.ts";

const props = defineProps<{ view: RunView; active: number }>();
const emit = defineEmits<{ (e: "select-participant", id: number): void }>();
const tabs = computed(() => showParticipantTabs(props.view));
</script>

<template>
  <section class="play-strip" aria-label="Where and when">
    <div v-if="tabs" class="play-tabs" role="tablist" aria-label="Participants">
      <button
        v-for="p in view.participants"
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
      <span v-if="hasClock(view)" class="play-place__clock">· {{ view.place?.clock }}</span>
    </div>
  </section>
</template>
