import { expect, test } from "bun:test";
import {
  choiceForKey, chronicleRows, hasClock, moveForKey, nearMissText, showParticipantTabs,
} from "../../src/web/studio/runModel.ts";
import type { ChoiceView, RunView } from "../../src/web/studio/contracts.ts";

function view(over: Partial<RunView> = {}, choices: ChoiceView[] = []): RunView {
  return {
    participant: 0,
    participants: [{ id: 0, name: "Ada" }],
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

test("moveForKey picks the nth move choice", () => {
  const mv = (to: number): ChoiceView => ({ action: { kind: "move", to }, label: `Go ${to}`, enabled: true, requirement: null });
  const v = view({}, [choose(0), mv(4), mv(5)]);
  expect(moveForKey(v, 1)).toEqual({ kind: "move", to: 4 });
  expect(moveForKey(v, 2)).toEqual({ kind: "move", to: 5 });
  expect(moveForKey(v, 3)).toBeNull();
});

test("timeless_project_hides_clock", () => {
  expect(hasClock(view())).toBe(false);
  expect(hasClock(view({ place: { id: 1, name: "Weir", clock: "Dusk" } }))).toBe(true);
  expect(hasClock(view({ place: null }))).toBe(false);
});

test("showParticipantTabs is true only for more than one human", () => {
  expect(showParticipantTabs(view())).toBe(false);
  expect(showParticipantTabs(view({ participants: [{ id: 0, name: "A" }, { id: 1, name: "B" }] }))).toBe(true);
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
