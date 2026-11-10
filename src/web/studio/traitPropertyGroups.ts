export function groupTraitProperties<T extends { category?: string | null }>(properties: T[]): Array<{ category: string; properties: T[] }> {
  const groups = new Map<string, T[]>();
  for (const property of properties) {
    const category = property.category?.trim() ?? "";
    if (!groups.has(category)) groups.set(category, []);
    groups.get(category)!.push(property);
  }
  return Array.from(groups, ([category, items]) => ({ category, properties: items }));
}

export function reorderProperties<T>(items: T[], from: number, to: number): T[] {
  const next = [...items];
  if (from < 0 || to < 0 || from >= next.length || to >= next.length || from === to) return next;
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item!);
  return next;
}
