const rules = new Intl.PluralRules("en");
const numbers = new Intl.NumberFormat("en");

/** "1 section", "2 sections", "1,284 steps": grouped digits and the right noun form for the count. */
export function countLabel(count: number, singular: string, plural = `${singular}s`): string {
  return `${numbers.format(count)} ${rules.select(count) === "one" ? singular : plural}`;
}
