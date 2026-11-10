<script setup lang="ts">
import StudioIcon from "../StudioIcon.vue";

/**
 * Square ghost button carrying only an icon. `label` is the accessible name and
 * the tooltip; `hint` extends the tooltip (e.g. a gesture shortcut). `danger`
 * stays quiet until hovered; `reveal` keeps the button faint until its row or
 * card is hovered or focused.
 */
withDefaults(defineProps<{ icon: string; label: string; hint?: string; tone?: "default" | "danger"; size?: "sm" | "md"; reveal?: boolean }>(), {
  tone: "default",
  size: "md",
  reveal: false,
});
</script>

<template>
  <button
    type="button"
    class="ui-icon-btn"
    :class="[`ui-icon-btn--${size}`, { 'ui-icon-btn--danger': tone === 'danger', 'ui-icon-btn--reveal': reveal }]"
    :aria-label="label"
    :title="hint ? `${label} — ${hint}` : label"
  >
    <StudioIcon :name="icon" />
  </button>
</template>
