<script setup lang="ts">
import { settleTemplatePicker, templatePicker, NO_TEMPLATE_ID } from "../view-models/templatePicker.ts";
import ReferencePicker from "./ReferencePicker.vue";
</script>

<template>
  <ReferencePicker
    :open="templatePicker.open"
    :anchor="templatePicker.anchor"
    title="Choose template"
    :items="templatePicker.items"
    :selected-id="NO_TEMPLATE_ID"
    @select="settleTemplatePicker"
    @close="settleTemplatePicker(null)"
  />
</template>
