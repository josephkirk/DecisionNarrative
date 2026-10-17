export interface TwineLink {
  display: string;
  target: string;
}

/**
 * Extracts Twine links from passage text conforming to twine-specs:
 * - [[target]]
 * - [[display->target]]
 * - [[target<-display]]
 * - [[display|target]]
 */
export function extractTwineLinks(text: string): TwineLink[] {
  const links: TwineLink[] = [];
  const regex = /\[\[([^\]]+)\]\]/g;
  let match: RegExpExecArray | null;

  while ((match = regex.exec(text)) !== null) {
    const inner = match[1].trim();
    if (!inner) continue;

    if (inner.includes("->")) {
      const [disp, tgt] = inner.split("->");
      links.push({ display: disp.trim(), target: tgt.trim() });
    } else if (inner.includes("<-")) {
      const [tgt, disp] = inner.split("<-");
      links.push({ display: disp.trim(), target: tgt.trim() });
    } else if (inner.includes("|")) {
      const [disp, tgt] = inner.split("|");
      links.push({ display: disp.trim(), target: tgt.trim() });
    } else {
      links.push({ display: inner, target: inner });
    }
  }

  return links;
}

/**
 * Generates an SVG cubic bezier path for a directed arrow between two passage tiles.
 */
export function calculateLinkCurve(
  x1: number,
  y1: number,
  w1: number,
  h1: number,
  x2: number,
  y2: number,
  w2: number,
  h2: number
): string {
  const startX = x1 + w1;
  const startY = y1 + h1 / 2;
  const endX = x2;
  const endY = y2 + h2 / 2;

  const dx = Math.abs(endX - startX) * 0.5;
  const cx1 = startX + Math.max(40, dx);
  const cy1 = startY;
  const cx2 = endX - Math.max(40, dx);
  const cy2 = endY;

  return `M ${startX} ${startY} C ${cx1} ${cy1}, ${cx2} ${cy2}, ${endX} ${endY}`;
}
