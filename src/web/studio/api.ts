import type {
  AuthoringOptions,
  ContentReference,
  LibraryPageResult,
  ProjectMetadata,
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
