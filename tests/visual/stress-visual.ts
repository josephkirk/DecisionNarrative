// Worst-case data visual audit: seeds realistic long, short, non-Latin, emoji and huge values, then scans
// every studio view for clipped, spilled, overlapping, occluded or low-contrast text.
// Run: bun run build && node tests/visual/stress-visual.ts  (Node: Playwright cannot drive Chromium from Bun).
// THEMES=kojima,cyberpunk,... (default kojima) VIEWPORTS=desk,narrow,phone,split3 (default all).
import { chromium, type Page } from "playwright-core";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn, type ChildProcess } from "node:child_process";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const root = path.resolve(import.meta.dirname, "../..");
const out = path.join(process.env.VISUAL_OUT ? path.resolve(process.env.VISUAL_OUT) : path.join(import.meta.dirname, "out"), "stress");
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

const port = 41_000 + Math.floor(Math.random() * 1_000);
const base = `http://127.0.0.1:${port}`;
const child: ChildProcess = spawn(binary, ["--fresh-database"], {
  cwd: root,
  env: { ...process.env, STORYLET_STUDIO_ADDR: `127.0.0.1:${port}`, STORYLET_STUDIO_DB_PATH: path.join(os.tmpdir(), `dn-stress-${port}.db`), STORYLET_RESOURCE_ROOT: "." },
  stdio: "ignore", windowsHide: true,
});
async function api(method: string, p: string, body?: unknown) {
  const r = await fetch(base + p, { method, headers: { "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined });
  const text = await r.text();
  if (!r.ok) throw new Error(`${method} ${p} -> ${r.status} ${text}`);
  return text ? JSON.parse(text) : null;
}

// ---- Worst-case fixture: every value is something a real author could type. ----
const PERSON = "Aleksandra Wiśniewska-Kowalczyk, Keeper of the Northwind Lighthouse";
const URL = "https://northwind-industries-holdings.example.com/archive/correspondence/2024/aleksandra.wisniewska-kowalczyk/letter-0042-final-v12.md";
const EMAIL = "bartholomew.fitzgerald@northwind-industries-holdings.example.com";
const DEEP_TAG = ["faction", "northwind_industries_holdings", "expeditionary_department", "archive_division", "correspondence_clerks"];
const STORY = "When Aleksandra Finally Opened the Forty-Second Letter From the Drowned Expedition";
const FOLDER = "Chapter 07 — Correspondence of the Northwind Archive (draft v12-final)";
const prose = `The letter was addressed to ${EMAIL}, which nobody had used since the flood. ` +
  `Its postscript pointed at ${URL} and ended mid-sentence, as if the writer had been called away from the desk and never returned.`;
const n = (id: string, type: string, x: number, y: number, extra: Record<string, unknown> = {}) => ({ id, title: id, type, x, y, width: 260, height: 160, ...extra });

async function seed() {
  await api("PATCH", "/api/v3/project", { name: "Northwind Industries Holdings — Expeditionary Narrative Department Archive" });
  for (let i = 1; i <= DEEP_TAG.length; i++) {
    const name = DEEP_TAG.slice(0, i).join(".");
    await api("POST", "/api/v3/content/gameplay_tags", { id: name, name, description: i === DEEP_TAG.length ? prose : "", protected: false });
  }
  for (const name of ["mood", "mood.melancholic", "x", "role", "role.aleksandra_wisniewska_kowalczyk_keeper_of_the_lamp", "role.jo"]) {
    await api("POST", "/api/v3/content/gameplay_tags", { id: name, name, description: "", protected: false });
  }
  const deepTag = DEEP_TAG.join(".");

  await api("POST", "/api/v3/content/folders", { id: "f-long", name: FOLDER, root: "entity_templates", parent_id: null });
  await api("POST", "/api/v3/content/folders", { id: "f-long-2", name: "Unsorted letters awaiting Aleksandra Wiśniewska-Kowalczyk's review", root: "entity_templates", parent_id: "f-long" });
  await api("POST", "/api/v3/content/folders", { id: "f-jo", name: "Jo", root: "entity_templates", parent_id: "f-long-2" });
  await api("POST", "/api/v3/content/folders", { id: "f-story", name: FOLDER, root: "storylets", parent_id: null });

  await api("POST", "/api/v3/content/trait_templates", {
    id: "ledger", key: "correspondence_ledger_entry_with_provenance", key_auto: false,
    name: "Correspondence Ledger Entry with Extremely Specific Provenance", description: prose, folder_id: null,
    allow_multiple: true, unique: false, system: false,
    properties: [
      { key: "archive_url", name: "Correspondence archive URL", type: "string", default: URL },
      { id: "lp2", key: "letters_written_since_the_first_winter_of_the_flood", name: "Letters written since the first winter of the flood", type: "int", default: 1284000 },
      { id: "lp3", key: "recipient", name: "Recipient", type: "entity", default: "" },
      { id: "lp4", key: "topics", name: "Topics", type: "tag", is_array: true, default: [deepTag, "mood.melancholic"] },
      { id: "lp5", key: "reliability", name: "Reliability", type: "float", default: 0.000001 },
      { id: "lp6", key: "x", name: "X", type: "bool", default: false },
    ],
  });
  await api("POST", "/api/v3/content/assets", {
    id: "asset-long", name: "Northwind_Lighthouse_Exterior_Dusk_Establishing_Shot_v12-final-FINAL.png", folder_id: null, thumbnail: null, tags: [deepTag],
    versions: [{ id: "v1", file_path: "assets/environments/northwind/lighthouse/exterior/Northwind_Lighthouse_Exterior_Dusk_Establishing_Shot_v12-final-FINAL.png",
      creator: PERSON, tool_version: "Blender 4.2.3 LTS (hash 0f8a2b1c9d) + Substance Painter 10.1.2", description: prose, created_at: 1_700_000_000_000, tags: [deepTag], custom_data: {}, thumbnail: null, storage: [] }],
  });
  await api("POST", "/api/v3/content/entity_templates", {
    id: "long-person", name: PERSON, description: prose, tags: [deepTag, "mood.melancholic", "x"], folder_id: "f-long", thumbnail: null,
    properties: [
      { id: "p1", key: "correspondence_archive_url", name: "Correspondence archive URL", type: "string", value: URL },
      { id: "p2", key: "letters_written_since_the_first_winter_of_the_flood", name: "Letters written since the first winter of the flood", type: "int", value: 1284000 },
      { id: "p3", key: "x", name: "X", type: "float", value: 0.000001 },
      { id: "p4", key: "is_reliable_narrator", name: "Is reliable narrator", type: "bool", value: false },
      { id: "p5", key: "motto", name: "Motto", type: "string", multiline: true, value: `${prose}\n\n— ${PERSON}` },
      { id: "p6", key: "aliases", name: "Aliases", type: "string", is_array: true, value: ["Ola", "Wiśniewska-Kowalczyk", EMAIL] },
    ],
    traits: [{ id: "t1", template_id: "ledger", values: { archive_url: URL, letters_written_since_the_first_winter_of_the_flood: 1284000, topics: [deepTag] } }],
  });
  for (const [id, name] of [["jo", "Jo"], ["nguyen", "Nguyễn Thị Minh Khai"], ["sakura", "渡辺さくら（北風灯台の灯台守）"], ["kitsune", "🦊 Kitsune of the Ninth Tail"]]) {
    await api("POST", "/api/v3/content/entity_templates", { id, name, description: "", tags: [], folder_id: "f-long-2", thumbnail: null, properties: [], traits: [] });
  }
  for (let i = 1; i <= 24; i++) {
    await api("POST", "/api/v3/content/entity_templates", { id: `clerk-${i}`, name: `Archive clerk ${i}`, description: "", tags: [], folder_id: null, thumbnail: null, properties: [], traits: [] });
  }
  await api("POST", "/api/v3/content/entities", {
    id: "inst-long", template_id: "long-person", name: `${PERSON} (as she was in the winter of the flood)`, tags: [deepTag], description: prose, folder_id: null,
    properties: [{ id: "ip1", key: "correspondence_archive_url", name: "Correspondence archive URL", type: "string", value: URL }], traits: [],
  });
  await api("POST", "/api/v3/content/events", {
    id: "ev-long", title: "The night the Northwind lamp went dark and the expedition's last letter washed ashore", description: prose,
    event_type: "canon", trigger: {}, effects: {}, story_time: 1_284_000, story_time_label: "Winter of the flood, third night, a quarter past the second bell", facts: [deepTag], text: prose,
  });
  await api("POST", "/api/v3/content/events", { id: "ev-short", title: "X", description: "", event_type: "canon", trigger: {}, effects: {}, story_time: 0, facts: [] });
  await api("POST", "/api/v3/content/storylets", {
    id: "long-story", title: STORY, content: { text_template: prose }, created_at: 1, folder_id: "f-story", tags: [deepTag],
    graph: {
      nodes: [
        n("start", "start", 0, 0, { text: "Start" }),
        n("line", "dialog", 320, 0, { title: "Aleksandra reads the forty-second letter aloud to the empty lamp room", text: prose, sections: [
          { id: "sec1", title: "The postscript, read twice because the first reading made no sense to anyone in the room", items: [
            { kind: "line", id: "l1", speaker_role: "role.aleksandra_wisniewska_kowalczyk_keeper_of_the_lamp", target_role: "player", content: prose, emotion_tag: "mood.melancholic" },
            { kind: "line", id: "l2", speaker_role: "role.jo", target_role: "player", content: "X", emotion_tag: "x" },
          ] },
          { id: "sec2", title: "X", items: [] },
        ] }),
        n("choice", "choice", 640, 0, { title: "What does she do with it?", choices: [
          { id: "a", text: `Forward it to ${EMAIL} even though the address bounced in 2019` },
          { id: "b", text: "Burn it" }, { id: "c", text: "X" },
          { id: "d", text: "Read the postscript again, slowly, and copy the link into the ledger before the ink runs" },
        ] }),
        n("end", "end", 960, 0, { is_terminal: true }),
      ],
      edges: [
        { id: "e0", from_node: "start", from_port: "output", to_node: "line", to_port: "input" },
        { id: "e1", from_node: "line", from_port: "output", to_node: "choice", to_port: "input" },
        ...[0, 1, 2, 3].map((i) => ({ id: `e${i + 2}`, from_node: "choice", from_port: `choice_${i}`, to_node: "end", to_port: "input" })),
      ],
    },
  });
  for (let i = 1; i <= 12; i++) {
    await api("POST", "/api/v3/content/storylets", { id: `s-${i}`, title: i === 1 ? "X" : `Letter ${i}: ${STORY.slice(0, 20 + i * 4)}`, content: { text_template: "x" }, created_at: 1 + i,
      graph: { nodes: [n("start", "start", 0, 0, { text: "Start" }), n("end", "end", 320, 0, { is_terminal: true })], edges: [{ id: "e", from_node: "start", from_port: "output", to_node: "end", to_port: "input" }] } });
  }
}

// ---- In-page scanner ----
type Finding = { kind: string; severity: "broken" | "warn"; sel: string; text: string; detail: string };
function scanPage(): Finding[] {
  const found: Finding[] = [];
  const vw = innerWidth, vh = innerHeight;
  const sel = (el: Element | null) => {
    const parts: string[] = [];
    for (let e = el, i = 0; e && e !== document.body && i < 4; e = e.parentElement, i++) {
      const classes = [...e.classList].filter((c) => !c.startsWith("is-")).slice(0, 2);
      parts.unshift(e.tagName.toLowerCase() + (classes.length ? `.${classes.join(".")}` : ""));
    }
    return parts.join(">");
  };
  // Hidden from users: inert/aria-hidden subtrees and the body of a closed <details>.
  const inert = (el: Element) => !!el.closest("[inert],[aria-hidden='true']") || (!!el.closest("details:not([open])") && !el.closest("summary"));
  const shown = (el: Element) => {
    const r = el.getBoundingClientRect();
    if (r.width < 1 || r.height < 1 || getComputedStyle(el).visibility !== "visible") return false;
    for (let e: Element | null = el; e; e = e.parentElement) if (Number(getComputedStyle(e).opacity) < 0.05) return false;
    return true;
  };
  const parse = (c: string) => { const m = c.match(/[\d.]+/g)?.map(Number) ?? [0, 0, 0, 0]; return { r: m[0], g: m[1], b: m[2], a: m[3] ?? 1 }; };
  const lum = (c: { r: number; g: number; b: number }) => {
    const f = (v: number) => { v /= 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; };
    return 0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b);
  };
  const over = (top: { r: number; g: number; b: number; a: number }, under: { r: number; g: number; b: number }) =>
    ({ r: top.r * top.a + under.r * (1 - top.a), g: top.g * top.a + under.g * (1 - top.a), b: top.b * top.a + under.b * (1 - top.a) });
  const clipAncestors = (el: Element) => {
    const list: { el: Element; rect: DOMRect; scroller: boolean }[] = [];
    for (let a = el.parentElement; a && a !== document.documentElement; a = a.parentElement) {
      const s = getComputedStyle(a);
      // Pan/zoom canvases clip like scroll containers: off-view content is reached by panning.
      if (s.overflowX !== "visible" || s.overflowY !== "visible") list.push({ el: a, rect: a.getBoundingClientRect(), scroller: /auto|scroll/.test(s.overflowX + s.overflowY) || a.classList.contains("graph-canvas-container") });
    }
    return list;
  };

  const textEls = new Set<Element>();
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  while (walker.nextNode()) {
    const t = walker.currentNode;
    const p = t.parentElement;
    if (p && t.textContent!.trim() && !["SCRIPT", "STYLE", "NOSCRIPT", "OPTION", "TEXTAREA"].includes(p.tagName)) textEls.add(p);
  }
  const boxes: { el: Element; r: { l: number; t: number; r: number; b: number } }[] = [];
  for (const el of textEls) {
    if (inert(el) || !shown(el)) continue;
    const range = document.createRange();
    let tr: { l: number; t: number; r: number; b: number } | null = null;
    for (const node of el.childNodes) {
      if (node.nodeType !== 3 || !node.textContent!.trim()) continue;
      range.selectNodeContents(node);
      for (const r of range.getClientRects()) {
        if (r.width < 1) continue;
        tr = tr ? { l: Math.min(tr.l, r.left), t: Math.min(tr.t, r.top), r: Math.max(tr.r, r.right), b: Math.max(tr.b, r.bottom) } : { l: r.left, t: r.top, r: r.right, b: r.bottom };
      }
    }
    if (!tr) continue;
    const text = el.textContent!.trim().replace(/\s+/g, " ").slice(0, 50);
    const cs = getComputedStyle(el);
    // Clipping: the first clipping ancestor that cuts the text. Scroll containers are fine (content scrolls into view).
    let scrolledAway = false;
    const ownClip = cs.overflowX !== "visible" && tr.r > el.getBoundingClientRect().right + 1;
    if (ownClip) {
      if (cs.textOverflow === "ellipsis") {
        if (!el.closest("[title]") && !el.getAttribute("aria-label")) found.push({ kind: "truncated-no-title", severity: "warn", sel: sel(el), text, detail: "ellipsis without title" });
      } else if (!/auto|scroll/.test(cs.overflowX)) {
        found.push({ kind: "clipped", severity: "broken", sel: sel(el), text, detail: "own overflow cuts text" });
      }
      scrolledAway = true;
    }
    for (const a of clipAncestors(el)) {
      if (scrolledAway) break;
      const outX = tr.l < a.rect.left - 1 || tr.r > a.rect.right + 1;
      const outY = tr.t < a.rect.top - 1 || tr.b > a.rect.bottom + 1;
      if (!outX && !outY) continue;
      const fullyOut = tr.r <= a.rect.left || tr.l >= a.rect.right || tr.b <= a.rect.top || tr.t >= a.rect.bottom;
      if (a.scroller) { scrolledAway = true; break; }
      if (getComputedStyle(a.el).textOverflow === "ellipsis") {
        if (!a.el.closest("[title]") && !el.closest("[title]")) found.push({ kind: "truncated-no-title", severity: "warn", sel: sel(el), text, detail: "ellipsis without title" });
      } else if (!fullyOut) {
        found.push({ kind: "clipped", severity: "broken", sel: sel(el), text, detail: `cut by ${sel(a.el)} (${outX ? "x" : ""}${outY ? "y" : ""})` });
      }
      scrolledAway = true;
      break;
    }
    if (scrolledAway) continue;
    if (tr.r > vw + 1 || tr.l < -1) found.push({ kind: "offscreen", severity: "broken", sel: sel(el), text, detail: `x ${Math.round(tr.l)}..${Math.round(tr.r)} of ${vw}` });
    // Spill: text leaves the nearest painted box (background or border) without being clipped.
    for (let a = el.parentElement, i = 0; a && i < 6; a = a.parentElement, i++) {
      const s = getComputedStyle(a);
      const painted = parse(s.backgroundColor).a > 0.05 || parseFloat(s.borderLeftWidth) > 0 || parseFloat(s.borderRightWidth) > 0;
      if (!painted) continue;
      const r = a.getBoundingClientRect();
      if (tr.r > r.right + 2 || tr.l < r.left - 2) found.push({ kind: "spill", severity: "broken", sel: sel(el), text, detail: `leaves ${sel(a)} by ${Math.round(Math.max(tr.r - r.right, r.left - tr.l))}px` });
      break;
    }
    // Contrast against composited backgrounds.
    let fg = parse(cs.color);
    if (fg.a > 0 && !el.closest(":disabled,[aria-disabled='true'],.is-disabled")) {
      let opacity = 1;
      const layers: { r: number; g: number; b: number; a: number }[] = [];
      let unknown = false;
      for (let a: Element | null = el; a; a = a.parentElement) {
        const s = getComputedStyle(a);
        opacity *= Number(s.opacity);
        if (s.backgroundImage !== "none" && !s.backgroundImage.startsWith("linear-gradient(rgba(0, 0, 0, 0)")) { unknown = true; break; }
        const bg = parse(s.backgroundColor);
        if (bg.a > 0) { layers.push(bg); if (bg.a >= 0.999) break; }
      }
      if (!unknown && layers.length) {
        let bg = { r: 0, g: 0, b: 0 };
        for (const layer of layers.reverse()) bg = over(layer, bg);
        fg = { ...fg, a: fg.a * opacity };
        const f = over(fg, bg);
        const [hi, lo] = [lum(f), lum(bg)].sort((x, y) => y - x);
        const ratio = (hi + 0.05) / (lo + 0.05);
        const size = parseFloat(cs.fontSize);
        const large = size >= 24 || (size >= 18.66 && Number(cs.fontWeight) >= 700);
        if (ratio < (large ? 3 : 4.5)) found.push({ kind: "contrast", severity: "broken", sel: sel(el), text, detail: `${ratio.toFixed(2)}:1` });
      }
    }
    boxes.push({ el, r: tr });
  }
  // Overlapping text from different elements, where one actually paints over the other.
  for (let i = 0; i < boxes.length; i++) {
    for (let j = i + 1; j < boxes.length; j++) {
      const a = boxes[i], b = boxes[j];
      if (a.el.contains(b.el) || b.el.contains(a.el)) continue;
      const w = Math.min(a.r.r, b.r.r) - Math.max(a.r.l, b.r.l);
      const h = Math.min(a.r.b, b.r.b) - Math.max(a.r.t, b.r.t);
      if (w < 4 || h < 4) continue;
      found.push({ kind: "overlap", severity: "broken", sel: `${sel(a.el)} × ${sel(b.el)}`, text: `${a.el.textContent!.trim().slice(0, 24)} × ${b.el.textContent!.trim().slice(0, 24)}`, detail: `${Math.round(w)}×${Math.round(h)}` });
    }
  }
  // Occluded or undersized controls.
  for (const el of document.querySelectorAll("button,a[href],input:not([type=hidden]),select,textarea,[role=button],[role=tab],[role=menuitem]")) {
    if (inert(el) || !shown(el) || (el as HTMLButtonElement).disabled || getComputedStyle(el).pointerEvents === "none") continue;
    const r = el.getBoundingClientRect();
    const cx = r.left + r.width / 2, cy = r.top + r.height / 2;
    if (cx < 0 || cy < 0 || cx > vw || cy > vh) continue;
    if (clipAncestors(el).some((a) => cx < a.rect.left || cx > a.rect.right || cy < a.rect.top || cy > a.rect.bottom)) continue;
    const top = document.elementFromPoint(cx, cy);
    if (top && !el.contains(top) && !top.contains(el) && !(top instanceof HTMLLabelElement && top.control === el)) {
      found.push({ kind: "occluded", severity: "broken", sel: sel(el), text: (el.textContent || el.getAttribute("aria-label") || "").trim().slice(0, 40), detail: `under ${sel(top)}` });
    }
  }
  return found;
}

// ---- Routes, states, viewports ----
type Step = { name: string; path: string; act?: (page: Page) => Promise<void> };
const steps: Step[] = [
  { name: "library", path: "/storylets" },
  { name: "storylet", path: "/storylets/long-story/edit" },
  { name: "storylet-fit", path: "/storylets/long-story/edit", act: async (p) => { await p.locator(".graph-toolbar").getByRole("button", { name: "Fit", exact: true }).click(); } },
  { name: "storylet-dialog", path: "/storylets/long-story/edit", act: async (p) => {
    await p.locator(".graph-toolbar").getByRole("button", { name: "Fit", exact: true }).click();
    await p.locator(".graph-node.dialog .node-body").first().click();
  } },
  { name: "storylet-choice", path: "/storylets/long-story/edit", act: async (p) => {
    await p.locator(".graph-toolbar").getByRole("button", { name: "Fit", exact: true }).click();
    await p.locator(".graph-node.choice .node-body .field-label").first().click();
  } },
  { name: "entity", path: "/entity-templates/long-person" },
  { name: "entity-tab-2", path: "/entity-templates/long-person", act: async (p) => { await p.locator(".entity-tab").nth(1).click(); } },
  { name: "entity-tab-3", path: "/entity-templates/long-person", act: async (p) => { await p.locator(".entity-tab").nth(2).click(); } },
  { name: "entity-short", path: "/entity-templates/jo" },
  { name: "instance", path: "/entities/inst-long" },
  { name: "trait", path: "/trait-templates/ledger", act: async (p) => {
    if (await p.getByRole("checkbox", { name: /Only one unique trait/ }).isDisabled()) throw new Error("Custom trait uniqueness must remain editable");
  } },
  { name: "system-trait", path: "/trait-templates/system.fact", act: async (p) => {
    const unique = p.getByRole("checkbox", { name: /Only one unique trait/ });
    if (!await unique.isDisabled()) throw new Error("System trait uniqueness must be read-only");
    if (!await p.getByRole("textbox", { name: "Trait name" }).isDisabled()) throw new Error("System trait name must be read-only");
    if (!await p.getByRole("textbox", { name: "Trait key" }).isDisabled()) throw new Error("System trait key must be read-only");
    if (!await p.getByRole("button", { name: "Delete", exact: true }).isDisabled()) throw new Error("System trait must not be deletable");
    if (!await p.getByText("Built-in system trait", { exact: true }).isVisible()) throw new Error("System trait indicator missing");
    if (!await p.getByText("This built-in trait's uniqueness cannot be changed.", { exact: true }).isVisible()) throw new Error("System trait restriction explanation missing");
  } },
  { name: "trait-focus", path: "/trait-templates/ledger", act: async (p) => {
    const row = p.locator(".property-list__row").first();
    for (const selector of [".property-name-input", ".property-key-input", "[aria-label^='Category for']", ".property-list__cell--value input"]) {
      const input = row.locator(selector);
      await input.scrollIntoViewIfNeeded();
      await input.click();
      await p.keyboard.press("End");
      const before = await input.inputValue();
      await p.keyboard.type("xyz", { delay: 30 });
      if (await input.inputValue() !== `${before}xyz` || !await input.evaluate((el) => el === document.activeElement)) {
        throw new Error(`Focus or input lost in ${selector}`);
      }
    }
    await row.locator(".property-drag").focus();
    await p.keyboard.press("ArrowDown");
    const moved = p.locator(".property-list__row").nth(1).locator(".property-name-input");
    await moved.click();
    await p.keyboard.press("End");
    const name = await moved.inputValue();
    await p.keyboard.type("ok", { delay: 30 });
    if (await moved.inputValue() !== `${name}ok` || !await moved.evaluate((el) => el === document.activeElement)) {
      throw new Error("Focus or input lost after reordering");
    }
  } },
  { name: "asset", path: "/assets/asset-long" },
  { name: "folder", path: "/browse/entity_templates/f-long-2" },
  { name: "folder-root", path: "/browse/entity_templates" },
  { name: "codex", path: "/codex" },
  { name: "tags", path: "/tags" },
  { name: "project", path: "/project" },
  { name: "plugins", path: "/plugins" },
  { name: "play", path: "/play" },
];
const VIEWPORTS: Record<string, { width: number; height: number; split?: boolean }> = {
  desk: { width: 1440, height: 900 }, narrow: { width: 1024, height: 720 }, phone: { width: 390, height: 844 }, split3: { width: 1440, height: 900, split: true },
};
const themes = (process.env.THEMES ?? "kojima").split(",");
const viewports = (process.env.VIEWPORTS ?? Object.keys(VIEWPORTS).join(",")).split(",");
const split3 = (main: string) => ({
  id: "root", kind: "split", direction: "row", ratio: 0.5, children: [
    { id: "main", kind: "pane", path: main },
    { id: "s2", kind: "split", direction: "column", ratio: 0.5, children: [
      { id: "p2", kind: "pane", path: "/entity-templates/long-person" }, { id: "p3", kind: "pane", path: "/browse/entity_templates/f-long-2" },
    ] },
  ],
});

let browser;
let broken = 0;
try {
  for (let i = 0; i < 50; i++) { try { if ((await fetch(base + "/api/v3/project")).ok) break; } catch { /* starting */ } await sleep(200); }
  await seed();
  browser = await chromium.launch({ executablePath: chromePath, headless: true });
  const report: string[] = [];
  for (const theme of themes) {
    for (const vpName of viewports) {
      const vp = VIEWPORTS[vpName];
      const context = await browser.newContext({ viewport: { width: vp.width, height: vp.height } });
      const page = await context.newPage();
      const errors: string[] = [];
      page.on("pageerror", (e) => errors.push(String(e)));
      page.on("dialog", (d) => void d.dismiss());
      for (const step of steps.filter((s) => !process.env.STEPS || process.env.STEPS.split(",").includes(s.name))) {
        if (vp.split && step.name !== "storylet-dialog" && step.name !== "library") continue;
        await page.goto(`${base}/#/storylets`);
        await page.evaluate(([t, layout]) => {
          localStorage.setItem("dn.studio.appearance", JSON.stringify({ preset: t, backgroundImage: null, blur: false }));
          if (layout) localStorage.setItem("dn.studio.panes", layout); else localStorage.removeItem("dn.studio.panes");
        }, [theme, vp.split ? JSON.stringify(split3(step.path)) : ""] as const);
        await page.goto(`${base}/#${step.path}`);
        await page.reload();
        await page.waitForLoadState("networkidle");
        await sleep(500);
        try { if (step.act) { await step.act(page); await sleep(350); } } catch (e) { errors.push(`${step.name}: ${String(e).split("\n").filter((line) => /intercepts|Timeout|waiting for|not stable|outside/.test(line)).slice(0, 4).join(" | ")}`); }
        await page.mouse.move(1, vp.height - 1);
        const findings = await page.evaluate(scanPage);
        const name = `${theme}__${vpName}__${step.name}`;
        await page.screenshot({ path: path.join(out, `${name}.png`) });
        const seen = new Set<string>();
        for (const f of findings) {
          const key = `${f.kind}|${f.sel}|${f.text}`;
          if (seen.has(key)) continue;
          seen.add(key);
          if (f.severity === "broken") broken++;
          report.push(`${f.severity === "broken" ? "BROKEN" : "warn  "} ${name}  ${f.kind}  ${f.sel}  "${f.text}"  ${f.detail}`);
        }
      }
      for (const e of errors) report.push(`ERROR  ${theme}__${vpName}  ${e}`);
      broken += errors.length;
      await context.close();
    }
  }
  fs.writeFileSync(path.join(out, "findings.txt"), report.join("\n") + "\n");
  console.log(report.join("\n"));
  console.log(`${broken === 0 ? "PASS" : "FAIL"}  worst-case data renders without clipped, spilled, overlapping, occluded or low-contrast text (${broken} broken, ${report.length - broken} warnings)`);
} finally {
  await browser?.close();
  child.kill();
}
process.exit(broken === 0 ? 0 : 1);
