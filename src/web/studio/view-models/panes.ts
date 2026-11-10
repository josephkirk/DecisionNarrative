import { reactive, shallowReactive, watch } from "vue";
import { isNavigationFailure, NavigationFailureType, type RouteLocationNormalizedLoaded, type Router } from "vue-router";
import { itemRoute } from "../projectTree.ts";
import {
  closePane,
  findPane,
  MAIN_PANE,
  MAX_PANES,
  panes,
  parseLayout,
  setPanePath,
  setRatio,
  singlePane,
  splitPane,
  type DropSide,
  type PaneNode,
} from "../paneModel.ts";
import { studioState } from "../state.ts";

const STORAGE_KEY = "dn.studio.panes";
export const TREE_DRAG_TYPE = "application/x-dn-tree";

function readLayout(): PaneNode {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    return (stored && parseLayout(JSON.parse(stored))) || singlePane();
  } catch {
    return singlePane();
  }
}

export const paneState = reactive<{ layout: PaneNode; activeId: string; maximizedId: string | null }>({
  layout: readLayout(),
  activeId: MAIN_PANE,
  maximizedId: null,
});

watch(() => paneState.layout, (layout) => {
  try { localStorage.setItem(STORAGE_KEY, JSON.stringify(layout)); } catch { /* Layout persistence is best effort. */ }
}, { deep: true });

/** Routers by pane id; the main pane registers the application router. */
const routers = shallowReactive(new Map<string, Router>());

export function registerPaneRouter(id: string, router: Router) { routers.set(id, router); }
export function unregisterPaneRouter(id: string) { routers.delete(id); }

export function paneCount(): number { return panes(paneState.layout).length; }
export function canSplit(): boolean { return paneCount() < MAX_PANES; }

export function focusPane(id: string) {
  if (findPane(paneState.layout, id)) paneState.activeId = id;
}

/** Phones show only the main pane, so navigation must land there. */
const phone = typeof window !== "undefined" && typeof window.matchMedia === "function" ? window.matchMedia("(max-width: 768px)") : null;

function activeRouter(): Router | undefined {
  if (phone?.matches) return routers.get(MAIN_PANE);
  return routers.get(paneState.activeId) ?? routers.get(MAIN_PANE);
}

/** Router-shaped facade for code that should navigate whichever pane has focus. */
export const focusedRouter = {
  push: (to: Parameters<Router["push"]>[0]) => activeRouter()?.push(to) ?? Promise.resolve(),
  replace: (to: Parameters<Router["replace"]>[0]) => activeRouter()?.replace(to) ?? Promise.resolve(),
};

export function focusedRoute(): RouteLocationNormalizedLoaded | undefined {
  return activeRouter()?.currentRoute.value;
}

export function recordPanePath(id: string, path: string) {
  if (id !== MAIN_PANE) paneState.layout = setPanePath(paneState.layout, id, path);
}

let sequence = 0;
function newPaneId(): string {
  sequence += 1;
  return `pane-${Date.now().toString(36)}-${sequence}`;
}

/** Opens `path` beside `targetId`. Returns the new pane id, or null at the pane limit. */
export function openSplit(targetId: string, side: DropSide, path: string): string | null {
  if (!canSplit()) return null;
  const id = newPaneId();
  paneState.layout = splitPane(paneState.layout, targetId, side, path, id);
  paneState.maximizedId = null;
  paneState.activeId = id;
  return id;
}

export function resizeSplit(splitId: string, ratio: number) {
  paneState.layout = setRatio(paneState.layout, splitId, ratio);
}

export function toggleMaximize(id: string) {
  paneState.maximizedId = paneState.maximizedId === id ? null : id;
}

/**
 * Closes a pane after its view agrees to leave (unsaved-change guards run).
 * Closing the main pane moves the first other pane's route into it.
 */
export async function closeStudioPane(id: string): Promise<void> {
  const others = panes(paneState.layout).filter((pane) => pane.id !== MAIN_PANE);
  if (!others.length) return;
  if (id === MAIN_PANE) {
    const next = others[0]!;
    const main = routers.get(MAIN_PANE);
    const leaving = routers.get(next.id);
    if (leaving && isNavigationFailure(await leaving.push("/storylets"), NavigationFailureType.aborted)) return;
    if (main && isNavigationFailure(await main.push(next.path || "/storylets"), NavigationFailureType.aborted)) return;
    paneState.layout = closePane(paneState.layout, next.id);
  } else {
    const router = routers.get(id);
    if (router && isNavigationFailure(await router.push("/storylets"), NavigationFailureType.aborted)) return;
    paneState.layout = closePane(paneState.layout, id);
  }
  if (!findPane(paneState.layout, paneState.activeId)) paneState.activeId = MAIN_PANE;
  if (paneState.maximizedId && !findPane(paneState.layout, paneState.maximizedId)) paneState.maximizedId = null;
}

/** Route of a dragged project-tree row, or null when the drag carries something else. */
export function treeDragPath(event: DragEvent): string | null {
  try {
    const payload = JSON.parse(event.dataTransfer?.getData(TREE_DRAG_TYPE) || "null") as { kind?: string; root?: string; id?: string } | null;
    if (!payload?.root || !payload.id) return null;
    return payload.kind === "folder" ? `/browse/${payload.root}/${encodeURIComponent(payload.id)}` : itemRoute(payload.root, payload.id);
  } catch {
    return null;
  }
}

export function isTreeDrag(event: DragEvent): boolean {
  return !!event.dataTransfer?.types.includes(TREE_DRAG_TYPE);
}

const STATIC_TITLES: Record<string, string> = {
  storylets: "Storylets",
  "new-storylet": "New storylet",
  codex: "Rules & events",
  plugins: "Plugins",
  project: "Project & export",
  tags: "Tag manager",
  play: "Play",
  "play-run": "Play",
};

const ROOT_TITLES: Record<string, string> = {
  storylets: "Storylets",
  entities: "Entities",
  entity_templates: "Entity templates",
  trait_templates: "Traits",
  assets: "Assets",
};

/** Human title and icon for the route a pane shows. */
export function paneHeading(route: RouteLocationNormalizedLoaded | undefined): { title: string; icon: string } {
  if (!route?.name) return { title: "Loading…", icon: "file" };
  const name = String(route.name);
  const id = typeof route.params.id === "string" ? route.params.id : "";
  switch (name) {
    case "entity":
      return { title: studioState.entities.find((entry) => entry.id === id)?.name || "Entity", icon: id === "world" ? "globe" : "entities" };
    case "entity-template":
      return { title: studioState.entityTemplates.find((entry) => entry.id === id)?.name || "Entity template", icon: "entity_templates" };
    case "trait-template":
      return { title: studioState.traitTemplates.find((entry) => entry.id === id)?.name || "Trait", icon: "trait_templates" };
    case "asset":
      return { title: studioState.assets.find((entry) => entry.id === id)?.name || "Asset", icon: "assets" };
    case "edit-storylet":
      return { title: studioState.storylets.find((entry) => entry.id === id)?.title || "Storylet", icon: "storylets" };
    case "browse": {
      const root = typeof route.params.root === "string" ? route.params.root : "";
      const folderId = typeof route.params.folderId === "string" ? route.params.folderId : "";
      const folder = folderId ? studioState.folders.find((entry) => entry.id === folderId)?.name : "";
      return { title: folder || ROOT_TITLES[root] || "Browse", icon: folderId ? "folder" : root || "folder" };
    }
    case "codex": return { title: STATIC_TITLES[name]!, icon: "scroll" };
    case "plugins": return { title: STATIC_TITLES[name]!, icon: "plug" };
    case "project": return { title: STATIC_TITLES[name]!, icon: "settings" };
    case "tags": return { title: STATIC_TITLES[name]!, icon: "tag" };
    case "play": case "play-run": return { title: "Play", icon: "play" };
    default: return { title: STATIC_TITLES[name] ?? "Studio", icon: "storylets" };
  }
}
