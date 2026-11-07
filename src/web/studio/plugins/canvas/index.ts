import { definePlugin } from "../runtime.ts";
import { StudioApi } from "../../api.ts";
import { notify, studioState } from "../../state.ts";
import { refreshWorkspace } from "../../view-models/workspace.ts";
import VectorCanvas from "./VectorCanvas.vue";
import ModifyCanvasInspector from "./ModifyCanvasInspector.vue";
import CanvasPropertySummary from "./CanvasPropertySummary.vue";
import { CANVAS_PROPERTY_KEY, removeNodeVersion } from "./model.ts";
import { updateEntityCanvas } from "./versionStore.ts";
import { modifyCanvasNode } from "./modifyCanvasNode.ts";
import { canvasTraitTemplate, findCanvasTrait, hasCanvasTrait } from "./trait.ts";

export { MODIFY_CANVAS } from "./modifyCanvasNode.ts";
export { CANVAS_TRAIT_KEY } from "./trait.ts";

export const vectorCanvasPlugin = definePlugin({
  name: "vector-canvas",
  version: "1.1.0",
  description: "Adds the Canvas trait. Entities with it get a layered vector canvas with markers, sticky notes and entity/storylet references, plus a Modify Canvas story node.",
  apply(ctx) {
    // Ensure the project has the "Canvas" trait template.
    let creating = false;
    async function ensureTrait() {
      if (creating || !studioState.project || findCanvasTrait(studioState.traitTemplates)) return;
      creating = true;
      try {
        await StudioApi.saveContent("trait_templates", canvasTraitTemplate());
        await refreshWorkspace(["trait_templates"]);
      } catch (error) {
        notify(`Canvas plugin could not create the Canvas trait: ${error instanceof Error ? error.message : String(error)}`, "error");
      } finally {
        creating = false;
      }
    }
    ctx.on("workspace/loaded", () => void ensureTrait());
    void ensureTrait();

    // UI attached to an entity: only entities with the Canvas trait.
    ctx.entityPanel({
      id: "vector-canvas",
      label: "Canvas",
      order: 10,
      appliesTo: (entity) => hasCanvasTrait(entity, studioState.traitTemplates),
      component: VectorCanvas,
    });

    // UI attached to a property.
    ctx.propertyEditor({
      id: "vector-canvas-doc",
      matches: (property) => property.key === CANVAS_PROPERTY_KEY,
      component: CanvasPropertySummary,
    });

    // Graph node that modifies its input and returns core nodes as output.
    // Deleting the node removes the one version it owns from the entity's canvas history.
    ctx.nodeType({
      ...modifyCanvasNode,
      inspector: ModifyCanvasInspector,
      async onDelete(config) {
        if (!config.canvas_entity || !config.version_id) return;
        try {
          await updateEntityCanvas(config.canvas_entity, (doc) => removeNodeVersion(doc, config.version_id));
          notify("Removed the node's canvas version from the entity's history.", "success");
        } catch (error) {
          notify(`Could not remove the canvas version: ${error instanceof Error ? error.message : String(error)}`, "error");
        }
      },
    });
  },
});
