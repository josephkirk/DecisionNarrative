<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, provide, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { NAVIGATION_ITEMS } from "./studio/definitions.ts";
import { studioState, notify } from "./studio/state.ts";
import { StudioApi } from "./studio/api.ts";
import { refreshWorkspace } from "./studio/view-models/workspace.ts";
import ProjectTree from "./studio/components/ProjectTree.vue";
import PaneWorkspace from "./studio/components/PaneWorkspace.vue";
import StudioIcon from "./studio/components/StudioIcon.vue";
import { buildInteractionContext, interactionContextKey, type InteractionFocus } from "./studio/interactionContext.ts";
import { focusedRoute, focusedRouter } from "./studio/view-models/panes.ts";
import AppearancePanel from "./studio/components/AppearancePanel.vue";
import BrandMark from "./studio/components/BrandMark.vue";
import { appearance, applyAppearance, onAppearanceStorageError } from "./studio/view-models/appearance.ts";

applyAppearance();
onAppearanceStorageError((message) => notify(message, "error"));

/** Toolbar destinations open in the focused split pane; modified clicks keep browser behavior. */
function navigate(path: string, event: MouseEvent) {
  if (event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
  event.preventDefault();
  void focusedRouter.push(path);
}
const focusedPath = computed(() => focusedRoute()?.path ?? "");
const isCurrent = (path: string) => focusedPath.value === path || focusedPath.value.startsWith(`${path}/`);

const route = useRoute();
const interactionFocus = ref<InteractionFocus | null>(null);
watch(() => route.fullPath, () => { interactionFocus.value = null; });
const interactionContext = computed(() => buildInteractionContext({
  route: { name: route.name, path: route.path, fullPath: route.fullPath, params: route.params, query: route.query },
  project: studioState.project,
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
// On phones the navigator is an overlay: start closed and close it after each navigation.
const phoneQuery = window.matchMedia("(max-width: 768px)");
const isTreeCollapsed = ref(phoneQuery.matches);
watch(() => route.fullPath, () => { if (phoneQuery.matches) isTreeCollapsed.value = true; });

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
    <div
      class="studio-backdrop"
      :class="{ 'has-image': appearance.backgroundImage, 'is-blurred': appearance.backgroundImage && appearance.blur }"
      :style="appearance.backgroundImage ? { backgroundImage: `url(${appearance.backgroundImage})` } : undefined"
      aria-hidden="true"
    ></div>
    <header class="studio-toolbar">
      <button
        type="button"
        class="sidebar-toggle-btn"
        :aria-label="isTreeCollapsed ? 'Expand project tree' : 'Collapse project tree'"
        :aria-pressed="!isTreeCollapsed"
        :title="isTreeCollapsed ? 'Expand project tree' : 'Collapse project tree'"
        @click="isTreeCollapsed = !isTreeCollapsed"
      >
        <StudioIcon name="sidebar" class="toggle-icon" />
      </button>

      <div class="studio-toolbar__brand">
        <BrandMark />
        <span class="studio-toolbar__title">Erzählstudio</span>
      </div>
      <nav class="studio-toolbar__nav" aria-label="Studio">
        <a
          v-for="item in NAVIGATION_ITEMS"
          :key="item.id"
          :href="`#${item.path}`"
          class="studio-toolbar__link"
          :class="{ 'router-link-active': isCurrent(item.path) }"
          :aria-current="isCurrent(item.path) ? 'page' : undefined"
          @click="navigate(item.path, $event)"
        >
          {{ item.label }}
        </a>
      </nav>

      <AppearancePanel />
      <div class="studio-toolbar__status" :title="`Project revision ${studioState.project?.revision ?? 0}`">
        <span class="studio-toolbar__status-dot" aria-hidden="true"></span>
        {{ studioState.project?.name ?? "Workspace" }} <span class="studio-toolbar__rev">rev {{ studioState.project?.revision ?? 0 }}</span>
      </div>
      <a href="#/play" class="btn btn-primary studio-toolbar__play" aria-label="Play the project" @click="navigate('/play', $event)"><StudioIcon name="play" /> Play</a>
    </header>

    <div class="studio-body" :class="{ 'is-resizing': resizing }">
      <aside
        :class="['project-tree-panel', isTreeCollapsed && 'is-collapsed']"
        :style="{ width: isTreeCollapsed ? '0px' : `${treeWidth}px` }"
        :aria-hidden="isTreeCollapsed"
        :inert="isTreeCollapsed || undefined"
      >
        <ProjectTree />
      </aside>
      <div
        v-if="!isTreeCollapsed"
        class="project-tree-resizer"
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize project tree"
        @mousedown="startResize"
      ></div>

      <main class="studio-main">
        <PaneWorkspace />

        <div
          v-if="studioState.notification"
          :class="['notification-toast', studioState.notification.type]"
          :role="studioState.notification.type === 'error' ? 'alert' : 'status'"
          aria-live="polite"
        >
          <span class="notification-toast__dot" aria-hidden="true"></span>
          <span>{{ studioState.notification.message }}</span>
        </div>
      </main>
    </div>
  </div>
</template>
