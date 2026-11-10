import { expect, test } from "bun:test";
import {
  closePane,
  dropSide,
  MAIN_PANE,
  MAX_PANES,
  paneGeometry,
  panes,
  parseLayout,
  setRatio,
  singlePane,
  splitPane,
  type PaneNode,
} from "../../src/web/studio/paneModel.ts";

test("splitting places the new pane on the requested side", () => {
  const right = splitPane(singlePane(), MAIN_PANE, "right", "/entities/a", "b");
  expect(right).toMatchObject({ kind: "split", direction: "row", children: [{ id: MAIN_PANE }, { id: "b", path: "/entities/a" }] });
  const top = splitPane(singlePane(), MAIN_PANE, "top", "/tags", "c");
  expect(top).toMatchObject({ kind: "split", direction: "column", children: [{ id: "c" }, { id: MAIN_PANE }] });
});

test("splitting stops at the pane limit and ignores unknown or duplicate ids", () => {
  let layout = singlePane();
  for (let i = 1; i < MAX_PANES + 3; i++) layout = splitPane(layout, MAIN_PANE, "right", "/storylets", `p${i}`);
  expect(panes(layout)).toHaveLength(MAX_PANES);
  expect(splitPane(singlePane(), "missing", "left", "/", "x")).toEqual(singlePane());
  const two = splitPane(singlePane(), MAIN_PANE, "left", "/", "x");
  expect(splitPane(two, MAIN_PANE, "left", "/", "x")).toBe(two);
});

test("closing a pane gives its space to the sibling; the main and last pane stay", () => {
  const nested = splitPane(splitPane(singlePane(), MAIN_PANE, "right", "/a", "b"), "b", "bottom", "/c", "c");
  expect(closePane(nested, "c")).toMatchObject({ kind: "split", children: [{ id: MAIN_PANE }, { id: "b" }] });
  expect(closePane(closePane(nested, "c"), "b")).toEqual({ kind: "pane", id: MAIN_PANE, path: "" });
  expect(closePane(nested, MAIN_PANE)).toBe(nested);
  expect(closePane(singlePane(), MAIN_PANE)).toEqual(singlePane());
});

test("split ratios are clamped so no pane collapses", () => {
  const layout = splitPane(singlePane(), MAIN_PANE, "right", "/", "b") as Extract<PaneNode, { kind: "split" }>;
  expect((setRatio(layout, layout.id, 0.01) as typeof layout).ratio).toBe(0.15);
  expect((setRatio(layout, layout.id, 2) as typeof layout).ratio).toBe(0.85);
});

test("stored layouts must be well formed and keep the main pane", () => {
  const valid = splitPane(singlePane(), MAIN_PANE, "right", "/a", "b");
  expect(parseLayout(JSON.parse(JSON.stringify(valid)))).toEqual(valid);
  expect(parseLayout({ kind: "pane", id: "other", path: "" })).toBeNull();
  expect(parseLayout({ kind: "split", id: "s", direction: "row", ratio: 0.5, children: [{ kind: "pane", id: MAIN_PANE, path: "" }, { kind: "pane", id: MAIN_PANE, path: "" }] })).toBeNull();
  expect(parseLayout({ kind: "split", id: "s", direction: "diagonal", ratio: 0.5, children: [] })).toBeNull();
  expect(parseLayout("nonsense")).toBeNull();
});

test("drop zones: outer quarter splits toward that edge, the middle opens in place", () => {
  expect(dropSide(10, 200, 400, 400)).toBe("left");
  expect(dropSide(395, 200, 400, 400)).toBe("right");
  expect(dropSide(200, 5, 400, 400)).toBe("top");
  expect(dropSide(200, 390, 400, 400)).toBe("bottom");
  expect(dropSide(200, 200, 400, 400)).toBe("center");
  expect(dropSide(0, 0, 0, 0)).toBe("center");
});

test("pane geometry tiles the workspace without gaps or overlap", () => {
  let layout = splitPane(singlePane(), MAIN_PANE, "right", "/a", "b");
  layout = splitPane(layout, "b", "bottom", "/c", "c");
  layout = setRatio(layout, (layout as Extract<PaneNode, { kind: "split" }>).id, 0.3);
  const { panes: boxes, handles } = paneGeometry(layout);
  const area = boxes.reduce((sum, { box }) => sum + box.w * box.h, 0);
  expect(area).toBeCloseTo(1, 10);
  expect(boxes.find((entry) => entry.id === MAIN_PANE)!.box).toEqual({ x: 0, y: 0, w: 0.3, h: 1 });
  expect(boxes.find((entry) => entry.id === "c")!.box).toMatchObject({ x: 0.3, y: 0.5 });
  expect(handles).toHaveLength(2);
});
