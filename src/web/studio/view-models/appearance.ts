import { reactive, watch } from "vue";
import { fitWithin, parseAppearance, presetById, THEME_PRESETS, type AppearanceSettings } from "../theme.ts";

const STORAGE_KEY = "dn.studio.appearance";

function readStored(): AppearanceSettings {
  try {
    return parseAppearance(JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null"));
  } catch {
    return parseAppearance(null);
  }
}

export const appearance = reactive<AppearanceSettings>(readStored());

/** Pane opacity over a user image: lower when blurred (glass), higher when the image stays sharp. */
const GLASS_ALPHA = { "image-blur": { pane: 72, sidebar: 66 }, image: { pane: 88, sidebar: 84 } } as const;

/** Writes the preset's variables and backdrop state onto <html> so every surface follows. */
export function applyAppearance(settings: AppearanceSettings = appearance, root: HTMLElement = document.documentElement) {
  const preset = presetById(settings.preset);
  for (const other of THEME_PRESETS) for (const name of Object.keys(other.vars)) root.style.removeProperty(name);
  root.style.removeProperty("--bg-pane");
  root.style.removeProperty("--bg-sidebar");
  for (const [name, value] of Object.entries(preset.vars)) root.style.setProperty(name, value);
  const backdrop = settings.backgroundImage ? (settings.blur ? "image-blur" : "image") : "glow";
  if (backdrop !== "glow") {
    const alpha = GLASS_ALPHA[backdrop];
    root.style.setProperty("--bg-pane", `color-mix(in srgb, var(--bg-app) ${alpha.pane}%, transparent)`);
    root.style.setProperty("--bg-sidebar", `color-mix(in srgb, var(--bg-app) ${alpha.sidebar}%, transparent)`);
  }
  root.dataset.theme = preset.id;
  root.dataset.backdrop = backdrop;
}

/** Persists settings; returns false when the browser refuses (e.g. storage quota for a large image). */
function persist(settings: AppearanceSettings): boolean {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
    return true;
  } catch {
    return false;
  }
}

let storageError: ((message: string) => void) | null = null;
export function onAppearanceStorageError(handler: (message: string) => void) { storageError = handler; }

watch(appearance, (settings) => {
  applyAppearance(settings);
  if (!persist(settings)) storageError?.("The background image is too large to remember on this device; it will reset on reload.");
}, { deep: true });

/** Decodes an image file and downscales it to a JPEG data URL suitable for the backdrop. */
export async function backdropFromFile(file: File): Promise<string> {
  if (!file.type.startsWith("image/")) throw new Error("Choose an image file (PNG, JPEG, WebP or GIF).");
  const bitmap = await createImageBitmap(file);
  try {
    const { width, height } = fitWithin(bitmap.width, bitmap.height);
    if (!width || !height) throw new Error("The selected image has no dimensions.");
    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const context = canvas.getContext("2d");
    if (!context) throw new Error("This browser cannot process images.");
    context.drawImage(bitmap, 0, 0, width, height);
    return canvas.toDataURL("image/jpeg", 0.85);
  } finally {
    bitmap.close();
  }
}
