import { chromium } from "playwright-core";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";

const root = path.resolve(import.meta.dirname, "../..");
const exe = process.platform === "win32" ? ".exe" : "";
const binary = path.resolve(process.env.STORYLET_SERVICE_BIN ?? `target/tree-graph/debug/storylet-engine-service${exe}`);
const cache = path.join(os.homedir(), "AppData/Local/ms-playwright");
const chrome = process.env.CHROMIUM_PATH ?? (fs.existsSync(cache) ? fs.readdirSync(cache).filter((name) => name.startsWith("chromium-")).sort().reverse().map((name) => path.join(cache, name, "chrome-win64/chrome.exe")).find(fs.existsSync) : undefined);
const port = 47000 + Math.floor(Math.random() * 1000);
const base = `http://127.0.0.1:${port}`;
const child = spawn(binary, ["--fresh-database"], { cwd: root, env: { ...process.env, STORYLET_STUDIO_ADDR: `127.0.0.1:${port}`, STORYLET_STUDIO_DB_PATH: ":memory:", STORYLET_RESOURCE_ROOT: "." }, stdio: "ignore", windowsHide: true });
async function api(method: string, route: string, body?: unknown) {
  const response = await fetch(base + route, { method, headers: { "Content-Type": "application/json" }, body: body === undefined ? undefined : JSON.stringify(body) });
  const text = await response.text();
  if (!response.ok) throw new Error(`${method} ${route}: ${response.status} ${text}`);
  return text ? JSON.parse(text) : null;
}
function assert(ok: unknown, message: string) { if (!ok) throw new Error(message); console.log(`PASS ${message}`); }
let browser;
try {
  for (let i = 0; i < 60; i++) { try { if ((await fetch(base + "/api/v3/project")).ok) break; } catch {} await sleep(200); }
  browser = await chromium.launch({ executablePath: chrome, headless: true });
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  page.on("dialog", (dialog) => void dialog.accept());
  await page.goto(`${base}/#/storylets`);
  let traits: Array<{ id: string; key: string }> = [];
  for (let i = 0; i < 50; i++) { traits = await api("GET", "/api/v3/content/trait_templates"); if (traits.some((trait) => trait.key === "tree_graph") && traits.some((trait) => trait.key === "tree_item")) break; await sleep(200); }
  const treeTrait = traits.find((trait) => trait.key === "tree_graph");
  const itemTrait = traits.find((trait) => trait.key === "tree_item");
  assert(treeTrait && itemTrait, "tree traits available");
  for (const [id, name] of [["pointer-root", "Root"], ["pointer-child", "Child"]]) {
    await api("POST", "/api/v3/content/entity_templates", { id, name, traits: [{ id: `${id}-trait`, template_id: itemTrait.id, values: {} }] });
  }
  await api("POST", "/api/v3/content/entity_templates", { id: "pointer-tree", name: "Skill tree", traits: [{ id: "tree-instance", template_id: treeTrait.id, values: { nodes: ["pointer-root", "pointer-child"], edges: ['["pointer-root","pointer-child"]'], direction: "right", positions: JSON.stringify({ right: { "pointer-root": { x: -200, y: -100 }, "pointer-child": { x: 60, y: -100 } } }) } }] });
  const source = { id: "pointer", type: "plugin", title: "Set Tree Position", x: 300, y: 150, width: 220, height: 120, plugin_type: "tree.set_position", plugin_config: { entity_id: "pointer-tree", target_id: "pointer-root" } };
  const input = { id: "owner-input", kind: "data", from_node: "owner", from_port: "output", to_node: "pointer", to_port: "entity" };
  const n = (id: string, type: string, x: number, extra: object = {}) => ({ id, type, title: id, x, y: 150, width: 180, height: 100, ...extra });
  await api("POST", "/api/v3/content/storylets", { id: "pointer-story", title: "Tree pointer", content: { text_template: "x" }, created_at: 1, graph: { nodes: [
    n("start", "start", 30, { text: "Start" }), n("owner", "get_entity", 40, { entity_path: "pointer-tree" }),
    n("pointer__position", "apply_command", 300, { property_key: "__tree_position", operation: "set", value_type: "string", value: "pointer-root", plugin: { group: "pointer", source, exit: "pointer__position", inputs: [input] } }),
    n("end", "end", 600, { is_terminal: true })
  ], edges: [
    { id: "flow-1", kind: "flow", from_node: "start", from_port: "output", to_node: "pointer__position", to_port: "input" },
    { id: "flow-2", kind: "flow", from_node: "pointer__position", from_port: "output", to_node: "end", to_port: "input" },
    { ...input, to_node: "pointer__position" }
  ] } });
  const before = await api("GET", "/api/v3/content/entity_templates/pointer-tree");
  await page.goto(`${base}/#/storylets/pointer-story/edit?node=pointer`);
  await page.reload();
  const picker = page.getByRole("region", { name: "Tree positions" });
  await picker.waitFor({ timeout: 5000 }).catch(async (error) => {
    console.error("Page:", await page.locator("body").innerText(), "Errors:", errors);
    await page.screenshot({ path: path.join(os.tmpdir(), "tree-pointer-failure.png") });
    throw error;
  });
  assert(await picker.getByRole("button", { name: "Select Root" }).getAttribute("aria-pressed") === "true", "configured position is highlighted");
  const viewport = await picker.boundingBox();
  const negativeNode = await picker.getByRole("button", { name: "Select Root" }).boundingBox();
  assert(viewport && negativeNode && negativeNode.x >= viewport.x && negativeNode.y >= viewport.y, "nodes with negative coordinates are visible");
  await picker.getByRole("button", { name: "Select Child" }).click();
  assert(await picker.getByRole("button", { name: "Select Child" }).getAttribute("aria-pressed") === "true", "click selects position in the read-only tree");
  if (process.env.TREE_POINTER_SCREENSHOT) await page.screenshot({ path: process.env.TREE_POINTER_SCREENSHOT });
  await Promise.all([
    page.waitForResponse((response) => response.url().endsWith("/api/v3/content/storylets/pointer-story") && response.request().method() === "PUT" && response.ok(), { timeout: 5000 }).catch(async (error) => {
      console.error("Save state:", await page.locator("body").innerText());
      throw error;
    }),
    page.getByRole("button", { name: "Save draft" }).click(),
  ]);
  const saved = await api("GET", "/api/v3/content/storylets/pointer-story");
  assert(saved.graph.nodes.find((node: { id: string; value?: unknown }) => node.id === "pointer__position")?.value === "pointer-child", "storylet saves the selected target");
  const after = await api("GET", "/api/v3/content/entity_templates/pointer-tree");
  assert(JSON.stringify(before.traits) === JSON.stringify(after.traits), "selection does not modify the tree");
  await page.reload();
  await picker.waitFor();
  assert(await picker.getByRole("button", { name: "Select Child" }).getAttribute("aria-pressed") === "true", "selected position survives reload");
  await api("POST", "/api/v3/content/entity_templates", { id: "pointer-location", name: "Location", traits: [{ id: "location", template_id: "system.location", values: {} }] });
  await api("POST", "/api/v3/content/scenarios", { id: "pointer-scenario", name: "Scenario", seed: 7, start_location_policy: { mode: "fixed", location_id: "pointer-location" } });
  const run = await api("POST", "/api/v4/runs", {
    start: { kind: "beginning" }, storyletId: "pointer-story", expectedRevision: (await api("GET", "/api/v3/project")).revision, idempotencyKey: "tree-pointer-visual"
  });
  assert(typeof run.id === "string", "saved pointer story starts in the real playtest service");
  await page.locator(".graph-node.get_entity .node-body").click();
  await page.locator(".property-inspector-panel select").selectOption("world");
  await page.locator(".graph-node.plugin .node-body").click();
  assert(await page.getByText("World does not have the TreeGraph trait.").isVisible(), "non-tree input explains the missing trait");

  const checkSource = { ...source, id: "check", title: "Is Tree Position", plugin_type: "tree.is_position", plugin_config: { entity_id: "pointer-tree", target_id: "pointer-child" } };
  const checkInput = { ...input, id: "check-input", to_node: "check" };
  await api("POST", "/api/v3/content/storylets", { id: "pointer-check", title: "Check tree pointer", content: { text_template: "x" }, created_at: 1, graph: { nodes: [
    n("start", "start", 20, { text: "Start" }), n("owner", "get_entity", 20, { entity_path: "pointer-tree" }),
    n("check__position", "get_entity_property", 250, { property_key: "__tree_position", value_type: "string", plugin: { group: "check", source: checkSource, exit: "check__compare", inputs: [checkInput] } }),
    n("check__compare", "compare", 275, { value_type: "string", operator: "eq", value: "pointer-child", plugin: { group: "check" } }),
    n("condition", "condition", 500), n("yes", "end", 720, { is_terminal: true }), n("no", "end", 720, { is_terminal: true })
  ], edges: [
    { id: "f1", kind: "flow", from_node: "start", from_port: "output", to_node: "condition", to_port: "input" },
    { id: "f2", kind: "flow", from_node: "condition", from_port: "true", to_node: "yes", to_port: "input" },
    { id: "f3", kind: "flow", from_node: "condition", from_port: "false", to_node: "no", to_port: "input" },
    { ...checkInput, to_node: "check__position" },
    { id: "value", kind: "data", from_node: "check__position", from_port: "output", to_node: "check__compare", to_port: "input" },
    { id: "result", kind: "data", from_node: "check__compare", from_port: "output", to_node: "condition", to_port: "input" }
  ] } });
  await page.goto(`${base}/#/storylets/pointer-check/edit?node=check`);
  await page.reload();
  await picker.waitFor();
  assert(await picker.getByRole("button", { name: "Select Child" }).getAttribute("aria-pressed") === "true", "boolean check restores its selected position");
  await picker.getByRole("button", { name: "Select Root" }).click();
  await Promise.all([
    page.waitForResponse((response) => response.url().endsWith("/api/v3/content/storylets/pointer-check") && response.request().method() === "PUT" && response.ok()),
    page.getByRole("button", { name: "Save draft" }).click(),
  ]);
  const savedCheck = await api("GET", "/api/v3/content/storylets/pointer-check");
  assert(savedCheck.graph.nodes.find((node: { id: string; value?: unknown }) => node.id === "check__compare")?.value === "pointer-root", "boolean check saves its selected position");
  await page.getByRole("button", { name: "Set Tree Position", exact: true }).first().click();
  assert(await page.getByText("Connect a Get Entity node to choose a tree.").isVisible(), "unconnected node explains how to choose a tree");
  assert(errors.length === 0, `no browser errors: ${errors.join("; ")}`);
} finally { await browser?.close(); child.kill(); }
