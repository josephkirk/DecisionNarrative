<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { useScenarioViewModel } from "../view-models/scenario.ts";
import { studioState } from "../state.ts";

const router = useRouter();
const {
  scenarios,
  selectedScenarioId,
  activeScenario,
  preview,
  loading,
  loadScenarios,
  selectScenario,
  triggerPreview,
  saveScenario,
  startPlaytestFromScenario,
} = useScenarioViewModel();

onMounted(async () => {
  await loadScenarios();
  if (scenarios.value.length > 0) {
    selectScenario(scenarios.value[0]);
  }
});

function createNewScenario() {
  const newId = `scen_${Date.now().toString(36)}`;
  activeScenario.id = newId;
  activeScenario.name = "New Scenario";
  activeScenario.seed = Math.floor(Math.random() * 1000) + 1;
  activeScenario.start_location_policy = { mode: "fixed", location_id: studioState.locations[0]?.id || "" };
  activeScenario.initial_qualities = {};
  activeScenario.actor_locations = {};
  activeScenario.actor_population_policy = {
    mode: "all",
    location_policy: { mode: "fixed" },
  };
  selectedScenarioId.value = newId;
  triggerPreview();
}

async function runPlaytest() {
  await saveScenario();
  const sid = await startPlaytestFromScenario();
  if (sid) {
    router.push("/playtest");
  }
}

function updateQualityOverride(key: string, val: string) {
  if (val.trim() === "") {
    delete activeScenario.initial_qualities[key];
  } else {
    const num = Number(val);
    activeScenario.initial_qualities[key] = isNaN(num) ? val : num;
  }
  triggerPreview();
}

function updateActorOverride(actorId: string, locId: string) {
  if (!locId) {
    delete activeScenario.actor_locations[actorId];
  } else {
    activeScenario.actor_locations[actorId] = locId;
  }
  triggerPreview();
}
</script>

<template>
  <div style="display:flex; flex-direction:column; height:100%;">
    <header class="view-header">
      <div style="display:flex; align-items:center; gap:12px;">
        <h1 class="view-title">Starting Scenarios</h1>
        <select
          :value="selectedScenarioId"
          class="form-select"
          style="width: 220px;"
          @change="selectScenario(scenarios.find(s => s.id === ($event.target as HTMLSelectElement).value)!)"
        >
          <option v-for="s in scenarios" :key="s.id" :value="s.id">
            {{ s.name }} ({{ s.id }})
          </option>
        </select>
        <button class="btn btn-secondary" @click="createNewScenario">+ New Preset</button>
      </div>

      <div style="display:flex; gap:10px;">
        <button class="btn btn-secondary" @click="saveScenario">Save Preset</button>
        <button class="btn btn-primary" @click="runPlaytest">Start Clean Playtest</button>
      </div>
    </header>

    <div class="view-content" style="max-width:860px; margin:0 auto; width:100%;">
      <!-- Basics -->
      <div style="display:grid; grid-template-columns:1fr 1fr; gap:16px; margin-bottom:16px;">
        <div class="form-group">
          <label class="form-label">Scenario ID</label>
          <input v-model="activeScenario.id" class="form-input" placeholder="e.g. first-interview" />
        </div>
        <div class="form-group">
          <label class="form-label">Display Name</label>
          <input v-model="activeScenario.name" class="form-input" @input="triggerPreview" />
        </div>
      </div>

      <div class="form-group">
        <label class="form-label">Random Seed</label>
        <input v-model.number="activeScenario.seed" type="number" class="form-input" style="width:180px;" @input="triggerPreview" />
      </div>

      <!-- Player Start Location Policy -->
      <div style="padding:16px; background:var(--bg-card); border:1px solid var(--border-subtle); border-radius:12px; margin-bottom:16px;">
        <h3 style="font-size:13px; font-weight:700; margin-bottom:12px;">PLAYER START LOCATION</h3>
        <div style="display:grid; grid-template-columns:1fr 1fr; gap:16px;">
          <div>
            <label class="form-label">Selection Mode</label>
            <select v-model="activeScenario.start_location_policy.mode" class="form-select" @change="triggerPreview">
              <option value="fixed">Fixed Location</option>
              <option value="random">Seeded Random</option>
              <option value="by_attribute">Match by Attribute</option>
            </select>
          </div>

          <div v-if="activeScenario.start_location_policy.mode === 'fixed'">
            <label class="form-label">Fixed Location ID</label>
            <select v-model="activeScenario.start_location_policy.location_id" class="form-select" @change="triggerPreview">
              <option value="">Select location...</option>
              <option v-for="l in studioState.locations" :key="l.id" :value="l.id">
                {{ l.name }} ({{ l.id }})
              </option>
            </select>
          </div>

          <div v-else-if="activeScenario.start_location_policy.mode === 'by_attribute'" style="display:flex; gap:10px;">
            <div style="flex:1;">
              <label class="form-label">Attribute Key</label>
              <input v-model="activeScenario.start_location_policy.attribute_key" class="form-input" placeholder="e.g. security" @input="triggerPreview" />
            </div>
            <div style="flex:1;">
              <label class="form-label">Attribute Value</label>
              <input v-model="activeScenario.start_location_policy.attribute_value" class="form-input" placeholder="e.g. high" @input="triggerPreview" />
            </div>
          </div>
        </div>
      </div>

      <!-- Actor Population Policy -->
      <div style="padding:16px; background:var(--bg-card); border:1px solid var(--border-subtle); border-radius:12px; margin-bottom:16px;">
        <h3 style="font-size:13px; font-weight:700; margin-bottom:12px;">ACTOR POPULATION POLICY</h3>
        <div style="display:grid; grid-template-columns:1fr 1fr; gap:16px; margin-bottom:12px;">
          <div>
            <label class="form-label">Population Mode</label>
            <select v-model="activeScenario.actor_population_policy.mode" class="form-select" @change="triggerPreview">
              <option value="all">Include All Project Characters</option>
              <option value="random">Random Subset Count</option>
              <option value="by_attribute">Filter by Character Attribute</option>
            </select>
          </div>

          <div v-if="activeScenario.actor_population_policy.mode === 'random'">
            <label class="form-label">Number of Characters</label>
            <input v-model.number="activeScenario.actor_population_policy.count" type="number" class="form-input" min="0" @input="triggerPreview" />
          </div>

          <div v-else-if="activeScenario.actor_population_policy.mode === 'by_attribute'" style="display:flex; gap:10px;">
            <div style="flex:1;">
              <label class="form-label">Role or Attribute Key</label>
              <input v-model="activeScenario.actor_population_policy.attribute_key" class="form-input" placeholder="role" @input="triggerPreview" />
            </div>
            <div style="flex:1;">
              <label class="form-label">Value</label>
              <input v-model="activeScenario.actor_population_policy.attribute_value" class="form-input" placeholder="investigator" @input="triggerPreview" />
            </div>
          </div>
        </div>

        <div style="border-top:1px solid var(--border-subtle); padding-top:12px;">
          <label class="form-label">Global Actor Placement Policy</label>
          <select
            v-if="activeScenario.actor_population_policy.location_policy"
            v-model="activeScenario.actor_population_policy.location_policy.mode"
            class="form-select"
            style="width:240px;"
            @change="triggerPreview"
          >
            <option value="fixed">Inherit Codex Current / Home</option>
            <option value="random">Seeded Random Distribution</option>
            <option value="by_attribute">Distribute by Location Attribute</option>
          </select>
        </div>
      </div>

      <!-- Initial Qualities Overrides -->
      <div style="padding:16px; background:var(--bg-card); border:1px solid var(--border-subtle); border-radius:12px; margin-bottom:16px;">
        <h3 style="font-size:13px; font-weight:700; margin-bottom:12px;">INITIAL QUALITY OVERRIDES (CODEX)</h3>
        <p style="font-size:12px; color:var(--text-muted); margin-bottom:12px;">Leave blank to inherit default value from Story Codex.</p>

        <div style="display:grid; grid-template-columns:repeat(auto-fill, minmax(240px, 1fr)); gap:10px;">
          <div v-for="q in studioState.qualities" :key="q.name" style="padding:8px 10px; background:var(--bg-input); border-radius:8px; border:1px solid var(--border-subtle);">
            <div style="font-weight:600; font-size:12px;">{{ q.name }}</div>
            <div style="font-size:11px; color:var(--text-muted); margin-bottom:4px;">Codex Default: {{ q.value }}</div>
            <input
              :value="activeScenario.initial_qualities[q.name] ?? ''"
              class="form-input"
              placeholder="Override value"
              @input="updateQualityOverride(q.name, ($event.target as HTMLInputElement).value)"
            />
          </div>
        </div>
      </div>

      <!-- Individual Character Location Overrides -->
      <div style="padding:16px; background:var(--bg-card); border:1px solid var(--border-subtle); border-radius:12px; margin-bottom:16px;">
        <h3 style="font-size:13px; font-weight:700; margin-bottom:12px;">INDIVIDUAL CHARACTER PLACEMENTS</h3>
        <p style="font-size:12px; color:var(--text-muted); margin-bottom:12px;">Specific location overrides take precedence over global population policies.</p>

        <div style="display:grid; grid-template-columns:repeat(auto-fill, minmax(260px, 1fr)); gap:10px;">
          <div v-for="c in studioState.characters" :key="c.id" style="padding:8px 10px; background:var(--bg-input); border-radius:8px; border:1px solid var(--border-subtle);">
            <div style="font-weight:600; font-size:12px;">{{ c.name }} ({{ c.role }})</div>
            <div style="font-size:11px; color:var(--text-muted); margin-bottom:4px;">Codex: {{ c.current_location || c.home_location }}</div>
            <select
              :value="activeScenario.actor_locations[c.id] ?? ''"
              class="form-select"
              @change="updateActorOverride(c.id, ($event.target as HTMLSelectElement).value)"
            >
              <option value="">Inherit Policy</option>
              <option v-for="l in studioState.locations" :key="l.id" :value="l.id">
                {{ l.name }}
              </option>
            </select>
          </div>
        </div>
      </div>

      <!-- Preview Agreement Card -->
      <div v-if="preview" style="padding:16px; background:var(--bg-card); border:1px solid var(--border-subtle); border-radius:12px; margin-bottom:32px;">
        <h3 style="font-size:13px; font-weight:700; margin-bottom:12px; color:var(--accent);">RESOLVED PREVIEW AGREEMENT</h3>
        <div style="display:grid; grid-template-columns:1fr 1fr; gap:14px; font-size:12px;">
          <div>
            <div style="color:var(--text-muted);">Resolved Player Start:</div>
            <div style="font-weight:600; margin-top:2px;">{{ preview.player_start_location || "Unresolved" }}</div>
          </div>
          <div>
            <div style="color:var(--text-muted);">Active Actors:</div>
            <div style="font-weight:600; margin-top:2px;">{{ preview.active_actor_ids.length }} of {{ studioState.characters.length }} characters</div>
          </div>
        </div>

        <div style="margin-top:14px;">
          <div style="font-weight:600; font-size:11px; color:var(--text-muted); margin-bottom:6px;">ACTOR DISTRIBUTION PER LOCATION</div>
          <div style="display:flex; flex-wrap:wrap; gap:8px;">
            <span
              v-for="(actors, locId) in preview.actors_per_location"
              :key="locId"
              class="badge"
              style="padding:4px 8px;"
            >
              {{ locId }}: {{ actors.length }} actor{{ actors.length === 1 ? '' : 's' }}
            </span>
          </div>
        </div>

        <div v-if="preview.issues.length > 0" style="margin-top:14px; padding:10px 14px; background:rgba(255,59,48,0.1); border-radius:8px; color:var(--status-danger); font-size:12px;">
          <strong>Configuration Warnings:</strong>
          <ul style="margin-left:16px; margin-top:4px;">
            <li v-for="(iss, idx) in preview.issues" :key="idx">{{ iss }}</li>
          </ul>
        </div>
      </div>
    </div>
  </div>
</template>
