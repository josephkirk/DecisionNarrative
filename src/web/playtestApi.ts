import { RustRuntimeClient, type RustWorldCommand } from "../engine/RustRuntimeClient.ts";
import { rankPlaytestWithLocalEncoder } from "./semanticDirector.ts";
import type { WorkspaceProjectResult } from "./workspaceProject.ts";

function response(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json; charset=utf-8" },
  });
}

function statusForError(message: string): number {
  if (/embedding service|fetch failed|connection refused|ECONNREFUSED/i.test(message)) return 503;
  if (/revision conflict|already exists|idempotency key was already used/i.test(message)) return 409;
  if (/was not found/i.test(message)) return 404;
  if (/project validation failed|not part of this project|not eligible|unavailable|insufficient|unknown (actor|quality|location)/i.test(message)) return 422;
  return 400;
}

const sessionLocks = new Map<string, Promise<void>>();

async function withSessionLock<T>(sessionId: string, operation: () => Promise<T>): Promise<T> {
  const previous = sessionLocks.get(sessionId) ?? Promise.resolve();
  let release!: () => void;
  const gate = new Promise<void>((resolve) => { release = resolve; });
  const queued = previous.then(() => gate);
  sessionLocks.set(sessionId, queued);
  await previous;
  try {
    return await operation();
  } finally {
    release();
    if (sessionLocks.get(sessionId) === queued) sessionLocks.delete(sessionId);
  }
}

/** HTTP adapter for workspace composition; the Rust service owns all session state and persistence. */
export async function handlePlaytestApi(
  request: Request,
  runtime: RustRuntimeClient,
  workspaceProject?: (startLocationId?: string, scenarioId?: string) => WorkspaceProjectResult,
  options: { embeddingServiceUrl?: string } = {},
): Promise<Response | null> {
  const url = new URL(request.url);
  if (url.pathname === "/api/v1/playtests/from-workspace") {
    if (request.method !== "POST") return response({ error: "Method not allowed" }, 405);
    if (!workspaceProject) return response({ error: "Workspace authoring data is unavailable" }, 503);
    try {
      const body = await request.json() as { startLocationId?: string; scenarioId?: string };
      const built = workspaceProject(body.startLocationId, body.scenarioId);
      if (built.issues.length) return response({ error: "Workspace content needs attention before playtesting", issues: built.issues }, 422);
      const sessionId = crypto.randomUUID();
      const snapshot = await runtime.createSession(sessionId, built.project, built.scenarioId);
      return response({ sessionId, snapshot }, 201);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      return response({ error: message }, statusForError(message));
    }
  }

  if (url.pathname === "/api/v1/playtests") {
    if (request.method === "GET") {
      try { return response(await runtime.listSessions()); }
      catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        return response({ error: message }, statusForError(message));
      }
    }
    if (request.method !== "POST") return response({ error: "Method not allowed" }, 405);
    try {
      const body = await request.json() as { sessionId?: string; scenarioId?: string; project?: Parameters<RustRuntimeClient["createSession"]>[1] };
      if (!body.project || typeof body.scenarioId !== "string") return response({ error: "project and scenarioId are required" }, 400);
      const sessionId = body.sessionId?.trim() || crypto.randomUUID();
      const snapshot = await runtime.createSession(sessionId, body.project, body.scenarioId);
      return response({ sessionId, snapshot }, 201);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      return response({ error: message }, statusForError(message));
    }
  }

  const forkMatch = url.pathname.match(/^\/api\/v1\/playtests\/([^/]+)\/fork$/);
  if (forkMatch) {
    if (request.method !== "POST") return response({ error: "Method not allowed" }, 405);
    try {
      const sourceSessionId = decodeURIComponent(forkMatch[1]);
      const body = await request.json() as { branchSessionId?: string; eventId?: string };
      if (!body.eventId?.trim()) return response({ error: "eventId is required" }, 400);
      const branchSessionId = body.branchSessionId?.trim() || crypto.randomUUID();
      return await withSessionLock(sourceSessionId, async () => {
        const snapshot = await runtime.forkSession(sourceSessionId, branchSessionId, body.eventId!);
        return response({ sessionId: branchSessionId, snapshot }, 201);
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      return response({ error: message }, statusForError(message));
    }
  }

  const rankMatch = url.pathname.match(/^\/api\/v1\/playtests\/([^/]+)\/director\/rank$/);
  if (rankMatch) {
    if (request.method !== "POST") return response({ error: "Method not allowed" }, 405);
    try {
      const sessionId = decodeURIComponent(rankMatch[1]);
      return await withSessionLock(sessionId, async () => response(await rankPlaytestWithLocalEncoder(
        runtime,
        sessionId,
        options.embeddingServiceUrl ?? Bun.env.STORYLET_EMBEDDING_URL ?? "http://127.0.0.1:4318",
      )));
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      return response({ error: message }, statusForError(message));
    }
  }

  const streamMatch = url.pathname.match(/^\/api\/v1\/playtests\/([^/]+)\/stream$/);
  if (streamMatch) {
    if (request.method !== "GET") return response({ error: "Method not allowed" }, 405);
    try {
      return await runtime.sessionStream(decodeURIComponent(streamMatch[1]), request.signal);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      return response({ error: message }, statusForError(message));
    }
  }

  const match = url.pathname.match(/^\/api\/v1\/playtests\/([^/]+)(?:\/(director|events|commands|choices))?$/);
  if (!match) return null;
  let sessionId: string;
  try { sessionId = decodeURIComponent(match[1]); }
  catch { return response({ error: "Invalid session ID encoding" }, 400); }
  const view = match[2];

  try {
    if (!view && request.method === "DELETE") {
      return await withSessionLock(sessionId, async () => response(await runtime.deleteSession(sessionId)));
    }
    if (view === "commands") {
      if (request.method !== "POST") return response({ error: "Method not allowed" }, 405);
      const body = await request.json() as { command?: RustWorldCommand };
      if (!body.command || typeof body.command.idempotency_key !== "string") return response({ error: "command with an idempotency_key is required" }, 400);
      return await withSessionLock(sessionId, async () => response(await runtime.submitCommand(sessionId, body.command!)));
    }
    if (view === "choices") {
      if (request.method !== "POST") return response({ error: "Method not allowed" }, 405);
      const body = await request.json() as { storyletId?: string; choiceId?: string; idempotencyKey?: string; expectedRevision?: number };
      if (!body.storyletId || !body.choiceId || !Number.isInteger(body.expectedRevision)) return response({ error: "storyletId, choiceId, and expectedRevision are required" }, 400);
      return await withSessionLock(sessionId, async () => response(await runtime.executeChoice(
        sessionId, body.storyletId!, body.choiceId!, body.idempotencyKey ?? crypto.randomUUID(), body.expectedRevision!,
      )));
    }
    if (request.method !== "GET") return response({ error: "Method not allowed" }, 405);
    if (!view) return await withSessionLock(sessionId, async () => response(await runtime.snapshot(sessionId)));
    if (view === "director") {
      const targetSlope = Number(url.searchParams.get("targetTensionSlope") ?? "0.15");
      if (!Number.isFinite(targetSlope)) return response({ error: "targetTensionSlope must be finite" }, 400);
      return await withSessionLock(sessionId, async () => response(await runtime.evaluate(sessionId, targetSlope)));
    }
    const snapshot = await withSessionLock(sessionId, () => runtime.snapshot(sessionId));
    return response({ sessionId, turn: snapshot.world.turn, events: snapshot.world.events });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return response({ error: message }, statusForError(message));
  }
}
