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
  for (let i = 0; i < 50; i++) { try { if ((await fetch(base + "/api/v3/project")).ok) break; } catch { } await sleep(200); }
  const traits = await api("GET", "/api/v3/content/trait_templates");
  const character = traits.find((t: { key: string }) => t.key === "character");
  await api("POST", "/api/v3/content/entities", { id: "knight", name: "Knight", description: "Armored", traits: [{ id: "k1", template_id: character.id, values: {} }] });
  await api("POST", "/api/v3/content/entities", { id: "rock", name: "Rock", traits: [] });

  browser = await chromium.launch({ executablePath: chromePath, headless: true });
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  const errors: string[] = [];
  page.on("console", (m) => { if (m.type() === "error" && !m.text().startsWith("Failed to load resource")) errors.push(m.text()); });
  page.on("pageerror", (e) => errors.push(String(e)));
  let nativeDialogs = 0;
  page.on("dialog", (d) => { nativeDialogs++; void d.dismiss(); });
  await page.goto(`${base}/#/entities/knight`);
  await page.locator(".project-tree__rows").waitFor();
  check("tree has no Entity Templates category", (await page.locator(".project-tree__row", { hasText: "Entity Templates" }).count()) === 0);
  const row = (name: string) => page.locator(".project-tree__row--item", { hasText: name }).first();
  await row("Knight").waitFor();
  check("plain entity has no template badge", (await row("Knight").locator(".project-tree__template-badge").count()) === 0);
  check("header shows Standalone Entity before toggle", (await page.locator(".template-pill").count()) === 0 && (await page.locator(".entity-standalone-text").count()) === 1);

  await row("Knight").click({ button: "right" });
  const toggle = page.locator('.project-tree__context-menu [aria-label="Use as template"]');
  await toggle.waitFor();
  check("context menu item unchecked initially", (await toggle.getAttribute("aria-checked")) === "false");
  await page.screenshot({ path: path.join(outDir, "entity-template-menu.png") });
  await toggle.click();
  await page.locator(".template-pill").waitFor();
  check("header shows Template indicator", (await page.locator(".template-pill").innerText()).trim() === "Template");
  check("tree row shows template badge", (await row("Knight").locator(".project-tree__template-badge").count()) === 1);
  check("backend persisted is_template", (await api("GET", "/api/v3/content/entities/knight")).is_template === true);
  await page.screenshot({ path: path.join(outDir, "entity-template-header.png") });
  await row("Knight").click({ button: "right" });
  check("context menu item checked after toggle", (await page.locator('.project-tree__context-menu [aria-label="Use as template"]').getAttribute("aria-checked")) === "true");
  await page.keyboard.press("Escape");
  await page.mouse.click(700, 500);

  const before = (await api("GET", "/api/v3/content/entities")).length;
  const entitiesRoot = page.locator(".project-tree__row--root", { hasText: "Entities" }).first();
  await entitiesRoot.hover();
  await entitiesRoot.getByRole("button", { name: /Actions for Entities|New/ }).first().click();
  await page.locator(".project-tree__menu-item").first().click();
  const picker = page.getByRole("dialog", { name: "Choose template" });
  await picker.waitFor();
  const options = (await picker.getByRole("option").allInnerTexts()).map((t) => t.replace(/\s+/g, " ").trim());
  check("picker lists None then only template entities", options.length === 2 && /None/.test(options[0]) && /Knight/.test(options[1]) && !options.some((o) => /Rock/.test(o)), options.join(" | "));
  await page.screenshot({ path: path.join(outDir, "entity-template-picker.png") });
  await page.keyboard.press("Escape");
  await sleep(300);
  check("Escape creates nothing", (await api("GET", "/api/v3/content/entities")).length === before);

  await entitiesRoot.hover();
  await entitiesRoot.getByRole("button", { name: /Actions for Entities|New/ }).first().click();
  await page.locator(".project-tree__menu-item").first().click();
  await picker.waitFor();
  await picker.getByRole("option").first().click();
  await page.waitForURL(/#\/entities\/(?!knight)/);
  const afterNone = await api("GET", "/api/v3/content/entities");
  const none = afterNone.find((e: { name: string }) => e.name === "New Entity");
  check("None creates an empty entity without template", !!none && none.template_id === null && none.traits.length === 0 && none.is_template === false, JSON.stringify(none));

  await entitiesRoot.hover();
  await entitiesRoot.getByRole("button", { name: /Actions for Entities|New/ }).first().click();
  await page.locator(".project-tree__menu-item").first().click();
  await picker.waitFor();
  await picker.getByRole("option", { name: /Knight/ }).click();
  await page.waitForURL(/#\/entities\/(?!knight)/);
  await sleep(1000); await page.screenshot({ path: path.join(outDir, "entity-template-instance.png") });
  await page.locator(".entity-parent-link").waitFor({ timeout: 4000 });
  check("header shows From <template>", (await page.locator(".entity-parent-link").innerText()).includes("Knight"));
  const fromTpl = (await api("GET", "/api/v3/content/entities")).find((e: { name: string }) => e.name === "New Knight");
  check("template choice copies traits and sets template_id", !!fromTpl && fromTpl.template_id === "knight" && fromTpl.traits.length === 1 && fromTpl.traits[0].id !== "k1", JSON.stringify(fromTpl));
  await page.screenshot({ path: path.join(outDir, "entity-template-instance.png") });

  await row("New Knight").click({ button: "right" });
  check("instance cannot be marked as template in the menu", await page.locator('.project-tree__context-menu [aria-label="Use as template"]').isDisabled());
  await page.keyboard.press("Escape");
  await page.mouse.click(700, 500);

  await row("Rock").click();
  await page.getByRole("checkbox", { name: "Use as template" }).check();
  await page.locator(".entity-unsaved-pill").waitFor();
  await row("Rock").click({ button: "right" });
  await page.locator('.project-tree__context-menu [aria-label="Use as template"]').click();
  await page.locator(".entity-unsaved-pill").waitFor({ state: "detached", timeout: 5000 });
  check("tree toggle clears matching unsaved template edit", (await api("GET", "/api/v3/content/entities/rock")).is_template === true);

  await row("Knight").click({ button: "right" });
  await page.locator('.project-tree__context-menu [aria-label="Use as template"]').click();
  await sleep(600);
  check("referenced template cannot be unmarked", (await api("GET", "/api/v3/content/entities/knight")).is_template === true);
  check("no native prompt dialogs used", nativeDialogs === 0, String(nativeDialogs));
  check("no unexpected console errors", errors.length === 0, errors.join(" | "));
} catch (error) {
  failures.push(`exception: ${error instanceof Error ? error.stack : String(error)}`);
  console.log(failures.at(-1));
} finally {
  await browser?.close();
  child.kill();
}
if (failures.length) { console.error(`${failures.length} failed`); process.exit(1); }
console.log("all entity template visual checks passed");
