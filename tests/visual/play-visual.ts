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
  // Knowledge content for the Inspect panel: a learn node, a reveal node, a canon event.
  await api("POST", "/api/v3/content/gameplay_tags", { id: "fact.clue", name: "fact.clue", protected: false });
  await api("POST", "/api/v3/content/events", {
    id: "flood", title: "The flood", description: "", event_type: "canon", trigger: {}, effects: {},
    story_time: 20, story_time_label: "D1 evening", facts: ["fact.clue"], text: "The river rose past the weir.",
  });
  await api("POST", "/api/v3/content/events", {
    id: "theft", title: "The theft", description: "", event_type: "canon", trigger: {}, effects: {}, story_time: 10, facts: [],
  });
  await api("POST", "/api/v3/content/storylets", {
    id: "clue", title: "A muddy ledger", content: { text_template: "x" }, created_at: 2,
    graph: {
      nodes: [
        node("start", "start", 0, { text: "Start" }),
        node("learn", "learn", 100, { fact_tag: "fact.clue", kind: "testimony", visibility: "private" }),
        node("reveal", "reveal", 200, { event_id: "flood", kind: "recollection", visibility: "private" }),
        node("line", "dialog", 300, { text: "The ledger is soaked through." }),
        node("choice", "choice", 400, { choices: [{ id: "a", text: "Read on" }, { id: "b", text: "Put it down" }] }),
        node("end", "end", 500, { is_terminal: true }),
      ],
      edges: [
        { id: "k0", from_node: "start", from_port: "output", to_node: "learn", to_port: "input" },
        { id: "k1", from_node: "learn", from_port: "output", to_node: "reveal", to_port: "input" },
        { id: "k2", from_node: "reveal", from_port: "output", to_node: "line", to_port: "input" },
        { id: "k3", from_node: "line", from_port: "output", to_node: "choice", to_port: "input" },
        { id: "k4", from_node: "choice", from_port: "choice_0", to_node: "end", to_port: "input" },
        { id: "k5", from_node: "choice", from_port: "choice_1", to_node: "end", to_port: "input" },
      ],
    },
  });

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
  await page.getByLabel("From a storylet…").check();
  await page.getByLabel("Find a storylet").fill("lantern");
  await page.getByLabel("Storylet", { exact: true }).selectOption({ label: "The Lantern on the Water" });
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

  // 7a. Inspect panel (Task 3.3): knowledge provenance and the chronicle.
  const knowRun = await api("POST", "/api/v4/runs", {
    start: { kind: "beginning" }, storyletId: "clue", expectedRevision: (await api("GET", "/api/v3/project")).revision, idempotencyKey: "visual-clue",
  });
  await page.goto(`${base}/#/play/${knowRun.id}`);
  await page.getByRole("heading", { name: "A muddy ledger" }).waitFor();
  check("inspect is closed by default", (await page.locator(".play-inspect").count()) === 0);
  await page.keyboard.press("i");
  await page.locator(".play-inspect").waitFor();
  await page.getByText("fact.clue").waitFor();
  await shot(page, "11-inspect-knows");
  check("knows tab lists the fact with its kind badge in words", await page.locator(".play-kind").first().textContent() === "testimony");
  check("testimony is labelled not confirmed", await page.getByText("not confirmed").first().isVisible());
  await page.getByRole("tab", { name: "Story so far" }).click();
  await page.locator(".play-chronicle").waitFor();
  await shot(page, "12-inspect-story");
  const chron = await page.locator(".play-chronicle li").allTextContents();
  check("story so far lists canon in story-time order", chron.length === 2 && chron[0].includes("THE THEFT".replace("THE THEFT", "The theft")) && chron[1].includes("D1 evening"), JSON.stringify(chron));
  check("told entry is numbered, untold entry says not told yet", chron[1].includes("①") && chron[0].includes("not told yet"), JSON.stringify(chron));
  await page.getByRole("tab", { name: "World" }).click();
  await page.getByText("Advanced").click();
  check("advanced fold shows read-only engine details", await page.getByText("Content revision").isVisible());
  await page.keyboard.press("Escape");
  check("Esc closes inspect", (await page.locator(".play-inspect").count()) === 0);

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

  // 7c. Space, clock and rule participants (Tasks 4.1-4.4), authored through system traits.
  const world = await api("GET", "/api/v3/content/entities/world");
  await api("PUT", "/api/v3/content/entities/world", {
    ...world,
    traits: [...(world.traits ?? []), { id: "clock", template_id: "system.clock", values: { slot_labels: ["D1 evening", "D2 morning", "D2 evening"], actions_per_slot: 2 } }],
  });
  await api("POST", "/api/v3/content/entity_templates", { id: "lighthouse", name: "Lighthouse", root_tag: "location", traits: [
    { id: "c1", template_id: "system.connection", values: { to: "harbor" } },
    { id: "c2", template_id: "system.connection", values: { to: "harbor", open_slots: [], on_close: "" } },
    { id: "c3", template_id: "system.connection", values: { to: "rock", open_slots: [1], on_close: "return", return_to: "lighthouse" } },
  ] });
  await api("POST", "/api/v3/content/entity_templates", { id: "rock", name: "Rock", root_tag: "location", traits: [] });
  const harbor = await api("GET", "/api/v3/content/entity_templates/harbor");
  await api("PUT", "/api/v3/content/entity_templates/harbor", { ...harbor, traits: [{ id: "h1", template_id: "system.connection", values: { to: "lighthouse" } }] });
  await api("POST", "/api/v3/content/entity_templates", { id: "mara", name: "Mara", root_tag: "character", traits: [
    { id: "p1", template_id: "system.participant", values: { controller: "human", seat: 0, start_location: "harbor" } },
    { id: "s1", template_id: "system.character_settings", values: { home_location: "harbor", current_location: "harbor" } },
  ] });
  await api("POST", "/api/v3/content/entity_templates", { id: "rosa", name: "Rosa", root_tag: "character", traits: [
    { id: "p2", template_id: "system.participant", values: { controller: "rule", seat: 0, start_location: "harbor" } },
    { id: "s2", template_id: "system.character_settings", values: { home_location: "harbor", current_location: "harbor" } },
  ] });
  await api("POST", "/api/v3/content/storylets", {
    id: "gossip", title: "Rosa gossips", content: { text_template: "x" }, created_at: 3, played_by: ["rosa"],
    graph: {
      nodes: [node("start", "start", 0, { text: "Start" }), node("learn", "learn", 100, { fact_tag: "fact.clue", kind: "told", visibility: "co_located" }), node("end", "end", 200, { is_terminal: true })],
      edges: [
        { id: "g0", from_node: "start", from_port: "output", to_node: "learn", to_port: "input" },
        { id: "g1", from_node: "learn", from_port: "output", to_node: "end", to_port: "input" },
      ],
    },
  });
  const spaceRun = await api("POST", "/api/v4/runs", {
    start: { kind: "beginning" }, storyletId: "lantern", expectedRevision: (await api("GET", "/api/v3/project")).revision, idempotencyKey: "visual-space",
  });
  await page.goto(`${base}/#/play/${spaceRun.id}`);
  await page.getByRole("heading", { name: "The Lantern on the Water" }).waitFor();
  await shot(page, "13-space-start");
  check("clock label and actions left are shown", await page.getByText("D1 evening").isVisible() && await page.getByText("2 actions left").isVisible());
  check("rule participants are not offered as tabs", (await page.getByRole("tab", { name: "Rosa" }).count()) === 0);
  check("Go to lists the lighthouse", await page.getByRole("button", { name: /Lighthouse/ }).isVisible());
  await page.getByRole("button", { name: /Lighthouse/ }).click();
  await page.getByText("1 action left").waitFor();
  check("moving spends an action and changes the place", (await page.locator(".play-place__name").textContent()) === "Lighthouse");
  const rockChip = page.getByRole("button", { name: /Rock/ });
  check("the rock is blocked until its slot, with the reason on focus", (await rockChip.getAttribute("aria-disabled")) === "true");
  await rockChip.focus();
  check("blocked move explains why", await page.getByText("Closed until D2 morning").isVisible());
  await shot(page, "14-space-blocked");
  await page.keyboard.press("e");
  await page.getByText("D2 morning").waitFor();
  await shot(page, "15-space-end-turn");
  check("End turn advances the clock and refills actions", await page.getByText("2 actions left").isVisible());
  check("Rosa played her storylet at the slot start", await page.getByText("Rosa: Rosa gossips").isVisible());
  check("the rock is open in D2 morning", (await page.getByRole("button", { name: /Rock/ }).getAttribute("aria-disabled")) === null);
  await page.getByRole("button", { name: /Rock/ }).click();
  await page.getByText("1 action left").waitFor();
  await page.keyboard.press("e");
  await page.getByText("D2 evening").waitFor();
  await shot(page, "16-space-driven-back");
  check("the closing tide drives the occupant back", await page.getByText(/is driven from Rock back to Lighthouse/).isVisible());
  check("the occupant is back at the lighthouse", (await page.locator(".play-place__name").textContent()) === "Lighthouse");

  // Coverage: use a real report, adding a synthetic violation so replay is testable
  // without deliberately corrupting the temporary project's content.
  await page.route("**/api/v4/coverage", async (route) => {
    const response = await route.fetch();
    const report = await response.json();
    report.violations = [{ invariant: { kind: "knowledge_without_source" }, seed: 123, persona: { kind: "wanderer" }, config: "solo", step: 1, actions: [[0, { kind: "wait" }]] }];
    await route.fulfill({ response, json: report });
  });
  await page.goto(`${base}/#/play`);
  await page.getByRole("button", { name: "Let bots play" }).click();
  await page.getByLabel("Runs per configuration").fill("1");
  await page.getByLabel("Seed (blank for scenario default)").fill("123");
  await page.getByRole("button", { name: "Run exploration" }).click();
  await page.getByText("1 invariant violation", { exact: true }).waitFor();
  check("coverage report shows all four configurations", (await page.locator(".coverage__cell").count()) === 4);
  check("coverage failure offers replay", await page.getByRole("button", { name: "Replay seed" }).isVisible());
  await shot(page, "17-coverage-report");
  await page.setViewportSize({ width: 390, height: 780 });
  await page.getByRole("button", { name: "Collapse project tree" }).click();
  await shot(page, "19-coverage-narrow");
  check("coverage controls remain visible on mobile", await page.getByRole("button", { name: "Run exploration" }).isVisible());
  check("coverage has no horizontal overflow on mobile", await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth + 1));
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.getByRole("button", { name: "Replay seed" }).click();
  await page.locator(".play-timeline").waitFor();
  check("replay opens at the recorded step", await page.getByRole("button", { name: /Step 1:.*Waited/ }).getAttribute("aria-current") === "step");
  await shot(page, "18-coverage-replay");
  await page.unroute("**/api/v4/coverage");
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
