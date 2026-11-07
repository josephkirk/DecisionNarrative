// Visual interaction test for the plugin system's vector canvas + Modify Canvas node.
// Run: bun run build && node tests/visual/canvas-visual.ts  (Node: Playwright cannot drive Chromium from Bun).
// Needs a built storylet-engine-service binary (STORYLET_SERVICE_BIN or target/*/debug).
import { chromium, type Page } from "playwright-core";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn, type ChildProcess } from "node:child_process";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const root = path.resolve(import.meta.dirname, "../..");
const out = path.join(import.meta.dirname, "out");
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
  await api("POST", "/api/v3/content/entity_templates", { id: "crypt", name: "Crypt", root_tag: "location", thumbnail: "https://example.invalid/crypt.png" });
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

  // 1. Entity panel contributed by the plugin.
  await page.goto(`${base}/#/entity-templates/crypt`);
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
  await page.getByLabel("Entity to place").selectOption("crypt");
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
  check("gameplay shapes declare state properties", keys.filter((k) => k.startsWith("map_")).length === 10, keys.join(","));
  const doc = JSON.parse(saved.properties.find((x: { key: string }) => x.key === "canvas_doc").value);
  const gate = doc.shapes.find((s: { label?: string }) => s.label === "Iron Gate");
  check("gate marker saved with icon and description", gate?.kind === "marker" && gate.icon === "door" && gate.description?.includes("rusted"));

  // 6. Property editor contributed by the plugin.
  await page.getByRole("button", { name: "Properties" }).click();
  check("canvas_doc property shows the plugin summary", await page.getByTestId("canvas-property-summary").isVisible());

  // 7. Modify Canvas node in the storylet graph.
  await page.goto(`${base}/#/storylets/unlock/edit`);
  await page.getByRole("button", { name: "Modify Canvas" }).first().click();
  const pluginNode = page.locator(".graph-node.plugin");
  await pluginNode.waitFor();
  await pluginNode.click({ position: { x: 60, y: 50 } });
  const inspector = page.getByTestId("modify-canvas-inspector");
  await inspector.waitFor();
  await inspector.locator("select").first().selectOption("crypt");
  for (const [element, action] of [["Iron Gate (marker)", "Change icon"], ["Key (marker)", "Remove marker"], ["Item (marker)", "Remove marker"], ["Unlock the crypt (storylet)", "Remove reference"]] as const) {
    await inspector.getByLabel("Canvas element").selectOption({ label: element });
    await inspector.getByLabel("Change", { exact: true }).selectOption({ label: action });
    if (action === "Change icon") await inspector.getByLabel("New icon").selectOption("door-open");
    await inspector.getByRole("button", { name: "Add", exact: true }).click();
  }
  check("inspector lists four changes", (await inspector.locator(".mc-ops li").count()) === 4);
  await shot(page, "canvas-03-modify-node");
  // Wire choice -> Modify Canvas -> End by dragging sockets.
  async function connect(from: ReturnType<Page["locator"]>, to: ReturnType<Page["locator"]>) {
    const a = (await from.boundingBox())!; const b = (await to.boundingBox())!;
    await page.mouse.move(a.x + a.width / 2, a.y + a.height / 2); await page.mouse.down();
    await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2, { steps: 8 }); await page.mouse.up();
  }
  await connect(page.locator(".graph-node.choice .output-socket").first(), page.locator(".graph-node.plugin .input-socket"));
  await connect(page.locator(".graph-node.plugin .output-socket"), page.locator(".graph-node.end .input-socket"));
  await shot(page, "canvas-wired");
  await page.getByRole("button", { name: "Save draft" }).click();
  await sleep(800);
  console.log("toast:", (await page.locator(".toast, [role=status], [role=alert]").allTextContents()).join(" | "));
  const stored = await api("GET", "/api/v3/content/storylets/unlock");
  const types = stored.graph.nodes.map((x: { type: string }) => x.type);
  check("stored graph has no plugin nodes (lowered)", !types.includes("plugin"), types.join(","));
  const commands = stored.graph.nodes.filter((x: { type: string }) => x.type === "apply_command");
  check("lowered to four Apply Command nodes", commands.length === 4, String(commands.length));
  check("plugin metadata survives storage", stored.graph.nodes.some((x: { plugin?: { source?: unknown } }) => x.plugin?.source));
  await page.reload();
  await page.locator(".graph-node.plugin").waitFor();
  check("reload raises one Modify Canvas node", (await page.locator(".graph-node.plugin").count()) === 1 && (await page.locator(".graph-node.apply_command").count()) === 0);
  await shot(page, "canvas-04-graph");

  // Run it: the engine executes the lowered nodes against the location's map state.
  const project = await api("GET", "/api/v3/project");
  const run = await api("POST", "/api/v4/runs", { start: { kind: "beginning" }, storyletId: "unlock", expectedRevision: project.revision, idempotencyKey: "r1" });
  const choice = run.view.scene?.choices?.[0];
  check("run reaches the choice", !!choice, JSON.stringify(run.view).slice(0, 200));
  const acted = await api("POST", `/api/v4/runs/${run.id}/act`, { participant: run.view.participant, action: choice.action, expectedRevision: run.revision, idempotencyKey: "a1" })
    .catch((e) => ({ error: String(e) }));
  check("choosing executes Modify Canvas without stalling", !("error" in acted) && !acted.stalled, JSON.stringify(acted).slice(0, 300));
  if (errors.length) console.log("console errors:\n" + errors.join("\n"));
  check("no console errors", errors.length === 0);
} finally {
  await browser?.close();
  child.kill();
}
if (failures.length) { console.log(`\n${failures.length} failure(s)`); process.exit(1); }
console.log("\nall canvas checks passed");
