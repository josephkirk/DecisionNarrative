// Pure helpers over a RunView. No Vue, no fetch: everything here is unit-testable.
import type { ChronicleRowView, NearMiss, RunAction, RunStep, RunView, StoryletRef } from "./contracts.ts";

/** "1".."9" -> the nth choice's action. Disabled choices and any other key give null. */
export function choiceForKey(view: RunView, key: string): RunAction | null {
  if (!/^[1-9]$/.test(key)) return null;
  const choice = view.scene?.choices[Number(key) - 1];
  return choice && choice.enabled ? choice.action : null;
}

/** After "G": the nth (1-based) enabled move choice. */
export function moveForKey(view: RunView, n: number): RunAction | null {
  const moves = (view.scene?.choices ?? []).filter((c) => c.action.kind === "move" && c.enabled);
  return moves[n - 1]?.action ?? null;
}

/** The clock row appears only for projects that define a clock. */
export function hasClock(view: RunView): boolean {
  return view.place?.clock !== undefined && view.place.clock !== "";
}

/** Tabs appear only when more than one human participant exists. */
export function showParticipantTabs(view: RunView): boolean {
  return view.participants.length > 1;
}

export function nearMissText(m: NearMiss): string {
  return `${m.storylet.title} — ${m.reason}`;
}

/** Story-time order; `told` is set only on rows the run has revealed. */
export function chronicleRows(rows: ChronicleRowView[]): Array<{ label: string; told: number | null; place: string }> {
  return [...rows]
    .sort((a, b) => a.storyTime - b.storyTime)
    .map((r) => ({ label: r.label, told: r.told, place: r.place ?? "" }));
}

/** Plain-language step text; the engine's own label is a debug string and is never shown. */
export function stepText(step: RunStep): string {
  switch (step.action.kind) {
    case "choose": return `Chose option ${step.action.outputPin + 1}`;
    case "play_storylet": return "Started a storylet";
    case "topic": return "Asked about a topic";
    case "move": return "Moved";
    case "wait": return "Waited";
  }
}

/** The branch the run is on now: that of the last applied step (the main branch before any step). */
export function activeBranchId(view: RunView): number {
  const last = view.steps[view.steps.length - 1];
  return last ? last.branch : (view.branches[0]?.id ?? 0);
}

export interface TimelineDot { index: number; state: "done" | "current" | "ghost"; step: RunStep | null }

/** Dot 0 is the start; applied steps are done, the last applied is current, later steps of this branch are ghosts. */
export function timelineDots(view: RunView): TimelineDot[] {
  const branch = view.branches.find((b) => b.id === activeBranchId(view));
  const all = branch?.steps ?? view.steps;
  const dots: TimelineDot[] = [{ index: 0, state: view.currentStep === 0 ? "current" : "done", step: null }];
  for (const step of all) {
    dots.push({
      index: step.index,
      state: step.index < view.currentStep ? "done" : step.index === view.currentStep ? "current" : "ghost",
      step,
    });
  }
  return dots;
}

/** Editor link for a storylet; null when the authored key is unknown. */
export function editLink(ref: StoryletRef): string | null {
  return ref.key ? `/storylets/${encodeURIComponent(ref.key)}/edit` : null;
}

export function stepCountText(n: number): string {
  return `${n} ${n === 1 ? "step" : "steps"}`;
}
