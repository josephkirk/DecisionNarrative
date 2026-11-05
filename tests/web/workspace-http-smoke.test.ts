import { expect, test } from "bun:test";
import path from "node:path";

test("fresh designer workspace exposes intrinsic World and canonical Entity Template APIs", async () => {
  const port = 39_000 + Math.floor(Math.random() * 1_000);
  const binaryCandidates = [
    Bun.env.STORYLET_SERVICE_BIN ? path.resolve(Bun.env.STORYLET_SERVICE_BIN) : null,
    path.resolve(import.meta.dir, `../../target/play-clean/debug/storylet-engine-service${process.platform === "win32" ? ".exe" : ""}`),
    path.resolve(import.meta.dir, `../../target/debug/storylet-engine-service${process.platform === "win32" ? ".exe" : ""}`),
    path.resolve(import.meta.dir, `../../crates/storylet-engine-service/target/debug/storylet-engine-service${process.platform === "win32" ? ".exe" : ""}`)
  ].filter(Boolean) as string[];

  const binaryPath = binaryCandidates.find((candidate) => {
    try { return require("node:fs").existsSync(candidate); } catch { return false; }
  }) ?? binaryCandidates[0];

  const child = Bun.spawn([binaryPath], {
    cwd: process.cwd(),
    env: {
      ...Bun.env,
      STORYLET_STUDIO_ADDR: `127.0.0.1:${port}`,
      STORYLET_STUDIO_DB_PATH: ":memory:",
      STORYLET_ENGINE_DB_PATH: ":memory:",
      STORYLET_RESOURCE_ROOT: ".",
    },
    stdout: "ignore",
    stderr: "ignore",
    windowsHide: true,
  });

  const origin = `http://127.0.0.1:${port}`;
  const request = (path: string, init?: RequestInit) => fetch(`${origin}${path}`, init);

  try {
    let health: Response | undefined;
    for (let attempt = 0; attempt < 150; attempt++) {
      try {
        health = await request("/health");
        if (health.ok) break;
      } catch {
        // Wait for bind
      }
      await Bun.sleep(100);
    }
    expect(health?.status).toBe(200);

    const studio = await request("/");
    const markup = await studio.text();
    expect(markup).toContain("DecisionNarrative — Storylet Studio");

    const entities = await request("/api/v3/content/entities");
    expect(entities.status).toBe(200);
    expect(await entities.json()).toEqual(expect.arrayContaining([expect.objectContaining({ id: "world", root_tag: "world", properties: [], traits: [] })]));
    const templates = await request("/api/v3/content/entity_templates");
    expect(templates.status).toBe(200);
    expect(await templates.json()).not.toEqual(expect.arrayContaining([expect.objectContaining({ id: "world" })]));
    const tags = await request("/api/v3/content/gameplay_tags");
    expect(tags.status).toBe(200);
    expect(await tags.json()).toEqual(expect.any(Array));
    const project = await request("/api/v3/project");
    expect(await project.json()).toMatchObject({ id: "workspace-project", name: "Workspace", revision: 0 });
    const created = await request("/api/v3/content/entity_templates", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ id: "", object_id: "", game_id: "", game_id_auto: true, name: "Station", root_tag: "location", description: "", folder_id: null, tags: [], properties: [], traits: [] }),
    });
    expect(created.status).toBe(201);
    expect((await created.json()).id).toBeTruthy();
  } finally {
    child.kill();
    await child.exited;
  }
}, 30_000);
