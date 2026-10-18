import type {
  AuthoringOptions,
  CharacterRecord,
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
};
