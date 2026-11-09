// Visual regression test: wire drag reconnect and new connection must not shift nodes.
// Run: node tests/visual/tree-wire-visual.ts
import { chromium, type Locator } from "playwright-core";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn, type ChildProcess } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";

const root = path.resolve(import.meta.dirname, "../..");

const exe = process.platform === "win32" ? ".exe" : "";
const binary = [
  process.env.STORYLET_SERVICE_BIN,
  path.join(root, `target/play-clean/debug/storylet-engine-service${exe}`),
  path.join(root, `target/plugin-test/debug/storylet-engine-service${exe}`),
  path.join(root, `target/tree-graph/debug/storylet-engine-service${exe}`),
  path.join(root, `crates/storylet-engine-service/target/debug/storylet-engine-service${exe}`),
].filter(Boolean).map((p) => path.resolve(p as string)).find((p) => fs.existsSync(p));

if (!binary) {
  console.error("storylet-engine-service binary not found; set STORYLET_SERVICE_BIN");
  process.exit(1);
}

const chromePath = process.env.CHROMIUM_PATH ?? (() => {
  const cache = path.join(os.homedir(), "AppData/Local/ms-playwright");
  if (!fs.existsSync(cache)) return undefined;
  for (const d of fs.readdirSync(cache).filter((d) => d.startsWith("chromium-")).sort().reverse()) {
    const candidate = path.join(cache, d, "chrome-win64/chrome.exe");
    if (fs.existsSync(candidate)) return candidate;
  }
  return undefined;
})();

const port = 42_000 + Math.floor(Math.random() * 1_000);
const base = `http://127.0.0.1:${port}`;

const child: ChildProcess = spawn(binary, ["--fresh-database"], {
  cwd: root,
  env: {
    ...process.env,
    STORYLET_STUDIO_ADDR: `127.0.0.1:${port}`,
    STORYLET_STUDIO_DB_PATH: ":memory:",
    STORYLET_RESOURCE_ROOT: ".",
  },
  stdio: "ignore",
  windowsHide: true,
});

async function api(method: string, p: string, body?: unknown) {
  const r = await fetch(base + p, {
    method,
    headers: { "Content-Type": "application/json" },
    body: body ? JSON.stringify(body) : undefined,
  });
  const text = await r.text();
  if (!r.ok) throw new Error(`${method} ${p} -> ${r.status} ${text}`);
  return text ? JSON.parse(text) : null;
}

const failures: string[] = [];
function check(name: string, ok: boolean, detail = "") {
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${ok || !detail ? "" : `  (${detail})`}`);
  if (!ok) failures.push(name);
}

function assertPositionsEqual(
  actual: { x: number; y: number },
  expected: { x: number; y: number },
  label: string,
) {
  const dx = Math.abs(actual.x - expected.x);
  const dy = Math.abs(actual.y - expected.y);
  const ok = dx < 1 && dy < 1;
  check(
    `${label} position unchanged`,
    ok,
    `expected (${expected.x.toFixed(1)}, ${expected.y.toFixed(1)}) got (${actual.x.toFixed(1)}, ${actual.y.toFixed(1)})`,
  );
}

async function getInlinePositions(nodes: Record<string, Locator>) {
  const result: Record<string, { x: number; y: number }> = {};
  for (const [id, locator] of Object.entries(nodes)) {
    result[id] = await locator.evaluate((el) => {
      const style = (el as HTMLElement).style;
      return { x: parseFloat(style.left), y: parseFloat(style.top) };
    });
  }
  return result;
}
async function getEdgeMidpoint(locator: Locator) {
  return await locator.evaluate((el) => {
    const path = el as SVGPathElement;
    const pt = path.getPointAtLength(path.getTotalLength() / 2);
    const matrix = path.getScreenCTM();
    if (matrix) {
      const screenPt = pt.matrixTransform(matrix);
      return { x: screenPt.x, y: screenPt.y };
    }
    const rect = path.getBoundingClientRect();
    return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
  });
}

let browser;
try {
  // Wait for service to be ready
  for (let i = 0; i < 50; i++) {
    try {
      if ((await fetch(base + "/api/v3/project")).ok) break;
    } catch {
      /* starting */
    }
    await sleep(200);
  }

  browser = await chromium.launch({ executablePath: chromePath, headless: true });
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  const errors: string[] = [];
  page.on("console", (m) => {
    if (m.type() === "error" && !m.text().startsWith("Failed to load resource")) errors.push(m.text());
  });
  page.on("pageerror", (e) => errors.push(String(e)));
  page.on("dialog", (d) => void d.accept());

  // 1. Initialize trait templates by navigating to the studio
  await page.goto(`${base}/#/storylets`);
  let graphTrait: { id: string; key: string } | undefined;
  let itemTrait: { id: string; key: string } | undefined;
  for (let i = 0; i < 50; i++) {
    const traits = await api("GET", "/api/v3/content/trait_templates");
    graphTrait = traits.find((t: { key: string }) => t.key === "tree_graph");
    itemTrait = traits.find((t: { key: string }) => t.key === "tree_item");
    if (graphTrait && itemTrait) break;
    await sleep(200);
  }
  check("tree traits created on app load", !!graphTrait && !!itemTrait);
  if (!graphTrait || !itemTrait) throw new Error("TreeGraph or TreeItem trait template not found");

  // 2. POST 4 TreeItem entity templates: Item A, B, C in graph, Item D available outside
  await api("POST", "/api/v3/content/entity_templates", {
    id: "item-a",
    name: "Item A",
    traits: [{ id: "ti-a", template_id: itemTrait.id, values: {} }],
  });
  await api("POST", "/api/v3/content/entity_templates", {
    id: "item-b",
    name: "Item B",
    traits: [{ id: "ti-b", template_id: itemTrait.id, values: {} }],
  });
  await api("POST", "/api/v3/content/entity_templates", {
    id: "item-c",
    name: "Item C",
    traits: [{ id: "ti-c", template_id: itemTrait.id, values: {} }],
  });
  await api("POST", "/api/v3/content/entity_templates", {
    id: "item-d",
    name: "Item D",
    traits: [{ id: "ti-d", template_id: itemTrait.id, values: {} }],
  });

  // Tree owner with initial 3 auto-layout nodes and 1 edge (A -> B)
  await api("POST", "/api/v3/content/entity_templates", {
    id: "tree-owner",
    name: "Tree Owner",
    traits: [
      {
        id: "ti-owner",
        template_id: graphTrait.id,
        values: {
          nodes: ["item-a", "item-b", "item-c"],
          edges: [JSON.stringify(["item-a", "item-b"])],
          direction: "right",
          positions: "{}",
        },
      },
    ],
  });

  // 3. Open Tree tab
  await page.goto(`${base}/#/entity-templates/tree-owner`);
  await page.reload();
  const treeTab = page.getByRole("button", { name: "Tree", exact: true });
  await treeTab.waitFor();
  await treeTab.click();

  const nodeA = page.locator('.tree-node[data-tree-id="item-a"]');
  const nodeB = page.locator('.tree-node[data-tree-id="item-b"]');
  const nodeC = page.locator('.tree-node[data-tree-id="item-c"]');
  await nodeA.waitFor();
  await nodeB.waitFor();
  await nodeC.waitFor();

  // 4. Check canvas toolbar: view options, export option, no add-item widget
  const viewOptionsSummary = page.locator(".tree-options summary");
  await viewOptionsSummary.waitFor();
  check("canvas has view options control", await viewOptionsSummary.isVisible());
  await viewOptionsSummary.click();

  const exportBtn = page.getByRole("button", { name: "Export JSON" });
  check("canvas has export JSON option", (await exportBtn.count()) > 0);
  await viewOptionsSummary.click();

  const addDropdown = page.locator('.tree-canvas-toolbar select[aria-label*="item" i], .tree-canvas-toolbar select[aria-label*="add" i]');
  const addToTreeBtn = page.getByRole("button", { name: "Add to tree" });
  check("no add-item widget in canvas toolbar", (await addDropdown.count()) === 0 && (await addToTreeBtn.count()) === 0);

  // 5. Add item through canvas context menu
  const canvasActionsBtn = page.getByRole("button", { name: "Canvas actions" });
  await canvasActionsBtn.waitFor();
  await canvasActionsBtn.click();

  const contextMenu = page.locator(".tree-context");
  await contextMenu.waitFor();

  const addCandidateBtn = contextMenu.locator(".tree-context-results button", { hasText: "Item D" });
  await addCandidateBtn.waitFor();
  await addCandidateBtn.click();

  const nodeD = page.locator('.tree-node[data-tree-id="item-d"]');
  await nodeD.waitFor();
  check("item-d added through canvas context menu", await nodeD.isVisible());

  const allNodes = { "item-a": nodeA, "item-b": nodeB, "item-c": nodeC, "item-d": nodeD };

  // 6. Record inline positions before wire reconnect
  const positionsBeforeReconnect = await getInlinePositions(allNodes);

  // Verify initial edge target via accessible label
  const initialEdge = page.locator('.tree-edge-hit[aria-label*="Connection from Item A to Item B"]');
  await initialEdge.waitFor({ state: "attached" });
  check("initial edge target verified via accessible label", (await initialEdge.count()) === 1);

  // 7. Wire drag reconnect: drag edge from Item B to Item C using real mouse
  const edgeMidpoint = await getEdgeMidpoint(initialEdge);

  const nodeCBox = (await nodeC.boundingBox())!;
  await page.mouse.move(edgeMidpoint.x, edgeMidpoint.y);
  await page.mouse.down();
  await page.mouse.move(nodeCBox.x + nodeCBox.width / 2, nodeCBox.y + nodeCBox.height / 2, { steps: 10 });
  await page.mouse.up();

  // Verify actual edge target after reconnect via accessible label
  const reconnectedEdge = page.locator('.tree-edge-hit[aria-label*="Connection from Item A to Item C"]');
  await reconnectedEdge.waitFor({ state: "attached" });
  check("reconnected edge target verified via accessible label", (await reconnectedEdge.count()) === 1);

  // Compare inline positions: all existing nodes must not move
  const positionsAfterReconnect = await getInlinePositions(allNodes);
  for (const id of Object.keys(allNodes)) {
    assertPositionsEqual(positionsAfterReconnect[id]!, positionsBeforeReconnect[id]!, `${id} after wire reconnect`);
  }

  // Restore the original auto-layout before checking a new connection.
  await page.reload();
  await page.getByRole("button", { name: "Tree", exact: true }).click();
  await canvasActionsBtn.click();
  await addCandidateBtn.click();
  await nodeD.waitFor();

  // 8. New connection: drag wire from Item B connect zone to Item D using real mouse
  const positionsBeforeNewConnect = await getInlinePositions(allNodes);

  const connectZone = nodeB.locator(".tree-connect-zone.zone-right");
  await connectZone.waitFor();
  const zoneBox = (await connectZone.boundingBox())!;
  const nodeDBox = (await nodeD.boundingBox())!;

  await page.mouse.move(zoneBox.x + zoneBox.width / 2, zoneBox.y + zoneBox.height / 2);
  await page.mouse.down();
  await page.mouse.move(nodeDBox.x + nodeDBox.width / 2, nodeDBox.y + nodeDBox.height / 2, { steps: 10 });
  await page.mouse.up();

  // Verify new edge target via accessible label
  const newEdge = page.locator('.tree-edge-hit[aria-label*="Connection from Item B to Item D"]');
  await newEdge.waitFor({ state: "attached" });
  check("new connection target verified via accessible label", (await newEdge.count()) === 1);

  // Compare inline positions: all existing nodes must not move
  const positionsAfterNewConnect = await getInlinePositions(allNodes);
  for (const id of Object.keys(allNodes)) {
    assertPositionsEqual(positionsAfterNewConnect[id]!, positionsBeforeNewConnect[id]!, `${id} after new connection`);
  }

  // 9. Double-click wire to name relationship
  const newEdgeMidpoint = await getEdgeMidpoint(newEdge);
  await page.mouse.dblclick(newEdgeMidpoint.x, newEdgeMidpoint.y);

  const relInput = page.locator("#tree-relationship-name");
  await relInput.waitFor({ state: "visible" });
  check("double-click wire opens relationship name input", await relInput.isVisible());

  const saveRelBtn = page.getByRole("button", { name: "Save relationship" });
  check("save relationship button initially disabled for empty name", await saveRelBtn.isDisabled());

  await relInput.fill("causes");
  check("save relationship button enabled after entering non-blank name", !(await saveRelBtn.isDisabled()));
  await saveRelBtn.click();

  // 10. Visible relationship label rendered on wire
  const edgeLabel = page.locator(".tree-edge-label", { hasText: "causes" });
  await edgeLabel.waitFor({ state: "visible" });
  check("visible relationship label rendered on wire", await edgeLabel.isVisible());

  const namedEdge = page.locator('.tree-edge-hit[aria-label*="Connection from Item B to Item D, causes"]');
  await namedEdge.waitFor({ state: "attached" });
  check("wire accessible label includes relationship name", (await namedEdge.count()) === 1);

  // 11. Reconnect preservation: drag named wire from Item D to Item C
  const namedEdgeMidpoint = await getEdgeMidpoint(namedEdge);
  await page.mouse.move(namedEdgeMidpoint.x, namedEdgeMidpoint.y);
  await page.mouse.down();
  await page.mouse.move(nodeCBox.x + nodeCBox.width / 2, nodeCBox.y + nodeCBox.height / 2, { steps: 10 });
  await page.mouse.up();

  const reconnectedNamedEdge = page.locator('.tree-edge-hit[aria-label*="Connection from Item B to Item C, causes"]');
  await reconnectedNamedEdge.waitFor({ state: "attached" });
  check("reconnected edge preserves relationship name in accessible label", (await reconnectedNamedEdge.count()) === 1);
  check("reconnected edge preserves visible relationship label", await edgeLabel.isVisible());

  const labelBox = await edgeLabel.boundingBox();
  if (!labelBox) throw new Error("Relationship label has no visible bounds");
  const labelPoint = { x: labelBox.x + labelBox.width / 2, y: labelBox.y + labelBox.height * .45 };
  check("relationship text is an interactive wire target", await page.evaluate(({ x, y }) =>
    document.elementsFromPoint(x, y).some((element) => element.matches(".tree-edge-label.is-editable")), labelPoint));
  await page.mouse.dblclick(labelPoint.x, labelPoint.y);
  await relInput.waitFor({ state: "visible" });
  check("double-clicking relationship text opens its editor", (await relInput.inputValue()) === "causes");
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  check("cancelling returns focus to the edit action", await page.locator(".tree-relationship-toggle").evaluate((element) => document.activeElement === element));
  await page.keyboard.press("Escape");

  // 12. Contextual edit: right-click wire opens context menu with Edit/Remove options
  const reconnectedEdgeMidpoint = await getEdgeMidpoint(reconnectedNamedEdge);
  await page.mouse.click(reconnectedEdgeMidpoint.x, reconnectedEdgeMidpoint.y, { button: "right" });

  const editRelBtn = page.getByRole("button", { name: "Edit relationship" });
  const removeRelBtn = page.getByRole("button", { name: "Remove relationship" });
  await editRelBtn.waitFor({ state: "visible" });
  await removeRelBtn.waitFor({ state: "visible" });
  check("context menu offers Edit relationship", await editRelBtn.isVisible());
  check("context menu offers Remove relationship", await removeRelBtn.isVisible());

  await editRelBtn.click();
  await relInput.waitFor({ state: "visible" });
  check("edit relationship shows current name", (await relInput.inputValue()) === "causes");

  await relInput.fill("triggers");
  await saveRelBtn.click();

  const editedLabel = page.locator(".tree-edge-label", { hasText: "triggers" });
  await editedLabel.waitFor({ state: "visible" });
  check("visible relationship label updated after contextual edit", await editedLabel.isVisible());

  const editedEdge = page.locator('.tree-edge-hit[aria-label*="Connection from Item B to Item C, triggers"]');
  await editedEdge.waitFor({ state: "attached" });
  check("wire accessible label updated after contextual edit", (await editedEdge.count()) === 1);

  // 13. Contextual removal: Remove relationship removes label but keeps wire
  const editedEdgeMidpoint = await getEdgeMidpoint(editedEdge);
  await page.mouse.click(editedEdgeMidpoint.x, editedEdgeMidpoint.y, { button: "right" });
  await removeRelBtn.waitFor({ state: "visible" });
  await removeRelBtn.click();

  const unnamedEdge = page.locator('.tree-edge-hit[aria-label="Connection from Item B to Item C; press Enter for options"]');
  await unnamedEdge.waitFor({ state: "attached" });
  check("wire remains after relationship removal", (await unnamedEdge.count()) === 1);
  check("relationship label removed after contextual removal", (await page.locator(".tree-edge-label").count()) === 0);

  // 14. Assert persisted data after saving and reloading
  const unnamedMidpoint = await getEdgeMidpoint(unnamedEdge);
  await page.mouse.dblclick(unnamedMidpoint.x, unnamedMidpoint.y);
  await relInput.waitFor({ state: "visible" });
  await relInput.fill("leads to");
  await saveRelBtn.click();

  const leadsToLabel = page.locator(".tree-edge-label", { hasText: "leads to" });
  await leadsToLabel.waitFor({ state: "visible" });
  check("relationship label set before save", await leadsToLabel.isVisible());

  const saveLayoutBtn = page.getByRole("button", { name: "Save layout" });
  const [saveResponse] = await Promise.all([
    page.waitForResponse((r) => r.url().includes("/api/v3/content/entity_templates") && r.request().method() === "POST"),
    saveLayoutBtn.click(),
  ]);
  check("relationship save accepted by service", saveResponse.status() === 201, `status ${saveResponse.status()}`);

  // Reload page to verify persisted state in UI
  await page.reload();
  const reloadTreeTab = page.getByRole("button", { name: "Tree", exact: true });
  await reloadTreeTab.waitFor();
  await reloadTreeTab.click();
  await page.locator('.tree-node[data-tree-id="item-a"]').waitFor();

  const persistedLabel = page.locator(".tree-edge-label", { hasText: "leads to" });
  await persistedLabel.waitFor({ state: "visible" });
  check("persisted relationship label visible after reload", await persistedLabel.isVisible());

  const persistedEdge = page.locator('.tree-edge-hit[aria-label*="Connection from Item B to Item C, leads to"]');
  await persistedEdge.waitFor({ state: "attached" });
  check("persisted wire accessible label preserved after reload", (await persistedEdge.count()) === 1);

  // Assert persisted data in the backend database
  const savedEntity = await api("GET", "/api/v3/content/entity_templates/tree-owner");
  const ownerTrait = savedEntity.traits.find((t: { template_id: string }) => t.template_id === graphTrait!.id);
  const persistedEdges = (ownerTrait?.values?.edges ?? []).map((e: string) => JSON.parse(e));
  const hasNamedEdge = persistedEdges.some((e: unknown[]) => Array.isArray(e) && e[0] === "item-b" && e[1] === "item-c" && e[2] === "leads to");
  const hasUnnamedEdge = persistedEdges.some((e: unknown[]) => Array.isArray(e) && e[0] === "item-a" && e[1] === "item-b" && e.length === 2);
  check("persisted entity template contains encoded 3-element named edge", hasNamedEdge);
  check("persisted entity template contains encoded 2-element unnamed edge", hasUnnamedEdge);

  await page.locator(".tree-options summary").click();
  const [download] = await Promise.all([
    page.waitForEvent("download"),
    page.getByRole("button", { name: "Export JSON" }).click(),
  ]);
  const exportPath = await download.path();
  if (!exportPath) throw new Error("Tree graph export did not produce a file");
  const exported = JSON.parse(fs.readFileSync(exportPath, "utf8"));
  check("export contains named relationship", exported.graph.edges.some((edge: { from: string; to: string; relationship?: string }) =>
    edge.from === "item-b" && edge.to === "item-c" && edge.relationship === "leads to"));

  if (errors.length) console.log("console errors:\n" + errors.join("\n"));
  check("no console errors", errors.length === 0);
} finally {
  await browser?.close();
  child.kill();
}

if (failures.length) {
  console.log(`\n${failures.length} failure(s) in tree wire visual regression`);
  process.exit(1);
}
console.log("\nall tree wire regression checks passed");
