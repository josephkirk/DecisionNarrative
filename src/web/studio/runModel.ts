import type { ChangeView, ChronicleRowView, KnowKind, KnowRowView, MoveBlock, NearMiss, ParticipantView, RunAction, RunStep, RunView, StoryletRef } from "./contracts.ts";

export function choiceForKey(view: RunView, key: string): RunAction | null {
  if (!/^[1-9]$/.test(key)) return null;
  const choice = view.scene?.choices[Number(key) - 1];
  return choice && choice.enabled ? choice.action : null;
}

export function moveForKey(view: RunView, n: number): RunAction | null {
  const moves = (view.place?.moves ?? []).filter((m) => m.enabled);
  const move = moves[n - 1];
  return move ? { kind: "move", to: move.to } : null;
}

export function hasClock(view: RunView): boolean {
  return !!view.place?.clockLabel;
}

export function humanParticipants(view: RunView): ParticipantView[] {
  return view.participants.filter((p) => p.human);
}

export function showParticipantTabs(view: RunView): boolean {
  return humanParticipants(view).length > 1;
}

export function nearMissText(m: NearMiss): string {
  return `${m.storylet.title} — ${m.reason}`;
}

export function chronicleRows(rows: ChronicleRowView[]): Array<{ label: string; told: number | null; place: string }> {
  return [...rows]
    .sort((a, b) => a.storyTime - b.storyTime)
    .map((r) => ({ label: r.label, told: r.told, place: r.place ?? "" }));
}

export function stepText(step: RunStep): string {
  switch (step.action.kind) {
    case "choose": return `Chose option ${step.action.outputPin + 1}`;
    case "play_storylet": return "Started a storylet";
    case "topic": return "Asked about a topic";
    case "move": return "Moved";
    case "wait": return "Waited";
  }
}

export function activeBranchId(view: RunView): number {
  const last = view.steps[view.steps.length - 1];
  return last ? last.branch : (view.branches[0]?.id ?? 0);
}

export interface TimelineDot { index: number; state: "done" | "current" | "ghost"; step: RunStep | null }

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

export function knowRowStatus(row: KnowRowView): string | null {
  return row.confirmed ? null : "not confirmed";
}

export function changeText(change: ChangeView): string {
  const delta = change.delta;
  const pieces = [
    Array.isArray(delta.graphDiffs) && delta.graphDiffs.length ? `${delta.graphDiffs.length} world graph change${delta.graphDiffs.length === 1 ? "" : "s"}` : "",
    Array.isArray(delta.participantMoves) && delta.participantMoves.length ? `${delta.participantMoves.length} participant move${delta.participantMoves.length === 1 ? "" : "s"}` : "",
    Array.isArray(delta.knowledgeAdded) && delta.knowledgeAdded.length ? `${delta.knowledgeAdded.length} knowledge change${delta.knowledgeAdded.length === 1 ? "" : "s"}` : "",
    Array.isArray(delta.tellingsAdded) && delta.tellingsAdded.length ? `${delta.tellingsAdded.length} information transfer${delta.tellingsAdded.length === 1 ? "" : "s"}` : "",
  ].filter(Boolean);
  return pieces.join(" · ") || "World state updated";
}

const CIRCLED = "①②③④⑤⑥⑦⑧⑨⑩⑪⑫⑬⑭⑮⑯⑰⑱⑲⑳";
export function toldMark(n: number | null): string {
  if (n === null) return "";
  return n >= 1 && n <= 20 ? CIRCLED[n - 1] : `(${n})`;
}

export function moveBlockText(block: MoveBlock): string {
  if (block.kind === "closed_until") return `Closed until ${block.slotLabel}`;
  return `Needs: ${block.fact.replace(/^fact\./, "").replace(/[_.]/g, " ")}`;
}

export function actionsLeftText(left: number | undefined): string {
  if (left === undefined) return "";
  if (left === 0) return "No actions left";
  return `${left} ${left === 1 ? "action" : "actions"} left`;
}

export function noteLines(step: RunStep, participants: ParticipantView[]): string[] {
  const names = new Map(participants.map((p) => [p.id, p.name]));
  return [
    ...(step.notes ?? []),
    ...(step.rulePlays ?? []).map((play) => `${names.get(play.participant) ?? "Someone"}: ${play.storylet.title}`),
  ];
}

export function alsoHereActions(view: RunView): Array<{ storylet: StoryletRef; action: RunAction }> {
  const hub = view.hub;
  if (hub === undefined) return [];
  return view.alsoHere.map((storylet) => ({ storylet, action: { kind: "play_storylet", nodeGuid: hub, storylet: storylet.id } }));
}

export function endingBars(endings: Array<[number, number]>): Array<{ id: number; count: number; percent: number }> {
  const total = endings.reduce((sum, [, count]) => sum + count, 0);
  return endings.map(([id, count]) => ({ id, count, percent: total ? count * 100 / total : 0 }));
}

export function invariantSeverity(report: { violations: unknown[] }): "pass" | "fail" {
  return report.violations.length ? "fail" : "pass";
}
