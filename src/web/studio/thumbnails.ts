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
  return `/api/v2/files/${path}`;
}

export function thumbnailInitial(label: string): string {
  const trimmed = label.trim();
  return trimmed ? trimmed.charAt(0).toUpperCase() : "?";
}
