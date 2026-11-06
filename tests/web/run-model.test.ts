import { expect, test } from "bun:test";
import {
  choiceForKey, chronicleRows, hasClock, moveBlockText, moveForKey, nearMissText, showParticipantTabs,
} from "../../src/web/studio/runModel.ts";
import type { ChoiceView, MoveOption, RunView } from "../../src/web/studio/contracts.ts";

function view(over: Partial<RunView> = {}, choices: ChoiceView[] = []): RunView {
  return {
    participant: 0,
    participants: [{ id: 0, name: "Ada", human: true }],
    place: { id: 1, name: "Weir" },
    scene: { storylet: { id: 1, title: "Arrival" }, lines: [], choices, nodeGuid: 7 },
    alsoHere: [], nothingHere: false, ending: null, steps: [], currentStep: 0, branches: [],
    ...over,
  };
}
const choose = (n: number, enabled = true): ChoiceView => ({
  action: { kind: "choose", nodeGuid: 7, outputPin: n }, label: `Option ${n}`, enabled, requirement: null,
});

test("keyboard_maps_only_first_nine_choices", () => {
  const v = view({}, Array.from({ length: 12 }, (_, i) => choose(i)));
  expect(choiceForKey(v, "9")).toEqual({ kind: "choose", nodeGuid: 7, outputPin: 8 });
  expect(choiceForKey(v, "1")).toEqual({ kind: "choose", nodeGuid: 7, outputPin: 0 });
  expect(choiceForKey(v, "0")).toBeNull();
  expect(choiceForKey(v, "10")).toBeNull();
  expect(choiceForKey(v, "a")).toBeNull();
});

test("disabled choice and missing scene give no action", () => {
  expect(choiceForKey(view({}, [choose(0, false)]), "1")).toBeNull();
  expect(choiceForKey(view({ scene: null }), "1")).toBeNull();
});

test("moveForKey picks the nth enabled move of the place", () => {
  const moves: MoveOption[] = [
    { to: 4, name: "Harbor", enabled: true, block: null },
    { to: 9, name: "Rock", enabled: false, block: { kind: "closed_until", slotLabel: "D3 morning" } },
    { to: 5, name: "Inn", enabled: true, block: null },
  ];
  const v = view({ place: { id: 1, name: "Weir", moves } });
  expect(moveForKey(v, 1)).toEqual({ kind: "move", to: 4 });
  expect(moveForKey(v, 2)).toEqual({ kind: "move", to: 5 });
  expect(moveForKey(v, 3)).toBeNull();
  expect(moveForKey(view({ place: null }), 1)).toBeNull();
});

test("moveBlockText says why a move is blocked", () => {
  expect(moveBlockText({ kind: "closed_until", slotLabel: "D3 morning" })).toBe("Closed until D3 morning");
  expect(moveBlockText({ kind: "needs_fact", fact: "tide table" })).toBe("Needs: tide table");
  expect(moveBlockText({ kind: "needs_fact", fact: "fact.tide_table" })).toBe("Needs: tide table");
});

test("timeless_project_hides_clock", () => {
  expect(hasClock(view())).toBe(false);
  expect(hasClock(view({ place: { id: 1, name: "Weir", clockLabel: "Dusk" } }))).toBe(true);
  expect(hasClock(view({ place: null }))).toBe(false);
});

test("showParticipantTabs is true only for more than one human", () => {
  expect(showParticipantTabs(view())).toBe(false);
  expect(showParticipantTabs(view({ participants: [{ id: 0, name: "A", human: true }, { id: 1, name: "B", human: true }] }))).toBe(true);
  // Rule participants are not seats.
  expect(showParticipantTabs(view({ participants: [{ id: 0, name: "A", human: true }, { id: 1, name: "Rosa", human: false }] }))).toBe(false);
});

test("nearMissText joins title and reason", () => {
  expect(nearMissText({ storylet: { id: 2, title: "The Ledger" }, reason: "needs Key" })).toBe("The Ledger — needs Key");
});

test("chronicleRows keeps story-time order and told only on revealed rows", () => {
  const rows = chronicleRows([
    { key: "b", label: "Flood", place: "Weir", storyTime: 20, told: 3 },
    { key: "a", label: "Theft", place: null, storyTime: 10, told: null },
  ]);
  expect(rows).toEqual([
    { label: "Theft", told: null, place: "" },
    { label: "Flood", told: 3, place: "Weir" },
  ]);
});

import { activeBranchId, editLink, stepText, timelineDots } from "../../src/web/studio/runModel.ts";
import type { BranchView, RunStep } from "../../src/web/studio/contracts.ts";

const stepOf = (index: number, branch: number, action: RunStep["action"]): RunStep => ({ index, branch, participant: 0, action, label: "debug" });

test("stepText describes each action kind in plain words", () => {
  expect(stepText(stepOf(1, 0, { kind: "choose", nodeGuid: 1, outputPin: 1 }))).toBe("Chose option 2");
  expect(stepText(stepOf(2, 0, { kind: "play_storylet", nodeGuid: 1, storylet: 3 }))).toBe("Started a storylet");
  expect(stepText(stepOf(3, 0, { kind: "topic", nodeGuid: 1, edgeId: 3 }))).toBe("Asked about a topic");
  expect(stepText(stepOf(4, 0, { kind: "move", to: 2 }))).toBe("Moved");
  expect(stepText(stepOf(5, 0, { kind: "wait" }))).toBe("Waited");
});

test("timelineDots marks done, current and ghost steps from the active branch", () => {
  const a = stepOf(1, 0, { kind: "wait" });
  const b = stepOf(2, 0, { kind: "wait" });
  const c = stepOf(3, 0, { kind: "wait" });
  const branches: BranchView[] = [{ id: 0, steps: [a, b, c] }];
  const v = view({ steps: [a], currentStep: 1, branches });
  expect(timelineDots(v).map((d) => [d.index, d.state])).toEqual([[0, "done"], [1, "current"], [2, "ghost"], [3, "ghost"]]);
  expect(timelineDots(view({ steps: [], currentStep: 0, branches: [] })).map((d) => d.state)).toEqual(["current"]);
});

test("activeBranchId follows the last applied step and branch chips need two branches", () => {
  const s = stepOf(1, 2, { kind: "wait" });
  expect(activeBranchId(view({ steps: [s], currentStep: 1, branches: [{ id: 0, steps: [] }, { id: 2, steps: [s] }] }))).toBe(2);
  expect(activeBranchId(view())).toBe(0);
});

test("editLink points at the authored storylet when its key is known", () => {
  expect(editLink({ id: 1, title: "A", key: "a b" })).toBe("/storylets/a%20b/edit");
  expect(editLink({ id: 1, title: "A" })).toBeNull();
});

import { stepCountText } from "../../src/web/studio/runModel.ts";
test("stepCountText pluralises", () => {
  expect(stepCountText(0)).toBe("0 steps");
  expect(stepCountText(1)).toBe("1 step");
  expect(stepCountText(12)).toBe("12 steps");
});

import { knowKindLabel, knowRowStatus, changeText, toldMark } from "../../src/web/studio/runModel.ts";

test("knowKindLabel names every knowledge kind in plain words", () => {
  expect(knowKindLabel("rumor")).toBe("rumour");
  expect(knowKindLabel("evidence")).toBe("evidence");
  expect(knowKindLabel("testimony")).toBe("testimony");
});

test("knowRowStatus labels unconfirmed rows in words, not colour alone", () => {
  expect(knowRowStatus({ fact: "fact.x", kind: "testimony", confirmed: false, sourceStep: 3 })).toBe("not confirmed");
  expect(knowRowStatus({ fact: "fact.x", kind: "evidence", confirmed: true, sourceStep: null })).toBeNull();
});

test("changeText shows before to after", () => {
  expect(changeText({ key: "mood", before: "1", after: "2" })).toBe("mood: 1 → 2");
});

test("toldMark uses circled numbers up to 20 and falls back to the number", () => {
  expect(toldMark(1)).toBe("①");
  expect(toldMark(3)).toBe("③");
  expect(toldMark(21)).toBe("(21)");
  expect(toldMark(null)).toBe("");
});

test("chronicleRows renders untold entries with told null", () => {
  const rows = chronicleRows([
    { key: "a", label: "A", place: null, storyTime: 1, told: null },
    { key: "b", label: "B", place: null, storyTime: 2, told: 1 },
  ]);
  expect(rows.map((r) => r.told)).toEqual([null, 1]);
});

import { actionsLeftText, noteLines } from "../../src/web/studio/runModel.ts";

test("actionsLeftText reads naturally and is empty without a clock", () => {
  expect(actionsLeftText(undefined)).toBe("");
  expect(actionsLeftText(0)).toBe("No actions left");
  expect(actionsLeftText(1)).toBe("1 action left");
  expect(actionsLeftText(2)).toBe("2 actions left");
});

test("noteLines lists occupancy notes and rule plays for a step", () => {
  const step = {
    index: 3, branch: 0, participant: 0, action: { kind: "wait" } as RunStep["action"], label: "x",
    notes: ["player is driven from Rock back to Lighthouse"],
    rulePlays: [{ participant: 1, storylet: { id: 4, title: "Rosa gossips" } }],
  } satisfies RunStep;
  expect(noteLines(step, [{ id: 0, name: "Mara", human: true }, { id: 1, name: "Rosa", human: false }])).toEqual([
    "player is driven from Rock back to Lighthouse",
    "Rosa: Rosa gossips",
  ]);
  expect(noteLines({ ...step, notes: undefined, rulePlays: undefined }, [])).toEqual([]);
});
