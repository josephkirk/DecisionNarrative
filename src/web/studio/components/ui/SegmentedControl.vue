<script setup lang="ts" generic="T extends string">
/** Mutually exclusive view switch (Tree / List, Grid / List, categories). Options may carry a count pill. */
export interface SegmentOption<V extends string = string> { value: V; label: string; count?: number }
const model = defineModel<T>({ required: true });
defineProps<{ options: SegmentOption<T>[]; label: string }>();
</script>

<template>
  <div class="tab-nav ui-segmented" role="group" :aria-label="label">
    <button
      v-for="option in options"
      :key="option.value"
      type="button"
      :class="['tab-btn', { active: model === option.value }]"
      :aria-pressed="model === option.value"
      @click="model = option.value"
    >
      {{ option.label }}<span v-if="option.count !== undefined" class="ui-segmented__count">{{ option.count }}</span>
    </button>
  </div>
</template>
