// Pure helpers over a RunView. No Vue, no fetch: everything here is unit-testable.
import type { ChronicleRowView, NearMiss, RunAction, RunView } from "./contracts.ts";

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
