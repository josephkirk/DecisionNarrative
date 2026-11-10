import { StudioApi } from "../../api.ts";
import type { TraitTemplateRecord } from "../../contracts.ts";
import { notify, studioState } from "../../state.ts";
import { refreshWorkspace } from "../../view-models/workspace.ts";
import { definePlugin } from "../runtime.ts";
import TreeGraphPanel from "./TreeGraphPanel.vue";
import TreeDefaultsEditor from "./TreeDefaultsEditor.vue";
import TreePointerInspector from "./TreePointerInspector.vue";
import { isTreePositionNode, setTreePositionNode } from "./pointerNodes.ts";
import { TREE_GRAPH_KEY, treeGraphTemplate, treeItemTemplate, treeTrait } from "./model.ts";

export const treeGraphPlugin = definePlugin({
  name: "tree-graph",
  version: "1.0.0",
  description: "Adds TreeGraph and TreeItem traits for visual graph authoring.",
  apply(ctx) {
    let creating = false;
    async function ensureTraits() {
      if (creating || !studioState.project) return;
      creating = true;
      try {
        // Decide from the stored list: workspace state may not have loaded trait templates yet, and a stale
        // empty list would make this overwrite user-edited traits with defaults.
        const stored = await StudioApi.listContent<TraitTemplateRecord>("trait_templates");
        const defaults = [treeGraphTemplate(), treeItemTemplate()];
        const templates = defaults.flatMap((template) => {
          const existing = stored.find((candidate) => candidate.key === template.key);
          if (!existing) return [template];
          const missing = template.properties.filter((property) => !existing.properties.some((candidate) => candidate.key === property.key));
          return missing.length ? [{ ...existing, properties: [...existing.properties, ...missing] }] : [];
        });
        if (!templates.length) return;
        for (const template of templates) await StudioApi.saveContent("trait_templates", template);
        await refreshWorkspace(["trait_templates"]);
      } catch (error) {
        notify(`Tree graph plugin could not create its traits: ${error instanceof Error ? error.message : String(error)}`, "error");
      } finally { creating = false; }
    }
    ctx.on("workspace/loaded", () => void ensureTraits());
    void ensureTraits();
    ctx.nodeType({ ...setTreePositionNode, inspector: TreePointerInspector });
    ctx.nodeType({ ...isTreePositionNode, inspector: TreePointerInspector });
    ctx.entityPanel({
      id: "tree-graph", label: "Tree", order: 30,
      appliesTo: (entity) => !!treeTrait(entity, studioState.traitTemplates, TREE_GRAPH_KEY),
      component: TreeGraphPanel,
    });
    ctx.traitInspector({
      id: "tree-defaults",
      traitKey: TREE_GRAPH_KEY,
      component: TreeDefaultsEditor,
    });
  },
});
