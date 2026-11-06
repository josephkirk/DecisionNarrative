<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { StudioApi } from "../api.ts";
import type { RunStartInput, RunSummary } from "../contracts.ts";
import { usePlay } from "../view-models/run.ts";
import PlayStart from "../components/play/PlayStart.vue";
import PlayStory from "../components/play/PlayStory.vue";
import PlayTimeline from "../components/play/PlayTimeline.vue";
import PlayWhereWhen from "../components/play/PlayWhereWhen.vue";
import PlayInspect from "../components/play/PlayInspect.vue";

const route = useRoute();
const router = useRouter();
const play = usePlay();
const { state } = play;

const runId = computed(() => (typeof route.params.runId === "string" ? route.params.runId : null));
const recent = ref<RunSummary[]>([]);
const scenarios = ref<Array<{ id: string; name?: string }>>([]);
const loading = ref(true);
const highlightStep = ref<number | null>(null);

async function loadStartData() {
  try {
    recent.value = await StudioApi.runs.list();
    scenarios.value = await StudioApi.listContent<{ id: string; name?: string }>("scenarios");
  } catch (e) {
    state.error = e instanceof Error ? e.message : String(e);
  }
}

async function startRun(input: RunStartInput) {
  await play.start(input);
  if (state.envelope && !state.error) await router.push({ name: "play-run", params: { runId: state.envelope.id } });
}

async function restart() {
  await startRun({ start: { kind: "beginning" } });
}

function isTyping(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  return !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT" || el.isContentEditable);
}

function onKeydown(e: KeyboardEvent) {
  if (isTyping(e.target) || e.altKey || e.metaKey) return;
  if (e.ctrlKey && e.key.toLowerCase() === "z") e.preventDefault();
  void play.onKey({ key: e.key, ctrlKey: e.ctrlKey });
}

onMounted(async () => {
  window.addEventListener("keydown", onKeydown);
  if (runId.value) await play.open(runId.value);
  else await loadStartData();
  loading.value = false;
});
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <div class="play-view">
    <p v-if="state.error" class="play-error" role="alert">{{ state.error }}</p>

    <PlayStart v-if="!runId" :recent="recent" :scenarios="scenarios" :busy="state.busy" @start="startRun" />

    <div v-else-if="loading || !state.envelope" class="play-skeleton" role="status" aria-live="polite" aria-busy="true" aria-label="Loading run">
      <div class="play-skeleton__bar"></div>
      <div class="play-skeleton__block"></div>
      <div class="play-skeleton__bar"></div>
    </div>

    <template v-else>
      <div v-if="state.envelope.stale" class="play-banner" role="status">
        <span>Content changed since this run started.</span>
        <button type="button" class="btn btn-secondary play-btn" :disabled="state.busy" @click="play.reload()">Reload run</button>
      </div>
      <p v-if="state.reloadNote" class="play-note" role="status">{{ state.reloadNote }}</p>

      <div class="play-layout" :class="{ 'has-inspect': state.inspectOpen }">
      <div class="play-main">
      <PlayWhereWhen :view="state.envelope.view" :active="state.activeParticipant" @select-participant="play.setParticipant" />
      <PlayStory
        :envelope="state.envelope"
        :busy="state.busy"
        :why-open="state.whyOpen"
        :near-misses="state.nearMisses"
        @act="play.act"
        @toggle-why="play.toggleWhy"
        @undo="play.undo"
        @restart="restart"
      />
      <PlayTimeline :view="state.envelope.view" :busy="state.busy" :highlight="highlightStep" @rewind="play.rewind" @undo="play.undo" />
      <button v-if="!state.inspectOpen" type="button" class="btn btn-secondary play-btn play-inspect-toggle" aria-keyshortcuts="I" @click="play.toggleInspect()">Inspect <kbd class="play-kbd">I</kbd></button>
      </div>
      <PlayInspect
        v-if="state.inspectOpen"
        :inspect="state.inspect"
        :tab="state.inspectTab"
        @tab="play.setInspectTab"
        @focus-step="highlightStep = $event"
        @close="play.toggleInspect()"
      />
      </div>
    </template>
  </div>
</template>
