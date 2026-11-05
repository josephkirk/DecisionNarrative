import { createRouter, createWebHashHistory } from "vue-router";

import LibraryView from "./views/LibraryView.vue";
import StoryletEditorView from "./views/StoryletEditorView.vue";
import CodexView from "./views/CodexView.vue";
import ProjectView from "./views/ProjectView.vue";
import TagManagerView from "./views/TagManagerView.vue";
import FolderView from "./views/FolderView.vue";
import EntityView from "./views/EntityView.vue";
import TraitTemplateView from "./views/TraitTemplateView.vue";
import AssetView from "./views/AssetView.vue";

export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", redirect: "/storylets" },
    { path: "/browse/:root/:folderId?", name: "browse", component: FolderView },
    { path: "/storylets", name: "storylets", component: LibraryView },
    { path: "/storylets/new", name: "new-storylet", component: StoryletEditorView },
    { path: "/storylets/:id/edit", name: "edit-storylet", component: StoryletEditorView },
    { path: "/entities/:id", name: "entity", component: EntityView },
    { path: "/entity-templates/:id", name: "entity-template", component: EntityView },
    { path: "/trait-templates/:id", name: "trait-template", component: TraitTemplateView },
    { path: "/assets/:id", name: "asset", component: AssetView },
    { path: "/codex", name: "codex", component: CodexView },
    { path: "/project", name: "project", component: ProjectView },
    { path: "/tags", name: "tags", component: TagManagerView },
  ],
});
