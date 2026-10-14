import { buildWorkspaceProject } from "../src/web/workspaceProject.ts";
import { RustRuntimeClient } from "../src/engine/RustRuntimeClient.ts";
import { rankPlaytestWithLocalEncoder } from "../src/web/semanticDirector.ts";

const embeddingAddress = "http://127.0.0.1:4318";
const embeddingProcess = Bun.spawn(
  ["uv", "run", "--project", "model", "--no-sync", "python", "model/embedding_service.py"],
  { stdin: "ignore", stdout: "ignore", stderr: "ignore", windowsHide: true },
);
const runtime = new RustRuntimeClient({ databasePath: ":memory:" });

try {
  let ready = false;
  for (let attempt = 0; attempt < 180; attempt++) {
    try {
      const response = await fetch(`${embeddingAddress}/health`);
      if (response.ok) {
        ready = true;
        break;
      }
    } catch { /* Service process is still starting. */ }
    await Bun.sleep(1000);
  }
  if (!ready) throw new Error("Local embedding service did not start.");

  const built = buildWorkspaceProject({
    qualities: [],
    locations: [{
      id: "records-room",
      name: "Receipt Archive",
      description: "A room where investigators search old receipts and ledgers.",
      connected_zones: [],
    }],
    characters: [],
    storylets: [
      {
        id: "receipt-lead", revision: 1, approved_revision: 1,
        title: "The Missing Receipt", location: "records-room", cluster_type: "player_controlled_passage",
        urgency_weight: 50, repeatability: "always", cooldown_turns: 0, parameters: [], preconditions: {},
        content: { text_template: "A receipt is hidden among the archive ledgers." }, choices: [], effects: {}, created_at: 1,
      },
      {
        id: "rain-delay", revision: 1, approved_revision: 1,
        title: "Rain Delays the Ferry", location: "records-room", cluster_type: "player_controlled_passage",
        urgency_weight: 50, repeatability: "always", cooldown_turns: 0, parameters: [], preconditions: {},
        content: { text_template: "Heavy rain delays a ferry departure." }, choices: [], effects: {}, created_at: 1,
      },
    ],
  }, "records-room");
  if (built.issues.length) throw new Error(built.issues.join("; "));
  await runtime.createSession("semantic-live-smoke", built.project, built.scenarioId);
  const result = await rankPlaytestWithLocalEncoder(runtime, "semantic-live-smoke", embeddingAddress);
  if (result.decision?.source !== "embedding_baseline_uncalibrated") {
    throw new Error("Rust did not accept the semantic ranking result.");
  }
  if (result.decision.modelVersion.endsWith("@unknown")) {
    throw new Error("The embedding model revision was not pinned.");
  }
  const snapshot = await runtime.snapshot("semantic-live-smoke");
  if (snapshot.world.revision !== 0 || result.candidates.filter((candidate) => candidate.eligible).length !== 2) {
    throw new Error("Ranking unexpectedly mutated state or changed the candidate set.");
  }
  console.log(JSON.stringify({
    selected: result.selected,
    model: result.decision.modelVersion,
    confidence: result.decision.confidence,
    probabilities: result.decision.ranking.map((item) => ({ id: item.storyletId, probability: item.probability })),
    noActionProbability: result.decision.noActionProbability,
    worldRevisionAfterRanking: snapshot.world.revision,
  }, null, 2));
} finally {
  await runtime.close();
  if (process.platform === "win32") {
    Bun.spawnSync(["taskkill", "/PID", String(embeddingProcess.pid), "/T", "/F"], { stdout: "ignore", stderr: "ignore" });
  } else {
    embeddingProcess.kill();
  }
  await embeddingProcess.exited;
}
