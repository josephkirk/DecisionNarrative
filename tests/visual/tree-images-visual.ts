// Visual regression test: TreeGraph image upload, drag, resize, lock/unlock, node lock, layering, and persistence.
// Run: node tests/visual/tree-images-visual.ts
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

const tempResourceRoot = fs.mkdtempSync(path.join(os.tmpdir(), "tree-images-res-"));
const port = 46_000 + Math.floor(Math.random() * 1_000);
const base = `http://127.0.0.1:${port}`;

const child: ChildProcess = spawn(binary, ["--fresh-database"], {
  cwd: root,
  env: {
    ...process.env,
    STORYLET_STUDIO_ADDR: `127.0.0.1:${port}`,
    STORYLET_STUDIO_DB_PATH: ":memory:",
    STORYLET_RESOURCE_ROOT: tempResourceRoot,
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

async function getPos(loc: Locator) {
  return loc.evaluate((el) => {
    const s = (el as HTMLElement).style;
    return { x: parseFloat(s.left) || 0, y: parseFloat(s.top) || 0 };
  });
}

async function getSize(loc: Locator) {
  return loc.evaluate((el) => {
    const s = (el as HTMLElement).style;
    return { width: parseFloat(s.width) || 0, height: parseFloat(s.height) || 0 };
  });
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
  let graphTrait: { id: string; key: string } | undefined;
  let itemTrait: { id: string; key: string } | undefined;
  for (let i = 0; i < 50; i++) {
    const traits = await api("GET", "/api/v3/content/trait_templates");
    graphTrait = traits.find((t: { key: string }) => t.key === "tree_graph");
    itemTrait = traits.find((t: { key: string }) => t.key === "tree_item");
    if (graphTrait && itemTrait) break;
    await sleep(200);
  }
  check("tree traits initialized on app load", !!graphTrait && !!itemTrait);
  if (!graphTrait || !itemTrait) throw new Error("TreeGraph or TreeItem trait template not found");

  // 2. Create at least two nodes and tree-owner with wire
  await api("POST", "/api/v3/content/entity_templates", {
    id: "item-a",
    name: "Node A",
    traits: [{ id: "ti-a", template_id: itemTrait.id, values: {} }],
  });
  await api("POST", "/api/v3/content/entity_templates", {
    id: "item-b",
    name: "Node B",
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
          edges: [JSON.stringify(["item-a", "item-b"])],
          direction: "right",
          positions: "{}",
        },
      },
    ],
  });

  // 3. Open Tree tab
  await page.goto(`${base}/#/entity-templates/tree-owner`);
  await page.reload();
  const treeTab = page.getByRole("button", { name: "Tree", exact: true });
  await treeTab.waitFor();
  await treeTab.click();

  const nodeA = page.locator('.tree-node[data-tree-id="item-a"]');
  const nodeB = page.locator('.tree-node[data-tree-id="item-b"]');
  await nodeA.waitFor();
  await nodeB.waitFor();

  const wire = page.locator(".tree-edge-hit");
  await wire.waitFor({ state: "attached" });
  check("nodes and wire rendered on tree canvas", (await nodeA.isVisible()) && (await nodeB.isVisible()) && (await wire.count()) === 1);

  // 4. Use canvas menu and real PNG upload (buffer via setInputFiles) to add image
  const canvasActionsBtn = page.getByRole("button", { name: "Canvas actions" });
  await canvasActionsBtn.waitFor();
  await canvasActionsBtn.click();

  const contextMenu = page.locator(".tree-context");
  await contextMenu.waitFor();

  const pngBuffer = fs.readFileSync(path.join(root, "src-tauri/icons/128x128.png"));

  const addImageBtn = contextMenu.getByRole("button", { name: "Add image…" });
  await addImageBtn.waitFor();

  const fileInput = page.locator('.tree-scroll input[type="file"]');
  const [fileChooser] = await Promise.all([
    page.waitForEvent("filechooser", { timeout: 3000 }).catch(() => null),
    addImageBtn.click(),
  ]);

  if (fileChooser) {
    await fileChooser.setFiles({
      name: "canvas-image.png",
      mimeType: "image/png",
      buffer: pngBuffer,
    });
  } else {
    await fileInput.setInputFiles({
      name: "canvas-image.png",
      mimeType: "image/png",
      buffer: pngBuffer,
    });
  }

  // Verify image renders
  const imageEl = page.locator(".tree-canvas-image");
  await imageEl.waitFor({ timeout: 10000 });
  check("image renders in .tree-image layer", await imageEl.isVisible());
  await imageEl.locator("img").evaluate(async (img: HTMLImageElement) => { await img.decode(); });
  check("uploaded image bytes render", await imageEl.locator("img").evaluate((img: HTMLImageElement) => img.naturalWidth > 0));

  // A short image menu should stay near its anchor even in a short canvas.
  const canvas = page.locator(".tree-scroll");
  await canvas.evaluate((element: HTMLElement) => { element.style.height = "390px"; });
  await imageEl.scrollIntoViewIfNeeded();
  await sleep(100);
  const compactImage = (await imageEl.boundingBox())!;
  const clickY = compactImage.y + compactImage.height / 2;
  await page.mouse.click(compactImage.x + compactImage.width / 2, clickY, { button: "right" });
  const compactMenu = (await contextMenu.boundingBox())!;
  const compactCanvas = (await canvas.boundingBox())!;
  const expectedY = Math.max(compactCanvas.y, Math.min(clickY, compactCanvas.y + compactCanvas.height - compactMenu.height));
  check("image menu stays near its anchor and within a short canvas", Math.abs(compactMenu.y - expectedY) <= 2);
  await page.keyboard.press("Escape");
  await canvas.evaluate((element: HTMLElement) => { element.style.height = ""; });
  await sleep(100);

  // 5. Verify image can drag and resize
  const initialPos = await getPos(imageEl);
  const initialBox = (await imageEl.boundingBox())!;

  // Drag image by moving mouse
  await page.mouse.move(initialBox.x + initialBox.width / 2, initialBox.y + 12);
  await page.mouse.down();
  await page.mouse.move(initialBox.x + initialBox.width / 2 + 50, initialBox.y + 52, { steps: 5 });
  await page.mouse.up();
  await sleep(150);

  const draggedPos = await getPos(imageEl);
  check(
    "image drags to new position",
    draggedPos.x !== initialPos.x || draggedPos.y !== initialPos.y,
    `from (${initialPos.x}, ${initialPos.y}) to (${draggedPos.x}, ${draggedPos.y})`,
  );

  // Resize image using south-east resize handle
  const initialSize = await getSize(imageEl);
  await imageEl.hover({ position: { x: 10, y: 10 } });
  const resizeHandle = imageEl.locator(".tree-image-resize.resize-se");
  await resizeHandle.waitFor();
  const rbox = (await resizeHandle.boundingBox())!;
  await page.mouse.move(rbox.x + rbox.width / 2, rbox.y + rbox.height / 2);
  await page.mouse.down();
  await page.mouse.move(rbox.x + rbox.width / 2 + 60, rbox.y + rbox.height / 2 + 50, { steps: 5 });
  await page.mouse.up();
  await sleep(150);

  const resizedSize = await getSize(imageEl);
  check(
    "image resizes to new dimensions",
    resizedSize.width > initialSize.width && resizedSize.height > initialSize.height,
    `from ${initialSize.width}x${initialSize.height} to ${resizedSize.width}x${resizedSize.height}`,
  );

  // Move the image under the wire using the keyboard, then check actual hit order.
  const wirePoint = await wire.evaluate((el: SVGPathElement) => {
    const p = el.getPointAtLength(el.getTotalLength() / 2);
    return { x: p.x, y: p.y };
  });
  const beforeWire = await getPos(imageEl);
  const stepsX = Math.round((wirePoint.x - resizedSize.width / 2 - beforeWire.x) / 10);
  const stepsY = Math.round((wirePoint.y - resizedSize.height / 2 - beforeWire.y) / 10);
  await imageEl.focus();
  for (let i = 0; i < Math.abs(stepsX); i++) await page.keyboard.press(stepsX < 0 ? "ArrowLeft" : "ArrowRight");
  for (let i = 0; i < Math.abs(stepsY); i++) await page.keyboard.press(stepsY < 0 ? "ArrowUp" : "ArrowDown");
  const wireScreen = await wire.evaluate((el: SVGPathElement) => {
    const p = el.getPointAtLength(el.getTotalLength() / 2).matrixTransform(el.getScreenCTM()!);
    return { x: p.x, y: p.y };
  });
  const wireOrder = await page.evaluate(({ x, y }) => {
    const elements = document.elementsFromPoint(x, y);
    return { wire: elements.findIndex((el) => el.classList.contains("tree-edge-hit")), image: elements.findIndex((el) => el.classList.contains("tree-canvas-image")) };
  }, wireScreen);
  check("wire receives pointer events above the image", wireOrder.wire >= 0 && wireOrder.image > wireOrder.wire, JSON.stringify(wireOrder));
  for (let i = 0; i < Math.abs(stepsX); i++) await page.keyboard.press(stepsX < 0 ? "ArrowRight" : "ArrowLeft");
  for (let i = 0; i < Math.abs(stepsY); i++) await page.keyboard.press(stepsY < 0 ? "ArrowDown" : "ArrowUp");

  // 6. Lock image prevents movement
  const imgBoxBeforeLock = (await imageEl.boundingBox())!;
  await page.mouse.click(imgBoxBeforeLock.x + imgBoxBeforeLock.width / 2, imgBoxBeforeLock.y + imgBoxBeforeLock.height / 2, { button: "right" });
  await contextMenu.waitFor();
  const lockImageBtn = contextMenu.getByRole("button", { name: "Lock image" });
  await lockImageBtn.waitFor();
  await lockImageBtn.click();
  await sleep(100);

  check("image receives is-locked class", await imageEl.evaluate((el) => el.classList.contains("is-locked")));

  const posLockedBeforeDrag = await getPos(imageEl);
  const lockedBox = (await imageEl.boundingBox())!;
  await page.mouse.move(lockedBox.x + lockedBox.width / 2, lockedBox.y + lockedBox.height / 2);
  await page.mouse.down();
  await page.mouse.move(lockedBox.x + lockedBox.width / 2 + 60, lockedBox.y + lockedBox.height / 2 + 40, { steps: 5 });
  await page.mouse.up();
  await sleep(150);

  const posLockedAfterDrag = await getPos(imageEl);
  check(
    "lock image prevents movement",
    posLockedAfterDrag.x === posLockedBeforeDrag.x && posLockedAfterDrag.y === posLockedBeforeDrag.y,
    `position remained (${posLockedAfterDrag.x}, ${posLockedAfterDrag.y})`,
  );

  // 7. Unlock permits movement
  const lockedBoxAfterPan = (await imageEl.boundingBox())!;
  await page.mouse.click(lockedBoxAfterPan.x + lockedBoxAfterPan.width / 2, lockedBoxAfterPan.y + lockedBoxAfterPan.height / 2, { button: "right" });
  await contextMenu.waitFor();
  const unlockImageBtn = contextMenu.getByRole("button", { name: "Unlock image" });
  await unlockImageBtn.waitFor();
  await unlockImageBtn.click();
  await sleep(100);

  const unlockedBox = (await imageEl.boundingBox())!;
  await page.mouse.move(unlockedBox.x + unlockedBox.width / 2, unlockedBox.y + unlockedBox.height / 2);
  await page.mouse.down();
  await page.mouse.move(unlockedBox.x + unlockedBox.width / 2 + 40, unlockedBox.y + unlockedBox.height / 2 + 30, { steps: 5 });
  await page.mouse.up();
  await sleep(150);

  const posAfterUnlockDrag = await getPos(imageEl);
  check(
    "unlock image permits movement",
    posAfterUnlockDrag.x !== posLockedAfterDrag.x || posAfterUnlockDrag.y !== posLockedAfterDrag.y,
    `moved to (${posAfterUnlockDrag.x}, ${posAfterUnlockDrag.y})`,
  );

  // 8. Node lock prevents movement/resize while still allowing context menu
  const nodeAOptions = nodeA.locator('.tree-remove[aria-label="Options for Node A"]');
  await nodeAOptions.waitFor();
  await nodeAOptions.click();
  await contextMenu.waitFor();

  const lockNodeBtn = contextMenu.getByRole("button", { name: "Lock node" });
  await lockNodeBtn.waitFor();
  await lockNodeBtn.click();
  await sleep(100);

  check("node A receives is-locked class", await nodeA.evaluate((el) => el.classList.contains("is-locked")));
  const nodeAResizeHandles = await nodeA.locator(".tree-resize").count();
  check("locked node removes resize handles", nodeAResizeHandles === 0);

  // Try dragging locked node A
  const nodeAPosBefore = await getPos(nodeA);
  const nodeABox = (await nodeA.boundingBox())!;
  await page.mouse.move(nodeABox.x + 30, nodeABox.y + 30);
  await page.mouse.down();
  await page.mouse.move(nodeABox.x + 80, nodeABox.y + 80, { steps: 5 });
  await page.mouse.up();
  await sleep(150);

  const nodeAPosAfter = await getPos(nodeA);
  check(
    "node lock prevents movement",
    nodeAPosAfter.x === nodeAPosBefore.x && nodeAPosAfter.y === nodeAPosBefore.y,
    `position remained (${nodeAPosAfter.x}, ${nodeAPosAfter.y})`,
  );

  // Context menu still opens on locked node
  await nodeAOptions.click();
  await contextMenu.waitFor();
  const unlockNodeBtn = contextMenu.getByRole("button", { name: "Unlock node" });
  check("context menu opens on locked node with Unlock node button", await unlockNodeBtn.isVisible());
  const widthInput = contextMenu.getByLabel("Width");
  check("node size inputs disabled when node locked", await widthInput.isDisabled());
  await page.keyboard.press("Escape");
  await sleep(100);

  // 9. Save/reload and assert image and node locks persisted
  // Lock image again before save
  const imgBoxBeforeSave = (await imageEl.boundingBox())!;
  await page.mouse.click(imgBoxBeforeSave.x + imgBoxBeforeSave.width / 2, imgBoxBeforeSave.y + imgBoxBeforeSave.height / 2, { button: "right" });
  await contextMenu.waitFor();
  await contextMenu.getByRole("button", { name: "Lock image" }).click();
  await sleep(100);

  const saveBtn = page.getByRole("button", { name: "Save layout" });
  await saveBtn.click();
  await sleep(600);

  await page.reload();
  await treeTab.waitFor();
  await treeTab.click();
  await nodeA.waitFor();
  await imageEl.waitFor();

  check("node lock persisted after reload", await nodeA.evaluate((el) => el.classList.contains("is-locked")));
  check("image lock persisted after reload", await imageEl.evaluate((el) => el.classList.contains("is-locked")));


  // Unlock image so we can reposition it under Node B
  const reloadedImgBox = (await imageEl.boundingBox())!;
  await page.mouse.click(reloadedImgBox.x + reloadedImgBox.width / 2, reloadedImgBox.y + reloadedImgBox.height / 2, { button: "right" });
  await contextMenu.waitFor();
  await contextMenu.getByRole("button", { name: "Unlock image" }).click();
  await sleep(100);

  // Drag image to overlap Node B
  const nodeBBox = (await nodeB.boundingBox())!;
  const curImgBox = (await imageEl.boundingBox())!;
  await page.mouse.move(curImgBox.x + curImgBox.width / 2, curImgBox.y + curImgBox.height / 2);
  await page.mouse.down();
  await page.mouse.move(nodeBBox.x + nodeBBox.width / 2, nodeBBox.y + nodeBBox.height / 2, { steps: 5 });
  await page.mouse.up();
  await sleep(150);

  // Test elementsFromPoint at Node B center
  const layering = await page.evaluate(
    ({ x, y }) => {
      const els = document.elementsFromPoint(x, y);
      const nodeIndex = els.findIndex((el) => el.closest(".tree-node"));
      const imageIndex = els.findIndex((el) => el.closest(".tree-canvas-image, .tree-image-layer"));
      return { nodeIndex, imageIndex, foundBoth: nodeIndex !== -1 && imageIndex !== -1 };
    },
    { x: nodeBBox.x + nodeBBox.width / 2, y: nodeBBox.y + nodeBBox.height / 2 },
  );
  check(
    "elementsFromPoint confirms node paints above image",
    layering.foundBoth && layering.nodeIndex < layering.imageIndex,
    `nodeIndex: ${layering.nodeIndex}, imageIndex: ${layering.imageIndex}`,
  );

  // 11. Exported graph contains image
  const viewOptionsSummary = page.locator(".tree-options summary");
  await viewOptionsSummary.click();
  const exportBtn = page.getByRole("button", { name: "Export JSON" });
  await exportBtn.waitFor();

  const [download] = await Promise.all([
    page.waitForEvent("download"),
    exportBtn.click(),
  ]);

  interface ExportedTreeData {
    graph?: {
      appearance?: {
        images?: unknown[];
      };
    };
  }
  let exportedData: ExportedTreeData | null = null;
  const dlPath = await download.path().catch(() => null);
  if (dlPath && fs.existsSync(dlPath)) {
    exportedData = JSON.parse(fs.readFileSync(dlPath, "utf-8")) as ExportedTreeData;
  } else {
    const stream = await download.createReadStream();
    if (stream) {
      const chunks: Buffer[] = [];
      for await (const chunk of stream) chunks.push(Buffer.from(chunk));
      exportedData = JSON.parse(Buffer.concat(chunks).toString("utf-8")) as ExportedTreeData;
    }
  }


  check(
    "exported graph contains image",
    Array.isArray(exportedData?.graph?.appearance?.images) && exportedData.graph.appearance.images.length > 0,
    `images count: ${exportedData?.graph?.appearance?.images?.length ?? 0}`,
  );

  if (errors.length) console.log("console errors:\n" + errors.join("\n"));
  check("no console errors", errors.length === 0);
} finally {
  try {
    await browser?.close();
  } catch {}
  child.kill();
  try {
    fs.rmSync(tempResourceRoot, { recursive: true, force: true });
  } catch {}
}

if (failures.length) {
  console.log(`\n${failures.length} failure(s) in tree images visual test`);
  process.exit(1);
}
console.log("\nall tree images visual checks passed");
