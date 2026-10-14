import path from "node:path";

export interface RustProjectBundle {
  schema_version: number;
  id: string;
  name: string;
  revision: number;
  qualities?: Record<string, unknown>;
  locations?: Record<string, unknown>;
  characters?: Record<string, unknown>;
  events?: Record<string, unknown>;
  storylets?: Record<string, unknown>;
  npc_behaviors?: Record<string, unknown>;
  scenarios?: Record<string, unknown>;
}

export interface RustWorldCommand {
  idempotency_key: string;
  expected_revision: number;
  actor_id: string;
  action: string;
  effects: Array<Record<string, unknown>>;
  caused_by?: string[];
  advance_turn?: boolean;
}

export interface RustSessionSnapshot {
  session_id: string;
  project_id: string;
  world: {
    revision: number;
    turn: number;
    current_location?: string | null;
    qualities: Record<string, unknown>;
    events: unknown[];
  };
}

export interface RustRuntimeSessionState {
  format_version: number;
  session_id: string;
  project_id: string;
  scenario_id: string;
  world: RustSessionSnapshot["world"] & Record<string, unknown>;
  storylet_ids: string[];
  project: RustProjectBundle;
  director: Record<string, unknown>;
}

export interface RustRuntimeCommandResult {
  player: { event: { id: string; actor_id: string; turn: number }; duplicate?: boolean };
  npc: { events: Array<{ id: string; actor_id: string; turn: number }>; limit_reached: boolean };
  world: RustSessionSnapshot["world"];
}

export interface RustDirectorDecision {
  formatVersion: 1;
  contextRevision: number;
  source: string;
  modelVersion: string;
  confidence: number;
  noActionProbability: number;
  ranking: Array<{ storyletId: string; score: number; probability: number }>;
}

export interface RustSemanticRankingRequest {
  formatVersion: 1;
  contextRevision: number;
  modelVersion: string;
  queryEmbedding: number[];
  rankingHead?: {
    schema_version: number; model_version: string; feature_names: string[];
    weights: Record<string, number>; bias: number; temperature: number; no_action_bias: number;
  };
  candidates: Array<{ storyletId: string; embedding: number[]; features?: Record<string, number> }>;
}

export interface RustRuntimeClientOptions {
  /** Connect to an already-running local service instead of launching one. */
  baseUrl?: string;
  /** Service listen address; a free loopback port is selected by default. */
  address?: string;
  /** Defaults to :memory: so standalone clients and tests retain no session DB. */
  databasePath?: string;
}

let serviceBuild: Promise<void> | null = null;

type SessionListItem = {
  session_id: string;
  project_id: string;
  scenario_id: string;
  revision: number;
  turn: number;
  current_location: string | null;
  event_count: number;
};

/** Client for the Rust-owned HTTP runtime. A standalone instance launches a local service. */
export class RustRuntimeClient {
  private child: ReturnType<typeof Bun.spawn> | null = null;
  private baseUrl: string | null;
  private readonly options: RustRuntimeClientOptions;
  private readonly manifestPath = path.resolve(import.meta.dir, "../../crates/storylet-engine-service/Cargo.toml");
  private readonly binaryPath = path.resolve(
    import.meta.dir,
    `../../crates/storylet-engine-service/target/debug/storylet-engine-service${process.platform === "win32" ? ".exe" : ""}`,
  );
  private queue: Promise<unknown> = Promise.resolve();

  constructor(options: RustRuntimeClientOptions = {}) {
    this.options = options;
    this.baseUrl = options.baseUrl?.replace(/\/$/, "") ?? null;
  }

  warmUp(): Promise<{ status: string }> {
    return this.enqueue(async () => {
      await this.ensureService();
      const health = await this.fetchJson<{ status: string }>("/health");
      return { status: health.status === "ok" ? "ready" : health.status };
    });
  }

  listSessions(): Promise<SessionListItem[]> {
    return this.get("/api/v1/playtests");
  }

  createSession(sessionId: string, project: RustProjectBundle, scenarioId: string): Promise<RustSessionSnapshot> {
    return this.enqueue(async () => {
      const result = await this.fetchJson<{ snapshot: RustSessionSnapshot }>("/api/v1/playtests", {
        method: "POST", body: JSON.stringify({ sessionId, project, scenarioId }),
      });
      return result.snapshot;
    });
  }

  snapshot(sessionId: string): Promise<RustSessionSnapshot> {
    return this.get(`/api/v1/playtests/${encodeURIComponent(sessionId)}`);
  }

  async sessionStream(sessionId: string, signal?: AbortSignal): Promise<Response> {
    await this.ensureService();
    return fetch(`${this.baseUrl}/api/v1/playtests/${encodeURIComponent(sessionId)}/stream`, {
      headers: { Accept: "text/event-stream" },
      signal,
    });
  }

  exportSession(sessionId: string): Promise<RustRuntimeSessionState> {
    return this.get(`/api/v1/playtests/${encodeURIComponent(sessionId)}/export`);
  }

  restoreSession(state: RustRuntimeSessionState): Promise<RustSessionSnapshot> {
    return this.enqueue(async () => {
      const result = await this.fetchJson<{ snapshot: RustSessionSnapshot }>("/api/v1/playtests/restore", {
        method: "POST", body: JSON.stringify({ state }),
      });
      return result.snapshot;
    });
  }

  deleteSession(sessionId: string): Promise<{ deleted: boolean }> {
    return this.enqueue(() => this.fetchJson(`/api/v1/playtests/${encodeURIComponent(sessionId)}`, { method: "DELETE" }));
  }

  forkSession(sourceSessionId: string, branchSessionId: string, eventId: string): Promise<RustSessionSnapshot> {
    return this.enqueue(async () => {
      const result = await this.fetchJson<{ snapshot: RustSessionSnapshot }>(`/api/v1/playtests/${encodeURIComponent(sourceSessionId)}/fork`, {
        method: "POST", body: JSON.stringify({ branchSessionId, eventId }),
      });
      return result.snapshot;
    });
  }

  evaluate(sessionId: string, targetTensionSlope = 0.15): Promise<{ candidates: unknown[]; selected: string | null }> {
    const query = new URLSearchParams({ targetTensionSlope: String(targetTensionSlope) });
    return this.get(`/api/v1/playtests/${encodeURIComponent(sessionId)}/director?${query}`);
  }

  applyDecision(sessionId: string, decision: RustDirectorDecision): Promise<{
    candidates: unknown[];
    selected: string | null;
    decision?: RustDirectorDecision;
  }> {
    return this.post(`/api/v1/playtests/${encodeURIComponent(sessionId)}/director/decision`, { decision });
  }

  rankSemantic(sessionId: string, request: RustSemanticRankingRequest): Promise<{
    candidates: unknown[];
    selected: string | null;
    decision?: RustDirectorDecision;
  }> {
    return this.post(`/api/v1/playtests/${encodeURIComponent(sessionId)}/director/semantic-rank`, { request });
  }

  submitCommand(sessionId: string, command: RustWorldCommand): Promise<RustRuntimeCommandResult> {
    return this.post(`/api/v1/playtests/${encodeURIComponent(sessionId)}/commands`, { command });
  }

  executeChoice(
    sessionId: string,
    storyletId: string,
    choiceId: string,
    idempotencyKey: string,
    expectedRevision: number,
  ): Promise<RustRuntimeCommandResult> {
    return this.post(`/api/v1/playtests/${encodeURIComponent(sessionId)}/choices`, {
      storyletId, choiceId, idempotencyKey, expectedRevision,
    });
  }

  close(): Promise<void> {
    return this.enqueue(async () => {
      if (!this.child) return;
      const child = this.child;
      this.child = null;
      child.kill();
      await child.exited;
      this.baseUrl = this.options.baseUrl?.replace(/\/$/, "") ?? null;
    });
  }

  private get<T>(route: string): Promise<T> {
    return this.enqueue(() => this.fetchJson<T>(route));
  }

  private post<T>(route: string, body: unknown): Promise<T> {
    return this.enqueue(() => this.fetchJson<T>(route, { method: "POST", body: JSON.stringify(body) }));
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const result = this.queue.then(operation);
    this.queue = result.then(() => undefined, () => undefined);
    return result;
  }

  private async ensureService(): Promise<void> {
    if (this.baseUrl) return;
    let address = this.options.address;
    if (!address) {
      const reservation = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch: () => new Response("reserved") });
      address = `127.0.0.1:${reservation.port}`;
      reservation.stop(true);
    }
    this.baseUrl = `http://${address}`;
    await this.ensureServiceBuilt();
    this.child = Bun.spawn(
      [this.binaryPath],
      {
        cwd: process.cwd(),
        env: {
          ...Bun.env,
          STORYLET_ENGINE_ADDR: address,
          STORYLET_ENGINE_DB_PATH: this.options.databasePath ?? ":memory:",
        },
        stdin: "ignore", stdout: "ignore", stderr: "ignore", windowsHide: true,
      },
    );
    let lastError: unknown;
    for (let attempt = 0; attempt < 120; attempt++) {
      try {
        const response = await fetch(`${this.baseUrl}/health`);
        if (response.ok) return;
        lastError = new Error(`Rust service health returned HTTP ${response.status}`);
      } catch (error) {
        lastError = error;
      }
      if (this.child.exitCode !== null) {
        throw new Error(`Rust storylet service exited with code ${this.child.exitCode}`, { cause: lastError });
      }
      await Bun.sleep(50);
    }
    this.child.kill();
    await this.child.exited;
    this.child = null;
    this.baseUrl = null;
    throw new Error("Rust storylet service did not become ready", { cause: lastError });
  }

  private async ensureServiceBuilt(): Promise<void> {
    if (!serviceBuild) {
      serviceBuild = (async () => {
        const build = Bun.spawn(
          ["cargo", "build", "--quiet", "--manifest-path", this.manifestPath, "--bin", "storylet-engine-service"],
          { cwd: process.cwd(), stdout: "ignore", stderr: "pipe", windowsHide: true },
        );
        const stderr = build.stderr ? new Response(build.stderr).text() : Promise.resolve("");
        const [exitCode, diagnostics] = await Promise.all([build.exited, stderr]);
        if (exitCode !== 0) throw new Error(`Could not build Rust storylet service (exit ${exitCode}): ${diagnostics}`);
      })().catch((error) => {
        serviceBuild = null;
        throw error;
      });
    }
    await serviceBuild;
  }

  private async fetchJson<T>(route: string, init?: RequestInit): Promise<T> {
    await this.ensureService();
    const response = await fetch(`${this.baseUrl}${route}`, {
      ...init,
      headers: { ...(init?.headers ?? {}), ...(init?.body ? { "content-type": "application/json" } : {}) },
    });
    const text = await response.text();
    let body: any;
    try { body = text ? JSON.parse(text) : {}; }
    catch { body = { error: text }; }
    if (!response.ok) throw new Error(body?.error ?? `Rust storylet service returned HTTP ${response.status}`);
    return body as T;
  }
}
