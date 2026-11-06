<script setup lang="ts">
import { computed, ref } from "vue";
import type { RunStartInput, RunSummary } from "../../contracts.ts";
import { studioState } from "../../state.ts";
import { stepCountText } from "../../runModel.ts";

const props = defineProps<{ recent: RunSummary[]; scenarios: Array<{ id: string; name?: string }>; busy: boolean }>();
const emit = defineEmits<{ (e: "start", input: RunStartInput): void }>();

const mode = ref<"beginning" | "storylet">("beginning");
const search = ref("");
const storyletId = ref("");
const scenarioId = ref("");
const seed = ref("");

const storylets = computed(() => {
  const q = search.value.trim().toLowerCase();
  return studioState.storylets.filter((s) => !q || s.title.toLowerCase().includes(q) || s.id.toLowerCase().includes(q));
});
const hasContent = computed(() => studioState.storylets.length > 0);
const canStart = computed(() => hasContent.value && !props.busy && (mode.value === "beginning" || storyletId.value !== ""));

function submit() {
  if (!canStart.value) return;
  const input: RunStartInput = { start: { kind: "beginning" } };
  if (mode.value === "storylet") input.storyletId = storyletId.value;
  if (scenarioId.value) input.scenarioId = scenarioId.value;
  if (seed.value.trim() !== "" && Number.isFinite(Number(seed.value))) input.seed = Number(seed.value);
  emit("start", input);
}
</script>

<template>
  <section class="play-start">
    <h2 class="play-title">Play</h2>
    <p class="play-lede">Read and choose. The engine runs itself until it needs you.</p>

    <div v-if="!hasContent" class="play-empty">
      <p>This project has no storylets to play yet.</p>
      <router-link class="btn btn-primary play-btn" to="/storylets/new">Create a storylet</router-link>
    </div>

    <form v-else class="play-form" @submit.prevent="submit">
      <fieldset class="play-fieldset">
        <legend>Start</legend>
        <label class="play-radio"><input v-model="mode" type="radio" value="beginning" /> From the beginning</label>
        <label class="play-radio"><input v-model="mode" type="radio" value="storylet" /> From a storylet…</label>
      </fieldset>

      <div v-if="mode === 'storylet'" class="play-field">
        <label for="play-search">Find a storylet</label>
        <input id="play-search" v-model="search" class="form-input" type="search" aria-label="Find a storylet" placeholder="Search by title" />
        <label for="play-storylet">Storylet</label>
        <select id="play-storylet" v-model="storyletId" class="form-input" size="6">
          <option v-for="s in storylets" :key="s.id" :value="s.id">{{ s.title }}</option>
        </select>
      </div>

      <div v-if="scenarios.length > 1" class="play-field">
        <label for="play-scenario">Scenario</label>
        <select id="play-scenario" v-model="scenarioId" class="form-input">
          <option value="">First scenario</option>
          <option v-for="s in scenarios" :key="s.id" :value="s.id">{{ s.name ?? s.id }}</option>
        </select>
      </div>

      <details class="play-options">
        <summary>Options</summary>
        <div class="play-field">
          <label for="play-seed">Seed (blank for the scenario default)</label>
          <input id="play-seed" v-model="seed" class="form-input" inputmode="numeric" />
        </div>
      </details>

      <button type="submit" class="btn btn-primary play-btn" :disabled="!canStart">Start playing</button>
    </form>

    <section v-if="recent.length" class="play-recent" aria-label="Recent runs">
      <h3 class="play-eyebrow">Recent runs</h3>
      <ul>
        <li v-for="r in recent" :key="r.id">
          <router-link :to="`/play/${r.id}`">
            <span class="play-recent__title">{{ r.title }}</span>
            <span class="play-recent__meta">{{ stepCountText(r.steps) }}</span>
            <span v-if="r.ending" class="badge play-ending-badge">Ending: {{ r.ending.title }}</span>
            <span v-if="r.stale" class="badge play-stale-badge">Content changed</span>
          </router-link>
        </li>
      </ul>
    </section>
  </section>
</template>
