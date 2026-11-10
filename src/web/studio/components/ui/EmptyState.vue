<script setup lang="ts">
import StudioIcon from "../StudioIcon.vue";

/** Centered zero-data message: icon, short title, optional hint, and the obvious next action in the slot. */
defineProps<{ icon: string; title: string; hint?: string }>();
</script>

<template>
  <div class="ui-empty">
    <span class="ui-empty__icon" aria-hidden="true"><StudioIcon :name="icon" /></span>
    <p class="ui-empty__title">{{ title }}</p>
    <p v-if="hint" class="ui-empty__hint">{{ hint }}</p>
    <div v-if="$slots.default" class="ui-empty__actions"><slot /></div>
  </div>
</template>
