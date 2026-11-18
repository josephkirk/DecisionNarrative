import type { EventRecord } from "./contracts.ts";

export function isChronicleEntry(event: EventRecord): boolean {
  return event.event_type === "canon" && typeof event.story_time === "number";
}

export function sortChronicle(events: EventRecord[]): EventRecord[] {
  return events
    .filter(isChronicleEntry)
    .sort((a, b) => (a.story_time as number) - (b.story_time as number) || a.id.localeCompare(b.id));
}
