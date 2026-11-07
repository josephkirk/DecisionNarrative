<script setup lang="ts">
import { computed } from "vue";
import { pluginManager } from "../plugins/manager.ts";

const plugins = computed(() => pluginManager.list());

function count(n: number, word: string) { return `${n} ${word}${n === 1 ? "" : "s"}`; }
function toggle(name: string, event: Event) { pluginManager.setEnabled(name, (event.target as HTMLInputElement).checked); }
</script>

<template>
  <div style="display:flex; flex-direction:column; height:100%;">
    <header class="view-header">
      <h1 class="view-title">Plugins</h1>
    </header>
    <div class="view-content" style="max-width:760px; margin:0 auto; width:100%;">
      <p style="font-size:12px; color:var(--text-muted); margin-bottom:16px;">
        Turning a plugin off removes its panels, property editors and node types. Storylets that use its nodes cannot be saved while it is off.
      </p>
      <p v-if="!plugins.length" style="color:var(--text-muted);">No plugins installed.</p>
      <article
        v-for="plugin in plugins"
        :key="plugin.name"
        class="plugin-card"
        :data-plugin="plugin.name"
        style="padding:16px; background:var(--bg-card); border:1px solid var(--border-subtle); border-radius:12px; margin-bottom:12px;"
      >
        <div style="display:flex; align-items:center; gap:10px; flex-wrap:wrap;">
          <h3 style="font-size:14px; font-weight:700; margin:0;">{{ plugin.name }}</h3>
          <span v-if="plugin.version" style="font-size:11px; color:var(--text-muted);">v{{ plugin.version }}</span>
          <span v-if="plugin.builtin" style="font-size:11px; color:var(--text-muted);">built-in</span>
          <label style="margin-left:auto; display:flex; align-items:center; gap:6px; font-size:12px;">
            <input type="checkbox" :checked="plugin.enabled" :aria-label="`Enable ${plugin.name}`" @change="toggle(plugin.name, $event)" />
            {{ plugin.enabled ? "Enabled" : "Disabled" }}
          </label>
          <button class="btn btn-secondary" :disabled="!plugin.enabled" @click="pluginManager.reload(plugin.name)">Reload</button>
        </div>
        <p v-if="plugin.description" style="font-size:12px; margin:8px 0 0;">{{ plugin.description }}</p>
        <p style="font-size:11px; color:var(--text-muted); margin:6px 0 0;">
          {{ count(plugin.contributions.panels, "entity panel") }} ·
          {{ count(plugin.contributions.propertyEditors, "property editor") }} ·
          {{ count(plugin.contributions.nodeTypes, "node type") }}
        </p>
        <p v-if="plugin.error" role="alert" style="font-size:12px; color:var(--danger, #c33); margin:6px 0 0;">{{ plugin.error }}</p>
      </article>
    </div>
  </div>
</template>
