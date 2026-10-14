/**
 * Storylet Studio — Designer Web Platform & Game Server
 * Exposes full REST API for:
 * - Entity Management (Characters, Locations, Qualities in SQLite)
 * - Storylet Workbench & staged Strata-backed generation pipeline
 * - Generic Rust director, model-ranking adapter, and causal playtest trace
 * - Butterfly Effect Simulation (Autonomous NPC Ticks, Observer Ripples, Rollback)
 */

import * as path from "node:path";
import * as fs from "node:fs";
import { StoryletRepository, type ScenarioRecord } from "../db/repository.ts";
import { UnslopStoryletPipeline, StoryletGenerationError, type StoryletGenerationRequest } from "../agents/UnslopStoryletPipeline.ts";
import { StoryCraftResourceRegistry } from "../agents/StoryCraftResources.ts";
import { GenerationWorkflow, GenerationWorkflowError, type GenerationJob } from "../agents/GenerationWorkflow.ts";
import { RustRuntimeClient } from "../engine/RustRuntimeClient.ts";
import { handlePlaytestApi } from "./playtestApi.ts";
import { buildWorkspaceProject } from "./workspaceProject.ts";
import { inspectProjectBundle, makeProjectBundle, type ProjectBundleContent } from "./projectBundle.ts";

const PORT = Number(Bun.env.STORYLET_WEB_PORT ?? 3000);
const PUBLIC_DIR = path.resolve("public");
const DATABASE_PATH = Bun.env.STORYLET_DB_PATH ?? "data/storylet_studio.db";

// Initialize Repositories and Engines
const repo = new StoryletRepository(DATABASE_PATH);
const unslopPipeline = new UnslopStoryletPipeline();
const storyCraftResources = new StoryCraftResourceRegistry();
const rustRuntime = new RustRuntimeClient({
  address: Bun.env.STORYLET_ENGINE_ADDR,
  databasePath: Bun.env.STORYLET_ENGINE_DB_PATH ?? "data/storylet_engine.db",
});
const generationWorkflow = new GenerationWorkflow({
  get: (id) => repo.getGenerationJob<GenerationJob>(id),
  list: () => repo.listGenerationJobs<GenerationJob>(),
  put: (job) => repo.saveGenerationJob(job),
  delete: (id) => repo.deleteGenerationJob(id),
}, (stage, request, previous) => unslopPipeline.generateWorkflowStage(stage, request, previous));
generationWorkflow.recoverInterruptedJobs();

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
console.log(`  Strata Endpoint: ${Bun.env.STRATA_BASE_URL ?? "not configured"}`);
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
    generationJobs: repo.listGenerationJobs<GenerationJob>(),
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
    || typeof body.player_start_location !== "string" || !body.player_start_location.trim()
    || !Number.isSafeInteger(body.seed) || (body.seed ?? -1) < 0
    || !body.initial_qualities || Array.isArray(body.initial_qualities) || typeof body.initial_qualities !== "object"
    || !body.actor_locations || Array.isArray(body.actor_locations) || typeof body.actor_locations !== "object"
    || !Array.isArray(body.storylet_ids) || !body.storylet_ids.every((id) => typeof id === "string")) {
    return jsonResponse({ error: "Scenario needs an ID, name, start location, nonnegative integer seed, quality and actor maps, and a storylet ID list." }, 400);
  }
  const scenario: ScenarioRecord = {
    id: body.id.trim(), name: body.name.trim(), player_start_location: body.player_start_location,
    initial_qualities: body.initial_qualities,
    actor_locations: body.actor_locations,
    storylet_ids: [...new Set(body.storylet_ids)], seed: body.seed!,
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
        strataEndpoint: Bun.env.STRATA_BASE_URL ?? null,
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
      }, draft.location || repo.listLocations()[0]?.id);
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

    // -------------------------------------------------------------------------
    // API: Resumable nine-stage storylet generation jobs
    // -------------------------------------------------------------------------
    if (pathname === "/api/v1/generation-jobs") {
      if (req.method === "GET") return jsonResponse(repo.listGenerationJobs());
      if (req.method === "POST") {
        try {
          const body = await req.json() as StoryletGenerationRequest;
          const request: StoryletGenerationRequest = {
            ...body,
            projectContext: {
              locations: repo.listLocations().map(({ id, name, description }) => ({ id, name, description })),
              characters: repo.listCharacters().map(({ id, name, role, agenda, red_line, schedule }) => ({ id, name, role, agenda, red_line, schedule })),
              qualities: repo.listQualities().map(({ name, type, description }) => ({ name, type, description })),
            },
          };
          return jsonResponse({ job: generationWorkflow.create(request) }, 201);
        } catch (error) {
          const status = error instanceof GenerationWorkflowError ? error.statusCode : 400;
          return jsonResponse({ error: error instanceof Error ? error.message : String(error) }, status);
        }
      }
    }

    if (pathname === "/api/v1/craft-resources" && req.method === "GET") {
      try {
        return jsonResponse({ resources: await storyCraftResources.catalog() });
      } catch (error) {
        return jsonResponse({ error: error instanceof Error ? error.message : String(error) }, 503);
      }
    }

    const generationJobMatch = pathname.match(/^\/api\/v1\/generation-jobs\/([^/]+)(?:\/(advance|events|draft|stages\/([^/]+)))?$/);
    if (generationJobMatch) {
      let jobId: string;
      try { jobId = decodeURIComponent(generationJobMatch[1]); }
      catch { return jsonResponse({ error: "Invalid generation job ID encoding" }, 400); }
      const action = generationJobMatch[2];
      try {
        if (!action && req.method === "GET") return jsonResponse({ job: generationWorkflow.get(jobId) });
        if (!action && req.method === "DELETE") {
          const job = generationWorkflow.get(jobId);
          if (job.status === "running") return jsonResponse({ error: "Wait for the active stage before deleting this job." }, 409);
          return jsonResponse({ deleted: generationWorkflow.delete(jobId) });
        }
        if (action === "advance" && req.method === "POST") {
          const body = await req.json() as { expectedRevision?: number };
          const job = await generationWorkflow.advance(jobId, body.expectedRevision as number);
          return jsonResponse({ job });
        }
        if (action === "events" && req.method === "GET") {
          let unsubscribe: (() => void) | undefined;
          let controllerRef: ReadableStreamDefaultController<Uint8Array> | undefined;
          const stream = new ReadableStream<Uint8Array>({
            start(controller) {
              controllerRef = controller;
              const encoder = new TextEncoder();
              const send = (job: GenerationJob) => controller.enqueue(encoder.encode(`event: generation\ndata: ${JSON.stringify(job)}\n\n`));
              send(generationWorkflow.get(jobId));
              unsubscribe = generationWorkflow.subscribe(jobId, send);
              req.signal.addEventListener("abort", () => {
                unsubscribe?.();
                try { controller.close(); } catch { /* stream already closed */ }
              }, { once: true });
            },
            cancel() { unsubscribe?.(); },
          });
          return new Response(stream, { headers: { "Content-Type": "text/event-stream", "Cache-Control": "no-cache", "Connection": "keep-alive" } });
        }
        if (action === "draft" && req.method === "POST") {
          const job = generationWorkflow.get(jobId);
          if (job.draftStoryletId) {
            const existing = repo.getStorylet(job.draftStoryletId);
            if (existing) return jsonResponse({ success: true, storylet: existing, alreadySaved: true }, 200);
          }
          const { storylet, audit } = unslopPipeline.storyletFromJob(job);
          repo.saveStorylet(storylet);
          const saved = repo.getStorylet(storylet.id);
          if (!saved || saved.revision == null) throw new Error("Storylet draft was not persisted with a revision.");
          const updatedJob = generationWorkflow.markDraftSaved(jobId, storylet.id, saved.revision);
          return jsonResponse({ success: true, storylet: saved, audit, job: updatedJob }, 201);
        }
        if (action?.startsWith("stages/") && req.method === "PUT") {
          const stage = decodeURIComponent(generationJobMatch[3]) as Parameters<typeof generationWorkflow.editStage>[1];
          const body = await req.json() as { expectedRevision?: number; artifact?: unknown };
          return jsonResponse({ job: generationWorkflow.editStage(jobId, stage, body.artifact, body.expectedRevision as number) });
        }
        return jsonResponse({ error: "Method not allowed" }, 405);
      } catch (error) {
        const status = error instanceof GenerationWorkflowError ? error.statusCode
          : error instanceof StoryletGenerationError ? error.statusCode : 400;
        return jsonResponse({ error: error instanceof Error ? error.message : String(error) }, status);
      }
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
