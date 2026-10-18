<script setup lang="ts">
import { inject, onMounted, ref, computed, watch } from "vue";
import { usePlaytestViewModel } from "../view-models/playtest.ts";
import { studioState, locationMap, characterMap } from "../state.ts";
import { StudioApi } from "../api.ts";
import type { StoryletRecord } from "../contracts.ts";
import { interactionContextKey } from "../interactionContext.ts";

const {
  activeTab,
  sessions,
  directorEval,
  events,
  loading,
  snapshot,
  sessionId,
  loadSessions,
  selectSession,
  refreshDirector,
  refreshEvents,
  waitTurn,
  moveToLocation,
  chooseOption,
  forkPlaytest,
} = usePlaytestViewModel();
const selectedStoryletRecord = ref<StoryletRecord | null>(null);
const selectedEventId = ref<string | null>(null);
const compareSessionId = ref<string>("");
const comparisonReport = ref<string>("");

const interactionContext = inject(interactionContextKey);
watch(() => [sessionId.value, snapshot.value, selectedStoryletRecord.value, selectedEventId.value, events.value], () => {
  const selectedEvent = events.value.find((event) => event.id === selectedEventId.value);
  if (selectedEvent) {
    interactionContext?.setFocus({
      entity: null,
      selection: { type: "playtest_event", id: selectedEvent.id, label: selectedEvent.action, data: selectedEvent },
    });
    return;
  }
  if (selectedStoryletRecord.value) {
    const storylet = selectedStoryletRecord.value;
    interactionContext?.setFocus({
      entity: { type: "storylet", id: storylet.id, label: storylet.title, data: JSON.parse(JSON.stringify(storylet)) },
      selection: { type: "playtest_scene", id: storylet.id, data: { sessionId: sessionId.value } },
    });
    return;
  }
  interactionContext?.setFocus(sessionId.value
    ? { entity: { type: "playtest_session", id: sessionId.value, data: { snapshot: snapshot.value } }, selection: null }
    : { entity: null, selection: null });
}, { deep: true });

onMounted(async () => {
  await loadSessions();
  if (sessionId.value) {
    await selectSession(sessionId.value);
  }
});

watch(() => directorEval.value?.selected, async (selId) => {
  if (selId) {
    try {
      selectedStoryletRecord.value = await StudioApi.getContent<StoryletRecord>("storylets", selId);
    } catch {
      selectedStoryletRecord.value = null;
    }
  } else {
    selectedStoryletRecord.value = null;
  }
});

const currentLocRecord = computed(() => {
  const locId = snapshot.value?.world.current_location;
  return locId ? locationMap.value.get(locId) : null;
});

const connectedLocations = computed(() => {
  const zones = currentLocRecord.value?.connected_zones ?? [];
  return zones.map((zid) => locationMap.value.get(zid) ?? { id: zid, name: zid, description: "" });
});

async function handleFork() {
  if (!sessionId.value || !selectedEventId.value) return;
  const res = await forkPlaytest(sessionId.value, selectedEventId.value);
  if (res?.sessionId) {
    await loadSessions();
    await selectSession(res.sessionId);
  }
}

function handleCompare() {
  if (!compareSessionId.value || !sessionId.value) return;
  const curTurn = snapshot.value?.world.turn ?? 0;
  comparisonReport.value = `Comparing active session turn ${curTurn} with branch ${compareSessionId.value.slice(0, 8)}. Branch history divergent.`;
}
</script>

<template>
  <div style="display:flex; flex-direction:column; height:100%;">
    <!-- Playtest Toolbar -->
    <header class="view-header">
      <div style="display:flex; align-items:center; gap:16px;">
        <h1 class="view-title">Playtest Runtime</h1>
        <div style="display:flex; align-items:center; gap:8px;">
          <label style="font-size:12px; color:var(--text-muted);">Saved Runs:</label>
          <select
            :value="sessionId ?? ''"
            class="form-select"
            style="width: 200px;"
            @change="selectSession(($event.target as HTMLSelectElement).value)"
          >
            <option value="" disabled>Select a playtest session...</option>
            <option v-for="s in sessions" :key="s.session_id" :value="s.session_id">
              {{ s.session_id.slice(0, 8) }} (Turn {{ s.turn }})
            </option>
          </select>
        </div>
      </div>

      <div style="display:flex; align-items:center; gap:10px;">
        <span v-if="snapshot" class="badge">
          Turn {{ snapshot.world.turn }} · Rev {{ snapshot.world.revision }}
        </span>
        <button class="btn btn-secondary" :disabled="!sessionId" @click="waitTurn">
          Wait 1 Turn
        </button>
      </div>
    </header>

    <!-- Workspace Split -->
    <div style="display:flex; flex:1; overflow:hidden;">
      <!-- Center: Scene & Choices & Movement Map -->
      <div style="flex:1; padding:24px 32px; overflow-y:auto;">
        <div v-if="!snapshot" style="padding:60px; text-align:center; color:var(--text-muted);">
          <p style="font-size:15px; font-weight:600;">No active playtest session</p>
          <p style="font-size:12px; margin-top:6px;">Select an existing session above or launch a run from Starting Scenarios.</p>
        </div>

        <div v-else style="max-width:720px;">
          <!-- Location breadcrumb -->
          <div style="font-size:12px; color:var(--accent); font-weight:700; text-transform:uppercase; letter-spacing:0.5px; margin-bottom:6px;">
            📍 {{ currentLocRecord?.name || snapshot.world.current_location || "Unknown Location" }}
          </div>

          <!-- Scene Title & Prose -->
          <div style="margin-bottom:24px;">
            <h2 style="font-size:22px; font-weight:700; margin-bottom:12px;">
              {{ selectedStoryletRecord?.title || directorEval?.selected || "No eligible scene" }}
            </h2>

            <p style="font-size:15px; line-height:1.65; color:var(--text-main); font-family:var(--font-serif, serif);">
              {{ selectedStoryletRecord?.content?.sceneOpening || selectedStoryletRecord?.content?.text_template || "The drama director will surface scenes when trigger conditions and role participants align with current world state." }}
            </p>
          </div>

          <!-- Storylet Choices -->
          <div style="margin-bottom:32px;">
            <div style="font-size:12px; font-weight:700; color:var(--text-muted); margin-bottom:10px; letter-spacing:0.5px;">AVAILABLE CHOICES</div>
            <div v-if="selectedStoryletRecord && selectedStoryletRecord.choices.length > 0" style="display:flex; flex-direction:column; gap:10px;">
              <button
                v-for="choice in selectedStoryletRecord.choices"
                :key="choice.choice_id"
                class="btn btn-secondary"
                style="justify-content:flex-start; text-align:left; padding:12px 16px; border-radius:10px;"
                @click="chooseOption(selectedStoryletRecord.id, choice.choice_id)"
              >
                <span style="font-weight:600;">{{ choice.label }}</span>
                <span v-if="choice.systemVerb" class="badge" style="margin-left:auto;">{{ choice.systemVerb }}</span>
              </button>
            </div>
            <div v-else style="font-size:13px; color:var(--text-muted); padding:12px; background:var(--bg-card); border-radius:8px; border:1px solid var(--border-subtle);">
              No choices offered in this scene. You may move or wait to change world conditions.
            </div>
          </div>

          <!-- Movement / Connected Locations -->
          <div>
            <div style="font-size:12px; font-weight:700; color:var(--text-muted); margin-bottom:10px; letter-spacing:0.5px;">CONNECTED LOCATIONS</div>
            <div v-if="connectedLocations.length > 0" style="display:flex; gap:10px; flex-wrap:wrap;">
              <button
                v-for="loc in connectedLocations"
                :key="loc.id"
                class="btn btn-secondary"
                @click="moveToLocation(loc.id)"
              >
                Move to {{ loc.name }}
              </button>
            </div>
            <div v-else style="font-size:13px; color:var(--text-muted);">
              No routes connected to this location.
            </div>
          </div>
        </div>
      </div>

      <!-- Right Inspector Panel -->
      <div style="width:360px; border-left:1px solid var(--border-subtle); display:flex; flex-direction:column; background:var(--bg-card);">
        <div style="display:flex; border-bottom:1px solid var(--border-subtle); padding:6px 8px; gap:4px;">
          <button :class="['tab-btn', activeTab === 'state' ? 'active' : '']" @click="activeTab = 'state'">State</button>
          <button :class="['tab-btn', activeTab === 'director' ? 'active' : '']" @click="activeTab = 'director'">Director</button>
          <button :class="['tab-btn', activeTab === 'causality' ? 'active' : '']" @click="activeTab = 'causality'">Causality</button>
        </div>

        <div style="flex:1; overflow-y:auto; padding:16px; font-size:12px;">
          <!-- STATE TAB -->
          <div v-if="activeTab === 'state'">
            <div style="font-weight:700; margin-bottom:8px; text-transform:uppercase; color:var(--text-muted);">World Qualities</div>
            <div v-if="snapshot" style="display:grid; gap:4px; margin-bottom:16px;">
              <div
                v-for="(val, qk) in snapshot.world.qualities"
                :key="qk"
                style="display:flex; justify-content:space-between; padding:4px 8px; background:var(--bg-input); border-radius:6px;"
              >
                <span style="font-family:var(--font-mono);">{{ qk }}</span>
                <strong>{{ val }}</strong>
              </div>
            </div>

            <div style="font-weight:700; margin-bottom:8px; text-transform:uppercase; color:var(--text-muted);">Actors Present</div>
            <div v-if="snapshot" style="display:grid; gap:6px;">
              <div
                v-for="(act, aid) in snapshot.world.actors"
                :key="aid"
                style="padding:6px 8px; background:var(--bg-input); border-radius:6px;"
              >
                <div style="font-weight:600;">{{ characterMap.get(aid)?.name || aid }}</div>
                <div style="font-size:11px; color:var(--text-muted);">At: {{ locationMap.get(act.current_location)?.name || act.current_location }}</div>
              </div>
            </div>
          </div>

          <!-- DIRECTOR TAB -->
          <div v-else-if="activeTab === 'director'">
            <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:12px;">
              <span style="font-weight:700; text-transform:uppercase; color:var(--text-muted);">Candidate Pool</span>
            </div>

            <div v-for="c in directorEval?.candidates ?? []" :key="c.storylet_id" style="margin-bottom:10px; padding:8px 10px; background:var(--bg-input); border-radius:8px;">
              <div style="display:flex; justify-content:space-between; align-items:center;">
                <span style="font-weight:600;">{{ c.storylet_id }}</span>
                <span :class="['badge', c.eligible ? 'approved' : '']">{{ c.eligible ? 'Eligible' : 'Rejected' }}</span>
              </div>
              <div v-if="!c.eligible && c.rejection" style="color:var(--status-danger); font-size:11px; margin-top:4px;">
                Reason: {{ c.rejection.reason }}
              </div>
              <div v-if="c.eligible && Object.keys(c.bound_roles).length" style="color:var(--text-muted); font-size:11px; margin-top:4px;">
                Roles: {{ JSON.stringify(c.bound_roles) }}
              </div>
            </div>
          </div>

          <!-- CAUSALITY TAB -->
          <div v-else-if="activeTab === 'causality'">
            <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:12px;">
              <span style="font-weight:700; text-transform:uppercase; color:var(--text-muted);">Event Causal Trace</span>
              <button
                class="btn btn-secondary"
                :disabled="!selectedEventId"
                style="font-size:11px; padding:3px 8px;"
                @click="handleFork"
              >
                Fork From Event
              </button>
            </div>

            <div v-for="ev in events" :key="ev.id"
              :style="{
                padding: '8px 10px',
                borderRadius: '8px',
                marginBottom: '6px',
                cursor: 'pointer',
                background: selectedEventId === ev.id ? 'var(--accent-subtle)' : 'var(--bg-input)',
                border: selectedEventId === ev.id ? '1px solid var(--accent)' : '1px solid transparent'
              }"
              @click="selectedEventId = ev.id"
            >
              <div style="display:flex; justify-content:space-between;">
                <strong>Turn {{ ev.turn }}: {{ ev.actor_id }}</strong>
                <span style="color:var(--text-dim); font-size:10px;">{{ ev.id.slice(0, 8) }}</span>
              </div>
              <div style="color:var(--text-muted); font-size:11px; margin-top:2px;">{{ ev.action }}</div>
            </div>

            <!-- Branch compare -->
            <div style="margin-top:20px; padding-top:14px; border-top:1px solid var(--border-subtle);">
              <label class="form-label">Compare with branch</label>
              <div style="display:flex; gap:8px;">
                <select v-model="compareSessionId" class="form-select" style="flex:1;">
                  <option value="">Select branch run...</option>
                  <option v-for="s in sessions.filter(s => s.session_id !== sessionId)" :key="s.session_id" :value="s.session_id">
                    {{ s.session_id.slice(0, 8) }}
                  </option>
                </select>
                <button class="btn btn-secondary" :disabled="!compareSessionId" @click="handleCompare">Compare</button>
              </div>
              <p v-if="comparisonReport" style="margin-top:8px; font-size:11px; color:var(--text-muted);">{{ comparisonReport }}</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
