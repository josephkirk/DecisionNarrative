<script setup lang="ts">
import { computed, onBeforeUnmount, provide, ref, shallowReactive } from "vue";
import {
  createMemoryHistory,
  createRouter,
  routeLocationKey,
  routerKey,
  routerViewLocationKey,
  RouterView,
  START_LOCATION,
  useRouter,
  type RouteLocationNormalizedLoaded,
  type Router,
} from "vue-router";
import { STUDIO_ROUTES } from "../routeTable.ts";
import { dropSide, MAIN_PANE, type DropSide, type PaneLeaf } from "../paneModel.ts";
import {
  canSplit,
  closeStudioPane,
  focusPane,
  isTreeDrag,
  openSplit,
  paneCount,
  paneHeading,
  paneState,
  recordPanePath,
  registerPaneRouter,
  toggleMaximize,
  treeDragPath,
  unregisterPaneRouter,
} from "../view-models/panes.ts";
import StudioIcon from "./StudioIcon.vue";
import IconButton from "./ui/IconButton.vue";

const props = defineProps<{ pane: PaneLeaf }>();
const isMain = props.pane.id === MAIN_PANE;

function isolatedRouter(path: string): Router {
  const history = createMemoryHistory();
  // Links inside a side pane still open the right studio page in a new browser tab.
  history.createHref = (location: string) => `${window.location.pathname}#${location}`;
  const router = createRouter({ history, routes: STUDIO_ROUTES });
  const route = {} as Record<string, unknown>;
  for (const key of Object.keys(START_LOCATION)) {
    Object.defineProperty(route, key, { get: () => router.currentRoute.value[key as keyof RouteLocationNormalizedLoaded], enumerable: true });
  }
  provide(routerKey, router);
  provide(routeLocationKey, shallowReactive(route) as unknown as RouteLocationNormalizedLoaded);
  provide(routerViewLocationKey, router.currentRoute);
  router.afterEach((to, _from, failure) => { if (!failure) recordPanePath(props.pane.id, to.fullPath); });
  void router.push(path || "/storylets");
  return router;
}

const router = isMain ? useRouter() : isolatedRouter(props.pane.path);
registerPaneRouter(props.pane.id, router);
onBeforeUnmount(() => unregisterPaneRouter(props.pane.id));

const heading = computed(() => paneHeading(router.currentRoute.value));
const split = computed(() => paneCount() > 1);
const active = computed(() => paneState.activeId === props.pane.id);
const maximized = computed(() => paneState.maximizedId === props.pane.id);

const zone = ref<DropSide | "center" | null>(null);
const body = ref<HTMLElement | null>(null);

function onDragOver(event: DragEvent) {
  if (!isTreeDrag(event) || event.defaultPrevented || !body.value) { zone.value = null; return; }
  event.preventDefault();
  const rect = body.value.getBoundingClientRect();
  const side = dropSide(event.clientX - rect.left, event.clientY - rect.top, rect.width, rect.height);
  zone.value = side !== "center" && !canSplit() ? "center" : side;
  if (event.dataTransfer) event.dataTransfer.dropEffect = "copy";
}
function onDragLeave(event: DragEvent) {
  if (!(event.relatedTarget instanceof Node) || !body.value?.contains(event.relatedTarget)) zone.value = null;
}
function onDrop(event: DragEvent) {
  const side = zone.value;
  zone.value = null;
  if (!side || event.defaultPrevented) return;
  const path = treeDragPath(event);
  if (!path) return;
  event.preventDefault();
  focusPane(props.pane.id);
  if (side === "center") void router.push(path);
  else openSplit(props.pane.id, side, path);
}

function splitHere(side: DropSide) {
  openSplit(props.pane.id, side, router.currentRoute.value.fullPath);
}
</script>

<template>
  <section
    class="studio-pane"
    :class="{ 'is-main': isMain, 'is-active': active && split, 'is-maximized': maximized, 'is-split': split }"
    :data-pane-id="pane.id"
    :aria-label="`${heading.title} pane`"
    @pointerdown.capture="focusPane(pane.id)"
    @focusin="focusPane(pane.id)"
  >
    <header class="studio-pane__head">
      <span class="studio-pane__title"><StudioIcon :name="heading.icon" /><span>{{ heading.title }}</span></span>
      <div class="studio-pane__actions">
        <IconButton icon="split-right" label="Split right" hint="or drag a project item onto a pane edge" size="sm" :disabled="!canSplit()" @click="splitHere('right')" />
        <IconButton icon="split-down" label="Split down" hint="or drag a project item onto a pane edge" size="sm" :disabled="!canSplit()" @click="splitHere('bottom')" />
        <template v-if="split">
          <IconButton :icon="maximized ? 'minimize' : 'maximize'" :label="maximized ? 'Restore pane' : 'Maximize pane'" size="sm" :aria-pressed="maximized" @click="toggleMaximize(pane.id)" />
          <IconButton icon="close" label="Close pane" size="sm" tone="danger" @click="closeStudioPane(pane.id)" />
        </template>
      </div>
    </header>
    <div ref="body" class="studio-pane__body" @dragover="onDragOver" @dragleave="onDragLeave" @drop="onDrop">
      <router-view v-if="isMain" :key="$route.fullPath" />
      <RouterView v-else v-slot="{ Component, route }">
        <component :is="Component" v-if="Component" :key="route.fullPath" />
      </RouterView>
      <div v-if="zone" class="studio-pane__drop" :class="`is-${zone}`" aria-hidden="true">
        <span>{{ zone === 'center' ? 'Open here' : `Split ${zone}` }}</span>
      </div>
    </div>
  </section>
</template>
