import { expect, test } from "bun:test";
import { usePlay, type RunsApi } from "../../src/web/studio/view-models/run.ts";
import type { RunAction, RunEnvelope, RunView } from "../../src/web/studio/contracts.ts";

function runView(over: Partial<RunView> = {}): RunView {
  return {
    participant: 0, participants: [{ id: 0, name: "Ada", human: true }], place: null,
    scene: { storylet: { id: 1, title: "Arrival" }, lines: [], choices: [], nodeGuid: 7 },
    alsoHere: [], nothingHere: false, ending: null, steps: [], currentStep: 0, branches: [], ...over,
  };
}
function env(over: Partial<RunEnvelope> = {}, v: Partial<RunView> = {}): RunEnvelope {
  return { id: "r1", revision: 0, stale: false, contentRevision: 1, view: runView(v), ...over };
}
function fakeApi(over: Partial<RunsApi> = {}): RunsApi & { calls: string[] } {
  const calls: string[] = [];
  const api: RunsApi = {
    start: async () => { calls.push("start"); return env(); },
    view: async () => { calls.push("view"); return env(); },
    act: async () => { calls.push("act"); return env({ revision: 1 }); },
    rewind: async (_id, step) => { calls.push(`rewind:${step}`); return env({ revision: 2 }); },
    whyNot: async () => { calls.push("whyNot"); return { nearMisses: [{ storylet: { id: 2, title: "X" }, reason: "no" }] }; },
    inspect: async () => {
      calls.push("inspect");
      return { knows: [], worldChanges: [], chronicle: [], advanced: { runSeed: 1, contentRevision: 1, nodeGuid: null, interaction: null } };
    },
    reload: async () => { calls.push("reload"); return { ...env({ revision: 3, stale: false }), replayed: 12, divergedAt: 13 }; },
    ...over,
  };
  return Object.assign(api, { calls });
}
const action: RunAction = { kind: "wait" };

test("act_while_busy_is_ignored", async () => {
  let release!: () => void;
  const gate = new Promise<void>((r) => { release = r; });
  const api = fakeApi({ act: async () => { api.calls.push("act"); await gate; return env({ revision: 1 }); } });
  const play = usePlay(api);
  await play.open("r1");
  const first = play.act(action);
  const second = play.act(action);
  release();
  await Promise.all([first, second]);
  expect(api.calls.filter((c) => c === "act").length).toBe(1);
  expect(play.state.busy).toBe(false);
  expect(play.state.envelope?.revision).toBe(1);
});

test("act sends a fresh idempotency key and the current revision", async () => {
  const seen: Array<{ rev: number; key: string }> = [];
  const api = fakeApi({ act: async (_id, _p, _a, rev, key) => { seen.push({ rev, key }); return env({ revision: rev + 1 }); } });
  const play = usePlay(api);
  await play.open("r1");
  await play.act(action);
  await play.act(action);
  expect(seen.map((s) => s.rev)).toEqual([0, 1]);
  expect(seen[0].key).not.toBe(seen[1].key);
  expect(seen[0].key.length).toBeGreaterThan(8);
});

test("undo_rewinds_one_step", async () => {
  const api = fakeApi({ view: async () => env({}, { currentStep: 3 }) });
  const play = usePlay(api);
  await play.open("r1");
  await play.undo();
  expect(api.calls).toContain("rewind:2");
});

test("undo at step zero does nothing", async () => {
  const api = fakeApi();
  const play = usePlay(api);
  await play.open("r1");
  await play.undo();
  expect(api.calls.some((c) => c.startsWith("rewind"))).toBe(false);
});

test("stale_envelope_sets_banner_and_reload_note", async () => {
  const api = fakeApi({ view: async () => env({ stale: true }, { currentStep: 14 }) });
  const play = usePlay(api);
  await play.open("r1");
  expect(play.state.envelope?.stale).toBe(true);
  await play.reload();
  expect(play.state.reloadNote).toBe("Replayed 12 of 14 steps; diverged at step 13");
});

test("error from api lands in state and clears busy", async () => {
  const api = fakeApi({ act: async () => { throw new Error("stale revision"); } });
  const play = usePlay(api);
  await play.open("r1");
  await play.act(action);
  expect(play.state.error).toBe("stale revision");
  expect(play.state.busy).toBe(false);
});

test("toggleWhy loads near misses once opened; inspect tab loads inspect", async () => {
  const api = fakeApi();
  const play = usePlay(api);
  await play.open("r1");
  await play.toggleWhy();
  expect(play.state.whyOpen).toBe(true);
  expect(play.state.nearMisses.length).toBe(1);
  play.toggleInspect();
  await play.setInspectTab("knows");
  expect(play.state.inspect).not.toBeNull();
  await play.toggleWhy();
  expect(play.state.whyOpen).toBe(false);
});

test("nextParticipant wraps and onKey dispatches choices", async () => {
  const choice = { action: { kind: "choose", nodeGuid: 7, outputPin: 1 } as RunAction, label: "Go", enabled: true, requirement: null };
  const api = fakeApi({
    view: async () => env({}, {
      participants: [{ id: 0, name: "A", human: true }, { id: 1, name: "B", human: true }],
      scene: { storylet: { id: 1, title: "t" }, lines: [], choices: [choice], nodeGuid: 7 },
    }),
  });
  const play = usePlay(api);
  await play.open("r1");
  play.nextParticipant(1);
  expect(play.state.activeParticipant).toBe(1);
  play.nextParticipant(1);
  expect(play.state.activeParticipant).toBe(0);
  play.nextParticipant(-1);
  expect(play.state.activeParticipant).toBe(1);
  await play.onKey({ key: "1", ctrlKey: false });
  expect(api.calls).toContain("act");
});

test("setParticipant selects a known participant and re-reads the view", async () => {
  const api = fakeApi({
    view: async (_id, p) => { api.calls.push(`view:${p}`); return env({}, { participant: p, participants: [{ id: 0, name: "A", human: true }, { id: 1, name: "B", human: true }] }); },
  });
  const play = usePlay(api);
  await play.open("r1");
  play.setParticipant(1);
  expect(play.state.activeParticipant).toBe(1);
  await Promise.resolve();
  expect(api.calls).toContain("view:1");
  play.setParticipant(9);
  expect(play.state.activeParticipant).toBe(1);
});

test("toggleInspect loads inspect data when it opens and Esc closes it", async () => {
  const api = fakeApi();
  const play = usePlay(api);
  await play.open("r1");
  play.toggleInspect();
  await new Promise((r) => setTimeout(r, 0));
  expect(play.state.inspectOpen).toBe(true);
  expect(play.state.inspect).not.toBeNull();
  await play.onKey({ key: "Escape", ctrlKey: false });
  expect(play.state.inspectOpen).toBe(false);
});

test("end_turn_sends_wait only when the project has a clock", async () => {
  const withClock = fakeApi({ view: async () => env({}, { place: { id: 1, name: "Weir", clockLabel: "D1 evening", actionsLeft: 2 } }) });
  const play = usePlay(withClock);
  await play.open("r1");
  await play.onKey({ key: "e", ctrlKey: false });
  expect(withClock.calls).toContain("act");
  const timeless = fakeApi();
  const idle = usePlay(timeless);
  await idle.open("r1");
  await idle.onKey({ key: "e", ctrlKey: false });
  expect(timeless.calls).not.toContain("act");
});

test("next_participant_skips_rule_participants", async () => {
  const api = fakeApi({
    view: async (_id, p) => env({}, {
      participant: p,
      participants: [{ id: 0, name: "A", human: true }, { id: 1, name: "Rosa", human: false }, { id: 2, name: "B", human: true }],
    }),
  });
  const play = usePlay(api);
  await play.open("r1");
  play.nextParticipant(1);
  expect(play.state.activeParticipant).toBe(2);
  play.nextParticipant(1);
  expect(play.state.activeParticipant).toBe(0);
  play.nextParticipant(-1);
  expect(play.state.activeParticipant).toBe(2);
  await play.onKey({ key: "Tab", ctrlKey: false, shiftKey: true });
  expect(play.state.activeParticipant).toBe(0);
});

test("G then a digit moves through the nth enabled connection", async () => {
  const moves = [
    { to: 4, name: "Harbor", enabled: false, block: { kind: "needs_fact" as const, fact: "tide table" } },
    { to: 5, name: "Inn", enabled: true, block: null },
  ];
  const api = fakeApi({ view: async () => env({}, { place: { id: 1, name: "Weir", moves } }) });
  const play = usePlay(api);
  await play.open("r1");
  await play.onKey({ key: "g", ctrlKey: false });
  await play.onKey({ key: "1", ctrlKey: false });
  expect(api.calls.filter((c) => c === "act").length).toBe(1);
});
