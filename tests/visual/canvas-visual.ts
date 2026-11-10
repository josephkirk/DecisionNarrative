// Visual interaction test for the plugin system's vector canvas + Modify Canvas node.
// Run: bun run build && node tests/visual/canvas-visual.ts  (Node: Playwright cannot drive Chromium from Bun).
// Needs a built storylet-engine-service binary (STORYLET_SERVICE_BIN or target/*/debug).
import { chromium, type Locator, type Page } from "playwright-core";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn, type ChildProcess } from "node:child_process";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const root = path.resolve(import.meta.dirname, "../..");
const out = process.env.VISUAL_OUT ? path.resolve(process.env.VISUAL_OUT) : path.join(import.meta.dirname, "out");
fs.mkdirSync(out, { recursive: true });
const exe = process.platform === "win32" ? ".exe" : "";
const binary = [
  process.env.STORYLET_SERVICE_BIN,
  path.join(root, `target/play-clean/debug/storylet-engine-service${exe}`),
  path.join(root, `target/plugin-test/debug/storylet-engine-service${exe}`),
].filter(Boolean).map((p) => path.resolve(p as string)).find((p) => fs.existsSync(p));
if (!binary) throw new Error("storylet-engine-service binary not found; set STORYLET_SERVICE_BIN");
const chromePath = process.env.CHROMIUM_PATH ?? (() => {
  const cache = path.join(os.homedir(), "AppData/Local/ms-playwright");
  if (!fs.existsSync(cache)) return undefined;
  for (const d of fs.readdirSync(cache).filter((d) => d.startsWith("chromium-")).sort().reverse()) {
    const candidate = path.join(cache, d, "chrome-win64/chrome.exe");
    if (fs.existsSync(candidate)) return candidate;
  }
  return undefined;
})();

const port = 40_000 + Math.floor(Math.random() * 1_000);
const base = `http://127.0.0.1:${port}`;
const child: ChildProcess = spawn(binary, ["--fresh-database"], {
  cwd: root,
  env: { ...process.env, STORYLET_STUDIO_ADDR: `127.0.0.1:${port}`, STORYLET_STUDIO_DB_PATH: path.join(os.tmpdir(), `dn-canvas-${port}.db`), STORYLET_RESOURCE_ROOT: "." },
  stdio: "ignore", windowsHide: true,
});
async function api(method: string, p: string, body?: unknown) {
  const r = await fetch(base + p, { method, headers: { "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined });
  const text = await r.text();
  if (!r.ok) throw new Error(`${method} ${p} -> ${r.status} ${text}`);
  return text ? JSON.parse(text) : null;
}
const failures: string[] = [];
function check(name: string, ok: boolean, detail = "") {
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${ok || !detail ? "" : `  (${detail})`}`);
  if (!ok) failures.push(name);
}
const shot = (page: Page, name: string) => page.screenshot({ path: path.join(out, `${name}.png`) });
const n = (id: string, type: string, x: number, extra: Record<string, unknown> = {}) => ({ id, title: id, type, x, y: 0, width: 160, height: 80, ...extra });

let browser;
try {
  for (let i = 0; i < 50; i++) { try { if ((await fetch(base + "/api/v3/project")).ok) break; } catch { /* starting */ } await sleep(200); }
  await api("POST", "/api/v3/content/scenarios", { id: "scenario", name: "Scenario", seed: 7, start_location_policy: { mode: "fixed", location_id: "crypt" } });
  await api("POST", "/api/v3/content/storylets", {
    id: "unlock", title: "Unlock the crypt", content: { text_template: "x" }, created_at: 1,
    graph: {
      nodes: [n("start", "start", 0, { text: "Start" }), n("line", "dialog", 200, { text: "An iron gate bars the way." }),
        n("choice", "choice", 400, { choices: [{ id: "a", text: "Use the key" }] }), { ...n("end", "end", 0, { is_terminal: true }), y: 320 }],
      edges: [
        { id: "e0", from_node: "start", from_port: "output", to_node: "line", to_port: "input" },
        { id: "e1", from_node: "line", from_port: "output", to_node: "choice", to_port: "input" },
        { id: "e2", from_node: "choice", from_port: "choice_0", to_node: "end", to_port: "input" },
      ],
    },
  });

  browser = await chromium.launch({ executablePath: chromePath, headless: true });
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  const errors: string[] = [];
  page.on("console", (m) => { if (m.type() === "error" && !m.text().startsWith("Failed to load resource")) errors.push(m.text()); });
  page.on("pageerror", (e) => errors.push(String(e)));
  page.on("dialog", (d) => void d.accept());

  // 0. The plugin creates the "Canvas" trait on load; only entities with it get the Canvas tab.
  await page.goto(`${base}/#/storylets`);
  let canvasTrait: { id: string } | undefined;
  for (let i = 0; i < 50 && !canvasTrait; i++) {
    canvasTrait = (await api("GET", "/api/v3/content/trait_templates")).find((t: { key: string }) => t.key === "canvas");
    if (!canvasTrait) await sleep(200);
  }
  check("plugin creates the Canvas trait", !!canvasTrait);
  const locationTraitId = "system.location";
  await api("POST", "/api/v3/content/entity_templates", { id: "plain", name: "Plain", traits: [{ id: "location", template_id: locationTraitId, values: {} }], thumbnail: "https://example.invalid/plain.png" });
  await api("POST", "/api/v3/content/entity_templates", { id: "crypt", name: "Crypt", traits: [{ id: "location", template_id: locationTraitId, values: {} }, { id: "ti_canvas", template_id: canvasTrait!.id, values: {} }], thumbnail: "https://example.invalid/crypt.png" });
  await page.goto(`${base}/#/entity-templates/plain`);
  await page.getByRole("button", { name: "Properties" }).waitFor();
  check("entity without the trait has no Canvas tab", (await page.getByRole("button", { name: "Canvas", exact: true }).count()) === 0);

  // 1. Entity panel contributed by the plugin.
  await page.goto(`${base}/#/entity-templates/crypt`);
  await page.reload();
  await page.getByRole("button", { name: "Canvas", exact: true }).click();
  const stage = page.locator(".vc-stage");
  await stage.waitFor();
  check("Canvas tab renders the vector canvas", await stage.isVisible());
  check("new canvas has one Default layer", (await page.locator(".vc-layer-name").count()) === 1 && (await page.locator(".vc-layer-name").inputValue()) === "Default");
  const box = (await stage.boundingBox())!;
  const at = (x: number, y: number) => ({ x: box.x + x, y: box.y + y });

  // 2. Room on Default, a wall on a second layer, then markers, a sticky note and references on Default.
  await page.getByRole("button", { name: "Room / Rectangle (R)" }).click();
  let p = at(80, 80); await page.mouse.move(p.x, p.y); await page.mouse.down();
  p = at(560, 420); await page.mouse.move(p.x, p.y, { steps: 5 }); await page.mouse.up();
  await page.getByRole("button", { name: "+ Add layer" }).click();
  await page.getByRole("button", { name: /Wall \/ Polyline/ }).click();
  for (const [x, y] of [[80, 250], [300, 250]]) { p = at(x, y); await page.mouse.click(p.x, p.y); }
  p = at(300, 380); await page.mouse.dblclick(p.x, p.y);
  await page.locator(".vc-layer", { has: page.locator('input[value="Default"]') }).click();
  await page.getByRole("button", { name: "Marker (M)" }).click();
  for (const [icon, x, y] of [["door", 400, 250], ["key", 150, 150], ["gem", 450, 350], ["scroll", 200, 350]] as const) {
    await page.getByLabel("Icon for new markers").selectOption(icon);
    p = at(x, y); await page.mouse.click(p.x, p.y);
  }
  await page.getByRole("button", { name: "Sticky note (N)" }).click();
  p = at(420, 60); await page.mouse.click(p.x, p.y);
  await page.locator(".vc-field", { hasText: "Note" }).locator("textarea").fill("Remember: the gate needs the key.");
  await page.locator(".vc-field", { hasText: "Note" }).locator("textarea").blur();
  check("sticky note shows its text", (await page.locator(".vc-note-text").textContent())?.includes("gate needs the key") ?? false);
  await page.getByRole("button", { name: "Entity reference" }).click();
  await page.getByLabel("Entity to place").selectOption("plain");
  p = at(500, 120); await page.mouse.click(p.x, p.y);
  check("entity reference shows its thumbnail", (await page.locator('.vc-kind-entity image').count()) === 1);
  await page.getByRole("button", { name: "Storylet reference" }).click();
  await page.getByLabel("Storylet to place").selectOption("unlock");
  p = at(520, 220); await page.mouse.click(p.x, p.y);
  check("storylet reference placed", (await page.locator('.vc-kind-storylet').count()) === 1);
  check("shapes created", (await page.locator(".vc-shape").count()) === 9, String(await page.locator(".vc-shape").count()));

  // 3. Undo / redo do not add history.
  const historyCount = async () => Number((await page.getByRole("button", { name: /^History/ }).textContent())!.replace(/\D+/g, ""));
  const beforeUndo = await historyCount();
  await page.keyboard.press("Control+z");
  check("undo removes the last shape", (await page.locator(".vc-shape").count()) === 8);
  await page.keyboard.press("Control+y");
  check("redo restores it", (await page.locator(".vc-shape").count()) === 9);
  check("undo/redo add no history", (await historyCount()) === beforeUndo, `${beforeUndo} -> ${await historyCount()}`);

  // 4. Name the gate marker and give it a hover description.
  await page.getByRole("button", { name: "Select (V)" }).click();
  const gateShape = page.locator('.vc-shape[data-kind="marker"]', { hasText: "Door (closed)" });
  await gateShape.click();
  const label = page.locator(".vc-field", { hasText: "Name on canvas" }).locator("input");
  await label.fill("Iron Gate"); await label.press("Enter");
  const description = page.locator(".vc-field", { hasText: "Description" }).locator("textarea");
  await description.fill("A rusted iron gate. It only opens with the crypt key."); await description.blur();
  await page.locator('.vc-shape[data-kind="marker"]', { hasText: "Iron Gate" }).hover();
  check("hover shows the description", (await page.getByRole("tooltip").textContent())?.includes("rusted iron gate") ?? false);
  await page.getByLabel("Show runtime state").check();
  await shot(page, "canvas-01-editor");
  await page.getByRole("button", { name: /^History/ }).click();
  check("history lists revisions", (await page.locator(".vc-history li").count()) >= 10);
  await shot(page, "canvas-02-history");

  // 5. Layer hide.
  await page.getByRole("button", { name: "Layers" }).click();
  await page.locator(".vc-layer", { has: page.locator('input[value="Layer 2"]') }).getByRole("button", { name: "Hide layer" }).click();
  check("hiding a layer hides its shapes", (await page.locator(".vc-shape").count()) === 8);
  await page.locator(".vc-layer", { has: page.locator('input[value="Layer 2"]') }).getByRole("button", { name: "Show layer" }).click();
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await sleep(800);
  console.log("toast:", (await page.locator(".toast, [role=status], [role=alert]").allTextContents()).join(" | "));
  const saved = await api("GET", "/api/v3/content/entity_templates/crypt");
  const keys: string[] = saved.properties.map((x: { key: string }) => x.key);
  check("entity saves canvas_doc", keys.includes("canvas_doc"));
  check("gameplay shapes declare state properties", keys.filter((k) => k.startsWith("map_")).length === 12, keys.join(","));
  const doc = JSON.parse(saved.properties.find((x: { key: string }) => x.key === "canvas_doc").value);
  const gate = doc.shapes.find((s: { label?: string }) => s.label === "Iron Gate");
  check("gate marker saved with icon and description", gate?.kind === "marker" && gate.icon === "door" && gate.description?.includes("rusted"));

  const plainRefs = (await api("GET", "/api/v3/content/entity_templates/plain/references")).references;
  check("entity placed on the canvas is referenced by the canvas holder", plainRefs.some((r: { source_id: string }) => r.source_id === "crypt"), JSON.stringify(plainRefs));
  const storyRefs = (await api("GET", "/api/v3/content/storylets/unlock/references")).references;
  check("storylet placed on the canvas is referenced by the canvas holder", storyRefs.some((r: { source_id: string }) => r.source_id === "crypt"), JSON.stringify(storyRefs));

  await page.goto(`${base}/#/entity-templates/plain`);
  await page.getByRole("button", { name: /^References/ }).click();
  await page.getByText("Canvas reference").first().waitFor({ timeout: 5000 }).catch(() => {});
  check("entity References tab lists the canvas holder", (await page.getByText("Canvas reference").count()) > 0);
  await shot(page, "canvas-06-entity-references");
  await page.goto(`${base}/#/entity-templates/crypt`);
  await page.getByRole("button", { name: "Canvas", exact: true }).click();

  // 6. Property editor contributed by the plugin.
  await page.getByRole("button", { name: "Properties" }).click();
  check("canvas_doc property shows the plugin summary", await page.getByTestId("canvas-property-summary").isVisible());

  // 7. Modify Canvas node in the storylet graph.
  await page.goto(`${base}/#/storylets/unlock/edit`);
  await page.getByTestId("storylet-references-tab").click();
  const refPane = page.locator(".storylet-references");
  await refPane.getByText("Crypt", { exact: true }).first().waitFor({ timeout: 5000 }).catch(() => {});
  check("storylet References tab lists the canvas holder", (await refPane.getByText("Crypt", { exact: true }).count()) > 0);
  await shot(page, "canvas-05-storylet-references");
  await page.getByRole("button", { name: "Story graph" }).click();
  const historyBefore = JSON.parse((await api("GET", "/api/v3/content/entity_templates/crypt")).properties.find((x: { key: string }) => x.key === "canvas_doc").value).revisions.length;
  async function connect(from: Locator, to: Locator) {
    const a = (await from.boundingBox())!; const b = (await to.boundingBox())!;
    await page.mouse.move(a.x + a.width / 2, a.y + a.height / 2); await page.mouse.down();
    await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2, { steps: 8 }); await page.mouse.up();
  }
  await page.getByRole("button", { name: "Get entity", exact: true }).click();
  await page.locator(".property-inspector-panel select").selectOption("crypt");
  await page.getByRole("button", { name: "Modify Canvas" }).first().click();
  const pluginNode = page.locator(".graph-node.plugin");
  await pluginNode.waitFor();
  const editor = page.getByTestId("modify-canvas-editor");
  check("disconnected node requires an entity input", await page.getByTestId("open-modify-canvas").isDisabled());
  const sourceHeader = (await page.locator(".graph-node.get_entity .node-header").boundingBox())!;
  await page.mouse.move(sourceHeader.x + 30, sourceHeader.y + 15); await page.mouse.down();
  await page.mouse.move(sourceHeader.x - 180, sourceHeader.y + 120, { steps: 8 }); await page.mouse.up();
  await connect(page.locator(".graph-node.get_entity .output-socket"), pluginNode.locator('[title="Entity data input"]'));
  await pluginNode.locator(".node-body").click();
  await editor.waitFor();
  check("connected node opens the entity canvas without a selector", await editor.isVisible() && await editor.getByLabel("Canvas entity").count() === 0);
  const nodeStage = editor.locator(".vc-stage");
  await nodeStage.waitFor();
  check("node editor starts from the entity canvas", (await editor.locator(".vc-shape").count()) === 9, String(await editor.locator(".vc-shape").count()));
  check("node editor has no History tab", (await editor.getByRole("button", { name: /^History/ }).count()) === 0);
  await editor.locator('.vc-shape[data-kind="marker"]', { hasText: "Iron Gate" }).click();
  await editor.getByRole("group", { name: "Marker icons" }).getByRole("button", { name: "Door (open)" }).click();
  await editor.locator('.vc-shape[data-kind="marker"]', { hasText: "Key" }).click();
  await page.keyboard.press("Delete");
  await editor.locator('.vc-shape[data-kind="marker"]', { hasText: "Item" }).click();
  await editor.getByLabel("On canvas after this node").uncheck();
  await editor.getByRole("button", { name: "Marker (M)" }).click();
  await editor.getByLabel("Icon for new markers").selectOption("skull");
  const nodeBox = (await nodeStage.boundingBox())!;
  await page.mouse.click(nodeBox.x + 120, nodeBox.y + 330);
  check("node editor edits the canvas freely", (await editor.locator(".vc-shape").count()) === 9 && (await editor.locator(".vc-shape", { hasText: "Danger" }).count()) === 1);
  await shot(page, "canvas-03-modify-node-editor");
  await editor.getByRole("button", { name: "Save version" }).click();
  await editor.waitFor({ state: "detached" });
  const inspector = page.getByTestId("modify-canvas-inspector");
  await inspector.locator(".mc-ops li").first().waitFor();
  const opTexts = await inspector.locator(".mc-ops li").allTextContents();
  check("inspector lists four gameplay changes", opTexts.length === 4, opTexts.join(" | "));
  const cryptDoc = async () => JSON.parse((await api("GET", "/api/v3/content/entity_templates/crypt")).properties.find((x: { key: string }) => x.key === "canvas_doc").value);
  let afterDoc = await cryptDoc();
  check("saving adds exactly one version to the entity history", afterDoc.revisions.length === historyBefore + 1 && afterDoc.revisions.filter((r: { source?: unknown }) => r.source).length === 1, `${historyBefore} -> ${afterDoc.revisions.length}`);
  check("the entity canvas itself is unchanged", afterDoc.shapes.length === 9);
  check("the new version is temp until the storylet is saved", afterDoc.revisions.at(-1)?.uncommitted === true);
  // Edit again: the node keeps its one version.
  // Deselect, then click the node: the editor opens again.
  const emptyCanvasPoint = async () => { const b = (await page.locator(".graph-editor-canvas").boundingBox())!; return { x: b.width - 40, y: b.height - 40 }; };
  await page.locator(".graph-editor-canvas").click({ position: await emptyCanvasPoint() });
  await pluginNode.click({ position: { x: 60, y: 50 } });
  await editor.waitFor();
  check("clicking the node opens the canvas editor", await editor.isVisible());
  check("re-opening shows the saved version", (await editor.locator(".vc-shape", { hasText: "Danger" }).count()) === 1);
  await editor.getByRole("button", { name: "Save version" }).click();
  await editor.waitFor({ state: "detached" });
  afterDoc = await cryptDoc();
  check("saving again still leaves one version", afterDoc.revisions.length === historyBefore + 1, String(afterDoc.revisions.length));
  await shot(page, "canvas-03-modify-node");
  // Wire choice -> Modify Canvas -> End by dragging sockets.
  await connect(page.locator(".graph-node.choice .output-socket").first(), page.locator('.graph-node.plugin [title="Flow input"]'));
  await connect(page.locator(".graph-node.plugin .output-socket"), page.locator(".graph-node.end .input-socket"));
  await shot(page, "canvas-wired");
  await page.getByRole("button", { name: "Save draft" }).click();
  await sleep(800);
  console.log("toast:", (await page.locator(".toast, [role=status], [role=alert]").allTextContents()).join(" | "));
  const stored = await api("GET", "/api/v3/content/storylets/unlock");
  const types = stored.graph.nodes.map((x: { type: string }) => x.type);
  check("stored graph has no plugin nodes (lowered)", !types.includes("plugin"), types.join(","));
  const commands = stored.graph.nodes.filter((x: { type: string }) => x.type === "apply_command");
  check("lowered to version switch + four Apply Command nodes", commands.length === 5 && commands[0].property_key === "canvas_version", commands.map((c: { property_key: string }) => c.property_key).join(","));
  check("plugin metadata survives storage", stored.graph.nodes.some((x: { plugin?: { source?: unknown } }) => x.plugin?.source));
  const committedVersion = async () => (await cryptDoc()).revisions.find((r: { source?: unknown }) => r.source);
  for (let i = 0; i < 25 && (await committedVersion())?.uncommitted; i++) await sleep(200);
  const committed = await committedVersion();
  check("saving the storylet commits the version", !!committed && !committed.uncommitted && !committed.draft, JSON.stringify(committed ?? {}).slice(0, 120));
  await page.reload();
  await page.locator(".graph-node.plugin").waitFor();
  check("reload raises one Modify Canvas node", (await page.locator(".graph-node.plugin").count()) === 1 && (await page.locator(".graph-node.apply_command").count()) === 0);
  await shot(page, "canvas-04-graph");

  await page.goto(`${base}/#/entity-templates/crypt`);
  await page.getByRole("button", { name: "Canvas", exact: true }).click();
  await page.getByRole("button", { name: /^History/ }).click();
  check("entity history shows the storylet version", (await page.locator(".vc-history li.is-node-version").count()) === 1);
  await shot(page, "canvas-07-entity-history");

  // Run it: the engine executes the lowered nodes against the location's map state.
  const project = await api("GET", "/api/v3/project");
  const run = await api("POST", "/api/v4/runs", { start: { kind: "beginning" }, storyletId: "unlock", expectedRevision: project.revision, idempotencyKey: "r1" });
  const choice = run.view.scene?.choices?.[0];
  check("run reaches the choice", !!choice, JSON.stringify(run.view).slice(0, 200));
  const acted = await api("POST", `/api/v4/runs/${run.id}/act`, { participant: run.view.participant, action: choice.action, expectedRevision: run.revision, idempotencyKey: "a1" })
    .catch((e) => ({ error: String(e) }));
  check("choosing executes Modify Canvas without stalling", !("error" in acted) && !acted.stalled, JSON.stringify(acted).slice(0, 300));

  // Deleting the node removes its version from the entity's history.
  const versionCount = async () => (await cryptDoc()).revisions.filter((r: { source?: unknown }) => r.source).length;
  const revisionsBeforeDelete = (await cryptDoc()).revisions.length;
  await page.goto(`${base}/#/storylets/unlock/edit`);
  await page.locator(".graph-node.plugin").waitFor();
  // Dragging the node moves it without opening the editor.
  const header = (await page.locator(".graph-node.plugin .node-header").boundingBox())!;
  await page.mouse.move(header.x + 40, header.y + header.height / 2); await page.mouse.down();
  await page.mouse.move(header.x + 120, header.y + header.height / 2 + 60, { steps: 6 }); await page.mouse.up();
  await sleep(300);
  const moved = (await page.locator(".graph-node.plugin .node-header").boundingBox())!;
  check("dragging the node moves it without opening the editor", (await page.getByTestId("modify-canvas-editor").count()) === 0 && moved.x > header.x + 40, `${header.x} -> ${moved.x}`);
  await page.locator(".graph-editor-canvas").click({ position: await emptyCanvasPoint() });
  const deleteNode = async () => {
    await page.locator(".graph-node.plugin").getByRole("button", { name: "Delete node" }).click();
    await page.locator(".graph-node.plugin").waitFor({ state: "detached" });
  };
  const nodeVersionNow = async () => (await cryptDoc()).revisions.find((r: { source?: unknown }) => r.source);
  await deleteNode();
  for (let i = 0; i < 25 && !(await nodeVersionNow())?.pending_delete; i++) await sleep(200);
  check("deleting the node marks its version removal pending", (await nodeVersionNow())?.pending_delete === true);
  await page.goto(`${base}/#/entity-templates/crypt`);
  await page.getByRole("button", { name: "Canvas", exact: true }).click();
  await page.getByRole("button", { name: /^History/ }).click();
  await shot(page, "canvas-08-history-pending");
  for (let i = 0; i < 25 && (await nodeVersionNow())?.pending_delete; i++) await sleep(200);
  const restoredVersion = await nodeVersionNow();
  check("leaving the storylet unsaved keeps the version", !!restoredVersion && !restoredVersion.pending_delete);
  // Delete again and save: now the version goes away.
  await page.goto(`${base}/#/storylets/unlock/edit`);
  await page.locator(".graph-node.plugin").waitFor();
  await deleteNode();
  // The Get Entity that fed the node is now unused.
  await page.locator(".graph-node.get_entity").getByRole("button", { name: "Delete node" }).click();
  await connect(page.locator(".graph-node.choice .output-socket").first(), page.locator(".graph-node.end .input-socket"));
  await page.getByRole("button", { name: "Save draft" }).click();
  for (let i = 0; i < 25 && (await versionCount()) > 0; i++) await sleep(200);
  const deletedDoc = await cryptDoc();
  check("saving after the delete removes the version", (await versionCount()) === 0 && deletedDoc.revisions.length === revisionsBeforeDelete - 1, `${revisionsBeforeDelete} -> ${deletedDoc.revisions.length}`);
  const presentKeys = (await api("GET", "/api/v3/content/entity_templates/crypt")).properties.filter((x: { key: string }) => x.key.startsWith("map_") && x.key.endsWith("_present"));
  check("properties only that version declared are removed", presentKeys.length === 6, String(presentKeys.length));
  await page.goto(`${base}/#/entity-templates/crypt`);
  await page.getByRole("button", { name: "Canvas", exact: true }).click();
  await page.getByRole("button", { name: /^History/ }).click();
  check("entity history no longer lists the storylet version", (await page.locator(".vc-history li.is-node-version").count()) === 0);
  await shot(page, "canvas-08-history-after-delete");
  if (errors.length) console.log("console errors:\n" + errors.join("\n"));
  check("no console errors", errors.length === 0);
} finally {
  await browser?.close();
  child.kill();
}
if (failures.length) { console.log(`\n${failures.length} failure(s)`); process.exit(1); }
console.log("\nall canvas checks passed");
