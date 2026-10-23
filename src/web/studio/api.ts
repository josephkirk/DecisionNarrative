import type {
  AuthoringOptions,
  CharacterRecord,
  ChronoPlaytestSessionSummary,
  ChronoPlaytestSnapshot,
  ContentReference,
  DirectorEvaluation,
  EventRecord,
  LibraryPageResult,
  LocationRecord,
  NpcBehaviorRecord,
  PlaytestSnapshot,
  ProjectMetadata,
  QualityRecord,
  ResolvedScenarioStart,
  ScenarioRecord,
  StoryletRecord,
} from "./contracts.ts";
import type { StudioInteractionContext } from "./interactionContext.ts";

export class ApiError extends Error {
  constructor(message: string, public readonly status: number, public readonly issues?: string[]) {
    super(message);
    this.name = "ApiError";
  }
}

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const resp = await fetch(path, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...options.headers,
    },
  });

  if (!resp.ok) {
    let errBody: { error?: string; message?: string; issues?: string[] } = {};
    try {
      errBody = await resp.json();
    } catch {
      errBody = { error: resp.statusText };
    }
    const msg = errBody.error || errBody.message || `Request failed with status ${resp.status}`;
    throw new ApiError(msg, resp.status, errBody.issues);
  }

  return (await resp.json()) as T;
}

export const StudioApi = {
  // Authoring options
  getAuthoringOptions(): Promise<AuthoringOptions> {
    return request<AuthoringOptions>("/api/v2/authoring-options");
  },

  // Project
  getProject(): Promise<ProjectMetadata> {
    return request<ProjectMetadata>("/api/v2/project");
  },
  patchProjectName(name: string): Promise<ProjectMetadata> {
    return request<ProjectMetadata>("/api/v2/project", {
      method: "PATCH",
      body: JSON.stringify({ name }),
    });
  },
  validateBundle(bundle: unknown): Promise<{ valid: boolean; issues: string[] }> {
    return request<{ valid: boolean; issues: string[] }>("/api/v2/project-bundle/validate", {
      method: "POST",
      body: JSON.stringify({ bundle }),
    });
  },
  importBundle(bundle: unknown): Promise<{ success: boolean; message: string }> {
    return request<{ success: boolean; message: string }>("/api/v2/project-bundle/import", {
      method: "POST",
      body: JSON.stringify({ bundle, confirmReplace: true }),
    });
  },

  publishInteractionContext(context: StudioInteractionContext): Promise<{ ok: boolean }> {
    return request<{ ok: boolean }>("/api/v2/context", {
      method: "POST",
      body: JSON.stringify(context),
    });
  },
  // Library
  queryLibrary(params: Record<string, string | number>): Promise<LibraryPageResult> {
    const q = new URLSearchParams();
    for (const [k, v] of Object.entries(params)) {
      if (v !== undefined && v !== null && v !== "") {
        q.set(k, String(v));
      }
    }
    return request<LibraryPageResult>(`/api/v2/library?${q.toString()}`);
  },

  // Content CRUD
  listContent<T>(entity: string): Promise<T[]> {
    return request<T[]>(`/api/v2/content/${encodeURIComponent(entity)}`);
  },
  getContent<T>(entity: string, id: string): Promise<T> {
    return request<T>(`/api/v2/content/${encodeURIComponent(entity)}/${encodeURIComponent(id)}`);
  },
  saveContent<T>(entity: string, payload: unknown): Promise<T> {
    return request<T>(`/api/v2/content/${encodeURIComponent(entity)}`, {
      method: "POST",
      body: JSON.stringify(payload),
    });
  },
  updateContent<T>(entity: string, id: string, payload: unknown): Promise<T> {
    return request<T>(`/api/v2/content/${encodeURIComponent(entity)}/${encodeURIComponent(id)}`, {
      method: "PUT",
      body: JSON.stringify(payload),
    });
  },
  deleteContent(entity: string, id: string): Promise<{ deleted: boolean }> {
    return request<{ deleted: boolean }>(`/api/v2/content/${encodeURIComponent(entity)}/${encodeURIComponent(id)}`, {
      method: "DELETE",
    });
  },
  moveContent<T>(kind: string, id: string, folderId: string | null): Promise<T> {
    return request<T>(
      `/api/v2/content/${encodeURIComponent(kind)}/${encodeURIComponent(id)}/move`,
      { method: "POST", body: JSON.stringify({ folder_id: folderId }) },
    );
  },
  getReferences(kind: string, id: string): Promise<{ references: ContentReference[] }> {
    return request<{ references: ContentReference[] }>(
      `/api/v2/content/${encodeURIComponent(kind)}/${encodeURIComponent(id)}/references`,
    );
  },
  async uploadThumbnail(file: File): Promise<{ path: string }> {
    const resp = await fetch(`/api/v2/files?name=${encodeURIComponent(file.name)}`, {
      method: "POST",
      headers: { "Content-Type": "application/octet-stream" },
      body: file,
    });
    if (!resp.ok) {
      let message = `Upload failed with status ${resp.status}`;
      try {
        const body = (await resp.json()) as { error?: string };
        if (body.error) message = body.error;
      } catch {
        message = resp.statusText || message;
      }
      throw new ApiError(message, resp.status);
    }
    return (await resp.json()) as { path: string };
  },
  approveStorylet(id: string, expectedRevision: number): Promise<StoryletRecord> {
    return request<StoryletRecord>(`/api/v2/storylets/${encodeURIComponent(id)}/approve`, {
      method: "POST",
      body: JSON.stringify({ expectedRevision }),
    });
  },

  // Scenarios
  listScenarios(): Promise<ScenarioRecord[]> {
    return request<ScenarioRecord[]>("/api/v2/scenarios");
  },
  previewScenario(payload: unknown): Promise<ResolvedScenarioStart> {
    return request<ResolvedScenarioStart>("/api/v2/scenarios/preview", {
      method: "POST",
      body: JSON.stringify(payload),
    });
  },

  // Playtests
  createPlaytestFromWorkspace(startLocationId?: string, scenarioId?: string): Promise<{ sessionId: string; snapshot: PlaytestSnapshot }> {
    return request<{ sessionId: string; snapshot: PlaytestSnapshot }>("/api/v1/playtests/from-workspace", {
      method: "POST",
      body: JSON.stringify({ startLocationId, scenarioId }),
    });
  },
  listPlaytestSessions(): Promise<Array<{ session_id: string; turn: number; revision: number; current_location?: string }>> {
    return request("/api/v1/playtests");
  },
  getPlaytestSnapshot(id: string): Promise<PlaytestSnapshot> {
    return request<PlaytestSnapshot>(`/api/v1/playtests/${encodeURIComponent(id)}`);
  },
  deletePlaytest(id: string): Promise<{ deleted: boolean }> {
    return request<{ deleted: boolean }>(`/api/v1/playtests/${encodeURIComponent(id)}`, {
      method: "DELETE",
    });
  },
  submitPlaytestCommand(sessionId: string, command: unknown): Promise<unknown> {
    return request(`/api/v1/playtests/${encodeURIComponent(sessionId)}/commands`, {
      method: "POST",
      body: JSON.stringify({ command }),
    });
  },
  submitPlaytestChoice(sessionId: string, storyletId: string, choiceId: string, expectedRevision: number, idempotencyKey: string): Promise<unknown> {
    return request(`/api/v1/playtests/${encodeURIComponent(sessionId)}/choices`, {
      method: "POST",
      body: JSON.stringify({ storyletId, choiceId, expectedRevision, idempotencyKey }),
    });
  },
  evaluateDirector(sessionId: string, targetTensionSlope = 0.15): Promise<DirectorEvaluation> {
    return request<DirectorEvaluation>(`/api/v1/playtests/${encodeURIComponent(sessionId)}/director?targetTensionSlope=${targetTensionSlope}`);
  },
  forkPlaytest(sourceSessionId: string, eventId: string, branchSessionId?: string): Promise<{ sessionId: string; snapshot: PlaytestSnapshot }> {
    return request(`/api/v1/playtests/${encodeURIComponent(sourceSessionId)}/fork`, {
      method: "POST",
      body: JSON.stringify({ eventId, branchSessionId }),
    });
  },
  getPlaytestEvents(sessionId: string): Promise<{ events: unknown[] }> {
    return request(`/api/v1/playtests/${encodeURIComponent(sessionId)}/events`);
  },
  listChronoPlaytests(): Promise<ChronoPlaytestSessionSummary[]> {
    return request<{ sessions: ChronoPlaytestSessionSummary[] }>("/api/v2/playtests").then((result) => result.sessions);
  },
  createChronoPlaytest(payload: { sessionId: string; scenarioId?: string; startLocationId?: string; storyletId?: string; seed?: number }): Promise<ChronoPlaytestSnapshot> {
    return request<ChronoPlaytestSnapshot>("/api/v2/playtests", {
      method: "POST",
      body: JSON.stringify(payload),
    });
  },
  getChronoPlaytest(sessionId: string): Promise<ChronoPlaytestSnapshot> {
    return request<ChronoPlaytestSnapshot>(`/api/v2/playtests/${encodeURIComponent(sessionId)}`);
  },
  tickChronoPlaytest(sessionId: string, frameBudgetMs: number): Promise<ChronoPlaytestSnapshot> {
    return request<ChronoPlaytestSnapshot>(`/api/v2/playtests/${encodeURIComponent(sessionId)}/tick`, {
      method: "POST",
      body: JSON.stringify({ frameBudgetMs }),
    });
  },
  signalChronoPlaytest(sessionId: string, eventName: string): Promise<ChronoPlaytestSnapshot> {
    return request<ChronoPlaytestSnapshot>(`/api/v2/playtests/${encodeURIComponent(sessionId)}/signal`, {
      method: "POST",
      body: JSON.stringify({ eventName }),
    });
  },
  divertChronoPlaytest(sessionId: string, nodeGuid: number, edgeId: string): Promise<ChronoPlaytestSnapshot> {
    return request<ChronoPlaytestSnapshot>(`/api/v2/playtests/${encodeURIComponent(sessionId)}/divert`, {
      method: "POST",
      body: JSON.stringify({ nodeGuid, edgeId }),
    });
  },
  chooseChronoPlaytest(sessionId: string, payload: { nodeGuid: number; outputPin?: string; storyletId?: string }): Promise<ChronoPlaytestSnapshot> {
    return request<ChronoPlaytestSnapshot>(`/api/v2/playtests/${encodeURIComponent(sessionId)}/choose`, {
      method: "POST",
      body: JSON.stringify(payload),
    });
  },
  invokeChronoPlaytest(sessionId: string, payload: { nodeGuid: number; declaredRoutes: Record<string, string> }): Promise<ChronoPlaytestSnapshot> {
    return request<ChronoPlaytestSnapshot>(`/api/v2/playtests/${encodeURIComponent(sessionId)}/invoke`, {
      method: "POST",
      body: JSON.stringify(payload),
    });
  },
  reconcileChronoPlaytest(sessionId: string, history: unknown[]): Promise<ChronoPlaytestSnapshot> {
    return request<ChronoPlaytestSnapshot>(`/api/v2/playtests/${encodeURIComponent(sessionId)}/reconcile`, {
      method: "POST",
      body: JSON.stringify({ history }),
    });
  },
};
