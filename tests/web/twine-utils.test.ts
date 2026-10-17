import { expect, test } from "bun:test";
import { extractTwineLinks, calculateLinkCurve } from "../../src/web/studio/twineUtils.ts";

test("extractTwineLinks parses all standard Twine link syntaxes", () => {
  const text = `
    Welcome to the cave.
    You can go [[NorthRoom]].
    Or [[Open the Door->Doorway]].
    Or [[SecretPassage<-Inspect Bookcase]].
    Or [[Climb the ladder|AtticRoom]].
  `;

  const links = extractTwineLinks(text);
  expect(links).toHaveLength(4);

  expect(links[0]).toEqual({ display: "NorthRoom", target: "NorthRoom" });
  expect(links[1]).toEqual({ display: "Open the Door", target: "Doorway" });
  expect(links[2]).toEqual({ display: "Inspect Bookcase", target: "SecretPassage" });
  expect(links[3]).toEqual({ display: "Climb the ladder", target: "AtticRoom" });
});

test("calculateLinkCurve produces a valid cubic bezier path", () => {
  const path = calculateLinkCurve(100, 100, 120, 80, 400, 200, 120, 80);
  expect(path).toStartWith("M 220 140 C");
  expect(path).toContain("400 240");
});
