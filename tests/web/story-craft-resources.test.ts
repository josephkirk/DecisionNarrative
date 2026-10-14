import { expect, test } from "bun:test";
import { StoryCraftResourceRegistry } from "../../src/agents/StoryCraftResources.ts";
import { UnslopStoryletPipeline } from "../../src/agents/UnslopStoryletPipeline.ts";
import type { GenerationJobRequest } from "../../src/agents/GenerationWorkflow.ts";

test("story craft resources expose source and prompt hashes for storylet-adapted guidance", async () => {
  const registry = new StoryCraftResourceRegistry();
  const resources = await registry.forStage("scene_variants");

  expect(resources.references.map((resource) => resource.id)).toEqual([
    "scene-craft", "interactive-fiction", "voice-style",
  ]);
  expect(resources.prompt).toContain("scene grain");
  expect(resources.prompt).toContain("branch consequences");
  expect(resources.references.every((resource) => resource.sourceHash.length === 64 && resource.promptHash.length === 64)).toBe(true);
  expect(resources.references.every((resource) => resource.version === "1.0.0" && resource.source.endsWith("SKILL.md"))).toBe(true);
});

test("the resource catalog reports generic project-owned craft guidance", async () => {
  const registry = new StoryCraftResourceRegistry();
  const catalog = await registry.catalog();

  expect(catalog.map((resource) => resource.id)).toEqual([
    "character-management", "worldbuilding", "scene-craft", "interactive-fiction",
    "voice-style", "line-editing", "revision-continuity",
  ]);
  expect(catalog.every((resource) => resource.source.includes("SKILL.md"))).toBe(true);
});

test("generation records the exact craft sources and injects their scene guidance into the model prompt", async () => {
  const originalFetch = globalThis.fetch;
  let requestBody: any;
  globalThis.fetch = async (_input, init) => {
    requestBody = JSON.parse(String(init?.body));
    return new Response(JSON.stringify({ choices: [{ message: { content: '{"artifact":{"variants":[]}}' } }] }), {
      headers: { "Content-Type": "application/json" },
    });
  };
  const request: GenerationJobRequest = {
    location: "Records Room",
    clusterType: "linear",
    characterRoles: ["detective"],
    triggerPreconditions: [],
    desiredOutcomes: { "clue.found": true },
    projectContext: { genre: "detective fiction", canon: ["A witness left a receipt"] },
  };

  try {
    const result = await new UnslopStoryletPipeline().generateWorkflowStage("scene_variants", request, {});
    const userPrompt = requestBody.messages[1].content as string;
    expect(userPrompt).toContain("CRAFT RESOURCE scene-craft/1.0.0");
    expect(userPrompt).toContain("CRAFT RESOURCE interactive-fiction/1.0.0");
    expect(userPrompt).toContain("A witness left a receipt");
    expect(result.craftResources?.map((resource) => resource.id)).toEqual(["scene-craft", "interactive-fiction", "voice-style"]);
    expect(result.promptVersion).toContain(result.craftResources?.[0].sourceHash);
  } finally {
    globalThis.fetch = originalFetch;
  }
});
