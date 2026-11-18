import { expect, test } from "bun:test";
import path from "node:path";

test("fresh designer workspace exposes intrinsic World and unified Entity APIs", async () => {
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
      }
      await Bun.sleep(100);
    }
    expect(health?.status).toBe(200);

    const studio = await request("/");
    const markup = await studio.text();
    expect(markup).toContain("<title>Erzählstudio</title>");

    const entities = await request("/api/v3/content/entities");
    expect(entities.status).toBe(200);
    expect(await entities.json()).toEqual(expect.arrayContaining([expect.objectContaining({ id: "world", properties: [], traits: [] })]));
    const tags = await request("/api/v3/content/gameplay_tags");
    expect(tags.status).toBe(200);
    expect(await tags.json()).toEqual(expect.any(Array));
    const project = await request("/api/v3/project");
    expect(await project.json()).toMatchObject({ id: "workspace-project", name: "Workspace", revision: 0 });
    const created = await request("/api/v3/content/entities", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ id: "", object_id: "", game_id: "", game_id_auto: true, template_id: null, is_template: true, name: "Station", description: "", folder_id: null, tags: [], properties: [], traits: [] }),
    });
    expect(created.status).toBe(201);
    expect(await created.json()).toMatchObject({ id: expect.any(String), is_template: true, template_id: null });
  } finally {
    child.kill();
    await child.exited;
  }
}, 30_000);
