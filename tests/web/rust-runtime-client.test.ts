import { expect, test } from "bun:test";
import { RustRuntimeClient } from "../../src/engine/RustRuntimeClient.ts";

test("studio runtime client executes a player turn through the Rust host", async () => {
  const client = new RustRuntimeClient();
  try {
    expect(await client.warmUp()).toEqual({ status: "ready" });
    await client.createSession("web-smoke", {
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
    }, "intro");


    const before = await client.snapshot("web-smoke");
    const turn = await client.submitCommand("web-smoke", {
      idempotency_key: "player:wait-1",
      expected_revision: 0,
      actor_id: "player",
      action: "wait",
      effects: [],
      caused_by: [],
      advance_turn: true,
    });

    expect(before.world.turn).toBe(0);
    expect(turn.world.turn).toBe(1);
    expect(turn.player.event.actor_id).toBe("player");
    expect(turn.npc.events).toEqual([]);
    const saved = await client.exportSession("web-smoke");
    await client.deleteSession("web-smoke");
    expect(await client.listSessions()).toEqual([]);
    await client.restoreSession(saved);
    const restored = await client.snapshot("web-smoke");
    expect(restored.session_id).toBe("web-smoke");
    expect(restored.world).toEqual(turn.world);
  } finally {
    await client.close();
  }
});

test("runtime client starts the Rust HTTP service with isolated in-memory session storage", async () => {
  const client = new RustRuntimeClient({ databasePath: ":memory:" });
  try {
    expect(await client.warmUp()).toEqual({ status: "ready" });
    expect(await client.listSessions()).toEqual([]);
  } finally {
    await client.close();
  }
});
