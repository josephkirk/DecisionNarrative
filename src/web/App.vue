<script setup lang="ts">
import { onMounted } from "vue";
import { NAVIGATION_ITEMS } from "./studio/definitions.ts";
import { studioState, notify } from "./studio/state.ts";
import { StudioApi } from "./studio/api.ts";

onMounted(async () => {
  try {
    const [project, options] = await Promise.all([
      StudioApi.getProject(),
      StudioApi.getAuthoringOptions(),
    ]);
    studioState.project = project;
    studioState.authoringOptions = options;
  } catch (e) {
    notify(e instanceof Error ? e.message : String(e), "error");
  }
});
</script>

<template>
  <div class="studio-shell">
    <aside class="studio-sidebar">
      <div>
        <div class="brand-section">
          <div class="brand-title">
            <span>DecisionNarrative</span>
          </div>
          <div class="brand-subtitle">Storylet Studio</div>
        </div>

        <nav class="nav-list">
          <router-link
            v-for="item in NAVIGATION_ITEMS"
            :key="item.id"
            :to="item.path"
            class="nav-link"
          >
            <span class="nav-icon">{{ item.icon }}</span>
            <span>{{ item.label }}</span>
          </router-link>
        </nav>
      </div>

      <div class="sidebar-footer">
        <div>{{ studioState.project?.name ?? "Workspace" }} · rev {{ studioState.project?.revision ?? 0 }}</div>
      </div>
    </aside>

    <main class="studio-main">
      <router-view />

      <div
        v-if="studioState.notification"
        :class="['notification-toast', studioState.notification.type]"
      >
        <span>{{ studioState.notification.message }}</span>
      </div>
    </main>
  </div>
</template>
