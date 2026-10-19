<script setup lang="ts">
import { computed, inject, onMounted, ref, watch } from "vue";
import { usePlaytestViewModel } from "../view-models/playtest.ts";
import { studioState, locationMap } from "../state.ts";
import { StudioApi } from "../api.ts";
import type { StoryletRecord } from "../contracts.ts";
import { interactionContextKey } from "../interactionContext.ts";

const {
  activeTab,
  sessions,
  loading,
  snapshot,
  sessionId,
  loadSessions,
  selectSession,
  createSession,
  tick,
  signal,
  chooseOutput,
  chooseStorylet,
  divertTopic,
  rollbackLatestCall,
} = usePlaytestViewModel();
const interactionContext = inject(interactionContextKey);
const scenarioId = ref("");
const eventName = ref("");
const currentStorylet = ref<StoryletRecord | null>(null);

const context = computed(() => snapshot.value?.context ?? null);
const world = computed(() => context.value?.world ?? null);
const currentLocation = computed(() => {
  const id = world.value?.currentLocation;
  return id ? locationMap.value.get(id) : null;
});
const currentStoryletId = computed(() => {
  const nodes = context.value?.nodes ?? [];
  return [...nodes].reverse().find((node) => node.storyletId)?.storyletId
    ?? snapshot.value?.session.startStoryletId
    ?? null;
});
const metricEntries = computed(() => Object.entries(world.value?.metrics ?? {}).sort(([a], [b]) => a.localeCompare(b)));
const entityEntries = computed(() => Object.entries(world.value?.entities ?? {}).sort(([a], [b]) => a.localeCompare(b)));
const interactionCount = computed(() => context.value?.activeInteractions.length ?? 0);
const history = computed(() => context.value?.globalCallHistory ?? []);

onMounted(async () => {
  await loadSessions();
  if (sessionId.value) await selectSession(sessionId.value);
});

watch(currentStoryletId, async (id) => {
  if (!id) {
    currentStorylet.value = null;
    return;
  }
  try {
    currentStorylet.value = await StudioApi.getContent<StoryletRecord>("storylets", id);
  } catch {
    currentStorylet.value = null;
  }
}, { immediate: true });

async function sendSignal() {
  const event = eventName.value;
  eventName.value = "";
  await signal(event);
}

watch(() => [sessionId.value, snapshot.value, currentStorylet.value, activeTab.value], () => {
  const storylet = currentStorylet.value;
  const selectedInteraction = context.value?.activeInteractions[0];
  interactionContext?.setFocus(sessionId.value
    ? {
      entity: storylet
        ? { type: "storylet", id: storylet.id, label: storylet.title, data: storylet }
        : { type: "playtest_session", id: sessionId.value, data: { snapshot: snapshot.value } },
      selection: selectedInteraction
        ? { type: "playtest_node", id: String(selectedInteraction.nodeGuid), label: selectedInteraction.kind, data: selectedInteraction }
        : null,
    }
    : { entity: null, selection: null });
}, { deep: true });
</script>

<template>
  <div style="display:flex; flex-direction:column; height:100%;">
    <header class="view-header">
      <div style="display:flex; align-items:center; gap:12px; flex-wrap:wrap;">
        <h1 class="view-title">ChronoFlow Playtest</h1>
        <select :value="sessionId ?? ''" class="form-select" style="width:220px;" @change="selectSession(($event.target as HTMLSelectElement).value)">
          <option value="">Select a run…</option>
          <option v-for="run in sessions" :key="run.sessionId" :value="run.sessionId">
            {{ run.sessionId.slice(0, 8) }} · {{ run.globalHistoryLength }} calls
          </option>
        </select>
        <select v-model="scenarioId" class="form-select" style="width:190px;">
          <option value="">Default start</option>
          <option v-for="scenario in studioState.scenarios" :key="scenario.id" :value="scenario.id">{{ scenario.name }}</option>
        </select>
        <button class="btn btn-primary" :disabled="loading" @click="createSession(scenarioId || undefined)">New run</button>
      </div>
      <div style="display:flex; align-items:center; gap:8px;">
        <span v-if="snapshot" class="badge">Turn {{ context?.turn }} · {{ context?.clockMs }} ms</span>
        <button class="btn btn-secondary" :disabled="!sessionId || loading" @click="tick(16)">Tick 16 ms</button>
        <button class="btn btn-secondary" :disabled="!history.length || loading" @click="rollbackLatestCall">Undo call</button>
      </div>
    </header>

    <div style="display:flex; flex:1; min-height:0; overflow:hidden;">
      <main style="flex:1; min-width:0; padding:24px 32px; overflow-y:auto;">
        <div v-if="!snapshot" style="padding:60px; text-align:center; color:var(--text-muted);">
          <strong style="display:block; color:var(--text-main); font-size:16px;">No ChronoFlow run selected</strong>
          <span>Create a run to execute the authored graph with the deterministic runtime.</span>
        </div>
        <template v-else>
          <div style="margin-bottom:18px; color:var(--accent); font-size:12px; font-weight:700; text-transform:uppercase;">
            {{ currentLocation?.name || world?.currentLocation || "No current location" }}
          </div>
          <section style="max-width:760px; margin-bottom:24px;">
            <h2 style="font-size:24px; margin:0 0 8px;">{{ currentStorylet?.title || snapshot.session.startStoryletId || "Narrative graph" }}</h2>
            <p style="font-size:14px; line-height:1.65; color:var(--text-muted);">
              {{ currentStorylet?.content?.sceneOpening || currentStorylet?.content?.text_template || "Runtime state is driven by the active graph nodes and signal tokens." }}
            </p>
          </section>

          <section style="max-width:760px; margin-bottom:24px;">
            <h3 style="font-size:12px; text-transform:uppercase; color:var(--text-muted);">Pending graph interactions · {{ interactionCount }}</h3>
            <div v-if="!interactionCount" class="chrono-empty">No player choice is pending. Tick the graph or send a signal event.</div>
            <div v-for="interaction in context?.activeInteractions ?? []" :key="interaction.nodeGuid" class="chrono-card">
              <div style="display:flex; justify-content:space-between; margin-bottom:10px;">
                <strong>{{ interaction.kind === 'qbn' ? 'QBN storylet deck' : interaction.kind === 'waypoint' ? 'Waypoint topic' : 'Choice node' }}</strong>
                <code>node {{ interaction.nodeGuid }}</code>
              </div>
              <template v-if="interaction.kind === 'choice'">
                <button v-for="option in interaction.options ?? []" :key="option.outputPin" class="btn btn-secondary chrono-option" :disabled="loading" @click="chooseOutput(interaction.nodeGuid, option.outputPin)">
                  <span>{{ option.label }}</span><code>{{ option.outputPin }}</code>
                </button>
              </template>
              <template v-else-if="interaction.kind === 'qbn'">
                <button v-for="storylet in interaction.eligibleStorylets ?? []" :key="storylet.id" class="btn btn-secondary chrono-option" :disabled="loading" @click="chooseStorylet(interaction.nodeGuid, storylet.id)">
                  <span>{{ storylet.title }} <small>{{ storylet.id }}</small></span>
                  <span class="badge">{{ storylet.urgencyColorTag }} · Priority {{ storylet.displayPriority }}</span>
                </button>
                <div v-if="!interaction.eligibleStorylets?.length" class="chrono-empty">No storylet currently satisfies this deck.</div>
              </template>
              <template v-else>
                <div class="chrono-empty">Current topic: {{ interaction.currentTopicId }}</div>
                <button v-for="topic in interaction.topics ?? []" :key="topic.edgeId" class="btn btn-secondary chrono-option" :disabled="loading" @click="divertTopic(interaction.nodeGuid, topic.edgeId)">
                  <span>{{ topic.topicId }} <small>{{ topic.edgeId }}<span v-if="topic.revealedClueQualityId"> · reveals {{ topic.revealedClueQualityId }}</span></small></span>
                  <span class="badge">{{ topic.triggersSceneExit ? 'Exit' : topic.playerOnly ? 'Player trigger' : 'Topic' }}</span>
                </button>
                <div v-if="!interaction.topics?.length" class="chrono-empty">No adjacent topic transitions.</div>
              </template>
            </div>
          </section>

          <section style="max-width:760px;">
            <h3 style="font-size:12px; text-transform:uppercase; color:var(--text-muted);">Send signal event</h3>
            <form style="display:flex; gap:8px;" @submit.prevent="sendSignal">
              <input v-model="eventName" class="form-input" placeholder="Event name" aria-label="Signal event name">
              <button class="btn btn-secondary" :disabled="!eventName.trim() || loading">Signal</button>
            </form>
          </section>
        </template>
      </main>

      <aside style="width:360px; flex:none; border-left:1px solid var(--border-subtle); display:flex; flex-direction:column; background:var(--bg-card);">
        <nav style="display:flex; border-bottom:1px solid var(--border-subtle); padding:6px; gap:4px;">
          <button :class="['tab-btn', activeTab === 'runtime' ? 'active' : '']" @click="activeTab = 'runtime'">Runtime</button>
          <button :class="['tab-btn', activeTab === 'state' ? 'active' : '']" @click="activeTab = 'state'">World state</button>
          <button :class="['tab-btn', activeTab === 'history' ? 'active' : '']" @click="activeTab = 'history'">Call history</button>
        </nav>
        <div style="flex:1; overflow-y:auto; padding:16px; font-size:12px;">
          <template v-if="activeTab === 'runtime'">
            <h3 class="chrono-heading">Graph nodes</h3>
            <div v-for="node in context?.nodes ?? []" :key="node.nodeGuid" class="chrono-row">
              <div><strong>{{ node.nodeTypeId }}</strong><code> #{{ node.nodeGuid }}</code></div>
              <div style="color:var(--text-muted);">{{ JSON.stringify(node.state) }}<span v-if="node.storyletId"> · {{ node.storyletId }}</span></div>
            </div>
            <h3 class="chrono-heading">Pending tokens · {{ context?.tokens.length ?? 0 }}</h3>
            <pre class="chrono-json">{{ JSON.stringify(context?.tokens ?? [], null, 2) }}</pre>
          </template>

          <template v-else-if="activeTab === 'state'">
            <h3 class="chrono-heading">Qualities</h3>
            <div v-for="([name, value]) in metricEntries" :key="name" class="chrono-row"><span>{{ name }}</span><strong>{{ value }}</strong></div>
            <h3 class="chrono-heading">Global tags</h3>
            <div v-if="!world?.globalTags.length" class="chrono-empty">No global tags.</div>
            <div v-for="tag in world?.globalTags ?? []" :key="tag" class="chrono-row">{{ tag }}</div>
            <h3 class="chrono-heading">Entities</h3>
            <div v-for="([id, tags]) in entityEntries" :key="id" class="chrono-row"><strong>{{ id }}</strong><div>{{ tags.join(', ') || 'No tags' }}</div></div>
            <h3 class="chrono-heading">Passions</h3>
            <pre class="chrono-json">{{ JSON.stringify(world?.passions ?? {}, null, 2) }}</pre>
            <h3 class="chrono-heading">Narrative Lego transitions</h3>
            <div v-if="!context?.passionEvents.length" class="chrono-empty">No passion threshold transitions yet.</div>
            <pre v-else class="chrono-json">{{ JSON.stringify(context.passionEvents, null, 2) }}</pre>
            <h3 class="chrono-heading">Configured passion thresholds</h3>
            <pre class="chrono-json">{{ JSON.stringify(context?.passionThresholds ?? [], null, 2) }}</pre>
          </template>

          <template v-else>
            <div style="display:flex; align-items:center; justify-content:space-between;">
              <h3 class="chrono-heading">Global call history · {{ history.length }}</h3>
              <button class="btn btn-secondary" :disabled="!history.length || loading" @click="rollbackLatestCall">Undo latest</button>
            </div>
            <div v-for="(entry, index) in history" :key="index" class="chrono-row">
              <strong>{{ index + 1 }}. {{ (entry as Record<string, unknown>).node_guid ?? 'Node call' }}</strong>
              <pre class="chrono-json">{{ JSON.stringify(entry, null, 2) }}</pre>
            </div>
            <div v-if="!history.length" class="chrono-empty">No completed calls to reconcile.</div>
          </template>
        </div>
      </aside>
    </div>
  </div>
</template>

<style scoped>
.chrono-card { padding: 14px; margin: 10px 0; border: 1px solid var(--border-subtle); border-radius: 10px; background: var(--bg-card); }
.chrono-option { width: 100%; display: flex; justify-content: space-between; gap: 10px; text-align: left; margin-top: 7px; }
.chrono-option small { display: block; color: var(--text-muted); font-size: 10px; }
.chrono-empty { padding: 12px; border-radius: 8px; color: var(--text-muted); background: var(--bg-input); }
.chrono-heading { margin: 14px 0 8px; color: var(--text-muted); font-size: 11px; text-transform: uppercase; }
.chrono-row { display: grid; gap: 4px; margin-bottom: 6px; padding: 8px; border-radius: 7px; background: var(--bg-input); overflow-wrap: anywhere; }
.chrono-row code { color: var(--text-muted); font-size: 10px; }
.chrono-json { max-height: 260px; overflow: auto; white-space: pre-wrap; overflow-wrap: anywhere; font-size: 10px; color: var(--text-muted); }
</style>
