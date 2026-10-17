import type { RustDirectorDecision, RustRuntimeClient } from "../engine/RustRuntimeClient.ts";

type EvaluatedCandidate = {
  storylet_id: string;
  eligible: boolean;
  arc_alignment_score?: number;
};

type RankingHead = NonNullable<import("../engine/RustRuntimeClient.ts").RustSemanticRankingRequest["rankingHead"]>;
const FEATURE_NAMES = ["semantic_relevance", "urgency", "pacing_arc_alignment", "novelty", "waiting_time", "goal_progress", "role_binding_quality", "predicted_consequence_alignment", "no_action"];

type EmbeddingResponse = {
  model: string;
  model_revision: string;
  dimensions: number;
  embeddings: number[][];
};

function objectRecord(value: unknown): Record<string, any> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, any>
    : {};
}

function sceneContext(session: Awaited<ReturnType<RustRuntimeClient["exportSession"]>>): string {
  const world = session.world;
  const project = session.project;
  const location = world.current_location ? objectRecord(project.locations?.[world.current_location]) : {};
  const actors = Object.entries(objectRecord(world.actors)).map(([id, actor]) => {
    const character = objectRecord(project.characters?.[id]);
    return `${character.name ?? id} (${(actor as any).role_tags?.join(", ") ?? "character"}) at ${(actor as any).current_location ?? "unknown location"}`;
  });
  const recent = (world.events ?? []).slice(-8).map((event: any) =>
    `Turn ${event.turn}: ${event.actor_id} ${event.action}`,
  );
  const qualities = Object.entries(objectRecord(world.qualities))
    .map(([key, value]) => `${key}=${JSON.stringify(value)}`);
  return [
    `Current location: ${location.name ?? world.current_location ?? "unset"}. ${location.description ?? ""}`,
    `Characters: ${actors.join("; ") || "none"}.`,
    `World state: ${qualities.join("; ") || "no qualities set"}.`,
    `Recent events: ${recent.join("; ") || "no events yet"}.`,
  ].join(" ").slice(0, 24_000);
}

function storyletDocument(id: string, project: Awaited<ReturnType<RustRuntimeClient["exportSession"]>>["project"]): string {
  const storylet = objectRecord(project.storylets?.[id]);
  const roles = Array.isArray(storylet.required_roles)
    ? storylet.required_roles.map((role: any) => role.role).join(", ")
    : "none";
  const choices = Array.isArray(storylet.choices)
    ? storylet.choices.map((choice: any) => choice.label).join("; ")
    : "";
  return [
    `Scene: ${storylet.title ?? id}.`,
    `Summary: ${storylet.summary ?? ""}.`,
    `Location tags: ${Array.isArray(storylet.location_tags) && storylet.location_tags.length ? storylet.location_tags.join(", ") : "anywhere"}.`,
    `Roles: ${roles}.`,
    `Opening: ${storylet.content?.opening ?? ""}.`,
    `Choices: ${choices}.`,
  ].join(" ").slice(0, 24_000);
}

async function requestEmbeddings(baseUrl: string, input: string | string[], inputType: "query" | "document") {
  const response = await fetch(`${baseUrl.replace(/\/$/, "")}/v1/embeddings`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ input, input_type: inputType }),
  });
  const body = await response.json() as EmbeddingResponse | { error?: string };
  if (!response.ok) throw new Error("error" in body ? body.error ?? "Embedding service failed." : "Embedding service failed.");
  if (!("embeddings" in body) || body.dimensions !== 768 || !body.model_revision) {
    throw new Error("Embedding service returned incomplete or incompatible model metadata.");
  }
  if (body.embeddings.length !== (Array.isArray(input) ? input.length : 1)
    || body.embeddings.some((vector) => vector.length !== 768 || vector.some((value) => !Number.isFinite(value)))) {
    throw new Error("Embedding service returned invalid vectors.");
  }
  return body;
}

async function requestRankingHead(baseUrl: string): Promise<RankingHead | undefined> {
  const response = await fetch(`${baseUrl.replace(/\/$/, "")}/v1/ranking-head`);
  if (response.status === 404) return undefined;
  const body = await response.json() as RankingHead | { error?: string };
  if (!response.ok) throw new Error("error" in body ? body.error ?? "Ranking head unavailable." : "Ranking head unavailable.");
  const head = body as RankingHead;
  if (head.schema_version !== 1 || !head.model_version || JSON.stringify(head.feature_names) !== JSON.stringify(FEATURE_NAMES)
    || !head.weights || FEATURE_NAMES.some((name) => !Number.isFinite(head.weights[name]))
    || !Number.isFinite(head.bias) || !Number.isFinite(head.temperature) || head.temperature <= 0 || !Number.isFinite(head.no_action_bias)) {
    throw new Error("Embedding service returned an incompatible ranking head.");
  }
  return head;
}

function candidateFeatures(candidate: EvaluatedCandidate, id: string, session: Awaited<ReturnType<RustRuntimeClient["exportSession"]>>): Record<string, number> {
  const storylet = objectRecord(session.project.storylets?.[id]);
  const required = Array.isArray(storylet.required_roles) ? storylet.required_roles.length : 0;
  const bound = Array.isArray((candidate as any).bound_roles) ? (candidate as any).bound_roles.length : 0;
  const events = session.world.events as Array<any>;
  const prior = events.filter((event) => String(event.action ?? "").includes(`storylet:${id}:`));
  const lastTurn = prior.length ? Number(prior[prior.length - 1].turn) : -100;
  return {
    semantic_relevance: 0,
    urgency: Math.max(0, Math.min(1, Number(storylet.urgency_weight ?? 50) / 100)),
    pacing_arc_alignment: Math.max(0, Math.min(1, Number(candidate.arc_alignment_score ?? 0.5))),
    novelty: 1 / (1 + prior.length),
    waiting_time: 1 - Math.exp(-Math.max(0, session.world.turn - lastTurn) / 5),
    goal_progress: 0.5,
    role_binding_quality: required ? Math.max(0, Math.min(1, bound / required)) : 1,
    predicted_consequence_alignment: 0.5,
    no_action: 0,
  };
}

/** Ask the optional local encoder to rank only Rust-eligible storylets, then
 * pass version-bound vectors into the Rust scoring and selection core. */
export async function rankPlaytestWithLocalEncoder(
  runtime: RustRuntimeClient,
  sessionId: string,
  embeddingServiceUrl: string,
) {
  const [session, evaluation] = await Promise.all([
    runtime.exportSession(sessionId),
    runtime.evaluate(sessionId),
  ]);
  const eligible = (evaluation.candidates as EvaluatedCandidate[]).filter((candidate) => candidate.eligible);
  let decision: RustDirectorDecision;
  if (!eligible.length) {
    decision = {
      formatVersion: 1,
      contextRevision: session.world.revision,
      source: "embedding_baseline_uncalibrated",
      modelVersion: "no-candidates",
      confidence: 0,
      noActionProbability: 1,
      ranking: [],
    };
  } else {
    const [query, rankingHead] = await Promise.all([
      requestEmbeddings(embeddingServiceUrl, sceneContext(session), "query"),
      requestRankingHead(embeddingServiceUrl),
    ]);
    const docs = eligible.map((candidate) => ({
      id: candidate.storylet_id,
      text: storyletDocument(candidate.storylet_id, session.project),
    }));
    const documents: number[][] = [];
    for (let offset = 0; offset < docs.length; offset += 64) {
      const batch = await requestEmbeddings(
        embeddingServiceUrl,
        docs.slice(offset, offset + 64).map((item) => item.text),
        "document",
      );
      if (batch.model !== query.model || batch.model_revision !== query.model_revision) {
        throw new Error("Embedding model revision changed while ranking this scene.");
      }
      documents.push(...batch.embeddings);
    }
    return runtime.rankSemantic(sessionId, {
      formatVersion: 1,
      contextRevision: session.world.revision,
      modelVersion: `${query.model}@${query.model_revision}`,
      queryEmbedding: query.embeddings[0],
      ...(rankingHead ? { rankingHead } : {}),
      candidates: docs.map((item, index) => ({ storyletId: item.id, embedding: documents[index], ...(rankingHead ? { features: candidateFeatures(eligible.find((candidate) => candidate.storylet_id === item.id)!, item.id, session) } : {}) })),
    });
  }
  return runtime.applyDecision(sessionId, decision);
}
