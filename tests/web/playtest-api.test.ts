import { afterEach, expect, test } from "bun:test";
import { RustRuntimeClient } from "../../src/engine/RustRuntimeClient.ts";
import { handlePlaytestApi } from "../../src/web/playtestApi.ts";
import { buildWorkspaceProject } from "../../src/web/workspaceProject.ts";

const runtimes: RustRuntimeClient[] = [];

afterEach(async () => {
  await Promise.all(runtimes.splice(0).map((runtime) => runtime.close()));
});

test("playtest API creates a clean session, advances a turn, and exposes its event trace", async () => {
  const runtime = new RustRuntimeClient();
  runtimes.push(runtime);
  const create = new Request("http://studio/api/v1/playtests", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      sessionId: "http-smoke",
      scenarioId: "intro",
      project: {
        schema_version: 1,
        id: "plain-project",
        name: "Plain Project",
        revision: 0,
        locations: {
          room: {
            id: "room",
            name: "Room",
            description: "",
            connected_locations: [],
            map_position: [0, 0],
          },
        },
        scenarios: {
          intro: {
            id: "intro",
            name: "Opening",
            player_start_location: "room",
            initial_qualities: {},
            actor_locations: {},
            storylet_ids: [],
            seed: 3,
          },
        },
      },
    }),
  });

  const created = await handlePlaytestApi(create, runtime);
  expect(created?.status).toBe(201);
  const createdBody = await created?.json();
  expect(createdBody.snapshot.world.turn).toBe(0);

  const turnResponse = await handlePlaytestApi(
    new Request("http://studio/api/v1/playtests/http-smoke/commands", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        command: {
          idempotency_key: "player:wait",
          expected_revision: 0,
          actor_id: "player",
          action: "wait",
          effects: [],
          caused_by: [],
          advance_turn: true,
        },
      }),
    }),
    runtime,
  );
  const turnBody = await turnResponse?.json();
  expect(turnResponse?.status).toBe(200);
  expect(turnBody.world.turn).toBe(1);

  const eventsResponse = await handlePlaytestApi(
    new Request("http://studio/api/v1/playtests/http-smoke/events"),
    runtime,
  );
  const eventsBody = await eventsResponse?.json();
  expect(eventsBody.events).toHaveLength(1);
  expect(eventsBody.events[0].action).toBe("wait");
}, 15_000);

test("playtest API streams the current snapshot and committed world updates", async () => {
  const runtime = new RustRuntimeClient();
  runtimes.push(runtime);
  const project = {
    schema_version: 1,
    id: "stream-project",
    name: "Stream Project",
    revision: 0,
    locations: { room: { id: "room", name: "Room", description: "", connected_locations: [], map_position: [0, 0] } },
    scenarios: { intro: { id: "intro", name: "Opening", player_start_location: "room", initial_qualities: {}, actor_locations: {}, storylet_ids: [], seed: 3 } },
  };
  const created = await handlePlaytestApi(new Request("http://studio/api/v1/playtests", {
    method: "POST", headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ sessionId: "stream-api", project, scenarioId: "intro" }),
  }), runtime);
  expect(created?.status).toBe(201);

  const response = await handlePlaytestApi(new Request("http://studio/api/v1/playtests/stream-api/stream"), runtime);
  expect(response?.headers.get("content-type")).toContain("text/event-stream");
  const reader = response!.body!.getReader();
  const initial = new TextDecoder().decode((await reader.read()).value);
  expect(initial).toContain("event: playtest.snapshot");
  expect(initial).toContain('"revision":0');

  await handlePlaytestApi(new Request("http://studio/api/v1/playtests/stream-api/commands", {
    method: "POST", headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ command: {
      idempotency_key: "api-stream-wait", expected_revision: 0, actor_id: "player", action: "wait", effects: [], advance_turn: true,
    } }),
  }), runtime);
  const updated = new TextDecoder().decode((await reader.read()).value);
  expect(updated).toContain("event: playtest.updated");
  expect(updated).toContain('"revision":1');
  await reader.cancel();
});

test("playtest API rejects stale revisions without changing session state", async () => {
  const runtime = new RustRuntimeClient();
  runtimes.push(runtime);
  const project = {
    schema_version: 1,
    id: "plain-project",
    name: "Plain Project",
    revision: 0,
    locations: {
      room: { id: "room", name: "Room", description: "", connected_locations: [], map_position: [0, 0] },
    },
    scenarios: {
      intro: { id: "intro", name: "Opening", player_start_location: "room", initial_qualities: {}, actor_locations: {}, storylet_ids: [], seed: 3 },
    },
  };
  await handlePlaytestApi(new Request("http://studio/api/v1/playtests", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ sessionId: "stale-test", scenarioId: "intro", project }),
  }), runtime);
  await handlePlaytestApi(new Request("http://studio/api/v1/playtests/stale-test/commands", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ command: { idempotency_key: "first", expected_revision: 0, actor_id: "player", action: "wait", effects: [], caused_by: [], advance_turn: true } }),
  }), runtime);

  const stale = await handlePlaytestApi(new Request("http://studio/api/v1/playtests/stale-test/commands", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ command: { idempotency_key: "stale", expected_revision: 0, actor_id: "player", action: "wait", effects: [], caused_by: [], advance_turn: true } }),
  }), runtime);
  const body = await stale?.json();
  const snapshot = await runtime.snapshot("stale-test");

  expect(stale?.status).toBe(409);
  expect(body.error).toContain("revision conflict");
  expect(snapshot.world.turn).toBe(1);
  expect(snapshot.world.events).toHaveLength(1);
});


test("playtest API forks at a selected event and returns an isolated branch", async () => {
  const runtime = new RustRuntimeClient();
  runtimes.push(runtime);
  const project = {
    schema_version: 1, id: "branch-project", name: "Branch Project", revision: 0,
    locations: { room: { id: "room", name: "Room", description: "", connected_locations: [], map_position: [0, 0] } },
    scenarios: { intro: { id: "intro", name: "Opening", player_start_location: "room", initial_qualities: {}, actor_locations: {}, storylet_ids: [], seed: 3 } },
  };
  await handlePlaytestApi(new Request("http://studio/api/v1/playtests", {
    method: "POST", headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ sessionId: "branch-source", project, scenarioId: "intro" }),
  }), runtime);
  const command = await handlePlaytestApi(new Request("http://studio/api/v1/playtests/branch-source/commands", {
    method: "POST", headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ command: { idempotency_key: "event:one", expected_revision: 0, actor_id: "player", action: "wait", effects: [], caused_by: [], advance_turn: true } }),
  }), runtime);
  const eventId = (await command?.json()).player.event.id;
  const forked = await handlePlaytestApi(new Request("http://studio/api/v1/playtests/branch-source/fork", {
    method: "POST", headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ branchSessionId: "branch-child", eventId }),
  }), runtime);
  const result = await forked?.json();

  expect(forked?.status).toBe(201);
  expect(result.snapshot.session_id).toBe("branch-child");
  expect(result.snapshot.world.events).toHaveLength(1);
  expect(result.snapshot.world.events[0].id).toBe(eventId);

  await handlePlaytestApi(new Request("http://studio/api/v1/playtests/branch-child/commands", {
    method: "POST", headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ command: { idempotency_key: "branch:event-two", expected_revision: 1, actor_id: "player", action: "wait", effects: [], caused_by: [], advance_turn: true } }),
  }), runtime);
  const source = await runtime.snapshot("branch-source");
  const branch = await runtime.snapshot("branch-child");
  expect(source.world.events).toHaveLength(1);
  expect(branch.world.events).toHaveLength(2);
});

test("playtest listing comes from the Rust service session store", async () => {
  const runtime = new RustRuntimeClient({ databasePath: ":memory:" });
  runtimes.push(runtime);
  const project = {
    schema_version: 1, id: "service-owned", name: "Service Owned", revision: 0,
    locations: { room: { id: "room", name: "Room", description: "", connected_locations: [], map_position: [0, 0] } },
    scenarios: { intro: { id: "intro", name: "Opening", player_start_location: "room", initial_qualities: {}, actor_locations: {}, storylet_ids: [], seed: 9 } },
  };
  await runtime.createSession("service-session", project, "intro");
  await runtime.submitCommand("service-session", {
    idempotency_key: "service:wait", expected_revision: 0, actor_id: "player", action: "wait", effects: [], advance_turn: true,
  });

  const listed = await handlePlaytestApi(new Request("http://studio/api/v1/playtests"), runtime);
  expect(await listed?.json()).toEqual([{
    session_id: "service-session", project_id: "service-owned", scenario_id: "intro",
    revision: 1, turn: 1, current_location: "room", event_count: 1,
  }]);
});
test("workspace endpoint creates a fresh Rust scenario from designer records", async () => {
  const runtime = new RustRuntimeClient();
  runtimes.push(runtime);
  const records = {
    qualities: [],
    locations: [{ id: "studio", name: "Studio", description: "A plain room", connected_zones: [] }],
    characters: [],
    storylets: [],
  };
  const response = await handlePlaytestApi(
    new Request("http://studio/api/v1/playtests/from-workspace", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ startLocationId: "studio" }),
    }),
    runtime,
    (startLocationId) => buildWorkspaceProject(records, startLocationId),
  );
  const body = await response?.json();

  expect(response?.status).toBe(201);
  expect(body.sessionId).toBeString();
  expect(body.snapshot.world.current_location).toBe("studio");
  expect(body.snapshot.world.events).toEqual([]);
});

test("workspace endpoint starts from an authored scenario rather than replacing its setup", async () => {
  const runtime = new RustRuntimeClient();
  runtimes.push(runtime);
  const records = {
    qualities: [{ name: "clue.seen", type: "tool" as const, value: 0 }],
    locations: [
      { id: "market", name: "Market", description: "", connected_zones: ["archive"] },
      { id: "archive", name: "Archive", description: "", connected_zones: ["market"] },
    ],
    characters: [{ id: "clerk", name: "Clerk", role: "Clerk", home_location: "archive", current_location: "archive", status_axes: {}, needs: {}, agenda: "", red_line: "" }],
    storylets: [],
  };
  const scenario = {
    id: "before-interview", name: "Before the interview", player_start_location: "archive",
    initial_qualities: { "clue.seen": 1 }, actor_locations: { clerk: "market" }, storylet_ids: [], seed: 32,
  };
  const response = await handlePlaytestApi(
    new Request("http://studio/api/v1/playtests/from-workspace", {
      method: "POST", headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ scenarioId: scenario.id }),
    }),
    runtime,
    (_startLocationId, scenarioId) => {
      expect(scenarioId).toBe(scenario.id);
      return buildWorkspaceProject(records, { scenario });
    },
  );
  const body = await response?.json();

  expect(response?.status).toBe(201);
  expect(body.snapshot.world.scenario_id).toBe(scenario.id);
  expect(body.snapshot.world.current_location).toBe("archive");
  expect(body.snapshot.world.qualities["clue.seen"]).toBe(1);
  expect(body.snapshot.world.actors.clerk.current_location).toBe("market");
  expect(body.snapshot.world.random_seed).toBe(32);
});

test("designer-created storylet candidates execute their authored choice effects in Rust", async () => {
  const runtime = new RustRuntimeClient();
  runtimes.push(runtime);
  const built = buildWorkspaceProject({
    qualities: [{ name: "clue.receipt", type: "tool", value: 0, description: "Receipt clue" }],
    locations: [{ id: "archive", name: "Archive", description: "A reading room", tags: ["archive"], connected_zones: [] }],
    characters: [{ id: "clerk", name: "Clerk", role: "Clerk", home_location: "archive", current_location: "archive", status_axes: {}, needs: {}, agenda: "", red_line: "" }],
    storylets: [{
      revision: 1, approved_revision: 1,
      id: "read-receipt", title: "Read the Receipt", location_tags: ["archive"], cluster_type: "player_controlled_passage",
      urgency_weight: 50, repeatability: "always", cooldown_turns: 0,
      parameters: [{ param_name: "clerk", query: "character.role == 'Clerk'" }],
      preconditions: { all_of: [{ variable: "clue.receipt", op: "eq", value: 0 }] },
      content: { text_template: "The clerk reads the receipt." },
      choices: [{ choice_id: "read", label: "Read it", effects: { "clue.receipt": 1 } }],
      effects: {}, created_at: 1,
    }],
  }, "archive");
  const created = await handlePlaytestApi(new Request("http://studio/api/v1/playtests", {
    method: "POST", headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ sessionId: "storylet-choice", project: built.project, scenarioId: built.scenarioId }),
  }), runtime);
  expect(created?.status).toBe(201);

  const director = await handlePlaytestApi(new Request("http://studio/api/v1/playtests/storylet-choice/director"), runtime);
  const evaluation = await director?.json();
  expect(evaluation.selected).toBe("read-receipt");
  const choice = await handlePlaytestApi(new Request("http://studio/api/v1/playtests/storylet-choice/choices", {
    method: "POST", headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ storyletId: "read-receipt", choiceId: "read", expectedRevision: 0, idempotencyKey: "choice:read" }),
  }), runtime);
  const result = await choice?.json();

  expect(choice?.status).toBe(200);
  expect(result.world.qualities["clue.receipt"]).toBe(1);
  expect(result.world.events[0].action).toBe("storylet:read-receipt:read");
});

test("workspace-authored NPC behavior reacts to a player action in the same turn", async () => {
  const runtime = new RustRuntimeClient();
  runtimes.push(runtime);
  const built = buildWorkspaceProject({
    qualities: [
      { name: "clue.visible", type: "tool", value: 0 },
      { name: "npc.response", type: "metric", value: 0 },
    ],
    locations: [{ id: "archive", name: "Archive", description: "", connected_zones: [] }],
    characters: [{ id: "keeper", name: "Keeper", role: "Archivist", home_location: "archive", current_location: "archive", status_axes: {}, needs: {}, agenda: "Protect the record", red_line: "" }],
    storylets: [],
    npcBehaviors: [{
      id: "keeper-notices-clue", actor_id: "keeper", priority: 10, action: "secure the exposed record",
      conditions: { all_of: [{ variable: "clue.visible", op: "eq", value: 1 }] },
      effects: [{ kind: "set_quality", key: "npc.response", value: 1 }],
    }],
  }, "archive");
  const created = await handlePlaytestApi(new Request("http://studio/api/v1/playtests", {
    method: "POST", headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ sessionId: "npc-reaction", project: built.project, scenarioId: built.scenarioId }),
  }), runtime);
  expect(created?.status).toBe(201);

  const response = await handlePlaytestApi(new Request("http://studio/api/v1/playtests/npc-reaction/commands", {
    method: "POST", headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ command: {
      idempotency_key: "player:show-clue", expected_revision: 0, actor_id: "player", action: "show clue",
      effects: [{ kind: "set_quality", key: "clue.visible", value: 1 }], caused_by: [], advance_turn: true,
    } }),
  }), runtime);
  const result = await response?.json();

  expect(response?.status).toBe(200);
  expect(result.world.qualities["npc.response"]).toBe(1);
  expect(result.npc.events).toHaveLength(1);
  expect(result.npc.events[0].actor_id).toBe("keeper");
  expect(result.npc.events[0].caused_by).toEqual([result.player.event.id]);
});
