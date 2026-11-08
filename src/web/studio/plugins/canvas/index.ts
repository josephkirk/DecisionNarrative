import { definePlugin } from "../runtime.ts";
import { StudioApi } from "../../api.ts";
import { notify, studioState } from "../../state.ts";
import { refreshWorkspace } from "../../view-models/workspace.ts";
import VectorCanvas from "./VectorCanvas.vue";
import ModifyCanvasInspector from "./ModifyCanvasInspector.vue";
import CanvasPropertySummary from "./CanvasPropertySummary.vue";
import { CANVAS_PROPERTY_KEY, commitStoryletVersions, discardStoryletVersions, hasTempVersions, stageNodeVersionRemoval } from "./model.ts";
import { updateCanvases, updateEntityCanvas } from "./versionStore.ts";
import { MODIFY_CANVAS, modifyCanvasNode } from "./modifyCanvasNode.ts";
import { canvasTraitTemplate, findCanvasTrait, hasCanvasTrait } from "./trait.ts";

export { MODIFY_CANVAS } from "./modifyCanvasNode.ts";
export { CANVAS_TRAIT_KEY } from "./trait.ts";

export const vectorCanvasPlugin = definePlugin({
  name: "vector-canvas",
  version: "1.1.0",
  description: "Adds the Canvas trait. Entities with it get a layered vector canvas with markers, sticky notes and entity/storylet references, plus a Modify Canvas story node.",
  apply(ctx) {
    // Ensure the project has the "Canvas" trait.
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
    // Deleting the node stages removal of the version it owns (temp history).
    ctx.nodeType({
      ...modifyCanvasNode,
      inspector: ModifyCanvasInspector,
      async onDelete(config) {
        if (!config.canvas_entity || !config.version_id) return;
        try {
          await updateEntityCanvas(config.canvas_entity, (doc) => stageNodeVersionRemoval(doc, config.version_id));
          notify("The node's canvas version will be removed when you save the storylet.", "success");
        } catch (error) {
          notify(`Could not stage the canvas version removal: ${error instanceof Error ? error.message : String(error)}`, "error");
        }
      },
    });

    // Temp history: commit staged canvas versions when the storylet is saved,
    // discard them when the storylet is closed or reopened unsaved.
    const report = (what: string) => (error: unknown) =>
      notify(`Canvas plugin could not ${what}: ${error instanceof Error ? error.message : String(error)}`, "error");
    ctx.on("storylet/saved", (storyletId, graph) => {
      const nodeIds = new Set(graph.nodes.filter((node) => node.type === "plugin" && node.plugin_type === MODIFY_CANVAS).map((node) => node.id));
      void updateCanvases(
        (doc) => doc.revisions.some((revision) => revision.source && (revision.source.storylet_id === storyletId || revision.source.storylet_id === "")),
        (doc) => commitStoryletVersions(doc, storyletId, nodeIds),
      ).catch(report("commit canvas versions"));
    });
    const discard = (storyletId: string) => void updateCanvases(
      (doc) => hasTempVersions(doc, storyletId),
      (doc) => discardStoryletVersions(doc, storyletId),
    ).catch(report("discard unsaved canvas versions"));
    ctx.on("storylet/opened", discard);
    ctx.on("storylet/closed", discard);
  },
});
