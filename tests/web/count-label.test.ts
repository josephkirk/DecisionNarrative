import { describe, expect, test } from "bun:test";
import { countLabel } from "../../src/web/studio/countLabel.ts";

describe("countLabel", () => {
  test("uses the singular only for exactly one", () => {
    expect(countLabel(1, "section")).toBe("1 section");
    expect(countLabel(0, "section")).toBe("0 sections");
    expect(countLabel(2, "property", "properties")).toBe("2 properties");
  });
  test("groups large counts", () => {
    expect(countLabel(1284000, "character")).toBe("1,284,000 characters");
  });
});
