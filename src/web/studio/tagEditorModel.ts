import { normalizeTagName, uniqueTagNames } from "./tagData.ts";

export function tagSuggestions(options: string[], selected: string[], query: string, limit = 8): string[] {
  const needle = normalizeTagName(query);
  return uniqueTagNames(options)
    .filter((option) => !selected.some((value) => normalizeTagName(value) === normalizeTagName(option)))
    .filter((option) => !needle || normalizeTagName(option).includes(needle))
    .slice(0, limit);
}

export function tagSuggestionQuery(query: string, selected: string[], multiple: boolean): string {
  const current = selected[0];
  return !multiple && current && normalizeTagName(query) === normalizeTagName(current) ? "" : query;
}

export function addTagSelection(selected: string[], raw: string, options: string[], multiple: boolean): string[] {
  const value = raw.trim();
  if (!normalizeTagName(value)) return uniqueTagNames(selected);
  const canonical = options.find((option) => normalizeTagName(option) === normalizeTagName(value)) ?? value;
  return uniqueTagNames(multiple ? [...selected, canonical] : [canonical]);
}

export function commitSingleTagEdit(selected: string[], query: string, isEditing: boolean, options: string[]): string[] {
  if (!isEditing) return uniqueTagNames(selected);
  return normalizeTagName(query) ? addTagSelection(selected, query, options, false) : [];
}

export function nextTagSuggestionIndex(activeIndex: number, direction: 1 | -1, count: number): number {
  if (count <= 0) return -1;
  if (activeIndex < 0 || activeIndex >= count) return direction === 1 ? 0 : count - 1;
  return (activeIndex + direction + count) % count;
}
