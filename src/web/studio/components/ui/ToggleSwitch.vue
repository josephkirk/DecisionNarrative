<script setup lang="ts">
/** On/off switch with its label; the whole row is the hit target. */
const model = defineModel<boolean>({ required: true });
defineProps<{ label: string; hint?: string; disabled?: boolean }>();
</script>

<template>
  <label class="ui-switch" :class="{ 'is-disabled': disabled }">
    <span class="ui-switch__text">
      <span class="ui-switch__label">{{ label }}</span>
      <span v-if="hint" class="ui-switch__hint">{{ hint }}</span>
    </span>
    <input v-model="model" class="ui-switch__input" type="checkbox" role="switch" :aria-checked="model" :disabled="disabled" />
    <span class="ui-switch__track" aria-hidden="true"><span class="ui-switch__thumb"></span></span>
  </label>
</template>
