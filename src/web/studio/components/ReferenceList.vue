<script setup lang="ts">
import { computed } from "vue";
import { useRouter } from "vue-router";
import type { ContentReference } from "../contracts.ts";

const props = defineProps<{ references: ContentReference[]; loading?: boolean }>();
const router = useRouter();

const GROUPS: Array<{ kind: string; label: string }> = [
  { kind: "storylet", label: "Storylets" },
  { kind: "scenario", label: "Scenarios" },
  { kind: "entity", label: "Entities" },
  { kind: "trait_template", label: "Trait Templates" },
  { kind: "npc_behavior", label: "NPC Rules" },
];

const grouped = computed(() =>
  GROUPS.map((group) => ({
    ...group,
    rows: props.references.filter((reference) => reference.source_kind === group.kind),
  })).filter((group) => group.rows.length > 0),
);

const unknown = computed(() =>
  props.references.filter((reference) => !GROUPS.some((group) => group.kind === reference.source_kind)),
);

function viaLabel(reference: ContentReference): string {
  if (reference.via === "role") return "by role";
  if (reference.via === "tag") return "by tag";
  return "direct";
}

function open(reference: ContentReference) {
  if (reference.source_kind === "storylet") {
    router.push({
      path: `/storylets/${encodeURIComponent(reference.source_id)}/edit`,
      query: reference.node_id ? { node: reference.node_id } : {},
    });
  } else if (reference.source_kind === "scenario") {
    router.push({ path: "/scenarios", query: { id: reference.source_id } });
  } else if (reference.source_kind === "entity") {
    router.push(`/entities/${encodeURIComponent(reference.source_id)}`);
  } else if (reference.source_kind === "trait_template") {
    router.push(`/trait-templates/${encodeURIComponent(reference.source_id)}`);
  } else if (reference.source_kind === "npc_behavior") {
    router.push({ path: "/codex", query: { category: "npc_behaviors", id: reference.source_id } });
  }
}
</script>

<template>
  <div class="reference-list">
    <div v-if="props.loading" class="reference-list__empty">Loading references…</div>
    <div v-else-if="!props.references.length" class="reference-list__empty">No references</div>

    <section v-for="group in grouped" :key="group.kind" class="reference-list__group">
      <h4 class="reference-list__title">{{ group.label }}</h4>
      <button
        v-for="(reference, index) in group.rows"
        :key="`${reference.source_id}-${reference.field}-${index}`"
        class="reference-list__row"
        type="button"
        @click="open(reference)"
      >
        <span class="reference-list__label">{{ reference.node_label || reference.source_label }}</span>
        <span class="reference-list__field">{{ reference.field }}</span>
        <span class="reference-list__via">{{ viaLabel(reference) }}</span>
      </button>
    </section>

    <section v-if="unknown.length" class="reference-list__group">
      <h4 class="reference-list__title">Other</h4>
      <button
        v-for="(reference, index) in unknown"
        :key="`other-${reference.source_id}-${index}`"
        class="reference-list__row"
        type="button"
        @click="open(reference)"
      >
        <span class="reference-list__label">{{ reference.source_label }}</span>
        <span class="reference-list__field">{{ reference.field }}</span>
        <span class="reference-list__via">{{ viaLabel(reference) }}</span>
      </button>
    </section>
  </div>
</template>
