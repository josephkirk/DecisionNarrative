<script setup lang="ts">
import { computed, onMounted, provide, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { NAVIGATION_ITEMS } from "./studio/definitions.ts";
import { studioState, notify } from "./studio/state.ts";
import { StudioApi } from "./studio/api.ts";
import { buildInteractionContext, interactionContextKey, type InteractionFocus } from "./studio/interactionContext.ts";

const route = useRoute();
const interactionFocus = ref<InteractionFocus | null>(null);
watch(() => route.fullPath, () => { interactionFocus.value = null; });
const interactionContext = computed(() => buildInteractionContext({
  route: { name: route.name, path: route.path, fullPath: route.fullPath, params: route.params, query: route.query },
  project: studioState.project,
  activePlaytestSessionId: studioState.activePlaytestSessionId,
  activePlaytestSnapshot: studioState.activePlaytestSnapshot,
  focus: interactionFocus.value,
}));
function setInteractionFocus(focus: InteractionFocus | null) {
  interactionFocus.value = focus;
}
provide(interactionContextKey, { context: interactionContext, setFocus: setInteractionFocus });
let contextPublishTimer: ReturnType<typeof setTimeout> | undefined;
let contextPublishErrorLogged = false;
let contextPublishQueue: Promise<unknown> = Promise.resolve();
watch(interactionContext, (context) => {
  const payload = JSON.stringify(context);
  if (contextPublishTimer) clearTimeout(contextPublishTimer);
  contextPublishTimer = setTimeout(() => {
    contextPublishQueue = contextPublishQueue
      .then(() => StudioApi.publishInteractionContext(JSON.parse(payload)))
      .then(() => { contextPublishErrorLogged = false; })
      .catch((error) => {
        if (!contextPublishErrorLogged) console.error("Unable to publish studio interaction context:", error);
        contextPublishErrorLogged = true;
      });
  }, 120);
}, { deep: true, flush: "post" });

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
