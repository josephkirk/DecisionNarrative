import { describe, expect, test } from "vitest";
import { appliedTemplates, newEntityTemplateDraft, newTraitInstance, rootTagColor, rootTagLabel } from "../../src/web/studio/entityModel.ts";
import type { TraitTemplateRecord } from "../../src/web/studio/contracts.ts";
const template: TraitTemplateRecord = { id: "t", key: "custom", name: "Custom", properties: [{ id: "p", key: "payload", name: "Payload", label: "Payload", type: "json", is_array: false, default: { value: 1 } }], allow_multiple: true };
describe("entity template model", () => {
 test("draft uses catalog root and preserves identity fields", () => { const draft = newEntityTemplateDraft("folder", "custom.root"); expect(draft).toMatchObject({ root_tag: "custom.root", folder_id: "folder", game_id: "", game_id_auto: true }); });
 test("stable repeated trait instances retain distinct IDs and JSON values", () => { const a = newTraitInstance(template); const b = newTraitInstance(template); expect(a.id).not.toBe(b.id); expect(a.values.payload).toEqual({ value: 1 }); const entity = { ...newEntityTemplateDraft(null), traits: [a, b] }; expect(appliedTemplates(entity, [template])).toHaveLength(2); });
 test("root labels and colors are catalog driven", () => { expect(rootTagLabel("custom.root")).toBe("Root"); expect(rootTagColor("unknown")).toBe("var(--type-other)"); });
});
