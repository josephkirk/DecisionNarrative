import { reactive } from "vue";
import type { EntityRecord, TraitTemplateRecord } from "../contracts.ts";
import { traitLabels } from "../entityModel.ts";

/** Picker row id for "no template"; ReferencePicker highlights it through `selectedId`. */
export const NO_TEMPLATE_ID = "";

export interface TemplateChoice {
  id: string;
  title: string;
  subtitle?: string;
  thumbnail?: string | null;
}

export const templatePicker = reactive<{ open: boolean; anchor: HTMLElement | null; items: TemplateChoice[] }>({
  open: false,
  anchor: null,
  items: [],
});

let pending: ((id: string | null) => void) | null = null;

/** Rows for the Choose template picker: the empty-entity row first, then every template entity. */
export function templatePickerItems(templates: EntityRecord[], traitTemplates: TraitTemplateRecord[]): TemplateChoice[] {
  return [
    { id: NO_TEMPLATE_ID, title: "None — empty entity", subtitle: "Start without description, properties or traits" },
    ...templates.map((template) => ({
      id: template.id,
      title: template.name,
      subtitle: traitLabels(template, traitTemplates).join(" · ") || undefined,
      thumbnail: template.thumbnail ?? null,
    })),
  ];
}

/** Opens the picker; resolves with the chosen id (`NO_TEMPLATE_ID` for none) or null when cancelled. */
export function chooseTemplate(items: TemplateChoice[], anchor: HTMLElement | null): Promise<string | null> {
  settleTemplatePicker(null);
  templatePicker.items = items;
  templatePicker.anchor = anchor;
  templatePicker.open = true;
  return new Promise((resolve) => { pending = resolve; });
}

export function settleTemplatePicker(id: string | null): void {
  templatePicker.open = false;
  const resolve = pending;
  pending = null;
  resolve?.(id);
}
