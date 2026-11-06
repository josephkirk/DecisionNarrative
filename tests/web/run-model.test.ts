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
