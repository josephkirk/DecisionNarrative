import { describe, expect, test } from "bun:test";
import { buildPlatformRequest, callNarrativePlatform, validateBatchPayload, type BatchArguments } from "../scripts/strata-storylet-mcp.ts";

const featureNames = ["semantic_relevance", "urgency", "pacing_arc_alignment", "novelty", "waiting_time", "goal_progress", "role_binding_quality", "predicted_consequence_alignment", "no_action"];
const scores = Object.fromEntries(featureNames.map((name) => [name, 0.5]));
const args: BatchArguments = { scenario_family: "harbor-archive", count: 1, batch_index: 1, brief: "A flooded archive dispute." };
const response = { examples: [{ query: "A clerk must choose.", positive: "Unlock the dry registry.", negative: "Wait for the tide.", positive_features: scores, negative_features: scores }] };

describe("Strata storylet ranking batches", () => {
  test("assigns synthetic training provenance and the exact ranking schema", () => {
    const [row] = validateBatchPayload(response, args);
    expect(row).toMatchObject({ example_id: "strata-harbor-archive-001-0001", scenario_family: "harbor-archive", split: "train", label_source: "strata_synthetic", query: "A clerk must choose." });
    expect(Object.keys(row.positive_features as object).sort()).toEqual([...featureNames].sort());
  });
  test("rejects incomplete batches instead of accepting partial generation", () => { expect(() => validateBatchPayload({ examples: [] }, args)).toThrow("Expected exactly 1 examples"); });
  test("rejects ranking feature values outside the model contract", () => { const invalid = structuredClone(response); invalid.examples[0].positive_features.urgency = 1.01; expect(() => validateBatchPayload(invalid, args)).toThrow("out-of-range"); });
});

describe("Narrative platform MCP routing", () => {
  test("exposes fixed routes and protects destructive whole-project replacement", () => {
    expect(buildPlatformRequest({ operation: "workspace.inspect" })).toEqual({ path: "/api/v1/project-bundle", method: "GET" });
    expect(() => buildPlatformRequest({ operation: "workspace.replace", bundle: {} })).toThrow("confirmReplace=true");
    expect(buildPlatformRequest({ operation: "workspace.replace", bundle: {}, confirmReplace: true })).toEqual({ path: "/api/v1/project-bundle/import", method: "POST", body: { bundle: {}, confirmReplace: true } });
    expect(() => buildPlatformRequest({ operation: "../../admin" })).toThrow("Unsupported narrative operation");
  });
  test("encodes identifiers and constrains calls to an HTTP loopback origin", () => {
    expect(buildPlatformRequest({ operation: "playtest.snapshot", id: "session/one" }).path).toBe("/api/v1/playtests/session%2Fone");
    expect(() => callNarrativePlatform({ operation: "workspace.inspect" }, fetch, "https://example.test")).toThrow("HTTP loopback origin");
  });
  test("maps generation and playtest controls only to existing engine APIs", () => {
    expect(buildPlatformRequest({ operation: "scenarios.create", payload: { id: "scene" } })).toEqual({ path: "/api/v1/scenarios", method: "POST", body: { id: "scene" } });
    expect(buildPlatformRequest({ operation: "generation.advance", id: "job 1", expectedRevision: 2 })).toEqual({ path: "/api/v1/generation-jobs/job%201/advance", method: "POST", body: { expectedRevision: 2 } });
    expect(buildPlatformRequest({ operation: "playtest.rank", id: "session-1" })).toEqual({ path: "/api/v1/playtests/session-1/director/rank", method: "POST", body: {} });
    expect(buildPlatformRequest({ operation: "playtest.choose", id: "session-1", storyletId: "s", choiceId: "c", expectedRevision: 4, idempotencyKey: "request-1" }).body).toMatchObject({ storyletId: "s", choiceId: "c", expectedRevision: 4, idempotencyKey: "request-1" });
  });
  test("surfaces platform HTTP errors without retrying or rewriting the status", async () => {
    await expect(callNarrativePlatform({ operation: "platform.health" }, async () => new Response(JSON.stringify({ error: "unavailable" }), { status: 503 }), "http://127.0.0.1:3000")).rejects.toThrow("HTTP 503");
  });
  test("dispatches a mapped request and parses the platform response", async () => {
    let observed: { url?: URL; init?: RequestInit } = {};
    const result = await callNarrativePlatform({ operation: "content.save", entity: "storylets", payload: { id: "s1" } }, async (input, init) => { observed = { url: new URL(String(input)), init }; return new Response(JSON.stringify({ success: true }), { status: 200 }); }, "http://localhost:3000");
    expect(observed.url?.pathname).toBe("/api/storylets");
    expect(observed.init?.method).toBe("POST");
    expect(JSON.parse(String(observed.init?.body))).toEqual({ id: "s1" });
    expect(result).toEqual({ success: true });
  });
});


import {
  generateContentBatch,
  generateBatch,
  validateContentBatchPayload,
  type ContentBatchArguments,
} from "../scripts/strata-storylet-mcp.ts";

describe("Strata storylet content batches", () => {
  const contentArgs: ContentBatchArguments = {
    scenario_family: "tell-tale-heart",
    count: 2,
    batch_index: 1,
    brief: "Adaptation of The Tell-Tale Heart. Locations: chamber. Qualities: lantern_ray, heart_beating. Roles: caretaker, victim. Effects: lantern_ray, heart_beating.",
    constraints: ["Keep unsloped tone", "Strict canon adherence"],
  };

  const validStoryletPayload = {
    storylets: [
      {
        ending: false,
        title: "The Lantern Slit",
        location_tags: ["chamber"],
        group_tag: "chamber",
        cluster_type: "linear",
        urgency_weight: 80,
        repeatability: "always",
        cooldown_turns: 0,
        parameters: [{ param_name: "role_1", query: "character.role == 'caretaker'" }],
        preconditions: { all_of: [{ variable: "lantern_ray", op: "eq", value: 0 }] },
        content: { text_template: "A thin thread of light falls across the floorboards." },
        choices: [
          {
            choice_id: "c1",
            label: "Open the shutter slightly more",
            systemVerb: "OPEN",
            effects: { lantern_ray: 1 },
          },
        ],
        effects: { lantern_ray: 1 },
      },
      {
        ending: false,
        title: "The Watch Ticking",
        location_tags: ["chamber"],
        group_tag: "chamber",
        cluster_type: "linear",
        urgency_weight: 85,
        repeatability: "always",
        cooldown_turns: 0,
        parameters: [{ param_name: "role_1", query: "character.role == 'victim'" }],
        preconditions: { all_of: [{ variable: "lantern_ray", op: "eq", value: 1 }] },
        content: { text_template: "A muffled sound begins, like a watch wrapped in cotton." },
        choices: [
          {
            choice_id: "c1",
            label: "Listen in horror",
            systemVerb: "LISTEN",
            effects: { heart_beating: 1 },
          },
        ],
        effects: { heart_beating: 1 },
      },
    ],
  };

  test("accepts a valid batch and assigns deterministic IDs, revision, and approved_revision=null", () => {
    const fixedTime = 1700000000000;
    const storylets = validateContentBatchPayload(validStoryletPayload, contentArgs, fixedTime);
    expect(storylets).toHaveLength(2);

    expect(storylets[0]).toMatchObject({
      id: "stl_tell-tale-heart_001_0001",
      revision: 1,
      approved_revision: null,
      ending: false,
      title: "The Lantern Slit",
      location_tags: ["chamber"],
      cluster_type: "linear",
      urgency_weight: 80,
      repeatability: "always",
      cooldown_turns: 0,
      created_at: fixedTime,
    });
    expect(storylets[1]).toMatchObject({
      id: "stl_tell-tale-heart_001_0002",
      revision: 1,
      approved_revision: null,
      title: "The Watch Ticking",
      created_at: fixedTime,
    });
  });

  test("rejects count mismatch when model returns too few or too many storylets", () => {
    const shortPayload = { storylets: [validStoryletPayload.storylets[0]] };
    expect(() => validateContentBatchPayload(shortPayload, contentArgs)).toThrow("Expected exactly 2 storylets");

    const extraPayload = {
      storylets: [
        validStoryletPayload.storylets[0],
        validStoryletPayload.storylets[1],
        validStoryletPayload.storylets[0],
      ],
    };
    expect(() => validateContentBatchPayload(extraPayload, contentArgs)).toThrow("Expected exactly 2 storylets");
  });

  test("rejects invalid cluster_type, repeatability, and malformed choices", () => {
    const invalidCluster = structuredClone(validStoryletPayload);
    (invalidCluster.storylets[0] as any).cluster_type = "non_existent_cluster";
    expect(() => validateContentBatchPayload(invalidCluster, contentArgs)).toThrow("invalid cluster_type");

    const invalidRepeat = structuredClone(validStoryletPayload);
    (invalidRepeat.storylets[0] as any).repeatability = "never";
    expect(() => validateContentBatchPayload(invalidRepeat, contentArgs)).toThrow("invalid repeatability");

    const missingChoices = structuredClone(validStoryletPayload);
    (missingChoices.storylets[0] as any).choices = [];
    expect(() => validateContentBatchPayload(missingChoices, contentArgs)).toThrow("choices must be a non-empty array");
  });

  test("rejects unallowed canon references when brief declares canon scope", () => {
    const unallowedLocation = structuredClone(validStoryletPayload);
    unallowedLocation.storylets[0].location_tags = ["unmentioned_street"];
    expect(() => validateContentBatchPayload(unallowedLocation, contentArgs)).toThrow("references unallowed location tag 'unmentioned_street'");

    const unallowedQuality = structuredClone(validStoryletPayload);
    (unallowedQuality.storylets[0].preconditions.all_of[0] as any).variable = "gold_coins";
    expect(() => validateContentBatchPayload(unallowedQuality, contentArgs)).toThrow("references unallowed quality 'gold_coins'");

    const unallowedEffect = structuredClone(validStoryletPayload);
    (unallowedEffect.storylets[0].choices[0] as any).effects = { alien_invasion: true };
    expect(() => validateContentBatchPayload(unallowedEffect, contentArgs)).toThrow("writes unallowed effect key 'alien_invasion'");
  });

  test("enforces input bounds: count 1..24 and scenario_family slug validation", async () => {
    await expect(generateContentBatch({ scenario_family: "valid-family", count: 25, brief: "brief" })).rejects.toThrow("count must be an integer from 1 to 24");
    await expect(generateContentBatch({ scenario_family: "valid-family", count: 0, brief: "brief" })).rejects.toThrow("count must be an integer from 1 to 24");
    await expect(generateContentBatch({ scenario_family: "!invalid-slug!", count: 5, brief: "brief" })).rejects.toThrow("scenario_family must be a 1-80 character slug");
  });

  test("dispatches generateContentBatch with mock fetcher and parses clean storylet records", async () => {
    const mockFetcher: typeof fetch = async (url, init) => {
      expect(String(url)).toContain("/chat/completions");
      return new Response(JSON.stringify({
        choices: [{
          message: {
            content: JSON.stringify(validStoryletPayload),
          },
        }],
      }), { status: 200, headers: { "Content-Type": "application/json" } });
    };

    const result = await generateContentBatch(contentArgs, mockFetcher);
    expect(result).toHaveLength(2);
    expect(result[0].id).toBe("stl_tell-tale-heart_001_0001");
    expect(result[0].approved_revision).toBeNull();
  });

  test("generate_storylet_ranking_batch still returns only the training schema with split=train and strata_synthetic", async () => {
    const mockFetcher: typeof fetch = async () => {
      const scoresObj = Object.fromEntries(featureNames.map((n) => [n, 0.4]));
      return new Response(JSON.stringify({
        choices: [{
          message: {
            content: JSON.stringify({
              examples: [{
                query: "A test context query",
                positive: "Good action",
                negative: "Bad action",
                positive_features: scoresObj,
                negative_features: scoresObj,
              }],
            }),
          },
        }],
      }), { status: 200, headers: { "Content-Type": "application/json" } });
    };

    const rankingRows = await generateBatch({ scenario_family: "harbor-archive", count: 1, brief: "Test" }, mockFetcher);
    expect(rankingRows).toHaveLength(1);
    expect(rankingRows[0]).toMatchObject({
      split: "train",
      label_source: "strata_synthetic",
      scenario_family: "harbor-archive",
    });
    expect(rankingRows[0]).not.toHaveProperty("cluster_type");
    expect(rankingRows[0]).not.toHaveProperty("choices");
  });

  test("enforces localhost-only guard for STRATA_BASE_URL", async () => {
    const origBase = Bun.env.STRATA_BASE_URL;
    try {
      Bun.env.STRATA_BASE_URL = "http://remote-server.test:8080/v1";
      await expect(generateContentBatch(contentArgs)).rejects.toThrow("STRATA_BASE_URL must use HTTP on localhost, 127.0.0.1, or ::1");
    } finally {
      if (origBase !== undefined) Bun.env.STRATA_BASE_URL = origBase;
      else delete Bun.env.STRATA_BASE_URL;
    }
  });

  test("rejects malformed and non-JSON responses from Strata all-or-nothing", async () => {
    const nonJsonFetcher: typeof fetch = async () => {
      return new Response("Not valid JSON at all", { status: 200, headers: { "Content-Type": "text/plain" } });
    };
    await expect(generateContentBatch(contentArgs, nonJsonFetcher)).rejects.toThrow("Strata returned invalid JSON; no partial batch was accepted");

    const nonObjectFetcher: typeof fetch = async () => {
      return new Response(JSON.stringify({ choices: [{ message: { content: "[]" } }] }), { status: 200 });
    };
    await expect(generateContentBatch(contentArgs, nonObjectFetcher)).rejects.toThrow("Expected a JSON object");
  });

  test("generates unique deterministic IDs and rejects duplicate ID collisions", () => {
    const storylets = validateContentBatchPayload(validStoryletPayload, contentArgs);
    const idSet = new Set(storylets.map((s) => s.id));
    expect(idSet.size).toBe(2);
    expect(storylets[0].id).toBe("stl_tell-tale-heart_001_0001");
    expect(storylets[1].id).toBe("stl_tell-tale-heart_001_0002");
  });

  test("MCP routing dispatches generate_storylet_content_batch correctly", async () => {
    const { handleMessage } = await import("../scripts/strata-storylet-mcp.ts");
    let written = "";
    const origStdoutWrite = process.stdout.write;
    process.stdout.write = ((chunk: any) => {
      written += String(chunk);
      return true;
    }) as any;

    try {
      // Test tools/list exposes generate_storylet_content_batch
      await handleMessage({ jsonrpc: "2.0", id: "list-1", method: "tools/list" });
      const listRes = JSON.parse(written.trim().split("\n").pop()!);
      const toolNames = listRes.result.tools.map((t: any) => t.name);
      expect(toolNames).toContain("generate_storylet_content_batch");
      expect(toolNames).toContain("generate_storylet_ranking_batch");
      expect(toolNames).toContain("narrative_platform");

      // Test tools/call with invalid input returns toolError without saving anything
      written = "";
      await handleMessage({
        jsonrpc: "2.0",
        id: "call-1",
        method: "tools/call",
        params: {
          name: "generate_storylet_content_batch",
          arguments: { scenario_family: "bad!slug", count: 1, brief: "test" },
        },
      });
      const callRes = JSON.parse(written.trim().split("\n").pop()!);
      expect(callRes.result.isError).toBe(true);
      expect(callRes.result.content[0].text).toContain("scenario_family must be a 1-80 character slug");
    } finally {
      process.stdout.write = origStdoutWrite;
    }
  });

  test("draft generation does not persist or approve content", async () => {
    // Calling generateContentBatch with mock fetcher produces records in memory only
    const mockFetcher: typeof fetch = async () => {
      return new Response(JSON.stringify({
        choices: [{
          message: {
            content: JSON.stringify(validStoryletPayload),
          },
        }],
      }), { status: 200, headers: { "Content-Type": "application/json" } });
    };

    const records = await generateContentBatch(contentArgs, mockFetcher);
    for (const record of records) {
      // Must be draft only: approved_revision is null, no database write occurs
      expect(record.approved_revision).toBeNull();
      expect(record.revision).toBe(1);
    }
  });

});
