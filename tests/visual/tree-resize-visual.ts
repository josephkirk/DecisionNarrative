// Visual regression test: resizing a TreeGraph node must not shift other nodes.
// Run: node tests/visual/tree-resize-visual.ts
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

const port = 41_000 + Math.floor(Math.random() * 1_000);
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

function assertBoxesEqual(
  actual: { x: number; y: number; width: number; height: number },
  expected: { x: number; y: number; width: number; height: number },
  label: string,
) {
  const dx = Math.abs(actual.x - expected.x);
  const dy = Math.abs(actual.y - expected.y);
  const dw = Math.abs(actual.width - expected.width);
  const dh = Math.abs(actual.height - expected.height);
  const ok = dx < 1 && dy < 1 && dw < 1 && dh < 1;
  check(
    `${label} unchanged`,
    ok,
    `expected (${expected.x.toFixed(1)}, ${expected.y.toFixed(1)}, ${expected.width.toFixed(1)}, ${expected.height.toFixed(1)}) got (${actual.x.toFixed(1)}, ${actual.y.toFixed(1)}, ${actual.width.toFixed(1)}, ${actual.height.toFixed(1)})`,
  );
}

let browser;
try {
  // Wait for service
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

  // 1. Create traits via opening the app
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

  // 2. POST two entity templates plus TreeGraph owner
  await api("POST", "/api/v3/content/entity_templates", {
    id: "item-a",
    name: "Item A",
    traits: [{ id: "ti-a", template_id: itemTrait.id, values: {} }],
  });
  await api("POST", "/api/v3/content/entity_templates", {
    id: "item-b",
    name: "Item B",
    traits: [{ id: "ti-b", template_id: itemTrait.id, values: {} }],
  });
  await api("POST", "/api/v3/content/entity_templates", {
    id: "tree-owner",
    name: "Tree Owner",
    traits: [
      {
        id: "ti-owner",
        template_id: graphTrait.id,
        values: {
          nodes: ["item-a", "item-b"],
          edges: [],
          direction: "right",
          positions: "{}",
        },
      },
    ],
  });

  // 3. Open Tree tab
  await page.goto(`${base}/#/entity-templates/tree-owner`);
  const treeTab = page.getByRole("button", { name: "Tree", exact: true });
  await treeTab.waitFor();
  await treeTab.click();

  const nodeA = page.locator('.tree-node[data-tree-id="item-a"]');
  const nodeB = page.locator('.tree-node[data-tree-id="item-b"]');
  await nodeA.waitFor();
  await nodeB.waitFor();

  // Compare world coordinates rather than viewport coordinates, which change on page scroll.
  const nodeBox = (node: typeof nodeA) => node.evaluate((element) => {
    const style = (element as HTMLElement).style;
    return { x: parseFloat(style.left), y: parseFloat(style.top), width: parseFloat(style.width), height: parseFloat(style.height) };
  });
  const initialBoxA = await nodeBox(nodeA);
  const initialBoxB = await nodeBox(nodeB);

  // 5. Drag bottom edge of first and assert second position/dimensions unchanged
  const resizeHandleS = nodeA.locator(".tree-resize.resize-s");
  await resizeHandleS.waitFor();
  const handleBox = (await resizeHandleS.boundingBox())!;

  await page.mouse.move(handleBox.x + handleBox.width / 2, handleBox.y + handleBox.height / 2);
  await page.mouse.down();
  await page.mouse.move(handleBox.x + handleBox.width / 2, handleBox.y + handleBox.height / 2 + 60, { steps: 5 });
  await page.mouse.up();

  const draggedBoxA = await nodeBox(nodeA);
  const draggedBoxB = await nodeBox(nodeB);

  check("first node resized vertically via pointer drag", draggedBoxA.height > initialBoxA.height + 30);
  assertBoxesEqual(draggedBoxB, initialBoxB, "second node after pointer resize");

  // 6. Reload without saving to reset
  await page.reload();
  await page.getByRole("button", { name: "Tree", exact: true }).click();
  await nodeA.waitFor();
  await nodeB.waitFor();

  const resetBoxA = await nodeBox(nodeA);
  const resetBoxB = await nodeBox(nodeB);
  check("first node reset height after reload without saving", Math.abs(resetBoxA.height - initialBoxA.height) < 2);
  assertBoxesEqual(resetBoxB, initialBoxB, "second node after reload");

  // 7. Change first node height in context menu and assert second unchanged
  const nodeAOptions = nodeA.locator('.tree-remove[aria-label="Options for Item A"]');
  await nodeAOptions.click();

  const heightInput = page.locator(".tree-context").getByLabel("Height");
  await heightInput.waitFor();
  await heightInput.fill("140");
  await heightInput.dispatchEvent("change");

  const menuResizedBoxA = await nodeBox(nodeA);
  const menuResizedBoxB = await nodeBox(nodeB);

  check("first node height updated via context menu", Math.abs(menuResizedBoxA.height - 140) < 2);
  assertBoxesEqual(menuResizedBoxB, resetBoxB, "second node after numeric height resize");

  if (errors.length) console.log("console errors:\n" + errors.join("\n"));
  check("no console errors", errors.length === 0);
} finally {
  await browser?.close();
  child.kill();
}

if (failures.length) {
  console.log(`\n${failures.length} failure(s) in tree resize regression`);
  process.exit(1);
}
console.log("\nall tree resize regression checks passed");
