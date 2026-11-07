import { StudioApi } from "../../api.ts";
import { notify, studioState } from "../../state.ts";
import { refreshWorkspace } from "../../view-models/workspace.ts";
import { definePlugin } from "../runtime.ts";
import InventoryPanel from "./InventoryPanel.vue";
import { INVENTORY_OWNER_KEY, inventoryOwnerTemplate, itemTemplate, traitInstance } from "./model.ts";
import { inventoryNodes } from "./nodes.ts";

export const inventoryPlugin = definePlugin({
  name: "inventory",
  version: "1.0.0",
  description: "Adds InventoryOwner and Item traits, an inventory grid, and story graph inventory operations.",
  apply(ctx) {
    let creating = false;
    async function ensureTraits() {
      if (creating || !studioState.project) return;
      const templates = [inventoryOwnerTemplate(), itemTemplate()].filter((template) => !studioState.traitTemplates.some((existing) => existing.key === template.key));
      if (!templates.length) return;
      creating = true;
      try {
        for (const template of templates) await StudioApi.saveContent("trait_templates", template);
        await refreshWorkspace(["trait_templates"]);
      } catch (error) {
        notify(`Inventory plugin could not create its traits: ${error instanceof Error ? error.message : String(error)}`, "error");
      } finally { creating = false; }
    }
    ctx.on("workspace/loaded", () => void ensureTraits());
    void ensureTraits();
    ctx.entityPanel({
      id: "inventory", label: "Inventory", order: 20,
      appliesTo: (entity) => !!traitInstance(entity, studioState.traitTemplates, INVENTORY_OWNER_KEY),
      component: InventoryPanel,
    });
    for (const node of inventoryNodes) ctx.nodeType(node);
  },
});
