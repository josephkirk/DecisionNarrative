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
  const longName = "The Grandmaster Archivist of the Forgotten Constellation";
  for (const [id, name] of [["pointer-root", "Root"], ["pointer-child", "Child"], ["pointer-long", longName]]) {
    await api("POST", "/api/v3/content/entity_templates", { id, name, thumbnail: id === "pointer-root" ? "docs/images/tree-pointer-inspector.png" : null, traits: [{ id: `${id}-trait`, template_id: itemTrait.id, values: {} }] });
  }
  await api("POST", "/api/v3/content/entity_templates", { id: "pointer-tree", name: "Skill tree", traits: [{ id: "tree-instance", template_id: treeTrait.id, values: { nodes: ["pointer-root", "pointer-child", "pointer-long"], edges: ['["pointer-root","pointer-child"]', '["pointer-root","pointer-long"]'], direction: "right", positions: JSON.stringify({ right: { "pointer-root": { x: -200, y: -100 }, "pointer-child": { x: 60, y: -100 }, "pointer-long": { x: 60, y: 30 } } }), appearance: JSON.stringify({ nodes: { "pointer-long": { background: "#ffffff" } }, edges: {}, defaults: { edge: { color: "#ff9900", width: 3, pattern: "dash" } } }) } }] });
  await api("POST", "/api/v3/content/entity_templates", { id: "second-tree", name: "Second tree", traits: [{ id: "second-tree-instance", template_id: treeTrait.id, values: { nodes: ["pointer-root"], edges: [], direction: "right" } }] });
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
  assert(await picker.getByRole("radio", { name: "Select Root" }).getAttribute("aria-checked") === "true", "configured position is highlighted");
  const thumbnail = picker.getByRole("radio", { name: "Select Root" }).locator("image");
  assert((await thumbnail.getAttribute("href"))?.includes("tree-pointer-inspector.png"), "tree preview shows the item's thumbnail");
  const wireStyle = await picker.locator(".tree-pointer-wire").first().evaluate((element) => ({ color: getComputedStyle(element).stroke, dash: getComputedStyle(element).strokeDasharray }));
  assert(wireStyle.color === "rgb(255, 153, 0)" && wireStyle.dash !== "none", "tree preview preserves authored wire color and pattern");
  const viewport = await picker.boundingBox();
  const negativeNode = await picker.getByRole("radio", { name: "Select Root" }).boundingBox();
  assert(viewport && negativeNode && negativeNode.x >= viewport.x && negativeNode.y >= viewport.y, "nodes with negative coordinates are visible");
  const longLabel = picker.getByRole("radio", { name: `Select ${longName}` }).locator(".tree-pointer-label span");
  const labelStyle = await longLabel.evaluate((element) => ({ overflow: element.scrollWidth > element.clientWidth, ellipsis: getComputedStyle(element).textOverflow, color: getComputedStyle(element).color }));
  assert(labelStyle.overflow && labelStyle.ellipsis === "ellipsis", "long names retain their beginning with an ellipsis");
  assert(labelStyle.color === "rgb(17, 19, 23)", "light node backgrounds use dark readable text");
  await picker.getByRole("radio", { name: "Select Child" }).click();
  assert(await picker.getByRole("radio", { name: "Select Child" }).getAttribute("aria-checked") === "true", "click selects position in the read-only tree");
  await picker.getByRole("radio", { name: "Select Child" }).focus();
  await page.keyboard.press("ArrowLeft");
  assert(await picker.getByRole("radio", { name: "Select Root" }).getAttribute("aria-checked") === "true", "arrow keys change the selected position");
  await page.keyboard.press("ArrowRight");
  assert(await picker.locator('[role="radio"][tabindex="0"]').count() === 1, "tree selection has one keyboard tab stop");
  await page.getByRole("button", { name: "Zoom in tree preview" }).click();
  const zoomed = await picker.getByRole("radio", { name: "Select Child" }).boundingBox();
  const zoomViewport = await picker.boundingBox();
  assert(zoomed && zoomViewport && zoomed.x < zoomViewport.x + zoomViewport.width && zoomed.x + zoomed.width > zoomViewport.x,
    "zoom keeps the selected node visible");
  await page.getByRole("button", { name: "Fit tree preview" }).click();
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
  await page.locator(".node-title-input").nth(1).click();
  await page.locator("select.form-select").selectOption("second-tree");
  await page.locator(".node-title-input").nth(3).click();
  const changedTreeWarning = page.getByRole("alert").filter({ hasText: "The connected tree changed" });
  assert(await changedTreeWarning.isVisible(), "changing the connected tree explains that a new position is required");
  await picker.getByRole("radio", { name: "Select Root" }).click();
  assert(await changedTreeWarning.count() === 0, "choosing a position in the new tree clears the warning");
  const removed = structuredClone(after);
  removed.traits[0].values.nodes = ["pointer-root", "pointer-long"];
  removed.traits[0].values.edges = ['["pointer-root","pointer-long"]'];
  removed.traits[0].values.positions = "";
  let removeError = "";
  try { await api("PUT", "/api/v3/content/entity_templates/pointer-tree", removed); } catch (error) { removeError = String(error); }
  assert(removeError.includes("storylet") && removeError.includes("removed"), "removing a referenced position is rejected");
  const noTrait = { ...after, traits: [] };
  let traitError = "";
  try { await api("PUT", "/api/v3/content/entity_templates/pointer-tree", noTrait); } catch (error) { traitError = String(error); }
  assert(traitError.includes("storylet"), "removing a referenced TreeGraph trait is rejected");
  await api("POST", "/api/v3/content/entity_templates", { id: "impostor", name: "Not a tree", properties: [
    { id: "p", key: "__tree_position", name: "position", type: "string", value: "" },
    { id: "n", key: "__tree_nodes", name: "nodes", type: "entity", is_array: true, value: ["pointer-root"] },
  ] });
  const fake = structuredClone(saved);
  fake.id = "impostor-story";
  fake.title = "Impostor";
  fake.graph.nodes.find((node: { type: string }) => node.type === "get_entity").entity_path = "impostor";
  fake.graph.nodes.find((node: { type: string }) => node.type === "apply_command").value = "pointer-root";
  let spoofError = "";
  try { await api("POST", "/api/v3/content/storylets", fake); } catch (error) { spoofError = String(error); }
  assert(spoofError.includes("TreeGraph"), "authored properties cannot impersonate the TreeGraph trait");
  const world = await api("GET", "/api/v3/content/entities/world");
  let worldError = "";
  try {
    await api("PUT", "/api/v3/content/entities/world", { ...world, traits: [{ id: "world-tree", template_id: treeTrait.id, values: { nodes: ["pointer-root"], edges: [], direction: "right" } }] });
  } catch (error) { worldError = String(error); }
  assert(worldError.includes("intrinsic World"), "intrinsic World rejects a TreeGraph trait");
  await api("POST", "/api/v3/content/entities", { id: "pointer-instance", name: "Concrete tree", traits: [
    { id: "instance-tree", template_id: treeTrait.id, values: { nodes: ["pointer-root", "pointer-child"], edges: [], direction: "right" } }
  ] });
  const instanceStory = structuredClone(saved);
  instanceStory.id = "instance-story";
  instanceStory.title = "Concrete owner story";
  instanceStory.graph.nodes.find((node: { type: string }) => node.type === "get_entity").entity_path = "pointer-instance";
  await api("POST", "/api/v3/content/storylets", instanceStory);
  let deleteError = "";
  try { await api("DELETE", "/api/v3/content/entities/pointer-instance"); } catch (error) { deleteError = String(error); }
  assert(deleteError.includes("storylet"), "referenced concrete tree owners cannot be deleted");
  await page.reload();
  await picker.waitFor();
  assert(await picker.getByRole("radio", { name: "Select Child" }).getAttribute("aria-checked") === "true", "selected position survives reload");
  await api("POST", "/api/v3/content/entity_templates", { id: "pointer-location", name: "Location", traits: [{ id: "location", template_id: "system.location", values: {} }] });
  await api("POST", "/api/v3/content/scenarios", { id: "pointer-scenario", name: "Scenario", seed: 7, start_location_policy: { mode: "fixed", location_id: "pointer-location" } });
  let scenarioError = "";
  try {
    await api("POST", "/api/v3/content/scenarios", { id: "override", name: "Override", seed: 7,
      start_location_policy: { mode: "fixed", location_id: "pointer-location" },
      initial_properties: { "pointer-tree": { "__tree_position": "missing" } } });
  } catch (error) { scenarioError = String(error); }
  assert(scenarioError.includes("internal tree state"), "scenarios cannot override the tree pointer");
  const run = await api("POST", "/api/v4/runs", {
    start: { kind: "beginning" }, storyletId: "pointer-story", expectedRevision: (await api("GET", "/api/v3/project")).revision, idempotencyKey: "tree-pointer-visual"
  });
  assert(typeof run.id === "string", "saved pointer story starts in the real playtest service");
  await page.reload();
  await picker.waitFor();
  await page.locator(".graph-node.get_entity .node-body").click();
  await page.locator(".property-inspector-panel select").selectOption("world");
  await page.locator(".graph-node.plugin .node-body").click();
  assert(await page.getByText("The intrinsic World cannot own a TreeGraph.").isVisible(), "World input explains that it cannot own a tree");
  await page.locator(".graph-node.get_entity .node-body").click();
  await page.locator(".property-inspector-panel select").selectOption("impostor");
  await page.locator(".graph-node.plugin .node-body").click();
  assert(await page.getByText("Not a tree does not have the TreeGraph trait.").isVisible(), "non-tree input explains the missing trait");

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
  assert(await picker.getByRole("radio", { name: "Select Child" }).getAttribute("aria-checked") === "true", "boolean check restores its selected position");
  await picker.getByRole("radio", { name: "Select Root" }).click();
  await Promise.all([
    page.waitForResponse((response) => response.url().endsWith("/api/v3/content/storylets/pointer-check") && response.request().method() === "PUT" && response.ok()),
    page.getByRole("button", { name: "Save draft" }).click(),
  ]);
  const savedCheck = await api("GET", "/api/v3/content/storylets/pointer-check");
  assert(savedCheck.graph.nodes.find((node: { id: string; value?: unknown }) => node.id === "check__compare")?.value === "pointer-root", "boolean check saves its selected position");
  const removeChecked = structuredClone(after);
  removeChecked.traits[0].values.nodes = ["pointer-child", "pointer-long"];
  removeChecked.traits[0].values.edges = [];
  removeChecked.traits[0].values.positions = "";
  let checkRemovalError = "";
  try { await api("PUT", "/api/v3/content/entity_templates/pointer-tree", removeChecked); } catch (error) { checkRemovalError = String(error); }
  assert(checkRemovalError.includes("checks a removed"), "removing a position used by a boolean check is rejected");
  const extraIds = Array.from({ length: 12 }, (_, index) => `skill-${index}`);
  for (const id of extraIds) {
    await api("POST", "/api/v3/content/entity_templates", { id, name: `Skill ${id}`, traits: [{ id: `${id}-trait`, template_id: itemTrait.id, values: {} }] });
  }
  const large = structuredClone(after);
  const largeNodes = ["pointer-root", "pointer-child", "pointer-long", ...extraIds];
  large.traits[0].values.nodes = largeNodes;
  large.traits[0].values.edges = largeNodes.slice(1).map((id: string, index: number) => JSON.stringify([largeNodes[index], id]));
  large.traits[0].values.positions = "";
  await api("PUT", "/api/v3/content/entity_templates/pointer-tree", large);
  await page.reload();
  await picker.waitFor();
  const zoomIn = page.getByRole("button", { name: "Zoom in tree preview" });
  await zoomIn.click();
  await zoomIn.click();
  assert(await zoomIn.isEnabled(), "large trees can zoom beyond four times fit");
  await zoomIn.click();
  const chosen = await picker.getByRole("radio", { name: "Select Root" }).boundingBox();
  const region = await picker.boundingBox();
  assert(chosen && region && chosen.x < region.x + region.width && chosen.x + chosen.width > region.x,
    "large-tree zoom keeps the selected position visible");
  await picker.getByRole("radio", { name: "Select Root" }).focus();
  await page.keyboard.press("ArrowLeft");
  const last = await picker.getByRole("radio", { name: "Select Skill skill-11" }).boundingBox();
  const visibleRegion = await picker.boundingBox();
  assert(last && visibleRegion && last.x < visibleRegion.x + visibleRegion.width && last.x + last.width > visibleRegion.x,
    "keyboard navigation scrolls the selected tree position into view");
  large.traits[0].values.direction = "down";
  await api("PUT", "/api/v3/content/entity_templates/pointer-tree", large);
  await page.reload();
  await picker.waitFor();
  await page.getByRole("button", { name: "Fit tree preview" }).click();
  const fitDimensions = await picker.evaluate((element) => ({ height: element.clientHeight, contentHeight: element.scrollHeight, svg: element.querySelector("svg")?.getBoundingClientRect().toJSON(), css: element.querySelector("svg")?.getAttribute("style") }));
  assert(fitDimensions.contentHeight <= fitDimensions.height + 1, `Fit shows the complete vertical tree without scrolling: ${JSON.stringify(fitDimensions)}`);
  await page.getByRole("button", { name: "Set Tree Position", exact: true }).first().click();
  assert(await page.getByText("Connect a Get Entity node to choose a tree.").isVisible(), "unconnected node explains how to choose a tree");
  assert(errors.length === 0, `no browser errors: ${errors.join("; ")}`);
} finally { await browser?.close(); child.kill(); }
