import { createRouter, createWebHashHistory } from "vue-router";

import { STUDIO_ROUTES } from "./routeTable.ts";

export const router = createRouter({
  history: createWebHashHistory(),
  routes: STUDIO_ROUTES,
});
