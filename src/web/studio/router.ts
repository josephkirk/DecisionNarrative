import { createRouter, createWebHashHistory } from "vue-router";

import LibraryView from "./views/LibraryView.vue";
import StoryletEditorView from "./views/StoryletEditorView.vue";
import CodexView from "./views/CodexView.vue";
import ScenarioView from "./views/ScenarioView.vue";
import PlaytestView from "./views/PlaytestView.vue";
import ProjectView from "./views/ProjectView.vue";
import TagManagerView from "./views/TagManagerView.vue";

export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", redirect: "/storylets" },
    { path: "/storylets", name: "storylets", component: LibraryView },
    { path: "/storylets/new", name: "new-storylet", component: StoryletEditorView },
    { path: "/storylets/:id/edit", name: "edit-storylet", component: StoryletEditorView },
    { path: "/codex", name: "codex", component: CodexView },
    { path: "/scenarios", name: "scenarios", component: ScenarioView },
    { path: "/playtest", name: "playtest", component: PlaytestView },
    { path: "/project", name: "project", component: ProjectView },
    { path: "/tags", name: "tags", component: TagManagerView },
  ],
});
