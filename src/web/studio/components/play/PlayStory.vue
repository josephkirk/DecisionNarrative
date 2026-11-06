<script setup lang="ts">
import { ref, watch } from "vue";
import type { NearMiss, RunAction, RunEnvelope } from "../../contracts.ts";
import { alsoHereActions, editLink, nearMissText, stepCountText } from "../../runModel.ts";

const props = defineProps<{
  envelope: RunEnvelope;
  busy: boolean;
  whyOpen: boolean;
  nearMisses: NearMiss[];
}>();
const emit = defineEmits<{
  (e: "act", action: RunAction): void;
  (e: "toggle-why"): void;
  (e: "undo"): void;
  (e: "restart"): void;
}>();

const announce = ref("");
watch(() => props.envelope.view.scene?.storylet.title, (title) => {
  announce.value = title ? `Scene: ${title}` : "";
}, { immediate: true });
</script>

<template>
  <article class="play-story" :aria-busy="busy">
    <p class="play-live" aria-live="polite" role="status">{{ announce }}</p>

    <template v-if="envelope.view.ending">
      <header class="play-ending">
        <p class="play-eyebrow">The end</p>
        <h2 class="play-title">{{ envelope.view.ending.title }}</h2>
        <p class="play-path">{{ stepCountText(envelope.view.currentStep) }} on this path</p>
        <div class="play-ending__actions">
          <button type="button" class="btn btn-primary play-btn" @click="emit('restart')">Play again</button>
          <button type="button" class="btn btn-secondary play-btn" :disabled="envelope.view.currentStep === 0" @click="emit('undo')">Rewind</button>
          <router-link class="btn btn-secondary play-btn" to="/play">Back to start</router-link>
        </div>
      </header>
    </template>

    <template v-else-if="envelope.stalled">
      <div class="play-stalled" role="status">
        <p>The story stopped advancing{{ envelope.view.scene ? ` at “${envelope.view.scene.storylet.title}”` : "" }}.</p>
        <router-link
          v-if="envelope.view.scene && editLink(envelope.view.scene.storylet)"
          class="btn btn-secondary play-btn"
          :to="editLink(envelope.view.scene.storylet)!"
        >Open storylet</router-link>
      </div>
    </template>

    <template v-else-if="envelope.view.scene">
      <header class="play-scene-head">
        <h2 class="play-title">{{ envelope.view.scene.storylet.title }}</h2>
        <router-link
          v-if="editLink(envelope.view.scene.storylet)"
          class="play-edit"
          :to="editLink(envelope.view.scene.storylet)!"
          aria-label="Edit this storylet"
        >✎ edit</router-link>
      </header>
      <p v-for="(line, i) in envelope.view.scene.lines" :key="i" class="play-line">{{ line.text }}</p>

      <ol class="play-choices" aria-label="Choices">
        <li v-for="(choice, i) in envelope.view.scene.choices" :key="i">
          <button
            type="button"
            class="play-choice"
            :disabled="!choice.enabled || busy"
            @click="emit('act', choice.action)"
          >
            <kbd v-if="i < 9" class="play-kbd">{{ i + 1 }}</kbd>
            <span class="play-choice__label">{{ choice.label }}</span>
            <span v-if="!choice.enabled && choice.requirement" class="play-choice__req">(needs: {{ choice.requirement }})</span>
          </button>
        </li>
      </ol>

    </template>

    <template v-else-if="envelope.view.nothingHere">
      <div class="play-nothing">
        <p class="play-title">Nothing happens here right now.</p>
        <button
          type="button"
          class="btn btn-secondary play-btn"
          :aria-expanded="whyOpen"
          aria-controls="play-why"
          @click="emit('toggle-why')"
        >Why?</button>
        <ul v-if="whyOpen" id="play-why" class="play-why">
          <li v-if="!nearMisses.length">No storylets are close to being available.</li>
          <li v-for="m in nearMisses.slice(0, 5)" :key="m.storylet.id">
            <router-link v-if="editLink(m.storylet)" :to="editLink(m.storylet)!">{{ nearMissText(m) }}</router-link>
            <span v-else>{{ nearMissText(m) }}</span>
          </li>
        </ul>
      </div>
    </template>
    <section v-if="!envelope.view.ending && !envelope.stalled && alsoHereActions(envelope.view).length" class="play-also" aria-label="Also here">
      <h3 class="play-eyebrow">Also here</h3>
      <ul class="play-also__list">
        <li v-for="item in alsoHereActions(envelope.view)" :key="item.storylet.id">
          <button type="button" class="play-choice" :disabled="busy" @click="emit('act', item.action)">
            <span class="play-choice__label">{{ item.storylet.title }}</span>
          </button>
        </li>
      </ul>
    </section>
  </article>
</template>
