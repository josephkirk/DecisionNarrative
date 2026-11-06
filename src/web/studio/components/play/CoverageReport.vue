<script setup lang="ts">
import { computed, ref } from "vue";
import type { CoverageCell, CoverageReport, CoverageViolation } from "../../contracts.ts";
import { endingBars } from "../../runModel.ts";

const props = defineProps<{ report: CoverageReport | null; busy: boolean }>();
const emit = defineEmits<{
  (event: "run", runs: number, seed?: string): void;
  (event: "replay", violation: CoverageViolation): void;
  (event: "close"): void;
}>();
const runs = ref(2000);
const seed = ref("");
const seedValid = computed(() => seed.value.trim() === "" || (/^\d+$/.test(seed.value.trim()) && BigInt(seed.value.trim()) <= 18446744073709551615n));
const valid = computed(() => Number.isInteger(runs.value) && runs.value >= 1 && runs.value <= 50000 && seedValid.value);
const cellLabel = (cell: CoverageCell) => `${cell.persona.kind === "wanderer" ? "Wanderer" : "Investigator"} · ${cell.config === "solo" ? "Solo" : "Co-op"}`;
const storyletName = (id: number) => props.report?.storylets[String(id)]?.title ?? `Storylet ${id}`;
const locationName = (id: number) => props.report?.locations[String(id)] ?? `Location ${id}`;
const percent = (count: number, total: number) => total ? `${(count * 100 / total).toFixed(1)}%` : "0.0%";
function submit() {
  if (!valid.value || props.busy) return;
  emit("run", runs.value, seed.value.trim() === "" ? undefined : seed.value.trim());
}
</script>

<template>
  <section class="coverage" aria-labelledby="coverage-title">
    <header class="coverage__header">
      <div><h2 id="coverage-title" class="play-title">Coverage</h2><p class="play-muted">Explore possible paths with seeded bots.</p></div>
      <button type="button" class="btn btn-secondary play-btn" :disabled="busy" @click="emit('close')">Close</button>
    </header>
    <form class="coverage__form" @submit.prevent="submit">
      <div class="play-field"><label for="coverage-runs">Runs per configuration</label><input id="coverage-runs" v-model.number="runs" class="form-input" type="number" min="1" max="50000" step="1" required /></div>
      <div class="play-field"><label for="coverage-seed">Seed (blank for scenario default)</label><input id="coverage-seed" v-model="seed" class="form-input" inputmode="numeric" /></div>
      <button type="submit" class="btn btn-primary play-btn" :disabled="busy || !valid">{{ busy ? 'Exploring…' : 'Run exploration' }}</button>
    </form>
    <p v-if="!valid" class="play-error" role="alert">Choose 1–50,000 runs and a non-negative whole-number seed.</p>
    <p v-if="busy" role="status" aria-live="polite">Bots are exploring. This may take a while.</p>
    <template v-if="report">
      <p class="play-muted" aria-live="polite">{{ report.runs.toLocaleString() }} {{ report.runs === 1 ? 'run' : 'runs' }} per configuration · seed {{ report.seed }} · content revision {{ report.content_revision }}</p>
      <div v-if="report.error_count || report.determinism_mismatches" class="coverage__warning" role="alert">
        Results are incomplete: {{ report.error_count }} run errors and {{ report.determinism_mismatches }} replay mismatches.
        <ul v-if="report.errors.length"><li v-for="(error, i) in report.errors" :key="i">Seed {{ error.seed }}, step {{ error.step }}: {{ error.message }}</li></ul>
      </div>
      <section class="coverage__invariants" :class="{ 'coverage__invariants--fail': report.violations.length }" aria-labelledby="coverage-invariants">
        <h3 id="coverage-invariants">Invariants</h3>
        <p>{{ report.violations.length }} invariant {{ report.violations.length === 1 ? 'violation' : 'violations' }}{{ report.violations.length === 20 ? ' (first 20 shown)' : '' }}</p>
        <ul v-if="report.violations.length" class="coverage__list">
          <li v-for="(violation, index) in report.violations" :key="index" class="coverage__violation">
            <span>{{ violation.invariant.kind.replaceAll('_', ' ') }} · {{ violation.config === 'solo' ? 'Solo' : 'Co-op' }} · seed {{ violation.seed }} · step {{ violation.step }}</span>
            <button type="button" class="btn btn-secondary play-btn" :disabled="busy" @click="emit('replay', violation)">Replay seed</button>
          </li>
        </ul>
      </section>
      <section v-for="(cell, index) in report.cells" :key="index" class="coverage__cell">
        <h3>{{ cellLabel(cell) }}</h3>
        <h4>Endings</h4>
        <p v-if="!cell.endings.length" class="play-muted">No endings reached.</p>
        <p v-if="cell.endings.length" class="play-muted">Percentages are among runs that reached an ending.</p>
        <ul v-if="cell.endings.length" class="coverage__list">
          <li v-for="bar in endingBars(cell.endings)" :key="bar.id" class="coverage__bar-row">
            <div><span>{{ storyletName(bar.id) }}</span><span>{{ bar.percent.toFixed(1) }}% ({{ bar.count }})</span></div>
            <div class="coverage__bar" aria-hidden="true"><span :style="{ width: `${bar.percent}%` }"></span></div>
          </li>
        </ul>
        <h4>Milestones</h4>
        <p v-if="!cell.milestone_reached.length" class="play-muted">No milestones declared.</p>
        <ul v-else class="coverage__list"><li v-for="[key, count] in cell.milestone_reached" :key="key">{{ report.milestones.find(m => m.key === key)?.name || key }}: {{ percent(count, report.runs) }} ({{ count }} / {{ report.runs }})</li></ul>
        <h4>Intent probes</h4>
        <p v-if="!cell.probe_hits.length" class="play-muted">No intent probes declared.</p>
        <ul v-else class="coverage__list"><li v-for="[name, hits, filled] in cell.probe_hits" :key="name">{{ name }}: {{ filled }} / {{ hits }} opportunities filled <template v-for="id in report.probes.find(p => p.name === name)?.expected_routes ?? []" :key="id"><router-link v-if="report.storylets[String(id)]?.key" :to="`/storylets/${encodeURIComponent(report.storylets[String(id)].key!)}/edit`">Open {{ storyletName(id) }}</router-link></template></li></ul>
        <h4>Dead air by location</h4>
        <p v-if="!cell.dead_air_by_location.length" class="play-muted">No locations observed.</p>
        <ul v-else class="coverage__list"><li v-for="[id, empty, total] in cell.dead_air_by_location" :key="id">{{ locationName(id) }}: {{ percent(empty, total) }} ({{ empty }} / {{ total }})</li></ul>
      </section>
    </template>
  </section>
</template>
