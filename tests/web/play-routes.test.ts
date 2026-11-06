import { expect, test } from "bun:test";
import { STUDIO_ROUTES } from "../../src/web/studio/routeTable.ts";
import { NAVIGATION_ITEMS } from "../../src/web/studio/definitions.ts";

test("router exposes play and play-run routes", () => {
  const byName = new Map(STUDIO_ROUTES.map((r) => [r.name, r]));
  expect(byName.get("play")?.path).toBe("/play");
  expect(byName.get("play-run")?.path).toBe("/play/:runId");
});

test("play is the toolbar primary action, not a navigation link", () => {
  expect(NAVIGATION_ITEMS.some((item) => item.id === "play" || item.path.startsWith("/play"))).toBe(false);
});

test("existing studio routes are unchanged", () => {
  const names = STUDIO_ROUTES.map((r) => r.name).filter(Boolean);
  for (const n of ["storylets", "edit-storylet", "codex", "project", "tags", "entity"]) expect(names).toContain(n);
});
