// Visual test: trait property editors are driven by property `editor` metadata, not trait/property names.
// Run: STORYLET_SERVICE_BIN=<bin> node tests/visual/trait-editor-visual.ts
import { chromium } from "playwright-core";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";

const root = path.resolve(import.meta.dirname, "../..");
const exe = process.platform === "win32" ? ".exe" : "";
const binary = [process.env.STORYLET_SERVICE_BIN, path.join(root, `target/play-clean/debug/storylet-engine-service${exe}`)]
  .filter(Boolean).map((p) => path.resolve(p as string)).find((p) => fs.existsSync(p));
if (!binary) { console.error("storylet-engine-service binary not found; set STORYLET_SERVICE_BIN"); process.exit(1); }
const chromePath = process.env.CHROMIUM_PATH ?? (() => {
  const cache = path.join(os.homedir(), "AppData/Local/ms-playwright");
  if (!fs.existsSync(cache)) return undefined;
  for (const d of fs.readdirSync(cache).filter((d) => d.startsWith("chromium-")).sort().reverse()) {
    const candidate = path.join(cache, d, "chrome-win64/chrome.exe");
    if (fs.existsSync(candidate)) return candidate;
  }
  return undefined;
})();
const port = 47_000 + Math.floor(Math.random() * 1_000);
const base = `http://127.0.0.1:${port}`;
const outDir = path.join(root, "tests/visual/out");
fs.mkdirSync(outDir, { recursive: true });
const child = spawn(binary, ["--fresh-database"], {
  cwd: root, stdio: "ignore", windowsHide: true,
  env: { ...process.env, STORYLET_STUDIO_ADDR: `127.0.0.1:${port}`, STORYLET_STUDIO_DB_PATH: ":memory:", STORYLET_RESOURCE_ROOT: "." },
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

let browser;
try {
  for (let i = 0; i < 50; i++) { try { if ((await fetch(base + "/api/v3/project")).ok) break; } catch { /* starting */ } await sleep(200); }
  await api("POST", "/api/v3/content/gameplay_tags", { id: "guard", name: "guard" });
  await api("POST", "/api/v3/content/gameplay_tags", { id: "thief", name: "thief" });
  const traits = await api("GET", "/api/v3/content/trait_templates");
  const byKey = (key: string) => traits.find((t: { key: string }) => t.key === key);
  const character = byKey("character"), settings = byKey("character_settings"), location = byKey("location");
  check("seeded character_settings carries editor metadata",
    settings.properties[0].editor?.selection_only === true && settings.properties[1].editor?.entity_trait_key === "location");
  // Custom trait with arbitrary key/property names uses the same metadata; a look-alike without metadata gets none.
  await api("POST", "/api/v3/content/trait_templates", { id: "faction_rules", key: "faction_rules", name: "Faction Rules", properties: [
    { id: "p1", key: "standing", name: "Standing", type: "string", default: "", editor: { option_source: "gameplay_tags", selection_only: true } },
    { id: "p2", key: "base", name: "Base", type: "entity", default: null, editor: { entity_trait_key: "location" } },
    { id: "p3", key: "score", name: "Score", type: "int", default: 0, min: 0, max: 10 },
  ] });
  await api("POST", "/api/v3/content/trait_templates", { id: "lookalike", key: "character_settings_copy", name: "Lookalike", properties: [
    { id: "q1", key: "role", name: "Role", type: "string", default: "" },
    { id: "q2", key: "home_location", name: "Home Location", type: "entity", default: null },
  ] });
  await api("POST", "/api/v3/content/entity_templates", { id: "harbor", name: "Harbor", traits: [{ id: "l1", template_id: location.id, values: {} }] });
  await api("POST", "/api/v3/content/entity_templates", { id: "crate", name: "Crate", traits: [] });
  await api("POST", "/api/v3/content/entity_templates", { id: "hero", name: "Hero", traits: [
    { id: "c1", template_id: character.id, values: {} },
    { id: "s1", template_id: settings.id, values: {} },
    { id: "f1", template_id: "faction_rules", values: {} },
    { id: "k1", template_id: "lookalike", values: {} },
  ] });

  browser = await chromium.launch({ executablePath: chromePath, headless: true });
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  const errors: string[] = [];
  page.on("console", (m) => { if (m.type() === "error" && !m.text().startsWith("Failed to load resource")) errors.push(m.text()); });
  page.on("pageerror", (e) => errors.push(String(e)));
  page.on("dialog", (d) => void d.accept());
  await page.goto(`${base}/#/entity-templates/hero`);
  const tab = page.locator(".entity-tab", { hasText: "Traits" });
  await tab.waitFor(); await tab.click();
  const card = (name: string) => page.locator(".trait-card", { has: page.locator(".trait-name", { hasText: new RegExp(`^${name}$`) }) });
  await card("Character Settings").waitFor();
  const options = async (c: ReturnType<typeof card>, label: string) =>
    c.locator(".trait-row", { hasText: label }).locator("select.form-select").first().locator("option").allTextContents();

  const roleSelect = card("Character Settings").locator(".trait-row", { hasText: "Role" }).locator("select");
  check("built-in role renders as selection-only select of gameplay tags",
    JSON.stringify((await roleSelect.locator("option").allTextContents()).map((t) => t.trim())) === JSON.stringify(["(none)", "guard", "thief"]));
  const builtInHome = (await options(card("Character Settings"), "Home Location")).map((t) => t.trim());
  check("built-in location property lists only location entities", builtInHome.includes("Harbor") && !builtInHome.includes("Crate"), builtInHome.join(","));
  const customStanding = (await card("Faction Rules").locator(".trait-row", { hasText: "Standing" }).locator("select option").allTextContents()).map((t) => t.trim());
  check("custom trait with metadata behaves like built-in (tag select)", customStanding.join() === "(none),guard,thief", customStanding.join(","));
  const customBase = (await options(card("Faction Rules"), "Base")).map((t) => t.trim());
  check("custom entity property honors trait filter", customBase.includes("Harbor") && !customBase.includes("Crate"), customBase.join(","));
  check("look-alike trait role is a plain text input", await card("Lookalike").locator(".trait-row", { hasText: "Role" }).locator("input").count() > 0
    && await card("Lookalike").locator(".trait-row", { hasText: "Role" }).locator("select").count() === 0);
  const lookHome = (await options(card("Lookalike"), "Home Location")).map((t) => t.trim());
  check("look-alike entity property is unfiltered", lookHome.includes("Harbor") && lookHome.includes("Crate"), lookHome.join(","));

  // Interaction: choose a role and a location, save, reload, values persist.
  await roleSelect.selectOption("thief");
  await card("Character Settings").locator(".trait-row", { hasText: "Home Location" }).locator("select").first().selectOption("harbor");
  await page.locator(".entity-unsaved-pill").waitFor();
  await page.screenshot({ path: path.join(outDir, "trait-editor-unsaved.png") });
  await page.getByRole("button", { name: /^Save/ }).first().click();
  await page.locator(".entity-unsaved-pill").waitFor({ state: "detached" });
  const saved = await api("GET", "/api/v3/content/entity_templates/hero");
  const vals = saved.traits.find((t: { id: string }) => t.id === "s1").values;
  check("selection persisted to backend", vals.role === "thief" && vals.home_location === "harbor", JSON.stringify(vals));
  await page.reload(); await tab.waitFor(); await tab.click(); await card("Character Settings").waitFor();
  check("selection restored after reload", (await roleSelect.inputValue()) === "thief");
  await page.screenshot({ path: path.join(outDir, "trait-editor-saved.png") });

  // Numeric bounds from property metadata clamp typed, stepped and keyboard values in the editor.
  const score = card("Faction Rules").locator(".trait-row", { hasText: "Score" });
  const scoreInput = score.locator(".num-stepper__input");
  await scoreInput.fill("99");
  check("typed value above max is clamped", (await scoreInput.inputValue()) === "10", await scoreInput.inputValue());
  await scoreInput.fill("-5");
  check("typed value below min is clamped", (await scoreInput.inputValue()) === "0", await scoreInput.inputValue());
  await score.locator('button[aria-label="Decrease value"]').dispatchEvent("pointerdown");
  await score.locator('button[aria-label="Decrease value"]').dispatchEvent("pointerup");
  await scoreInput.press("ArrowDown");
  check("stepping below min stays at min", (await scoreInput.inputValue()) === "0", await scoreInput.inputValue());
  await scoreInput.fill("10");
  await scoreInput.press("ArrowUp");
  check("arrow above max stays at max", (await scoreInput.inputValue()) === "10", await scoreInput.inputValue());
  await page.screenshot({ path: path.join(outDir, "trait-editor-bounds.png") });
  await page.getByRole("button", { name: /^Save/ }).first().click();
  await page.locator(".entity-unsaved-pill").waitFor({ state: "detached" });
  const bounded = await api("GET", "/api/v3/content/entity_templates/hero");
  check("clamped value saves successfully", bounded.traits.find((t: { id: string }) => t.id === "f1").values.score === 10);

  // Fresh-project inventory plugin: owns its two traits; bounded grid size clamps in the shared editor.
  await page.goto(`${base}/#/entity-templates/crate`);
  let inv: { id: string } | undefined, item: { id: string; unique?: boolean } | undefined;
  for (let i = 0; i < 50 && !(inv && item); i++) {
    const all = await api("GET", "/api/v3/content/trait_templates");
    inv = all.find((t: { key: string }) => t.key === "inventory_owner"); item = all.find((t: { key: string }) => t.key === "item");
    if (!(inv && item)) await sleep(200);
  }
  check("inventory plugin provisions fresh traits (Item unique)", !!inv && !!item && item.unique === true);
  await api("POST", "/api/v3/content/entity_templates", { id: "bag", name: "Bag", traits: [{ id: "b1", template_id: inv.id, values: {} }] });
  await page.goto(`${base}/#/entity-templates/bag`);
  await page.reload();
  await tab.waitFor(); await tab.click();
  const columns = card("InventoryOwner").locator(".trait-row", { hasText: "Columns" }).locator(".num-stepper__input");
  await columns.fill("50");
  check("inventory columns clamp to declared max", (await columns.inputValue()) === "20", await columns.inputValue());
  await columns.fill("0");
  check("inventory columns clamp to declared min", (await columns.inputValue()) === "1", await columns.inputValue());
  await page.screenshot({ path: path.join(outDir, "trait-editor-inventory.png") });
  await page.goto(`${base}/#/entity-templates/hero`);

  // Backend rejects an out-of-range bounded value.
  const bad = await fetch(`${base}/api/v3/content/entity_templates`, { method: "POST", headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ ...saved, traits: saved.traits.map((t: { id: string }) => t.id === "f1" ? { ...t, values: { score: 11 } } : t) }) });
  check("out-of-range bounded value is rejected", bad.status >= 400, String(bad.status));
  check("no unexpected console errors", errors.length === 0, errors.join(" | "));
} catch (error) {
  failures.push(`exception: ${error instanceof Error ? error.stack : String(error)}`);
  console.log(failures.at(-1));
} finally {
  await browser?.close();
  child.kill();
}
if (failures.length) { console.error(`${failures.length} failed`); process.exit(1); }
console.log("all trait editor visual checks passed");
