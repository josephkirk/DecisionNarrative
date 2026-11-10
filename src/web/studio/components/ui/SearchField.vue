<script setup lang="ts">
import StudioIcon from "../StudioIcon.vue";

/** Search input with a leading magnifier. Root attributes (class, style) land on the wrapper. */
const model = defineModel<string>({ required: true });
defineProps<{ label: string; placeholder?: string; inputId?: string }>();
</script>

<template>
  <div class="ui-search">
    <StudioIcon name="search" class="ui-search__icon" />
    <input :id="inputId" v-model="model" class="form-input ui-search__input" type="search" :placeholder="placeholder" :aria-label="label" />
  </div>
</template>
