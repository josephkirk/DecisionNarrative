import type {
  AuthoringOptions,
  ContentReference,
  CoverageReport,
  InspectView,
  LibraryPageResult,
  NearMiss,
  ProjectMetadata,
  RunAction,
  RunEnvelope,
  RunReloadResult,
  RunStartInput,
  RunSummary,
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

/** The slice of the runs client the play view-model depends on (kept small so tests can fake it). */
export interface RunsApi {
  start(input: RunStartInput): Promise<RunEnvelope>;
  view(id: string, participant: number): Promise<RunEnvelope>;
  act(id: string, participant: number, action: RunAction, expectedRevision: number, idempotencyKey: string): Promise<RunEnvelope>;
  rewind(id: string, toStep: number, expectedRevision: number, idempotencyKey: string): Promise<RunEnvelope>;
  whyNot(id: string, participant: number): Promise<{ nearMisses: NearMiss[] }>;
  inspect(id: string, participant: number): Promise<InspectView>;
  reload(id: string, expectedRevision: number, idempotencyKey: string): Promise<RunReloadResult>;
}

function post<T>(path: string, body: unknown): Promise<T> {
  return request<T>(path, { method: "POST", body: JSON.stringify(body) });
}
const runPath = (id: string) => `/api/v4/runs/${encodeURIComponent(id)}`;

const runs: RunsApi & {
  coverage(runs: number, seed?: string): Promise<CoverageReport>;
  list(): Promise<RunSummary[]>;
  remove(id: string, expectedRevision: number, idempotencyKey: string): Promise<{ ok: boolean }>;
} = {
  async start(input) {
    const expectedRevision = input.expectedRevision ?? (await request<ProjectMetadata>("/api/v3/project")).revision;
    return post<RunEnvelope>("/api/v4/runs", { ...input, expectedRevision, idempotencyKey: crypto.randomUUID() });
  },
  coverage: (runs, seed) => post<CoverageReport>("/api/v4/coverage", { runs, seed }),
  list: () => request<RunSummary[]>("/api/v4/runs"),
  view: (id, participant) => request<RunEnvelope>(`${runPath(id)}?participant=${participant}`),
  act: (id, participant, action, expectedRevision, idempotencyKey) =>
    post<RunEnvelope>(`${runPath(id)}/act`, { participant, action, expectedRevision, idempotencyKey }),
  rewind: (id, toStep, expectedRevision, idempotencyKey) =>
    post<RunEnvelope>(`${runPath(id)}/rewind`, { toStep, expectedRevision, idempotencyKey }),
  whyNot: (id, participant) => request<{ nearMisses: NearMiss[] }>(`${runPath(id)}/why-not?participant=${participant}`),
  inspect: (id, participant) => request<InspectView>(`${runPath(id)}/inspect?participant=${participant}`),
  reload: (id, expectedRevision, idempotencyKey) =>
    post<RunReloadResult>(`${runPath(id)}/reload`, { expectedRevision, idempotencyKey }),
  remove: (id, expectedRevision, idempotencyKey) =>
    request<{ ok: boolean }>(runPath(id), { method: "DELETE", body: JSON.stringify({ expectedRevision, idempotencyKey }) }),
};

export const StudioApi = {
  runs,

  // Authoring options
  getAuthoringOptions(): Promise<AuthoringOptions> {
    return request<AuthoringOptions>("/api/v3/authoring-options");
  },

  // Project
  getProject(): Promise<ProjectMetadata> {
    return request<ProjectMetadata>("/api/v3/project");
  },
  patchProjectName(name: string): Promise<ProjectMetadata> {
    return request<ProjectMetadata>("/api/v3/project", {
      method: "PATCH",
      body: JSON.stringify({ name }),
    });
  },
  validateBundle(bundle: unknown): Promise<{ valid: boolean; issues: string[] }> {
    return request<{ valid: boolean; issues: string[] }>("/api/v3/project-bundle/validate", {
      method: "POST",
      body: JSON.stringify({ bundle }),
    });
  },
  importBundle(bundle: unknown): Promise<{ success: boolean; message: string }> {
    return request<{ success: boolean; message: string }>("/api/v3/project-bundle/import", {
      method: "POST",
      body: JSON.stringify({ bundle, confirmReplace: true }),
    });
  },

  publishInteractionContext(context: StudioInteractionContext): Promise<{ ok: boolean }> {
    return request<{ ok: boolean }>("/api/v3/context", {
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
    return request<LibraryPageResult>(`/api/v3/library?${q.toString()}`);
  },

  // Content CRUD
  listContent<T>(entity: string): Promise<T[]> {
    return request<T[]>(`/api/v3/content/${encodeURIComponent(entity)}`);
  },
  getContent<T>(entity: string, id: string): Promise<T> {
    return request<T>(`/api/v3/content/${encodeURIComponent(entity)}/${encodeURIComponent(id)}`);
  },
  saveContent<T>(entity: string, payload: unknown): Promise<T> {
    return request<T>(`/api/v3/content/${encodeURIComponent(entity)}`, {
      method: "POST",
      body: JSON.stringify(payload),
    });
  },
  updateContent<T>(entity: string, id: string, payload: unknown): Promise<T> {
    return request<T>(`/api/v3/content/${encodeURIComponent(entity)}/${encodeURIComponent(id)}`, {
      method: "PUT",
      body: JSON.stringify(payload),
    });
  },
  deleteContent(entity: string, id: string): Promise<{ deleted: boolean }> {
    return request<{ deleted: boolean }>(`/api/v3/content/${encodeURIComponent(entity)}/${encodeURIComponent(id)}`, {
      method: "DELETE",
    });
  },
  moveContent<T>(kind: string, id: string, folderId: string | null): Promise<T> {
    return request<T>(
      `/api/v3/content/${encodeURIComponent(kind)}/${encodeURIComponent(id)}/move`,
      { method: "POST", body: JSON.stringify({ folder_id: folderId }) },
    );
  },
  getReferences(kind: string, id: string): Promise<{ references: ContentReference[] }> {
    return request<{ references: ContentReference[] }>(
      `/api/v3/content/${encodeURIComponent(kind)}/${encodeURIComponent(id)}/references`,
    );
  },
  async uploadThumbnail(file: File): Promise<{ path: string }> {
    const resp = await fetch(`/api/v3/files?name=${encodeURIComponent(file.name)}`, {
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
    return request<StoryletRecord>(`/api/v3/storylets/${encodeURIComponent(id)}/approve`, {
      method: "POST",
      body: JSON.stringify({ expectedRevision }),
    });
  },
};
