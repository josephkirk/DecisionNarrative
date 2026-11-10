/** Resolve a stored thumbnail reference to a URL the browser can load. */
export function thumbnailUrl(reference: string | null | undefined): string | null {
  const trimmed = (reference ?? "").trim();
  if (!trimmed) return null;
  if (trimmed.startsWith("http://") || trimmed.startsWith("https://")) {
    return trimmed;
  }
  const path = trimmed
    .split("/")
    .filter((segment) => segment.length > 0)
    .map((segment) => encodeURIComponent(segment))
    .join("/");
  return `/api/v3/files/${path}`;
}

const graphemes = new Intl.Segmenter(undefined, { granularity: "grapheme" });

/** First user-perceived character, so emoji and combined marks never render as a broken half. */
export function thumbnailInitial(label: string): string {
  const first = graphemes.segment(label.trim())[Symbol.iterator]().next().value?.segment;
  return first ? first.toLocaleUpperCase() : "?";
}
