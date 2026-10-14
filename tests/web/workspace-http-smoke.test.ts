import { expect, test } from "bun:test";

test("fresh designer workspace can author and start a scenario through the real HTTP server", async () => {
  const port = 39_000 + Math.floor(Math.random() * 1_000);
  const child = Bun.spawn(["bun", "run", "src/web/server.ts"], {
    cwd: process.cwd(),
    env: {
      ...Bun.env,
      STORYLET_DB_PATH: ":memory:",
      STORYLET_ENGINE_DB_PATH: ":memory:",
      STORYLET_WEB_PORT: String(port),
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
        health = await request("/api/health");
        if (health.ok) break;
      } catch {
        // The server is still binding its local port.
      }
      await Bun.sleep(100);
    }
    expect(health?.status).toBe(200);
    const studio = await request("/");
    const markup = await studio.text();
    expect(markup).toContain("/dist/client.js");
    expect(markup).toContain("scenario-initial-quality-fields");
    expect(markup).toContain("scenario-actor-location-fields");
    expect(markup).toContain("btn-export-project");
    expect(markup).toContain("btn-import-project");
    expect(markup).toContain("btn-select-project-bundle");
    expect(markup).toContain("generation-stage-provenance");
    expect(markup).toContain("No scene draft yet");
    expect(markup).toContain("Choice drafts appear here after generation.");
    expect(markup).toContain("STORYLET LIBRARY");
    expect(markup).toContain("Project storage");
    expect(markup).not.toContain("A condensation line runs along the storage room");
    expect(markup).not.toContain("Civilians");
    expect(markup).not.toContain("ACTIVE DECK (SQLite)");
    expect(markup).not.toContain("JSON object");
    const clientBundle = await (await request("/dist/client.js")).text();
    expect(clientBundle).not.toContain("data-demo-project");
    expect(await (await request("/api/locations")).json()).toEqual([]);
    expect(await (await request("/api/characters")).json()).toEqual([]);
    expect(await (await request("/api/qualities")).json()).toEqual([]);
    expect(await (await request("/api/storylets")).json()).toEqual([]);
    expect(await (await request("/api/v1/scenarios")).json()).toEqual([]);
    expect(await (await request("/api/v1/project")).json()).toMatchObject({ id: "workspace-project", name: "Workspace", revision: 0 });
    const craftResources = await (await request("/api/v1/craft-resources")).json() as { resources: Array<{ id: string; sourceHash: string; promptHash: string }> };
    expect(craftResources.resources).toHaveLength(7);
    expect(craftResources.resources.every((resource) => resource.sourceHash.length === 64 && resource.promptHash.length === 64)).toBe(true);
    expect((await request("/api/v1/project", { method: "PATCH", headers: { "content-type": "application/json" }, body: "null" })).status).toBe(400);
    expect((await request("/api/v1/project-bundle/validate", { method: "POST", headers: { "content-type": "application/json" }, body: "null" })).status).toBe(400);

    const location = await request("/api/locations", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ id: "station", name: "Station", description: "A quiet platform." }),
    });
    expect(location.status).toBe(200);

    const scenario = {
      id: "opening",
      name: "Before first arrival",
      player_start_location: "station",
      initial_qualities: {},
      actor_locations: {},
      storylet_ids: [],
      seed: 73,
    };
    const saved = await request("/api/v1/scenarios", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(scenario),
    });
    expect(saved.status).toBe(201);
    expect((await saved.json()).scenario).toEqual(scenario);

    const renamed = await request("/api/v1/project", {
      method: "PATCH",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ name: "Station Mystery" }),
    });
    expect(await renamed.json()).toEqual({ id: "workspace-project", name: "Station Mystery", revision: 3 });

    const exported = await request("/api/v1/project-bundle");
    const bundle = await exported.json();
    expect(exported.status).toBe(200);
    expect(exported.headers.get("content-disposition")).toContain("station-mystery.storylet.json");
    expect(bundle).toMatchObject({ format: "decision-narrative-project", schema_version: 1, project: { name: "Station Mystery", revision: 3 } });
    expect(bundle.content.scenarios).toEqual([scenario]);

    await request("/api/locations", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ id: "temporary", name: "Temporary", description: "Should be removed by import." }),
    });
    const preview = await request("/api/v1/project-bundle/validate", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ bundle }),
    });
    expect(await preview.json()).toMatchObject({ valid: true, project: { name: "Station Mystery" }, counts: { locations: 1, scenarios: 1 } });
    expect((await (await request("/api/locations")).json()).map((item: any) => item.id)).toEqual(["station", "temporary"]);

    const invalidBundle = structuredClone(bundle);
    invalidBundle.schema_version = 999;
    const invalidPreview = await request("/api/v1/project-bundle/validate", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ bundle: invalidBundle }),
    });
    expect(invalidPreview.status).toBe(422);
    expect((await invalidPreview.json()).issues.join(" ").toLowerCase()).toContain("unsupported schema version");
    expect((await (await request("/api/locations")).json()).map((item: any) => item.id)).toEqual(["station", "temporary"]);

    const unconfirmed = await request("/api/v1/project-bundle/import", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ bundle }),
    });
    expect(unconfirmed.status).toBe(409);
    expect((await (await request("/api/locations")).json()).map((item: any) => item.id)).toEqual(["station", "temporary"]);

    const imported = await request("/api/v1/project-bundle/import", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ bundle, confirmReplace: true }),
    });
    expect(imported.status).toBe(200);
    expect(await (await request("/api/v1/project")).json()).toMatchObject({ name: "Station Mystery", revision: 3 });
    expect((await (await request("/api/locations")).json()).map((item: any) => item.id)).toEqual(["station"]);

    const playtest = await request("/api/v1/playtests/from-workspace", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ scenarioId: scenario.id }),
    });
    expect(playtest.status).toBe(201);
    const started = await playtest.json();
    expect(started.snapshot.project_id).toBe("workspace-project");
    expect(started.snapshot.world.current_location).toBe("station");
    expect(started.snapshot.world.turn).toBe(0);
  } finally {
    child.kill();
    await child.exited;
  }
}, 30_000);
