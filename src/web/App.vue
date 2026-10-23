<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, provide, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { NAVIGATION_ITEMS } from "./studio/definitions.ts";
import { studioState, notify } from "./studio/state.ts";
import { StudioApi } from "./studio/api.ts";
import { refreshWorkspace } from "./studio/view-models/workspace.ts";
import ProjectTree from "./studio/components/ProjectTree.vue";
import { buildInteractionContext, interactionContextKey, type InteractionFocus } from "./studio/interactionContext.ts";

const route = useRoute();
const interactionFocus = ref<InteractionFocus | null>(null);
watch(() => route.fullPath, () => { interactionFocus.value = null; });
const interactionContext = computed(() => buildInteractionContext({
  route: { name: route.name, path: route.path, fullPath: route.fullPath, params: route.params, query: route.query },
  project: studioState.project,
  activePlaytestSessionId: studioState.activePlaytestSessionId,
  activeChronoPlaytestSessionId: studioState.activeChronoPlaytestSessionId,
  activeChronoPlaytestSnapshot: studioState.activeChronoPlaytestSnapshot,
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

const TREE_WIDTH_KEY = "dn.studio.treeWidth";
const treeWidth = ref(readTreeWidth());
const resizing = ref(false);

function readTreeWidth(): number {
  try {
    const raw = Number(localStorage.getItem(TREE_WIDTH_KEY));
    if (Number.isFinite(raw) && raw >= 220 && raw <= 520) return raw;
  } catch {
    // Ignore unreadable storage.
  }
  return 300;
}

function startResize(event: MouseEvent) {
  event.preventDefault();
  resizing.value = true;
  window.addEventListener("mousemove", resizeTo);
  window.addEventListener("mouseup", stopResize);
}

function resizeTo(event: MouseEvent) {
  if (!resizing.value) return;
  treeWidth.value = Math.min(520, Math.max(220, event.clientX));
}

function stopResize() {
  resizing.value = false;
  window.removeEventListener("mousemove", resizeTo);
  window.removeEventListener("mouseup", stopResize);
  try {
    localStorage.setItem(TREE_WIDTH_KEY, String(treeWidth.value));
  } catch {
    // Persisting the panel width is best effort.
  }
}

onBeforeUnmount(() => {
  window.removeEventListener("mousemove", resizeTo);
  window.removeEventListener("mouseup", stopResize);
});

onMounted(async () => {
  try {
    const options = await StudioApi.getAuthoringOptions();
    studioState.authoringOptions = options;
  } catch (e) {
    notify(e instanceof Error ? e.message : String(e), "error");
  }
  await refreshWorkspace();
});
</script>

<template>
  <div class="studio-root">
    <header class="studio-toolbar">
      <div class="studio-toolbar__brand">
        <span class="studio-toolbar__title">DecisionNarrative</span>
        <span class="studio-toolbar__subtitle">Storylet Studio</span>
      </div>

      <nav class="studio-toolbar__nav">
        <router-link
          v-for="item in NAVIGATION_ITEMS"
          :key="item.id"
          :to="item.path"
          class="studio-toolbar__link"
        >
          {{ item.label }}
        </router-link>
      </nav>

      <div class="studio-toolbar__status">
        {{ studioState.project?.name ?? "Workspace" }} · rev {{ studioState.project?.revision ?? 0 }}
      </div>
    </header>

    <div class="studio-body">
      <aside class="project-tree-panel" :style="{ width: `${treeWidth}px` }">
        <ProjectTree />
      </aside>
      <div
        class="project-tree-resizer"
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize project tree"
        @mousedown="startResize"
      ></div>

      <main class="studio-main">
        <router-view :key="$route.fullPath" />

        <div
          v-if="studioState.notification"
          :class="['notification-toast', studioState.notification.type]"
        >
          <span>{{ studioState.notification.message }}</span>
        </div>
      </main>
    </div>
  </div>
</template>
