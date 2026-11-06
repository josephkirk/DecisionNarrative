// Pure helpers for the canon chronicle (events that happened, in story-time order).
import type { EventRecord } from "./contracts.ts";

/** A canon event with a story time is a chronicle entry; anything else is not. */
export function isChronicleEntry(event: EventRecord): boolean {
  return event.event_type === "canon" && typeof event.story_time === "number";
}

/** Chronicle entries only, ordered by story time and then id. */
export function sortChronicle(events: EventRecord[]): EventRecord[] {
  return events
    .filter(isChronicleEntry)
    .sort((a, b) => (a.story_time as number) - (b.story_time as number) || a.id.localeCompare(b.id));
}
