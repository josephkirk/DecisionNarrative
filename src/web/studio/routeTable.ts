import type { RouteRecordRaw } from "vue-router";

// Components load lazily so this table can be imported (and tested) without a Vue compiler.
export const STUDIO_ROUTES: RouteRecordRaw[] = [
  { path: "/", redirect: "/storylets" },
  { path: "/browse/:root/:folderId?", name: "browse", component: () => import("./views/FolderView.vue") },
  { path: "/storylets", name: "storylets", component: () => import("./views/LibraryView.vue") },
  { path: "/storylets/new", name: "new-storylet", component: () => import("./views/StoryletEditorView.vue") },
  { path: "/storylets/:id/edit", name: "edit-storylet", component: () => import("./views/StoryletEditorView.vue") },
  { path: "/entities/:id", name: "entity", component: () => import("./views/EntityView.vue") },
  { path: "/entity-templates/:id", name: "entity-template", component: () => import("./views/EntityView.vue") },
  { path: "/trait-templates/:id", name: "trait-template", component: () => import("./views/TraitTemplateView.vue") },
  { path: "/assets/:id", name: "asset", component: () => import("./views/AssetView.vue") },
  { path: "/codex", name: "codex", component: () => import("./views/CodexView.vue") },
  { path: "/plugins", name: "plugins", component: () => import("./views/PluginsView.vue") },
  { path: "/project", name: "project", component: () => import("./views/ProjectView.vue") },
  { path: "/tags", name: "tags", component: () => import("./views/TagManagerView.vue") },
  { path: "/play", name: "play", component: () => import("./views/PlayView.vue") },
  { path: "/play/:runId", name: "play-run", component: () => import("./views/PlayView.vue") },
];
