import { chromium } from "playwright-core";
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

const port = 44_000 + Math.floor(Math.random() * 1_000);
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

let browser;
try {
  for (let i = 0; i < 50; i++) {
    try {
      if ((await fetch(base + "/api/v3/project")).ok) break;
    } catch {
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

  await page.goto(`${base}/#/storylets`);
  let graphTrait: { id: string; key: string } | undefined;
  let itemTrait: { id: string; key: string } | undefined;
  for (let i = 0; i < 50; i++) {
    const traits = await api("GET", "/api/v3/content/trait_templates");
    graphTrait = traits.find((t: { key: string }) => t.key === "tree_graph");
    itemTrait = traits.find((t: { key: string }) => t.key === "tree_item");
    if (graphTrait && itemTrait) break;
    await sleep(200);
  }
  check("tree traits created on app load", !!graphTrait && !!itemTrait);
  if (!graphTrait || !itemTrait) throw new Error("TreeGraph or TreeItem trait template not found");

  await api("POST", "/api/v3/content/entities", {
    id: "item-a",
    name: "Item A",
    traits: [{ id: "ti-a", template_id: itemTrait.id, values: {} }],
  });
  await api("POST", "/api/v3/content/entities", {
    id: "item-b",
    name: "Item B",
    traits: [{ id: "ti-b", template_id: itemTrait.id, values: {} }],
  });
  await api("POST", "/api/v3/content/entities", {
    id: "tree-owner",
    name: "Tree Owner",
    traits: [
      {
        id: "ti-owner",
        template_id: graphTrait.id,
        values: {
          nodes: ["item-a", "item-b"],
          edges: [JSON.stringify(["item-a", "item-b"])],
          direction: "right",
          positions: "{}",
          shapes: JSON.stringify({ "item-b": "diamond" }),
          appearance: JSON.stringify({
            nodes: { "item-b": { width: 240 } },
            edges: {},
          }),
        },
      },
    ],
  });

  await page.goto(`${base}/#/entities/tree-owner`);
  const traitsTab = page.locator(".entity-tab", { hasText: "Traits" });
  await traitsTab.waitFor();
  await traitsTab.click();

  const defaultsEditor = page.locator(".tree-defaults-editor");
  await defaultsEditor.waitFor();
  check("TreeGraph defaults editor rendered", (await defaultsEditor.count()) === 1);

  const rawPropertyTextareas = page.locator('.trait-card textarea:not(#tree-custom-css-textarea)');
  check("raw trait fields not shown for TreeGraph", (await rawPropertyTextareas.count()) === 0);

  const shapeSelect = page.locator("#tree-default-shape");
  await shapeSelect.waitFor();
  await shapeSelect.selectOption("rounded");

  const widthInput = page.locator("#tree-default-width");
  await widthInput.waitFor();
  await widthInput.fill("200");
  await widthInput.dispatchEvent("change");

  const wirePatternSelect = page.locator("#tree-default-wire-pattern");
  await wirePatternSelect.waitFor();
  await wirePatternSelect.selectOption("dash");

  const cssFileRule = `.tree-node-copy { color: rgb(255, 0, 0); }`;
  const cssFileInput = page.locator("#tree-css-upload-input");
  await cssFileInput.setInputFiles({
    name: "theme.css",
    mimeType: "text/css",
    buffer: Buffer.from(cssFileRule, "utf-8"),
  });

  const cssStatus = page.locator(".css-status-text");
  await cssStatus.waitFor();
  const statusUpdated = await page.waitForFunction(() => /theme\.css|loaded/.test(document.querySelector(".css-status-text")?.textContent ?? ""), null, { timeout: 5000 }).then(() => true, () => false);
  check("CSS file upload reflected in status", statusUpdated);

  const saveBtn = page.getByRole("button", { name: "Save", exact: true });
  await saveBtn.waitFor();
  await saveBtn.click();
  await page.locator(".entity-unsaved-pill").waitFor({ state: "detached" });

  const treeTab = page.getByRole("button", { name: "Tree", exact: true });
  await treeTab.waitFor();
  await treeTab.click();

  const nodeA = page.locator('.tree-node[data-tree-id="item-a"]');
  const nodeB = page.locator('.tree-node[data-tree-id="item-b"]');
  await nodeA.waitFor();
  await nodeB.waitFor();

  const nodeAClasses = await nodeA.getAttribute("class");
  check("node A has default shape class shape-rounded", nodeAClasses?.includes("shape-rounded") ?? false);

  const nodeAWidth = await nodeA.evaluate((el) => parseFloat((el as HTMLElement).style.width));
  check("node A has default width 200px", Math.abs(nodeAWidth - 200) < 2, `got ${nodeAWidth}`);

  const nodeBClasses = await nodeB.getAttribute("class");
  check("node B retained per-node override shape-diamond", nodeBClasses?.includes("shape-diamond") ?? false);

  const nodeBWidth = await nodeB.evaluate((el) => parseFloat((el as HTMLElement).style.width));
  check("node B retained per-node override width 240px", Math.abs(nodeBWidth - 240) < 2, `got ${nodeBWidth}`);

  const edgePath = page.locator(".tree-edge.wire-dash");
  await edgePath.waitFor({ state: "attached" });
  check("edge wire pattern has wire-dash class", (await edgePath.count()) >= 1);

  const nodeACopy = nodeA.locator(".tree-node-copy");
  const copyColor = await nodeACopy.evaluate((el) => window.getComputedStyle(el).color);
  check("scoped CSS applies red color to canvas .tree-node-copy", copyColor === "rgb(255, 0, 0)", `got ${copyColor}`);

  const externalStyled = await page.evaluate(() => {
    const dummy = document.createElement("div");
    dummy.className = "tree-node-copy";
    dummy.id = "external-test-element";
    document.body.appendChild(dummy);
    const color = window.getComputedStyle(dummy).color;
    dummy.remove();
    return color === "rgb(255, 0, 0)";
  });
  check("external UI element with .tree-node-copy is NOT styled", !externalStyled);

  await page.reload();
  await page.getByRole("button", { name: "Tree", exact: true }).click();
  await nodeA.waitFor();
  await nodeB.waitFor();

  const reloadedAClasses = await nodeA.getAttribute("class");
  check("persisted node A default shape-rounded after reload", reloadedAClasses?.includes("shape-rounded") ?? false);

  const reloadedAWidth = await nodeA.evaluate((el) => parseFloat((el as HTMLElement).style.width));
  check("persisted node A default width 200px after reload", Math.abs(reloadedAWidth - 200) < 2, `got ${reloadedAWidth}`);

  const reloadedBClasses = await nodeB.getAttribute("class");
  check("persisted node B override shape-diamond after reload", reloadedBClasses?.includes("shape-diamond") ?? false);

  const reloadedBWidth = await nodeB.evaluate((el) => parseFloat((el as HTMLElement).style.width));
  check("persisted node B override width 240px after reload", Math.abs(reloadedBWidth - 240) < 2, `got ${reloadedBWidth}`);

  const reloadedEdgePath = page.locator(".tree-edge.wire-dash");
  await reloadedEdgePath.waitFor({ state: "attached" });
  check("persisted edge wire pattern wire-dash after reload", (await reloadedEdgePath.count()) >= 1);

  const reloadedCopyColor = await nodeA.locator(".tree-node-copy").evaluate((el) => window.getComputedStyle(el).color);
  check("persisted scoped CSS on .tree-node-copy after reload", reloadedCopyColor === "rgb(255, 0, 0)", `got ${reloadedCopyColor}`);

  if (errors.length) console.log("console errors:\n" + errors.join("\n"));
  check("no unexpected console errors", errors.length === 0);
} finally {
  await browser?.close();
  child.kill();
}

if (failures.length) {
  console.log(`\n${failures.length} failure(s) in tree defaults visual test`);
  process.exit(1);
}
console.log("\nall tree defaults visual checks passed");
