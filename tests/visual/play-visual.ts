// Visual interaction test for the Play view (plan Task 2.4, step 6).
// Run: bun run test:visual   (runs under Node: Playwright cannot drive Chromium from Bun).
// Needs `bun run build` first and a built storylet-engine-service binary (STORYLET_SERVICE_BIN).
// Spawns the service on a temp database, seeds one storylet, drives a real Chromium, takes screenshots
// into tests/visual/out/ and asserts the behaviour of the script in the plan.
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
  path.join(root, `crates/storylet-engine-service/target/debug/storylet-engine-service${exe}`),
].filter(Boolean).map((p) => path.resolve(p as string)).find((p) => fs.existsSync(p));
if (!binary) throw new Error("storylet-engine-service binary not found; set STORYLET_SERVICE_BIN");

const chromePath = process.env.CHROMIUM_PATH ?? (() => {
  const cache = path.join(os.homedir(), "AppData/Local/ms-playwright");
  if (!fs.existsSync(cache)) return undefined;
  const dirs = fs.readdirSync(cache).filter((d) => d.startsWith("chromium-")).sort().reverse();
  for (const d of dirs) {
    const candidate = path.join(cache, d, "chrome-win64/chrome.exe");
    if (fs.existsSync(candidate)) return candidate;
  }
  return undefined;
})();

const port = 39_000 + Math.floor(Math.random() * 1_000);
const base = `http://127.0.0.1:${port}`;
const dbPath = path.join(os.tmpdir(), `dn-visual-${port}.db`);
const child: ChildProcess = spawn(binary, ["--fresh-database"], {
  cwd: root,
  env: { ...process.env, STORYLET_STUDIO_ADDR: `127.0.0.1:${port}`, STORYLET_STUDIO_DB_PATH: dbPath, STORYLET_RESOURCE_ROOT: "." },
  stdio: "ignore", windowsHide: true,
});

async function api(method: string, p: string, body?: unknown) {
  const r = await fetch(base + p, { method, headers: { "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined });
  const text = await r.text();
  if (!r.ok) throw new Error(`${method} ${p} -> ${r.status} ${text}`);
  return text ? JSON.parse(text) : null;
}
const node = (id: string, type: string, x: number, extra: Record<string, unknown> = {}) =>
  ({ id, title: id, type, x, y: 0, width: 100, height: 80, ...extra });
function lantern(extraLine = "") {
  return {
    id: "lantern", title: "The Lantern on the Water", content: { text_template: "A lantern bobs." }, created_at: 1,
    graph: {
      nodes: [
        node("start", "start", 0, { text: "Start" }),
        node("line", "dialog", 100, { text: `The shingle is black glass under the rain.${extraLine}` }),
        node("choice", "choice", 200, { choices: [{ id: "a", text: "Wade out toward it" }, { id: "b", text: "Call out to the boat" }, { id: "c", text: "Turn back" }] }),
        node("end", "end", 300, { is_terminal: true }),
      ],
      edges: [
        { id: "e0", from_node: "start", from_port: "output", to_node: "line", to_port: "input" },
        { id: "e1", from_node: "line", from_port: "output", to_node: "choice", to_port: "input" },
        { id: "e2", from_node: "choice", from_port: "choice_0", to_node: "end", to_port: "input" },
        { id: "e3", from_node: "choice", from_port: "choice_1", to_node: "end", to_port: "input" },
        { id: "e4", from_node: "choice", from_port: "choice_2", to_node: "end", to_port: "input" },
      ],
    },
  };
}

const failures: string[] = [];
function check(name: string, ok: boolean, detail = "") {
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${ok ? "" : detail ? `  (${detail})` : ""}`);
  if (!ok) failures.push(name);
}
async function shot(page: Page, name: string) {
  await page.screenshot({ path: path.join(out, `${name}.png`) });
}

let browser;
try {
  for (let i = 0; i < 50; i++) {
    try { if ((await fetch(base + "/api/v3/project")).ok) break; } catch { /* not up yet */ }
    await sleep(200);
  }
  await api("POST", "/api/v3/content/entity_templates", { id: "harbor", name: "Harbor", root_tag: "location" });
  await api("POST", "/api/v3/content/scenarios", { id: "scenario", name: "Scenario", seed: 7, start_location_policy: { mode: "fixed", location_id: "harbor" } });
  await api("POST", "/api/v3/content/storylets", lantern());

  browser = await chromium.launch({ executablePath: chromePath, headless: true });
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const errors: string[] = [];
  page.on("console", (m) => { if (m.type() === "error" && !m.text().startsWith("Failed to load resource")) errors.push(m.text()); });
  page.on("response", (r) => { if (r.status() >= 400) errors.push(`${r.status()} ${r.url()}`); });
  page.on("pageerror", (e) => errors.push(String(e)));

  // 1. Toolbar has a primary Play action that opens the start screen.
  await page.goto(`${base}/#/storylets`);
  await page.getByRole("link", { name: "Play the project" }).click();
  await page.getByRole("heading", { name: "Play" }).waitFor();
  await shot(page, "01-start");
  check("start screen offers Start playing", await page.getByRole("button", { name: "Start playing" }).isEnabled());

  // 2. Start: the scene shows, with numbered choices.
  await page.getByRole("button", { name: "Start playing" }).click();
  await page.getByRole("heading", { name: "The Lantern on the Water" }).waitFor();
  await shot(page, "02-scene");
  check("run route opened", /#\/play\/[0-9a-f-]{36}$/.test(page.url()), page.url());
  check("place name is shown", await page.locator(".play-place__name").textContent() === "Harbor");
  check("prose is shown", await page.getByText("The shingle is black glass").isVisible());
  check("three numbered choices", (await page.locator(".play-choice").count()) === 3);
  const choiceBox = await page.locator(".play-choice").first().boundingBox();
  check("choice target is at least 44px tall", !!choiceBox && choiceBox.height >= 44, JSON.stringify(choiceBox));
  const dotBox = await page.locator(".play-dot").first().boundingBox();
  check("timeline dot target is at least 44x44", !!dotBox && dotBox.width >= 44 && dotBox.height >= 44, JSON.stringify(dotBox));
  check("live region announces the scene", (await page.locator(".play-live").textContent())?.includes("The Lantern on the Water") === true);
  const storyBox = await page.locator(".play-story").boundingBox();
  check("story column keeps a readable measure (<= 70ch)", !!storyBox && storyBox.width <= 820, JSON.stringify(storyBox));

  // 3. Press 1: a step is added and the run ends.
  await page.keyboard.press("1");
  await page.getByText("The end").waitFor();
  await shot(page, "03-ending");
  check("pressing 1 adds a step (dot 0 + step 1)", (await page.locator(".play-dot").count()) === 2);
  check("ending shows its actions", await page.getByRole("button", { name: "Play again" }).isVisible());

  // 4. Ctrl+Z rewinds one step.
  await page.keyboard.press("Control+z");
  await page.getByRole("heading", { name: "The Lantern on the Water" }).waitFor();
  await shot(page, "04-undo");
  check("undo restores the scene", await page.locator(".play-choice").count() === 3);
  check("undone step stays as a ghost dot", (await page.locator(".play-dot.is-ghost").count()) === 1);

  // 5. Choose differently: a branch selector appears.
  check("no branch selector yet", (await page.locator(".play-branches").count()) === 0);
  await page.keyboard.press("2");
  await page.getByText("The end").waitFor();
  await shot(page, "05-branch");
  check("branch selector appears after choosing differently", (await page.locator(".play-branches").count()) === 1);
  await page.keyboard.press("Control+z");
  await page.getByRole("heading", { name: "The Lantern on the Water" }).waitFor();

  // 6. Edit the storylet elsewhere: stale banner; Reload run shows the note.
  const stored = await api("GET", "/api/v3/content/storylets/lantern");
  await api("PUT", "/api/v3/content/storylets/lantern", { ...stored, ...lantern(" A second line."), revision: stored.revision });
  await page.reload();
  await page.getByText("Content changed since this run started.").waitFor();
  await shot(page, "06-stale");
  check("stale banner is shown", await page.getByRole("button", { name: "Reload run" }).isVisible());
  await page.getByRole("button", { name: "Reload run" }).click();
  await page.getByText(/Replayed \d+ of \d+ steps/).waitFor();
  await shot(page, "07-reloaded");
  check("reload note appears", await page.getByText(/Replayed \d+ of \d+ steps/).isVisible());
  check("stale banner clears after reload", (await page.getByText("Content changed since this run started.").count()) === 0);
  check("edited prose is now shown", await page.getByText("A second line.").isVisible());

  // 7. Play from here in the storylet editor.
  await page.goto(`${base}/#/storylets/lantern/edit`);
  await page.getByRole("button", { name: /Play from here/ }).waitFor();
  await page.getByRole("button", { name: /Play from here/ }).click();
  await page.getByRole("heading", { name: "The Lantern on the Water" }).waitFor();
  check("Play from here opens a run", /#\/play\/[0-9a-f-]{36}$/.test(page.url()), page.url());
  await shot(page, "08-play-from-here");

  // 7b. Codex: a canon event is authored through visible-label chronicle fields (Task 3.2).
  await page.goto(`${base}/#/codex`);
  await page.getByRole("button", { name: "New event" }).first().click();
  await page.getByLabel("Event type").selectOption("canon");
  await page.getByRole("group", { name: "Chronicle entry" }).waitFor();
  await shot(page, "10-codex-canon");
  check("chronicle fields have visible labels", (await page.getByLabel("Story time", { exact: true }).count()) === 1
    && (await page.getByLabel("Place").count()) === 1 && (await page.getByLabel("Text", { exact: true }).count()) === 1);
  check("canon form hides the raw JSON editors", (await page.getByText("Trigger condition (JSON)").count()) === 0);
  await page.getByLabel("Event ID").fill("flood");
  await page.getByLabel("Title", { exact: true }).fill("The flood");
  await page.getByLabel("Story time", { exact: true }).fill("20");
  await page.getByLabel("Story time label").fill("D1 evening");
  await page.getByLabel("Text", { exact: true }).fill("The river rose past the weir.");
  await page.getByRole("button", { name: "Save record" }).click();
  await page.getByText("Saved events item").waitFor();
  const savedEvent = await api("GET", "/api/v3/content/events/flood");
  check("canon event saved with story time and text", savedEvent.story_time === 20 && savedEvent.text === "The river rose past the weir." && savedEvent.story_time_label === "D1 evening", JSON.stringify(savedEvent));
  await page.getByLabel("Event type").selectOption("world");
  check("switching to a world event shows the trigger editor again", (await page.getByText("Trigger condition (JSON)").count()) === 1);

  // 8. Narrow viewport keeps the layout usable.
  await page.setViewportSize({ width: 390, height: 780 });
  await shot(page, "09-narrow");
  check("no horizontal overflow at 390px", await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth + 1));

  check("no console errors", errors.length === 0, errors.join(" | "));
} catch (e) {
  failures.push(`exception: ${e instanceof Error ? e.stack : e}`);
  console.error(e);
} finally {
  await browser?.close().catch(() => {});
  child.kill();
  await sleep(500);
  for (const suffix of ["", "-wal", "-shm"]) {
    try { fs.rmSync(dbPath + suffix, { force: true }); } catch { /* still locked; temp file */ }
  }
}
if (failures.length) {
  console.error(`\n${failures.length} visual check(s) failed`);
  process.exit(1);
}
console.log(`\nAll visual checks passed. Screenshots: ${out}`);
