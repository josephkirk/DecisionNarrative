/**
 * Appearance preferences: a game-inspired theme plus an optional backdrop image the
 * floating panes sit on. A theme retunes every design token (surfaces, borders, text,
 * corner radii, heading typography, accent), not just the backdrop. Stored per
 * browser (local-first); never sent to the service.
 */
export interface ThemePreset {
  id: string;
  label: string;
  /** Swatch colors shown in the picker: [backdrop, accent, secondary]. */
  swatch: [string, string, string];
  vars: Record<string, string>;
}

const glow = (a: string, b: string, c: string) =>
  `radial-gradient(1200px 700px at 12% -10%, ${a}, transparent 60%), radial-gradient(900px 600px at 95% 10%, ${b}, transparent 60%), radial-gradient(1000px 800px at 60% 120%, ${c}, transparent 60%)`;

/** Corner-radius scales [xs, sm, md, lg, xl] in px: the single biggest lever on a theme's character. */
const RADII = {
  sharp: [2, 2, 3, 4, 6],
  crisp: [3, 4, 6, 8, 10],
  soft: [4, 6, 9, 12, 16],
  round: [6, 9, 12, 16, 22],
} as const;

const SANS = `"Sora Variable", "Sora", "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif`;
const SERIF = `"EB Garamond", "Iowan Old Style", "Palatino Linotype", Palatino, Georgia, serif`;
/** DIN-like condensed faces that ship with Windows/macOS; falls back to the UI sans. */
const TECHNICAL = `Bahnschrift, "DIN Alternate", "DIN Condensed", "Roboto Condensed", ${SANS}`;

interface PresetSpec {
  id: string;
  label: string;
  bg: string;
  /** Card/panel fill; hover, active, input and popover fills derive from it. */
  surface: string;
  /** "r, g, b" of hairline borders (used at three opacities). */
  border: string;
  text: [main: string, muted: string, dim: string];
  accent: string;
  hover: string;
  /** Text color on accent fills; must keep ≥ 4.5:1 contrast with `accent`. */
  ink: string;
  secondary: string;
  glows: [string, string, string];
  radius: keyof typeof RADII;
  display: { font: string; weight: number; tracking: string; transform?: "uppercase" };
}

function preset(spec: PresetSpec): ThemePreset {
  const [xs, sm, md, lg, xl] = RADII[spec.radius];
  const mix = (a: string, b: string, pct: number) => `color-mix(in srgb, ${a} ${pct}%, ${b})`;
  const fade = (color: string, pct: number) => `color-mix(in srgb, ${color} ${pct}%, transparent)`;
  const [main, muted, dim] = spec.text;
  return {
    id: spec.id,
    label: spec.label,
    swatch: [spec.bg, spec.accent, spec.secondary],
    vars: {
      "--bg-app": spec.bg,
      "--bg-card": spec.surface,
      "--bg-card-hover": mix(spec.surface, "white", 96),
      "--bg-card-active": mix(spec.surface, "white", 91),
      "--bg-badge": mix(spec.surface, "white", 91),
      "--surface-container-high": mix(spec.surface, "white", 91),
      "--surface-container-highest": mix(spec.surface, "white", 85),
      "--bg-input": mix(spec.bg, spec.surface, 55),
      "--bg-elevated": fade(mix(spec.surface, "white", 94), 97),
      "--bg-pane": fade(mix(spec.bg, spec.surface, 70), 88),
      "--bg-sidebar": fade(mix(spec.bg, spec.surface, 60), 84),
      "--border-subtle": `rgba(${spec.border}, 0.13)`,
      "--border-muted": `rgba(${spec.border}, 0.22)`,
      "--border-strong": `rgba(${spec.border}, 0.32)`,
      "--border-faint": `rgba(${spec.border}, 0.08)`,
      "--outline-variant": `rgba(${spec.border}, 0.2)`,
      "--text-main": main,
      "--text-primary": main,
      "--text-muted": muted,
      "--text-dim": dim,
      "--accent": spec.accent,
      "--accent-hover": spec.hover,
      "--accent-subtle": fade(spec.accent, 14),
      "--accent-ink": spec.ink,
      "--accent-2": spec.secondary,
      "--border-focus": spec.hover,
      "--backdrop-glow": glow(...spec.glows),
      "--radius-xs": `${xs}px`,
      "--radius-sm": `${sm}px`,
      "--radius-md": `${md}px`,
      "--radius-lg": `${lg}px`,
      "--radius-xl": `${xl}px`,
      "--font-display": spec.display.font,
      "--display-weight": String(spec.display.weight),
      "--display-tracking": spec.display.tracking,
      "--display-transform": spec.display.transform ?? "none",
    },
  };
}

/** Game-inspired themes; the first (Kojima Productions) is the default studio look. */
export const THEME_PRESETS: ThemePreset[] = [
  // Kojima Productions: ink black, warm paper white, restrained gilt; no coloured haze.
  preset({ id: "kojima", label: "Kojima Productions", bg: "#0b0b0a", surface: "#161614", border: "232, 222, 200",
    text: ["#f1ede4", "#a8a397", "#8c877d"], accent: "#c9a24a", hover: "#dfbf75", ink: "#140f03", secondary: "#f1ede4",
    glows: ["rgba(241, 237, 228, 0.05)", "rgba(201, 162, 74, 0.06)", "rgba(201, 162, 74, 0.04)"],
    radius: "crisp", display: { font: SANS, weight: 400, tracking: "0.02em" } }),
  // Mass Effect: deep-space navy, holographic cyan, Normandy indigo; soft glass.
  preset({ id: "mass-effect", label: "Mass Effect", bg: "#0a0c12", surface: "#161b28", border: "148, 163, 196",
    text: ["#eef1f8", "#9ba5bd", "#838da2"], accent: "#22c5e0", hover: "#67e3f5", ink: "#04161c", secondary: "#6366f1",
    glows: ["rgba(99, 102, 241, 0.16)", "rgba(34, 197, 224, 0.09)", "rgba(168, 85, 247, 0.10)"],
    radius: "soft", display: { font: SANS, weight: 600, tracking: "-0.025em" } }),
  // Cyberpunk: night-city black, signal yellow, neon magenta haze; hard edges, DIN caps.
  preset({ id: "cyberpunk", label: "Cyberpunk", bg: "#0b0a0f", surface: "#16141d", border: "5, 217, 232",
    text: ["#f4f1e1", "#a9a5b8", "#89859a"], accent: "#f3e600", hover: "#fff35c", ink: "#141100", secondary: "#ff2a6d",
    glows: ["rgba(255, 42, 109, 0.16)", "rgba(5, 217, 232, 0.10)", "rgba(243, 230, 0, 0.06)"],
    radius: "sharp", display: { font: TECHNICAL, weight: 700, tracking: "0.04em", transform: "uppercase" } }),
  // RPG: candle-lit tavern, gilded gold, parchment text, ember and forest; serif headings.
  preset({ id: "rpg", label: "RPG", bg: "#110d09", surface: "#1e1812", border: "217, 180, 120",
    text: ["#f3ead8", "#b5a68b", "#988971"], accent: "#d9a441", hover: "#ecc06c", ink: "#1d1405", secondary: "#8b5a2b",
    glows: ["rgba(180, 83, 9, 0.16)", "rgba(217, 164, 65, 0.08)", "rgba(22, 101, 52, 0.12)"],
    radius: "soft", display: { font: SERIF, weight: 600, tracking: "0" } }),
  // Death Stranding: cold timefall slate, chiral amber, teal mist; thin, wide-tracked caps.
  preset({ id: "death-stranding", label: "Death Stranding", bg: "#0a0d10", surface: "#141a1f", border: "160, 190, 200",
    text: ["#e9eef0", "#98a7ae", "#7f8d94"], accent: "#f2a54a", hover: "#f8c27f", ink: "#1e1103", secondary: "#4fb3bf",
    glows: ["rgba(79, 179, 191, 0.12)", "rgba(148, 163, 184, 0.08)", "rgba(242, 165, 74, 0.08)"],
    radius: "crisp", display: { font: SANS, weight: 400, tracking: "0.08em", transform: "uppercase" } }),
  // Anime: twilight violet, sakura pink, sky blue; round, bold and friendly.
  preset({ id: "anime", label: "Anime", bg: "#100c16", surface: "#1d1627", border: "220, 180, 255",
    text: ["#fbf3ff", "#bba9cb", "#9484a4"], accent: "#ff7ab6", hover: "#ffa6cd", ink: "#2a0817", secondary: "#7cc7ff",
    glows: ["rgba(255, 122, 182, 0.14)", "rgba(124, 199, 255, 0.10)", "rgba(139, 92, 246, 0.14)"],
    radius: "round", display: { font: SANS, weight: 700, tracking: "-0.02em" } }),
  // Modern Warfare: tactical night, NVG green, sand; squared panels, stencil-like caps.
  preset({ id: "modern-warfare", label: "Modern Warfare", bg: "#0a0c09", surface: "#151913", border: "180, 190, 150",
    text: ["#e8eadf", "#a3a791", "#888c77"], accent: "#9ccc3d", hover: "#bde36a", ink: "#0f1603", secondary: "#c2a36b",
    glows: ["rgba(101, 113, 59, 0.18)", "rgba(156, 204, 61, 0.06)", "rgba(194, 163, 107, 0.08)"],
    radius: "sharp", display: { font: TECHNICAL, weight: 600, tracking: "0.06em", transform: "uppercase" } }),
];

export const DEFAULT_PRESET = THEME_PRESETS[0]!.id;

export interface AppearanceSettings {
  preset: string;
  /** Downscaled image as a data URL, or null for the preset's glow backdrop. */
  backgroundImage: string | null;
  /** Blur the backdrop image so pane content stays legible. */
  blur: boolean;
}

export function defaultAppearance(): AppearanceSettings {
  return { preset: DEFAULT_PRESET, backgroundImage: null, blur: true };
}

export function presetById(id: string): ThemePreset {
  return THEME_PRESETS.find((preset) => preset.id === id) ?? THEME_PRESETS[0]!;
}

/** Accepts stored settings field by field; anything invalid falls back to its default. */
export function parseAppearance(raw: unknown): AppearanceSettings {
  const fallback = defaultAppearance();
  if (!raw || typeof raw !== "object") return fallback;
  const value = raw as Record<string, unknown>;
  const image = typeof value.backgroundImage === "string" && value.backgroundImage.startsWith("data:image/") ? value.backgroundImage : null;
  return {
    preset: typeof value.preset === "string" && THEME_PRESETS.some((preset) => preset.id === value.preset) ? value.preset : fallback.preset,
    backgroundImage: image,
    blur: typeof value.blur === "boolean" ? value.blur : fallback.blur,
  };
}

/** Longest edge for stored backdrops; large enough for 4K-ish screens once blurred, small enough for localStorage. */
export const BACKDROP_MAX_EDGE = 1920;

export function fitWithin(width: number, height: number, maxEdge = BACKDROP_MAX_EDGE): { width: number; height: number } {
  if (width <= 0 || height <= 0) return { width: 0, height: 0 };
  const scale = Math.min(1, maxEdge / Math.max(width, height));
  return { width: Math.round(width * scale), height: Math.round(height * scale) };
}
