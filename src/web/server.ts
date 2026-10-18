/**
 * Storylet Studio — Designer Web Platform & Game Server
 * Exposes full REST API for:
 * - Entity Management (Characters, Locations, Qualities in SQLite)
 * - Generic Rust director and causal playtest trace
 * - Butterfly Effect Simulation (Autonomous NPC Ticks, Observer Ripples, Rollback)
 */

import * as path from "node:path";
import * as fs from "node:fs";
import { StoryletRepository, type ScenarioRecord } from "../db/repository.ts";
import { RustRuntimeClient } from "../engine/RustRuntimeClient.ts";
import { handlePlaytestApi } from "./playtestApi.ts";
import { buildWorkspaceProject } from "./workspaceProject.ts";
import { inspectProjectBundle, makeProjectBundle, type ProjectBundleContent } from "./projectBundle.ts";

const PORT = Number(Bun.env.STORYLET_WEB_PORT ?? 3000);
const PUBLIC_DIR = path.resolve("public");
const DATABASE_PATH = Bun.env.STORYLET_DB_PATH ?? "data/storylet_studio.db";

// Initialize Repositories and Engines
const repo = new StoryletRepository(DATABASE_PATH);
const rustRuntime = new RustRuntimeClient({
  address: Bun.env.STORYLET_ENGINE_ADDR,
  databasePath: Bun.env.STORYLET_ENGINE_DB_PATH ?? "data/storylet_engine.db",
});

let rustRuntimeReady = false;
try {
  await rustRuntime.warmUp();
  rustRuntimeReady = true;
} catch (error) {
  console.error("Rust playtest host did not start; authoring remains available:", error);
}

console.log("================================================================================");
console.log("  STORYLET STUDIO — DESIGN & PLAYTEST SERVER                                  ");
console.log(`  Serving on: http://127.0.0.1:${PORT}`);
console.log(`  Authoring Database: ${DATABASE_PATH}`);
console.log(`  Playtest State: ${rustRuntimeReady ? "Rust HTTP service ready; sessions owned by Rust" : "Rust service unavailable"}`);
console.log("================================================================================");

function jsonResponse(data: unknown, status = 200) {
  return new Response(JSON.stringify(data), {
    status,
    headers: {
      "Content-Type": "application/json",
      "Access-Control-Allow-Origin": "*"
    }
  });
}

function currentWorkspaceRecords() {
  return {
    qualities: repo.listQualities(),
    locations: repo.listLocations(),
    characters: repo.listCharacters(),
    storylets: repo.listStorylets(),
    npcBehaviors: repo.listNpcBehaviors(),
    scenarios: repo.listScenarios(),
  };
}

function currentProjectContent(): ProjectBundleContent {
  const records = currentWorkspaceRecords();
  return {
    qualities: records.qualities,
    locations: records.locations,
    characters: records.characters,
    storylets: records.storylets,
    npcBehaviors: records.npcBehaviors,
    scenarios: records.scenarios,
  };
}

function currentProjectBundle() {
  return makeProjectBundle(repo.getProjectMetadata(), currentProjectContent());
}

function buildCurrentWorkspaceProject(startLocationId?: string, scenarioId?: string) {
  const scenario = scenarioId ? repo.getScenario(scenarioId) : undefined;
  const metadata = repo.getProjectMetadata();
  const built = buildWorkspaceProject(currentWorkspaceRecords(), {
    playerStartLocation: startLocationId,
    scenario: scenario ?? undefined,
    projectId: metadata.id,
    projectName: metadata.name,
    projectRevision: metadata.revision,
  });
  if (scenarioId && !scenario) built.issues.unshift(`Unknown scenario '${scenarioId}'`);
  return built;
}

function saveScenarioRecord(body: Partial<ScenarioRecord>, status = 201): Response {
  if (typeof body.id !== "string" || !body.id.trim() || typeof body.name !== "string" || !body.name.trim()
    || typeof body.player_start_location !== "string"
    || !Number.isSafeInteger(body.seed) || (body.seed ?? -1) < 0
    || !body.initial_qualities || Array.isArray(body.initial_qualities) || typeof body.initial_qualities !== "object"
    || !body.actor_locations || Array.isArray(body.actor_locations) || typeof body.actor_locations !== "object"
    || !Array.isArray(body.storylet_ids) || !body.storylet_ids.every((id) => typeof id === "string")) {
    return jsonResponse({ error: "Scenario needs an ID, name, start location, nonnegative integer seed, quality and actor maps, and a storylet ID list." }, 400);
  }

  // Validate start_location_policy if present
  if (body.start_location_policy !== undefined && body.start_location_policy !== null) {
    const slp = body.start_location_policy;
    if (typeof slp !== "object" || Array.isArray(slp)) {
      return jsonResponse({ error: "start_location_policy must be an object" }, 400);
    }
    if (slp.mode !== "fixed" && slp.mode !== "random" && slp.mode !== "by_attribute") {
      return jsonResponse({ error: "start_location_policy.mode must be 'fixed', 'random', or 'by_attribute'" }, 400);
    }
    if (slp.mode === "fixed" && slp.location_id !== undefined && (typeof slp.location_id !== "string" || !slp.location_id.trim())) {
      return jsonResponse({ error: "start_location_policy.location_id must be a non-empty string when provided" }, 400);
    }
    if (slp.mode === "by_attribute" && (typeof slp.attribute_key !== "string" || !slp.attribute_key.trim())) {
      return jsonResponse({ error: "start_location_policy.attribute_key must be a non-empty string when mode is 'by_attribute'" }, 400);
    }
  }

  // Validate actor_population_policy if present
  if (body.actor_population_policy !== undefined && body.actor_population_policy !== null) {
    const app = body.actor_population_policy;
    if (typeof app !== "object" || Array.isArray(app)) {
      return jsonResponse({ error: "actor_population_policy must be an object" }, 400);
    }
    if (app.mode !== "all" && app.mode !== "random" && app.mode !== "by_attribute") {
      return jsonResponse({ error: "actor_population_policy.mode must be 'all', 'random', or 'by_attribute'" }, 400);
    }
    if (app.mode === "random" && (typeof app.count !== "number" || !Number.isInteger(app.count) || app.count < 0)) {
      return jsonResponse({ error: "actor_population_policy.count must be a non-negative integer when mode is 'random'" }, 400);
    }
    if (app.mode === "by_attribute" && (typeof app.attribute_key !== "string" || !app.attribute_key.trim())) {
      return jsonResponse({ error: "actor_population_policy.attribute_key must be a non-empty string when mode is 'by_attribute'" }, 400);
    }
    if (app.location_policy !== undefined && app.location_policy !== null) {
      const alp = app.location_policy;
      if (typeof alp !== "object" || Array.isArray(alp)) {
        return jsonResponse({ error: "actor_population_policy.location_policy must be an object" }, 400);
      }
      if (alp.mode !== "fixed" && alp.mode !== "random" && alp.mode !== "by_attribute") {
        return jsonResponse({ error: "actor_population_policy.location_policy.mode must be 'fixed', 'random', or 'by_attribute'" }, 400);
      }
      if (alp.mode === "fixed" && alp.location_id !== undefined && (typeof alp.location_id !== "string" || !alp.location_id.trim())) {
        return jsonResponse({ error: "actor_population_policy.location_policy.location_id must be a non-empty string when provided" }, 400);
      }
      if (alp.mode === "by_attribute" && (typeof alp.attribute_key !== "string" || !alp.attribute_key.trim())) {
        return jsonResponse({ error: "actor_population_policy.location_policy.attribute_key must be a non-empty string when mode is 'by_attribute'" }, 400);
      }
    }
  }

  if (body.event_definitions !== undefined && (!Array.isArray(body.event_definitions) || body.event_definitions.some((event) =>
    !event || typeof event !== "object" || Array.isArray(event) || typeof event.id !== "string" || !event.id.trim()
    || typeof event.title !== "string" || !event.title.trim() || typeof event.description !== "string"
    || (event.event_type !== "canon" && event.event_type !== "world")
    || !event.trigger || typeof event.trigger !== "object" || Array.isArray(event.trigger)
    || (event.trigger.all_of !== undefined && (!Array.isArray(event.trigger.all_of) || event.trigger.all_of.some((predicate) => !predicate || typeof predicate.variable !== "string" || !predicate.variable.trim() || typeof predicate.op !== "string" || !predicate.op.trim() || !Object.hasOwn(predicate, "value"))))
    || (event.trigger.any_of !== undefined && (!Array.isArray(event.trigger.any_of) || event.trigger.any_of.some((predicate) => !predicate || typeof predicate.variable !== "string" || !predicate.variable.trim() || typeof predicate.op !== "string" || !predicate.op.trim() || !Object.hasOwn(predicate, "value"))))
    || !event.effects || typeof event.effects !== "object" || Array.isArray(event.effects)
    || (event.delay_turns !== undefined && (!Number.isSafeInteger(event.delay_turns) || event.delay_turns < 0))
    || (event.repeatable !== undefined && typeof event.repeatable !== "boolean")))) {
    return jsonResponse({ error: "event_definitions must contain valid canon/world events with typed triggers and effects" }, 400);
  }

  const scenario: ScenarioRecord = {
    id: body.id.trim(), name: body.name.trim(), player_start_location: body.player_start_location,
    initial_qualities: body.initial_qualities,
    actor_locations: body.actor_locations,
    storylet_ids: [...new Set(body.storylet_ids)], seed: body.seed!,
    start_location_policy: body.start_location_policy,
    actor_population_policy: body.actor_population_policy,
    event_definitions: body.event_definitions ?? repo.getScenario(body.id.trim())?.event_definitions ?? [],
  };
  const validation = buildWorkspaceProject(currentWorkspaceRecords(), { scenario });
  if (validation.issues.length) return jsonResponse({ error: "Scenario needs correction before saving", issues: validation.issues }, 422);
  repo.saveScenario(scenario);
  return jsonResponse({ success: true, scenario: repo.getScenario(scenario.id) }, status);
}

const server = Bun.serve({
  port: PORT,
  async fetch(req) {
    const url = new URL(req.url);
    const pathname = url.pathname;

    // Handle CORS preflight
    if (req.method === "OPTIONS") {
      return new Response(null, {
        headers: {
          "Access-Control-Allow-Origin": "*",
          "Access-Control-Allow-Methods": "GET, POST, DELETE, OPTIONS",
          "Access-Control-Allow-Headers": "Content-Type"
        }
      });
    }

    const rustPlaytestResponse = await handlePlaytestApi(req, rustRuntime, (startLocationId, scenarioId) =>
      buildCurrentWorkspaceProject(startLocationId, scenarioId));
    if (rustPlaytestResponse) return rustPlaytestResponse;

    // -------------------------------------------------------------------------
    // API: Authored scenario starting states
    // -------------------------------------------------------------------------
    if (pathname === "/api/v1/scenarios") {
      if (req.method === "GET") return jsonResponse(repo.listScenarios());
      if (req.method === "POST") {
        return saveScenarioRecord(await req.json() as Partial<ScenarioRecord>);
      }
      return jsonResponse({ error: "Method not allowed" }, 405);
    }

    if (pathname === "/api/v1/project") {
      if (req.method === "GET") return jsonResponse(repo.getProjectMetadata());
      if (req.method === "PATCH") {
        let input: unknown;
        try { input = await req.json(); }
        catch { return jsonResponse({ error: "Request body must be valid JSON" }, 400); }
        if (!input || typeof input !== "object" || Array.isArray(input)) return jsonResponse({ error: "Request body must be a JSON object" }, 400);
        const body = input as { name?: unknown };
        if (typeof body.name !== "string" || !body.name.trim()) {
          return jsonResponse({ error: "Project name cannot be empty" }, 400);
        }
        const current = repo.getProjectMetadata();
        const updated = { ...current, name: body.name.trim(), revision: current.revision + 1 };
        repo.saveProjectMetadata(updated);
        return jsonResponse(updated);
      }
      return jsonResponse({ error: "Method not allowed" }, 405);
    }

    if (pathname === "/api/v1/project-bundle") {
      if (req.method === "GET") {
        const bundle = currentProjectBundle();
        const filename = bundle.project.name.toLowerCase().normalize("NFKD").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "storylet-project";
        return new Response(JSON.stringify(bundle, null, 2), {
          headers: {
            "Content-Type": "application/json; charset=utf-8",
            "Content-Disposition": `attachment; filename="${filename}.storylet.json"`,
          },
        });
      }
      return jsonResponse({ error: "Method not allowed" }, 405);
    }

    if (pathname === "/api/v1/project-bundle/validate" || pathname === "/api/v1/project-bundle/import") {
      if (req.method !== "POST") return jsonResponse({ error: "Method not allowed" }, 405);
      let input: unknown;
      try { input = await req.json(); }
      catch { return jsonResponse({ error: "Request body must be valid JSON" }, 400); }
      if (!input || typeof input !== "object" || Array.isArray(input)) return jsonResponse({ error: "Request body must be a JSON object" }, 400);
      const body = input as { bundle?: unknown; confirmReplace?: unknown };
      const inspection = inspectProjectBundle(body.bundle);
      if (!inspection.valid || !inspection.bundle) {
        return jsonResponse({ valid: false, issues: inspection.issues, counts: inspection.counts }, 422);
      }
      if (pathname.endsWith("/validate")) {
        return jsonResponse({ valid: true, project: inspection.bundle.project, counts: inspection.counts });
      }
      if (body.confirmReplace !== true) {
        return jsonResponse({
          error: "Explicit replacement confirmation is required",
          existingProject: repo.getProjectMetadata(),
          incomingProject: inspection.bundle.project,
          incomingCounts: inspection.counts,
        }, 409);
      }
      try {
        // Validate again immediately before the single transactional replacement.
        const finalInspection = inspectProjectBundle(inspection.bundle);
        if (!finalInspection.valid || !finalInspection.bundle) {
          return jsonResponse({ valid: false, issues: finalInspection.issues }, 422);
        }
        repo.replaceProjectContent(finalInspection.bundle.project, finalInspection.bundle.content);
        return jsonResponse({ success: true, project: repo.getProjectMetadata(), counts: finalInspection.counts });
      } catch (error) {
        return jsonResponse({ error: `Project replacement failed: ${error instanceof Error ? error.message : String(error)}` }, 500);
      }
    }

    const scenarioRoute = pathname.match(/^\/api\/v1\/scenarios\/([^/]+)$/);
    if (scenarioRoute) {
      let id: string;
      try { id = decodeURIComponent(scenarioRoute[1]); }
      catch { return jsonResponse({ error: "Invalid scenario ID encoding" }, 400); }
      if (req.method === "DELETE") return jsonResponse({ success: repo.deleteScenario(id), id });
      if (req.method === "PUT") {
        const body = await req.json() as Partial<ScenarioRecord>;
        if (body.id && body.id !== id) return jsonResponse({ error: "Scenario ID cannot change during an update." }, 400);
        return saveScenarioRecord({ ...body, id }, 200);
      }
      if (req.method === "GET") return jsonResponse(repo.getScenario(id));
      return jsonResponse({ error: "Method not allowed" }, 405);
    }

    // -------------------------------------------------------------------------
    // API: System Health & Info
    // -------------------------------------------------------------------------
    if (pathname === "/api/health") {
      return jsonResponse({
        status: "ok",
        system: "Generic Storylet Studio",
        port: PORT,
        playtestRuntime: "Rust, SQLite-backed sessions",
        authoringDatabase: DATABASE_PATH
      });
    }

    // -------------------------------------------------------------------------
    // API: Qualities (The Symbolic Quality Economy)
    // -------------------------------------------------------------------------
    if (pathname === "/api/qualities") {
      if (req.method === "GET") {
        return jsonResponse(repo.listQualities());
      }
      if (req.method === "POST") {
        const body = (await req.json()) as any;
        repo.saveQuality(body);
        return jsonResponse({ success: true, quality: repo.getQuality(body.name) });
      }
    }

    // -------------------------------------------------------------------------
    // API: Characters
    // -------------------------------------------------------------------------
    if (pathname === "/api/characters") {
      if (req.method === "GET") {
        return jsonResponse(repo.listCharacters());
      }
      if (req.method === "POST") {
        const body = (await req.json()) as any;
        repo.saveCharacter(body);
        return jsonResponse({ success: true, character: repo.getCharacter(body.id) });
      }
    }

    if (pathname.startsWith("/api/characters/") && req.method === "DELETE") {
      const id = pathname.replace("/api/characters/", "");
      const deleted = repo.deleteCharacter(id);
      return jsonResponse({ success: deleted, id });
    }

    // -------------------------------------------------------------------------
    // API: Project-authored NPC behavior rules
    // -------------------------------------------------------------------------
    if (pathname === "/api/npc-behaviors") {
      if (req.method === "GET") return jsonResponse(repo.listNpcBehaviors());
      if (req.method === "POST") {
        const body = await req.json() as any;
        if (typeof body.id !== "string" || !body.id.trim()
          || typeof body.actor_id !== "string" || !repo.getCharacter(body.actor_id)
          || typeof body.action !== "string" || !body.action.trim()
          || !Number.isInteger(body.priority)
          || !Array.isArray(body.effects)) {
          return jsonResponse({ error: "Provide a rule ID, existing character, action, integer priority, and effects array." }, 400);
        }
        const rule = {
          id: body.id.trim(),
          actor_id: body.actor_id,
          priority: body.priority,
          action: body.action.trim(),
          conditions: body.conditions ?? {},
          effects: body.effects,
        };
        repo.saveNpcBehavior(rule);
        return jsonResponse({ success: true, behavior: rule });
      }
    }
    if (pathname.startsWith("/api/npc-behaviors/") && req.method === "DELETE") {
      const id = decodeURIComponent(pathname.slice("/api/npc-behaviors/".length));
      return jsonResponse({ success: repo.deleteNpcBehavior(id), id });
    }

    // -------------------------------------------------------------------------
    // API: Locations (SQLite backed)
    // -------------------------------------------------------------------------
    if (pathname === "/api/locations") {
      if (req.method === "GET") {
        const list = repo.listLocations();
        return jsonResponse(list);
      }
      if (req.method === "POST") {
        const body = (await req.json()) as any;
        repo.saveLocation(body);
        return jsonResponse({ success: true, location: repo.getLocation(body.id) });
      }
    }

    if (pathname.startsWith("/api/locations/") && req.method === "DELETE") {
      const id = pathname.replace("/api/locations/", "");
      const deleted = repo.deleteLocation(id);
      return jsonResponse({ success: deleted, id });
    }

    // -------------------------------------------------------------------------
    // API: Storylets (The Content Deck)
    // -------------------------------------------------------------------------
    if (pathname === "/api/storylets") {
      if (req.method === "GET") {
        return jsonResponse(repo.listStorylets());
      }
      if (req.method === "POST") {
        const body = (await req.json()) as any;
        repo.saveStorylet(body);
        return jsonResponse({ success: true, storylet: repo.getStorylet(body.id) });
      }
    }

    if (pathname.startsWith("/api/storylets/") && pathname.endsWith("/approve") && req.method === "POST") {
      const id = decodeURIComponent(pathname.slice("/api/storylets/".length, -"/approve".length));
      const body = await req.json() as { expectedRevision?: number };
      if (!Number.isInteger(body.expectedRevision)) {
        return jsonResponse({ error: "expectedRevision must be an integer" }, 400);
      }
      const draft = repo.getStorylet(id);
      if (!draft) return jsonResponse({ error: "Storylet was not found" }, 404);
      if (draft.revision !== body.expectedRevision) {
        return jsonResponse({ error: "Storylet revision changed. Reload the draft before approving." }, 409);
      }
      const validation = buildWorkspaceProject({
        qualities: repo.listQualities(),
        locations: repo.listLocations(),
        characters: repo.listCharacters(),
        storylets: [{ ...draft, approved_revision: draft.revision }],
      }, repo.listLocations()[0]?.id);
      const blockingIssues = validation.issues.filter((issue) => issue.startsWith(`storylets.${id}.`));
      if (blockingIssues.length) {
        return jsonResponse({ error: "Storylet needs correction before approval", issues: blockingIssues }, 422);
      }
      const approved = repo.approveStorylet(id, body.expectedRevision!);
      if (!approved) {
        return jsonResponse({ error: "Storylet was not found or its revision changed. Reload the draft before approving." }, 409);
      }
      return jsonResponse({ success: true, storylet: repo.getStorylet(id) });
    }

    if (pathname.startsWith("/api/storylets/") && req.method === "DELETE") {
      const id = pathname.replace("/api/storylets/", "");
      const deleted = repo.deleteStorylet(id);
      return jsonResponse({ success: deleted, id });
    }

    if (pathname === "/api/history" && req.method === "GET") {
      return jsonResponse(repo.getCallHistory(50));
    }

    // -------------------------------------------------------------------------
    // Static file serving
    // -------------------------------------------------------------------------
    let filePath = pathname === "/" ? "/index.html" : pathname;
    const absPath = path.join(PUBLIC_DIR, filePath);

    if (fs.existsSync(absPath) && fs.statSync(absPath).isFile()) {
      const file = Bun.file(absPath);
      let contentType = "application/octet-stream";
      if (absPath.endsWith(".html")) contentType = "text/html; charset=utf-8";
      else if (absPath.endsWith(".js")) contentType = "application/javascript; charset=utf-8";
      else if (absPath.endsWith(".css")) contentType = "text/css; charset=utf-8";
      else if (absPath.endsWith(".png")) contentType = "image/png";
      else if (absPath.endsWith(".json")) contentType = "application/json";

      return new Response(file, {
        headers: { "Content-Type": contentType }
      });
    }

    return new Response("Not Found", { status: 404 });
  }
});

export default server;

for (const signal of ["SIGINT", "SIGTERM"] as const) {
  process.once(signal, async () => {
    await rustRuntime.close();
    repo.close();
    server.stop(true);
    process.exit(0);
  });
}
