import { expect, test } from "bun:test";
import { DEFAULT_PRESET, fitWithin, parseAppearance, presetById, THEME_PRESETS } from "../../src/web/studio/theme.ts";

function luminance(hex: string): number {
  const channel = (offset: number) => {
    const value = parseInt(hex.slice(offset, offset + 2), 16) / 255;
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5);
}
function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}

test("presets cover the requested game themes with unique ids", () => {
  expect(THEME_PRESETS.map((preset) => preset.label)).toEqual(["Mass Effect", "Cyberpunk", "RPG", "Death Stranding", "Kojima Productions", "Anime", "Modern Warfare"]);
  expect(new Set(THEME_PRESETS.map((preset) => preset.id)).size).toBe(THEME_PRESETS.length);
});

test("every preset keeps body and secondary text readable on its cards", () => {
  for (const preset of THEME_PRESETS) {
    const card = preset.vars["--bg-card"]!;
    expect(contrast(preset.vars["--text-main"]!, card), `${preset.id} main text`).toBeGreaterThanOrEqual(7);
    expect(contrast(preset.vars["--text-muted"]!, card), `${preset.id} muted text`).toBeGreaterThanOrEqual(4.5);
  }
});

test("every preset keeps primary-button text readable and its accent visible on the backdrop", () => {
  for (const preset of THEME_PRESETS) {
    const accent = preset.vars["--accent"]!, ink = preset.vars["--accent-ink"]!, bg = preset.vars["--bg-app"]!;
    expect(contrast(accent, ink), `${preset.id} ink on accent`).toBeGreaterThanOrEqual(4.5);
    expect(contrast(accent, bg), `${preset.id} accent on backdrop`).toBeGreaterThanOrEqual(3);
  }
});

test("stored appearance is validated field by field", () => {
  expect(parseAppearance({ preset: "cyberpunk", backgroundImage: "data:image/jpeg;base64,AAAA", blur: false }))
    .toEqual({ preset: "cyberpunk", backgroundImage: "data:image/jpeg;base64,AAAA", blur: false });
  expect(parseAppearance({ preset: "unknown", backgroundImage: "https://example.com/x.png", blur: "yes" }))
    .toEqual({ preset: DEFAULT_PRESET, backgroundImage: null, blur: true });
  expect(parseAppearance(null)).toEqual({ preset: DEFAULT_PRESET, backgroundImage: null, blur: true });
  expect(presetById("missing").id).toBe(DEFAULT_PRESET);
});

test("backdrop images are downscaled to the longest-edge limit, never upscaled", () => {
  expect(fitWithin(3840, 2160)).toEqual({ width: 1920, height: 1080 });
  expect(fitWithin(1000, 4000)).toEqual({ width: 480, height: 1920 });
  expect(fitWithin(800, 600)).toEqual({ width: 800, height: 600 });
  expect(fitWithin(0, 600)).toEqual({ width: 0, height: 0 });
});
