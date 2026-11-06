<script setup lang="ts">
import { computed } from "vue";
import type { InspectView } from "../../contracts.ts";
import { changeText, chronicleRows, knowKindLabel, knowRowStatus, toldMark } from "../../runModel.ts";
import type { PlayState } from "../../view-models/run.ts";

const props = defineProps<{ inspect: InspectView | null; tab: PlayState["inspectTab"] }>();
const emit = defineEmits<{
  (e: "tab", tab: PlayState["inspectTab"]): void;
  (e: "focus-step", step: number | null): void;
  (e: "close"): void;
}>();

const TABS: Array<{ id: PlayState["inspectTab"]; label: string }> = [
  { id: "knows", label: "Knows" },
  { id: "world", label: "World" },
  { id: "story", label: "Story so far" },
];
const story = computed(() => chronicleRows(props.inspect?.chronicle ?? []));
</script>

<template>
  <aside class="play-inspect" aria-label="Inspect">
    <header class="play-inspect__head">
      <h2 class="play-eyebrow">Inspect</h2>
      <button type="button" class="btn btn-secondary play-btn" aria-label="Close inspect" @click="emit('close')">Close</button>
    </header>

    <div class="play-inspect__tabs" role="tablist" aria-label="Inspect sections">
      <button
        v-for="t in TABS"
        :id="`inspect-tab-${t.id}`"
        :key="t.id"
        type="button"
        role="tab"
        class="play-tab"
        :class="{ 'is-active': tab === t.id }"
        :aria-selected="tab === t.id"
        :aria-controls="`inspect-panel-${t.id}`"
        @click="emit('tab', t.id)"
      >{{ t.label }}</button>
    </div>

    <p v-if="!inspect" class="play-muted" role="status">Loading…</p>

    <section v-else-if="tab === 'knows'" id="inspect-panel-knows" role="tabpanel" aria-labelledby="inspect-tab-knows">
      <p v-if="!inspect.knows.length" class="play-muted">This participant does not know anything yet.</p>
      <ul class="play-knows">
        <li v-for="row in inspect.knows" :key="row.fact" :class="{ 'is-unconfirmed': !row.confirmed }">
          <span class="play-fact">{{ row.fact }}</span>
          <span class="play-kind">{{ knowKindLabel(row.kind) }}</span>
          <span v-if="knowRowStatus(row)" class="play-unconfirmed">{{ knowRowStatus(row) }}</span>
          <button
            v-if="row.sourceStep !== null"
            type="button"
            class="play-source"
            :aria-label="`Show step ${row.sourceStep} on the timeline`"
            @mouseenter="emit('focus-step', row.sourceStep)"
            @mouseleave="emit('focus-step', null)"
            @focus="emit('focus-step', row.sourceStep)"
            @blur="emit('focus-step', null)"
            @click="emit('focus-step', row.sourceStep)"
          >step {{ row.sourceStep }}</button>
        </li>
      </ul>
    </section>

    <section v-else-if="tab === 'world'" id="inspect-panel-world" role="tabpanel" aria-labelledby="inspect-tab-world">
      <h3 class="play-eyebrow">Changed in the last step</h3>
      <p v-if="!inspect.worldChanges.length" class="play-muted">Nothing changed in the last step.</p>
      <ul class="play-changes">
        <li v-for="c in inspect.worldChanges" :key="c.key">{{ changeText(c) }}</li>
      </ul>
      <details class="play-advanced">
        <summary>Advanced</summary>
        <dl>
          <dt>Seed</dt><dd>{{ inspect.advanced.runSeed }}</dd>
          <dt>Content revision</dt><dd>{{ inspect.advanced.contentRevision }}</dd>
          <dt>Current node</dt><dd>{{ inspect.advanced.nodeGuid ?? "none" }}</dd>
          <dt>Pending interaction</dt><dd>{{ inspect.advanced.interaction ?? "none" }}</dd>
        </dl>
      </details>
    </section>

    <section v-else id="inspect-panel-story" role="tabpanel" aria-labelledby="inspect-tab-story">
      <p v-if="!story.length" class="play-muted">This project has no canon events yet.</p>
      <ol class="play-chronicle">
        <li v-for="(row, i) in story" :key="i" :class="row.told === null ? 'is-untold' : 'is-told'">
          <span class="play-told" :aria-label="row.told === null ? 'not told yet' : `told ${row.told}`">{{ row.told === null ? "" : toldMark(row.told) }}</span>
          <span class="play-chron-label">{{ row.label }}</span>
          <span v-if="row.place" class="play-muted">· {{ row.place }}</span>
          <span v-if="row.told === null" class="play-muted">not told yet</span>
        </li>
      </ol>
    </section>
  </aside>
</template>
