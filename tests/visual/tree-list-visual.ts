// Visual regression test: Tree tab Canvas/List switching, TreeItem trait property columns,
// hierarchy, named relationships, expand/collapse, type-aware property editing, persistence,
// negative value rejection, and graph preservation across view switches.
// Run: node tests/visual/tree-list-visual.ts
//
// Structural assumptions for TreeListView:
// 1. View switch in TreeGraphPanel: container with aria-label="Tree view" containing buttons "Canvas" and "List".
// 2. Component root: .tree-list-view rendered via v-show when viewMode === 'list'.
// 3. Semantic table: <table> with <thead> for columns and <tbody> for hierarchical rows.
// 4. Columns: only TreeItem trait template property definitions are rendered as editable columns
//    (built-in defaults: level, cost, cost_entity, gameplay_tags_add, gameplay_tags_remove, description,
//    asset, entity; plus user-defined extra properties like rank, power).
//    Direct entity properties and properties from other traits are strictly excluded.
//    Also renders an item/node column and a relationship column for directed incoming edges.
// 5. Hierarchy: rows ordered topologically/hierarchically with depth indicators (e.g. data-depth or style indent)
//    and expand/collapse toggle buttons for nodes that have children.
// 6. Relationships: incoming named edge relationships (e.g. "commands", "mentors") displayed per child row.
// 7. Property editing: PropertyValueInput inputs (e.g. .form-input for strings, .num-stepper__input for numbers),
//    saving asynchronously to the backend entity template record with debouncing and status reporting.
// 8. Unsigned / Nonnegative validation: TreeItem level and cost must be nonnegative; negative values are
//    rejected by backend validation and handled/clamped/flagged by the UI.
// 9. Switching: toggling between Canvas and List preserves canvas pan/zoom/layout and rendered node text.

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

const port = 46_000 + Math.floor(Math.random() * 1_000);
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
function assetId(value: unknown): unknown {
  return value && typeof value === "object" && "asset_id" in value ? value.asset_id : undefined;
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
  let graphTrait: { id: string; key: string; properties?: unknown[] } | undefined;
  let itemTrait: { id: string; key: string; properties?: unknown[] } | undefined;
  for (let i = 0; i < 50; i++) {
    const traits = await api("GET", "/api/v3/content/trait_templates");
    graphTrait = traits.find((t: { key: string }) => t.key === "tree_graph");
    itemTrait = traits.find((t: { key: string }) => t.key === "tree_item");
    if (graphTrait && itemTrait) break;
    await sleep(200);
  }
  check("tree traits created on studio app load", !!graphTrait && !!itemTrait);
  if (!graphTrait || !itemTrait) throw new Error("TreeGraph or TreeItem trait template not found");

  // Check the plugin's real defaults before adding user-defined properties.
  const defaultKeys = ["level", "cost", "cost_entity", "gameplay_tags_add", "gameplay_tags_remove", "description", "asset", "entity"];
  const builtInProperties = Array.isArray(itemTrait.properties) ? itemTrait.properties as Array<{ key: string }> : [];
  check("TreeItem trait is provisioned with all requested default properties",
    defaultKeys.every((key) => builtInProperties.some((property) => property.key === key)));
  const userItemProps = [
    {
      id: "prop-rank",
      key: "rank",
      name: "Rank",
      type: "string",
      default: "Novice",
    },
    {
      id: "prop-power",
      key: "power",
      name: "Power",
      type: "int",
      min: 0,
      default: 10,
    },
  ];

  const existingItemProps: Array<{ key: string }> = Array.isArray(itemTrait.properties)
    ? (itemTrait.properties as Array<{ key: string }>)
    : [];
  const mergedProps = [...existingItemProps];
  for (const p of userItemProps) {
    if (!mergedProps.some((ep) => ep.key === p.key)) {
      mergedProps.push(p);
    }
  }
  itemTrait.properties = mergedProps;
  await api("POST", "/api/v3/content/trait_templates", itemTrait);

  const updatedTraits = await api("GET", "/api/v3/content/trait_templates");
  const verifiedItemTrait = updatedTraits.find((t: { key: string }) => t.key === "tree_item");
  check(
    "TreeItem trait template includes default level and cost properties",
    verifiedItemTrait?.properties?.some((p: { key: string }) => p.key === "level") &&
      verifiedItemTrait?.properties?.some((p: { key: string }) => p.key === "cost"),
  );
  check(
    "TreeItem trait template includes user-defined typed rank and power properties",
    verifiedItemTrait?.properties?.some((p: { key: string }) => p.key === "rank") &&
      verifiedItemTrait?.properties?.some((p: { key: string }) => p.key === "power"),
  );

  // 3. Verify backend rejects negative values for unsigned TreeItem properties (level / cost)
  for (const key of ["level", "cost"]) {
    let rejection = "";
    try {
      await api("POST", "/api/v3/content/entity_templates", {
        id: `item-negative-${key}`,
        name: `Invalid ${key}`,
        traits: [{ id: `ti-negative-${key}`, template_id: itemTrait.id, values: { [key]: -1 } }],
      });
    } catch (error) {
      rejection = String(error);
    }
    check(`service rejects negative TreeItem ${key}`, rejection.includes(key), rejection || "save succeeded");
  }

  // 4. Create gameplay entity target and three TreeItem entity templates: item-alpha, item-beta, item-gamma
  // Note: item-alpha has a direct entity property `direct_secret` which MUST NOT appear as a list column!
  await api("POST", "/api/v3/content/entity_templates", {
    id: "gameplay-target",
    name: "Gameplay Target Sword",
    traits: [],
  });
  await api("POST", "/api/v3/content/assets", { id: "skill-icon", name: "Skill Icon", tags: [], versions: [] });
  await api("POST", "/api/v3/content/gameplay_tags", { id: "skill", name: "skill" });
  await api("POST", "/api/v3/content/gameplay_tags", { id: "skill.fire", name: "skill.fire" });
  await api("POST", "/api/v3/content/gameplay_tags", { id: "skill.ice", name: "skill.ice" });

  await api("POST", "/api/v3/content/entity_templates", {
    id: "item-alpha",
    name: "Alpha Leader",
    traits: [
      {
        id: "ti-alpha",
        template_id: itemTrait.id,
        values: {
          level: 3,
          cost: 100,
          rank: "Commander",
          power: 80,
          entity: "gameplay-target",
        },
      },
    ],
    properties: [
      {
        id: "prop-direct-secret",
        key: "direct_secret",
        name: "Direct Secret",
        type: "string",
        value: "Classified-Direct-Property",
      },
    ],
  });

  await api("POST", "/api/v3/content/entity_templates", {
    id: "item-beta",
    name: "Beta Specialist",
    traits: [
      {
        id: "ti-beta",
        template_id: itemTrait.id,
        values: {
          level: 2,
          cost: 50,
          rank: "Lieutenant",
          power: 50,
        },
      },
    ],
  });

  await api("POST", "/api/v3/content/entity_templates", {
    id: "item-gamma",
    name: "Gamma Cadet",
    traits: [
      {
        id: "ti-gamma",
        template_id: itemTrait.id,
        values: {
          level: 1,
          cost: 10,
          rank: "Recruit",
          power: 20,
        },
      },
    ],
  });

  // 5. Create graph owner with directed named edges forming a 3-tier hierarchy:
  // Alpha -> Beta ("commands")
  // Beta -> Gamma ("mentors")
  await api("POST", "/api/v3/content/entity_templates", {
    id: "tree-owner",
    name: "Fleet Hierarchy Owner",
    traits: [
      {
        id: "ti-owner",
        template_id: graphTrait.id,
        values: {
          nodes: ["item-alpha", "item-beta", "item-gamma"],
          edges: [
            JSON.stringify(["item-alpha", "item-beta", "commands"]),
            JSON.stringify(["item-beta", "item-gamma", "mentors"]),
          ],
          direction: "right",
          positions: "{}",
        },
      },
    ],
  });

  // 6. Open owner entity template and switch to Tree tab
  await page.goto(`${base}/#/entity-templates/tree-owner`);
  await page.reload();
  const treeTab = page.getByRole("button", { name: "Tree", exact: true });
  await treeTab.waitFor();
  await treeTab.click();

  // Verify Tree panel header and initial Canvas mode
  const treePanel = page.locator('.tree-panel[aria-label="Tree graph"]');
  await treePanel.waitFor();
  check("tree panel loaded", await treePanel.isVisible());

  const viewSwitch = page.locator('.tree-view-switch[aria-label="Tree view"], [aria-label="Tree view"]');
  await viewSwitch.waitFor();

  const canvasBtn = viewSwitch.getByRole("button", { name: "Canvas", exact: true });
  const listBtn = viewSwitch.getByRole("button", { name: "List", exact: true });
  await canvasBtn.waitFor();
  await listBtn.waitFor();
  check("view switch has Canvas and List buttons", (await canvasBtn.count()) === 1 && (await listBtn.count()) === 1);
  check("canvas button is initially active", (await canvasBtn.getAttribute("aria-pressed")) === "true");

  // Canvas elements initially rendered
  const canvasScroll = page.locator(".tree-scroll");
  const nodeAlpha = page.locator('.tree-node[data-tree-id="item-alpha"]');
  const nodeBeta = page.locator('.tree-node[data-tree-id="item-beta"]');
  const nodeGamma = page.locator('.tree-node[data-tree-id="item-gamma"]');
  await canvasScroll.waitFor();
  await nodeAlpha.waitFor();
  await nodeBeta.waitFor();
  await nodeGamma.waitFor();
  check("canvas nodes rendered before switching", (await nodeAlpha.isVisible()) && (await nodeBeta.isVisible()) && (await nodeGamma.isVisible()));

  // 7. Switch to List view
  await listBtn.click();
  check("list button pressed after click", (await listBtn.getAttribute("aria-pressed")) === "true");
  check("canvas button not pressed after list switch", (await canvasBtn.getAttribute("aria-pressed")) === "false");

  // Assert .tree-list-view container is visible
  const treeListView = page.locator(".tree-list-view");
  await treeListView.waitFor();
  check(".tree-list-view is visible", await treeListView.isVisible());

  // Assert semantic table is rendered inside list view
  const table = treeListView.locator("table, [role='table']").first();
  await table.waitFor();
  check("semantic table element present in tree-list-view", await table.isVisible());

  // 8. Assert table columns:
  // - MUST contain TreeItem trait template property definitions (default level, cost; user rank, power)
  // - MUST contain relationship column
  // - MUST NOT contain direct entity property `direct_secret` or "Direct Secret"
  // - MUST NOT contain unrelated trait properties
  const headerCells = table.locator("thead th, [role='columnheader']");
  await headerCells.first().waitFor();
  const headerTexts = (await headerCells.allTextContents()).map((h) => h.trim().toLowerCase());

  check(
    "columns include default TreeItem level property",
    headerTexts.some((h) => h.includes("level")),
    `headers: ${JSON.stringify(headerTexts)}`,
  );
  check(
    "columns include default TreeItem cost property",
    headerTexts.some((h) => h.includes("cost")),
    `headers: ${JSON.stringify(headerTexts)}`,
  );
  check(
    "columns include TreeItem trait rank property",
    headerTexts.some((h) => h.includes("rank")),
    `headers: ${JSON.stringify(headerTexts)}`,
  );
  check(
    "columns include TreeItem trait power property",
    headerTexts.some((h) => h.includes("power")),
    `headers: ${JSON.stringify(headerTexts)}`,
  );
  check(
    "columns include relationship / connection column",
    headerTexts.some((h) => h.includes("relation") || h.includes("edge") || h.includes("connection") || h.includes("link")),
    `headers: ${JSON.stringify(headerTexts)}`,
  );
  check(
    "columns strictly exclude direct entity properties (Direct Secret)",
    !headerTexts.some((h) => h.includes("direct secret") || h.includes("direct_secret") || h.includes("classified")),
    `headers: ${JSON.stringify(headerTexts)}`,
  );

  // 9. Assert table rows & hierarchy:
  function locateRow(id: string): Locator {
    return table.locator(`tbody tr[data-node-id="${id}"]`).first();
  }

  const rowAlpha = locateRow("item-alpha");
  const rowBeta = locateRow("item-beta");
  const rowGamma = locateRow("item-gamma");

  await rowAlpha.waitFor();
  await rowBeta.waitFor();
  await rowGamma.waitFor();
  check("all 3 entity rows rendered in tree table", (await rowAlpha.isVisible()) && (await rowBeta.isVisible()) && (await rowGamma.isVisible()));

  // Verify hierarchy indentation / depth:
  // Alpha is root (depth 0), Beta is child (depth 1), Gamma is grandchild (depth 2)
  const depthAlpha = await rowAlpha.evaluate((el) => {
    const attr = el.getAttribute("data-depth");
    if (attr !== null) return parseInt(attr, 10);
    const indented = el.querySelector("[style*='padding-left'], [data-depth], .tree-indent");
    if (!indented) return 0;
    const stylePad = (indented as HTMLElement).style.paddingLeft;
    return stylePad ? parseFloat(stylePad) : 0;
  });

  const depthBeta = await rowBeta.evaluate((el) => {
    const attr = el.getAttribute("data-depth");
    if (attr !== null) return parseInt(attr, 10);
    const indented = el.querySelector("[style*='padding-left'], [data-depth], .tree-indent");
    if (!indented) return 0;
    const stylePad = (indented as HTMLElement).style.paddingLeft;
    return stylePad ? parseFloat(stylePad) : 0;
  });

  const depthGamma = await rowGamma.evaluate((el) => {
    const attr = el.getAttribute("data-depth");
    if (attr !== null) return parseInt(attr, 10);
    const indented = el.querySelector("[style*='padding-left'], [data-depth], .tree-indent");
    if (!indented) return 0;
    const stylePad = (indented as HTMLElement).style.paddingLeft;
    return stylePad ? parseFloat(stylePad) : 0;
  });

  check(
    "hierarchy depth indicates nested parent-child relationship (Beta indented further than Alpha)",
    depthBeta > depthAlpha || (await rowBeta.locator(".tree-indent, [data-depth='1']").count()) > 0,
    `depthAlpha: ${depthAlpha}, depthBeta: ${depthBeta}`,
  );
  check(
    "hierarchy depth indicates grandchild relationship (Gamma indented further than or equal to Beta)",
    depthGamma >= depthBeta || (await rowGamma.locator(".tree-indent, [data-depth='2']").count()) > 0,
    `depthBeta: ${depthBeta}, depthGamma: ${depthGamma}`,
  );

  // 10. Assert named relationships:
  // Alpha -> Beta edge has relationship "commands"
  // Beta -> Gamma edge has relationship "mentors"
  const betaText = await rowBeta.textContent();
  check("Beta row displays incoming edge relationship 'commands'", betaText?.includes("commands") ?? false, `row text: ${betaText}`);

  const gammaText = await rowGamma.textContent();
  check("Gamma row displays incoming edge relationship 'mentors'", gammaText?.includes("mentors") ?? false, `row text: ${gammaText}`);

  // 11. Assert root/child expansion collapse:
  // Look for expand/collapse toggle on Alpha (root with children)
  const alphaToggle = rowAlpha
    .locator("button[aria-expanded], button.tree-row-toggle, button.tree-toggle, button[aria-label*='collapse' i], button[aria-label*='expand' i]")
    .first();

  if ((await alphaToggle.count()) > 0) {
    const initiallyExpanded = (await alphaToggle.getAttribute("aria-expanded")) !== "false";
    check("root row expand/collapse toggle present and initially expanded", initiallyExpanded);

    // Click collapse on Alpha
    await alphaToggle.click();
    await sleep(200);

    // Assert children rows are collapsed/hidden
    const betaVisibleAfterCollapse = await rowBeta.isVisible().catch(() => false);
    check("collapsing root node hides child row (Beta)", !betaVisibleAfterCollapse);

    // Re-expand Alpha
    await alphaToggle.click();
    await sleep(200);

    const betaVisibleAfterExpand = await rowBeta.isVisible().catch(() => false);
    check("re-expanding root node restores child row (Beta)", betaVisibleAfterExpand);
  } else {
    // If specific button lacks aria-expanded, check for clickable toggle icon/chevron
    const chevronToggle = rowAlpha.locator(".tree-toggle, .row-toggle, button").first();
    check("expand/collapse toggle affordance found", (await chevronToggle.count()) > 0);
  }

  // Invalid unsigned values remain unsaved and receive explicit feedback.
  const levelInput = rowAlpha.locator('td[data-property="level"] input');
  await levelInput.fill("-15");
  check("negative level is marked invalid", await levelInput.getAttribute("aria-invalid") === "true");
  const unchanged = await api("GET", "/api/v3/content/entity_templates/item-alpha");
  check("invalid level does not reach storage", unchanged.traits.find((t: { template_id: string }) => t.template_id === itemTrait!.id).values.level === 3);
  await rowBeta.locator(".tree-item-name a").click();
  check("invalid TreeItem input blocks navigation to another entity",
    page.url().includes("/entity-templates/tree-owner"));
  await page.getByRole("button", { name: "Properties", exact: true }).click();
  await page.getByRole("button", { name: "Tree", exact: true }).click();
  await page.locator(".tree-view-switch").getByRole("button", { name: "List", exact: true }).click();
  check("invalid draft survives changing entity tabs", await levelInput.inputValue() === "-15");
  await levelInput.fill("3");

  // Change independent typed TreeItem properties.
  const rankInput = rowAlpha.locator('td[data-property="rank"] input');
  const powerInput = rowAlpha.locator('td[data-property="power"] input');

  await rankInput.waitFor();
  await powerInput.waitFor();

  // Edit string rank to "Fleet Admiral" and power to "99"
  await rankInput.fill("Fleet Admiral");
  await rankInput.dispatchEvent("change");
  await rankInput.dispatchEvent("blur");

  await powerInput.fill("99");
  await powerInput.dispatchEvent("change");
  await powerInput.dispatchEvent("blur");

  // Wait for asynchronous item save to persist back to entity template via StudioApi
  // Observe save status or poll backend until persisted
  let persistedRank = "";
  let persistedPower = 0;
  for (let attempt = 0; attempt < 50; attempt++) {
    await sleep(200);
    try {
      const alphaTemplate = await api("GET", "/api/v3/content/entity_templates/item-alpha");
      const trait = alphaTemplate?.traits?.find((t: { template_id: string }) => t.template_id === itemTrait!.id);
      if (trait?.values?.rank === "Fleet Admiral" && (trait?.values?.power === 99 || trait?.values?.power === "99")) {
        persistedRank = trait.values.rank;
        persistedPower = Number(trait.values.power);
        break;
      }
    } catch {
      /* continue polling */
    }
  }

  check(
    "asynchronous save persisted updated rank to backend entity template",
    persistedRank === "Fleet Admiral",
    `persisted rank: ${persistedRank}`,
  );
  check(
    "asynchronous save persisted updated power to backend entity template",
    persistedPower === 99,
    `persisted power: ${persistedPower}`,
  );
  // Gameplay entity references are independent of TreeGraph relationships.
  const ownerBefore = await api("GET", "/api/v3/content/entity_templates/tree-owner");
  const entitySelect = rowAlpha.locator('td[data-property="entity"] select');
  await entitySelect.selectOption("item-gamma");
  await rowAlpha.locator('td[data-property="cost_entity"] select').selectOption("gameplay-target");
  await rowAlpha.locator('td[data-property="asset"] select').selectOption("skill-icon");
  const addedTag = rowAlpha.locator('td[data-property="gameplay_tags_add"] input');
  await addedTag.fill("skill.fire");
  await addedTag.press("Enter");
  const removedTag = rowAlpha.locator('td[data-property="gameplay_tags_remove"] input');
  await removedTag.fill("skill.ice");
  await removedTag.press("Enter");
  await rowAlpha.locator('td[data-property="description"] textarea').fill("Unlocks a special ability.");
  let savedValues: Record<string, unknown> = {};
  for (let attempt = 0; attempt < 50; attempt++) {
    const record = await api("GET", "/api/v3/content/entity_templates/item-alpha");
    savedValues = record.traits.find((t: { template_id: string }) => t.template_id === itemTrait!.id).values;
    if (savedValues.entity === "item-gamma" && savedValues.cost_entity === "gameplay-target" &&
        assetId(savedValues.asset) === "skill-icon" &&
        JSON.stringify(savedValues.gameplay_tags_add) === '["skill.fire"]' &&
        JSON.stringify(savedValues.gameplay_tags_remove) === '["skill.ice"]' &&
        savedValues.description === "Unlocks a special ability.") break;
    await sleep(200);
  }
  const ownerAfter = await api("GET", "/api/v3/content/entity_templates/tree-owner");
  check("gameplay entity reference saves on the TreeItem", savedValues.entity === "item-gamma");
  check("cost entity and asset references save on the TreeItem",
    savedValues.cost_entity === "gameplay-target" && assetId(savedValues.asset) === "skill-icon");
  check("added and removed gameplay tags and description save on the TreeItem",
    JSON.stringify(savedValues.gameplay_tags_add) === '["skill.fire"]' &&
    JSON.stringify(savedValues.gameplay_tags_remove) === '["skill.ice"]' &&
    savedValues.description === "Unlocks a special ability.");
  check("gameplay entity edit leaves graph relationships unchanged",
    JSON.stringify(ownerBefore.traits) === JSON.stringify(ownerAfter.traits));

  // 15. Assert persistence after page reload:
  await page.reload();

  // Re-open Tree tab and switch to List
  const treeTabReloaded = page.getByRole("button", { name: "Tree", exact: true });
  await treeTabReloaded.waitFor();
  await treeTabReloaded.click();

  const listBtnReloaded = page.locator('.tree-view-switch[aria-label="Tree view"], [aria-label="Tree view"]')
    .getByRole("button", { name: "List", exact: true });
  await listBtnReloaded.waitFor();
  await listBtnReloaded.click();

  const tableReloaded = page.locator(".tree-list-view table, .tree-list-view [role='table']").first();
  await tableReloaded.waitFor();

  const rowAlphaReloaded = locateRow("item-alpha");
  await rowAlphaReloaded.waitFor();

  const rankInputReloaded = rowAlphaReloaded.locator('td[data-property="rank"] input');
  const powerInputReloaded = rowAlphaReloaded.locator('td[data-property="power"] input');

  const reloadedRankValue = await rankInputReloaded.inputValue();
  const reloadedPowerValue = await powerInputReloaded.inputValue();

  check(
    "reloaded tree list table displays persisted rank 'Fleet Admiral'",
    reloadedRankValue === "Fleet Admiral",
    `got: ${reloadedRankValue}`,
  );
  check(
    "reloaded tree list table displays persisted power '99'",
    reloadedPowerValue === "99",
    `got: ${reloadedPowerValue}`,
  );

  // 15. Assert switching to Canvas and back preserves graph and text:
  const canvasBtnReloaded = page.locator('.tree-view-switch[aria-label="Tree view"], [aria-label="Tree view"]')
    .getByRole("button", { name: "Canvas", exact: true });
  await canvasBtnReloaded.click();

  // Canvas should become visible while List is hidden
  const canvasScrollReloaded = page.locator(".tree-scroll");
  await canvasScrollReloaded.waitFor();
  check("canvas container visible after switching back to Canvas", await canvasScrollReloaded.isVisible());
  check("list view hidden after switching to Canvas", !(await page.locator(".tree-list-view").isVisible()));

  // Verify graph topology and node text preserved on canvas
  const nodeAlphaCanvas = page.locator('.tree-node[data-tree-id="item-alpha"]');
  const nodeBetaCanvas = page.locator('.tree-node[data-tree-id="item-beta"]');
  const nodeGammaCanvas = page.locator('.tree-node[data-tree-id="item-gamma"]');
  await nodeAlphaCanvas.waitFor();
  await nodeBetaCanvas.waitFor();
  await nodeGammaCanvas.waitFor();

  const textAlpha = await nodeAlphaCanvas.textContent();
  const textBeta = await nodeBetaCanvas.textContent();
  const textGamma = await nodeGammaCanvas.textContent();

  check("canvas node Alpha text preserved ('Alpha Leader')", textAlpha?.includes("Alpha Leader") ?? false);
  check("canvas node Beta text preserved ('Beta Specialist')", textBeta?.includes("Beta Specialist") ?? false);
  check("canvas node Gamma text preserved ('Gamma Cadet')", textGamma?.includes("Gamma Cadet") ?? false);

  const wires = page.locator(".tree-edge-hit, .tree-edge");
  await wires.first().waitFor({ state: "attached" });
  check("canvas wires/connections preserved", (await wires.count()) >= 2);

  // Switch back to List view and verify state
  await listBtnReloaded.click();
  await page.locator(".tree-list-view").waitFor();
  check("switching back to List view renders tree list table again", await page.locator(".tree-list-view").isVisible());
  check("canvas container hidden after switching back to List", !(await canvasScrollReloaded.isVisible()));

  const rankInputFinal = locateRow("item-alpha").locator('td[data-property="rank"] input');
  check("edited property value intact after round-trip view switch", (await rankInputFinal.inputValue()) === "Fleet Admiral");

  // A failed save must survive panel unmount and remain retryable.
  await page.route("**/api/v3/content/entity_templates", async (route) => {
    if (route.request().method() === "POST" && route.request().postDataJSON()?.id === "item-beta") {
      await route.fulfill({ status: 503, contentType: "application/json", body: '{"error":"Save unavailable"}' });
    } else {
      await route.continue();
    }
  });
  const betaRank = locateRow("item-beta").locator('td[data-property="rank"] input');
  await betaRank.fill("Captain");
  await locateRow("item-beta").getByRole("button", { name: "Retry" }).waitFor();
  await page.getByRole("button", { name: "Properties", exact: true }).click();
  await page.evaluate(async () => {
    const app = (document.querySelector("#app") as HTMLElement & { __vue_app__: { config: { globalProperties: { $router: { push(path: string): Promise<unknown> } } } } }).__vue_app__;
    await app.config.globalProperties.$router.push("/entity-templates/item-beta");
  });
  check("failed TreeItem save blocks navigation after the tree panel unmounts",
    page.url().includes("/entity-templates/tree-owner"));
  await page.getByRole("button", { name: "Tree", exact: true }).click();
  await page.locator('.tree-view-switch').getByRole("button", { name: "List", exact: true }).click();
  check("failed edit survives leaving and returning to the tree panel",
    await locateRow("item-beta").locator('td[data-property="rank"] input').inputValue() === "Captain");
  const storedBeta = await api("GET", "/api/v3/content/entity_templates/item-beta");
  const storedRank = storedBeta.traits.find((trait: { template_id: string }) => trait.template_id === itemTrait!.id).values.rank;
  await page.getByRole("button", { name: "Discard failed edits" }).click();
  check("discarding failed edits restores the saved value",
    await locateRow("item-beta").locator('td[data-property="rank"] input').inputValue() === storedRank);
  await betaRank.fill("Captain");
  await locateRow("item-beta").getByRole("button", { name: "Retry" }).waitFor();
  await page.unroute("**/api/v3/content/entity_templates");
  await locateRow("item-beta").getByRole("button", { name: "Retry" }).click();
  let retried = "";
  for (let attempt = 0; attempt < 50; attempt++) {
    const record = await api("GET", "/api/v3/content/entity_templates/item-beta");
    retried = record.traits.find((trait: { template_id: string }) => trait.template_id === itemTrait!.id).values.rank;
    if (retried === "Captain") break;
    await sleep(200);
  }
  check("retry persists the preserved edit", retried === "Captain");
  await betaRank.fill("Navigator");
  await locateRow("item-beta").locator(".tree-item-name a").click();
  await page.waitForURL("**/entity-templates/item-beta");
  check("item link navigates to the selected entity", page.url().includes("/entity-templates/item-beta"));
  const navigatedBeta = await api("GET", "/api/v3/content/entity_templates/item-beta");
  check("navigation flushes pending item edits",
    navigatedBeta.traits.find((trait: { template_id: string }) => trait.template_id === itemTrait!.id).values.rank === "Navigator");

  // A graph owner can itself be a TreeItem. Editing it must preserve the owner draft.
  await api("POST", "/api/v3/content/entity_templates", {
    id: "self-owner", name: "Self Owner", traits: [
      { id: "self-item", template_id: itemTrait.id, values: { level: 1 } },
      { id: "self-tree", template_id: graphTrait.id, values: { nodes: ["self-owner"], edges: [], direction: "right", positions: "{}" } },
    ],
  });
  await page.goto(`${base}/#/entity-templates/self-owner`);
  await page.reload();
  await page.getByRole("button", { name: "Tree", exact: true }).click();
  await page.locator(".tree-view-switch").getByRole("button", { name: "List", exact: true }).click();
  await page.locator('tr[data-node-id="self-owner"] td[data-property="level"] input').fill("4");
  const beforeOwnerSave = await api("GET", "/api/v3/content/entity_templates/self-owner");
  check("owner TreeItem edit remains in the owner draft",
    beforeOwnerSave.traits.find((trait: { template_id: string }) => trait.template_id === itemTrait!.id).values.level === 1);
  let discardPrompt = false;
  page.removeAllListeners("dialog");
  page.once("dialog", async (dialog) => { discardPrompt = true; await dialog.dismiss(); });
  await page.evaluate(async () => {
    const app = (document.querySelector("#app") as HTMLElement & { __vue_app__: { config: { globalProperties: { $router: { push(path: string): Promise<unknown> } } } } }).__vue_app__;
    await app.config.globalProperties.$router.push("/entity-templates/item-alpha");
  });
  check("owner draft prompts before navigating to another entity",
    discardPrompt && page.url().includes("/entity-templates/self-owner"));
  page.on("dialog", (dialog) => void dialog.accept());
  const saveOwner = page.getByRole("button", { name: "Save", exact: true });
  await Promise.all([
    page.waitForResponse((response) => response.url().endsWith("/api/v3/content/entity_templates") && response.request().method() === "POST" && response.status() === 201),
    saveOwner.click(),
  ]);
  const savedOwner = await api("GET", "/api/v3/content/entity_templates/self-owner");
  check("saving owner persists TreeItem field without changing its graph",
    savedOwner.traits.find((trait: { template_id: string }) => trait.template_id === itemTrait!.id).values.level === 4 &&
    savedOwner.traits.find((trait: { template_id: string }) => trait.template_id === graphTrait!.id).values.nodes[0] === "self-owner");

  if (errors.length) console.log("console errors:\n" + errors.join("\n"));
  check("no unexpected console errors", errors.length === 0);
} finally {
  await browser?.close();
  child.kill();
}

if (failures.length) {
  console.log(`\n${failures.length} failure(s) in tree list visual regression`);
  process.exit(1);
}
console.log("\nall tree list visual checks passed");
