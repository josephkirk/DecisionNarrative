// Pure helpers over a RunView. No Vue, no fetch: everything here is unit-testable.
import type { ChangeView, ChronicleRowView, KnowKind, KnowRowView, MoveBlock, NearMiss, ParticipantView, RunAction, RunStep, RunView, StoryletRef } from "./contracts.ts";

/** "1".."9" -> the nth choice's action. Disabled choices and any other key give null. */
export function choiceForKey(view: RunView, key: string): RunAction | null {
  if (!/^[1-9]$/.test(key)) return null;
  const choice = view.scene?.choices[Number(key) - 1];
  return choice && choice.enabled ? choice.action : null;
}

/** After "G": the nth (1-based) enabled move out of the current place. */
export function moveForKey(view: RunView, n: number): RunAction | null {
  const moves = (view.place?.moves ?? []).filter((m) => m.enabled);
  const move = moves[n - 1];
  return move ? { kind: "move", to: move.to } : null;
}

/** The clock row appears only for projects that define a clock. */
export function hasClock(view: RunView): boolean {
  return !!view.place?.clockLabel;
}

/** Seats: participants a person plays. Rule participants act on their own. */
export function humanParticipants(view: RunView): ParticipantView[] {
  return view.participants.filter((p) => p.human);
}

/** Tabs appear only when more than one human participant exists. */
export function showParticipantTabs(view: RunView): boolean {
  return humanParticipants(view).length > 1;
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

const KIND_LABELS: Record<KnowKind, string> = {
  evidence: "evidence", witnessed: "witnessed", told: "told", rumor: "rumour",
  recollection: "recollection", testimony: "testimony", inference: "inference",
};
export function knowKindLabel(kind: KnowKind): string {
  return KIND_LABELS[kind];
}

/** Rumour and testimony may be wrong; the panel says so in words. */
export function knowRowStatus(row: KnowRowView): string | null {
  return row.confirmed ? null : "not confirmed";
}

export function changeText(change: ChangeView): string {
  return `${change.key}: ${change.before} → ${change.after}`;
}

const CIRCLED = "①②③④⑤⑥⑦⑧⑨⑩⑪⑫⑬⑭⑮⑯⑰⑱⑲⑳";
/** ①②③ for the first twenty tellings, "(n)" after that, empty when untold. */
export function toldMark(n: number | null): string {
  if (n === null) return "";
  return n >= 1 && n <= 20 ? CIRCLED[n - 1] : `(${n})`;
}

/** Why a move is unavailable, in plain words. */
export function moveBlockText(block: MoveBlock): string {
  if (block.kind === "closed_until") return `Closed until ${block.slotLabel}`;
  return `Needs: ${block.fact.replace(/^fact\./, "").replace(/[_.]/g, " ")}`;
}

export function actionsLeftText(left: number | undefined): string {
  if (left === undefined) return "";
  if (left === 0) return "No actions left";
  return `${left} ${left === 1 ? "action" : "actions"} left`;
}

/** What the world did because of a step: occupancy notes, then rule-participant plays. */
export function noteLines(step: RunStep, participants: ParticipantView[]): string[] {
  const names = new Map(participants.map((p) => [p.id, p.name]));
  return [
    ...(step.notes ?? []),
    ...(step.rulePlays ?? []).map((play) => `${names.get(play.participant) ?? "Someone"}: ${play.storylet.title}`),
  ];
}

/** Storylets listed under "Also here", as playable actions; empty unless a hub is pending. */
export function alsoHereActions(view: RunView): Array<{ storylet: StoryletRef; action: RunAction }> {
  const hub = view.hub;
  if (hub === undefined) return [];
  return view.alsoHere.map((storylet) => ({ storylet, action: { kind: "play_storylet", nodeGuid: hub, storylet: storylet.id } }));
}

/** Normalize ending counts within the endings that were reached. */
export function endingBars(endings: Array<[number, number]>): Array<{ id: number; count: number; percent: number }> {
  const total = endings.reduce((sum, [, count]) => sum + count, 0);
  return endings.map(([id, count]) => ({ id, count, percent: total ? count * 100 / total : 0 }));
}

export function invariantSeverity(report: { violations: unknown[] }): "pass" | "fail" {
  return report.violations.length ? "fail" : "pass";
}
