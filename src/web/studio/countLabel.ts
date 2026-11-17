const rules = new Intl.PluralRules("en");
const numbers = new Intl.NumberFormat("en");

export function countLabel(count: number, singular: string, plural = `${singular}s`): string {
  return `${numbers.format(count)} ${rules.select(count) === "one" ? singular : plural}`;
}
