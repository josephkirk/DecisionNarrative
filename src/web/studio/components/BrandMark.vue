<script setup lang="ts">
/**
 * Erzählstudio mark: an "E" whose middle arm forks into two outcomes (a told story
 * with its moment of choice). Same geometry as src/web/assets/erzaehlstudio-logo.svg,
 * but painted with theme tokens so it follows the active theme.
 */
</script>

<template>
  <svg class="brand-mark" viewBox="0 0 64 64" aria-hidden="true" focusable="false">
    <rect x="2.75" y="2.75" width="58.5" height="58.5" rx="13.25" class="brand-mark__tile" />
    <g class="brand-mark__letter">
      <path d="M21 17v30" />
      <path d="M21 17h21" />
      <path d="M21 47h21" />
    </g>
    <g class="brand-mark__fork">
      <path d="M21 32h8" />
      <path d="M29 32c4 0 5-5 9-5" />
      <path d="M29 32c4 0 5 5 9 5" />
    </g>
    <circle cx="41" cy="27" r="3.25" class="brand-mark__node" />
    <circle cx="41" cy="37" r="3.25" class="brand-mark__node brand-mark__node--open" />
  </svg>
</template>

<style scoped>
.brand-mark { display: block; width: 28px; height: 28px; flex: none; }
.brand-mark__tile { fill: var(--bg-app); stroke: color-mix(in srgb, var(--accent) 55%, transparent); stroke-width: 1.5; }
.brand-mark__letter { fill: none; stroke: var(--text-main); stroke-width: 4.5; stroke-linecap: round; }
.brand-mark__fork { fill: none; stroke: var(--accent); stroke-width: 3.5; stroke-linecap: round; stroke-linejoin: round; }
.brand-mark__node { fill: var(--accent); }
.brand-mark__node--open { fill: none; stroke: var(--accent); stroke-width: 2; }
</style>
