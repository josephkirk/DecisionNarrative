import { describe, expect, test } from "bun:test";
import { thumbnailInitial } from "../../src/web/studio/thumbnails.ts";

describe("thumbnailInitial", () => {
  test("keeps emoji, flags and combining marks whole", () => {
    expect(thumbnailInitial("🦊 Kitsune")).toBe("🦊");
    expect(thumbnailInitial("🇵🇱 Gdańsk")).toBe("🇵🇱");
    expect(thumbnailInitial("e\u0301clair")).toBe("E\u0301");
  });
  test("uppercases letters and falls back for blank names", () => {
    expect(thumbnailInitial("  jo")).toBe("J");
    expect(thumbnailInitial("   ")).toBe("?");
  });
});
