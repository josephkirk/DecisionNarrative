import { expect, test } from "bun:test";
import path from "node:path";

test("fresh designer workspace can author and start a scenario through the real HTTP server", async () => {
  const port = 39_000 + Math.floor(Math.random() * 1_000);
  const binaryPath = path.resolve(
    import.meta.dir,
    `../../crates/storylet-engine-service/target/debug/storylet-engine-service${process.platform === "win32" ? ".exe" : ""}`
  );

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

    expect(await (await request("/api/locations")).json()).toEqual([]);
    expect(await (await request("/api/characters")).json()).toEqual([]);
    expect(await (await request("/api/qualities")).json()).toEqual([]);
    expect(await (await request("/api/storylets")).json()).toEqual([]);
    expect(await (await request("/api/v1/scenarios")).json()).toEqual([]);
    expect(await (await request("/api/v1/project")).json()).toMatchObject({ id: "workspace-project", name: "Workspace", revision: 0 });

    expect((await request("/api/v1/project", { method: "PATCH", headers: { "content-type": "application/json" }, body: "null" })).status).toBe(400);
    expect((await request("/api/v1/project-bundle/validate", { method: "POST", headers: { "content-type": "application/json" }, body: "null" })).status).toBe(400);

    const location = await request("/api/locations", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ id: "station", name: "Station", description: "A quiet platform." }),
    });
    expect(location.status).toBe(201);

    const scenario = {
      id: "opening",
      name: "Before first arrival",
      seed: 73,
      start_location_policy: { mode: "fixed", location_id: "station" },
      initial_qualities: {},
      actor_locations: {},
      actor_population_policy: { mode: "all" },
    };
    const saved = await request("/api/v1/scenarios", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(scenario),
    });
    expect(saved.status).toBe(201);

    const renamed = await request("/api/v1/project", {
      method: "PATCH",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ name: "Station Mystery" }),
    });
    expect(await renamed.json()).toEqual({ id: "workspace-project", name: "Station Mystery", revision: 3 });
  } finally {
    child.kill();
    await child.exited;
  }
}, 30_000);
