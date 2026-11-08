import { StudioApi } from "../../api.ts";
import { notify, studioState } from "../../state.ts";
import { refreshWorkspace } from "../../view-models/workspace.ts";
import { definePlugin } from "../runtime.ts";
import TreeGraphPanel from "./TreeGraphPanel.vue";
import { TREE_GRAPH_KEY, treeGraphTemplate, treeItemTemplate, treeTrait } from "./model.ts";

export const treeGraphPlugin = definePlugin({
  name: "tree-graph",
  version: "1.0.0",
  description: "Adds TreeGraph and TreeItem traits for visual graph authoring.",
  apply(ctx) {
    let creating = false;
    async function ensureTraits() {
      if (creating || !studioState.project) return;
      const graphTemplate = treeGraphTemplate();
      const existing = studioState.traitTemplates.find((template) => template.key === TREE_GRAPH_KEY);
      const templates = [graphTemplate, treeItemTemplate()].filter((template) => !studioState.traitTemplates.some((candidate) => candidate.key === template.key));
      if (existing && !existing.properties.some((property) => property.key === "shapes")) {
        templates.push({ ...existing, properties: [...existing.properties, graphTemplate.properties.find((property) => property.key === "shapes")!] });
      }
      if (!templates.length) return;
      creating = true;
      try {
        for (const template of templates) await StudioApi.saveContent("trait_templates", template);
        await refreshWorkspace(["trait_templates"]);
      } catch (error) {
        notify(`Tree graph plugin could not create its traits: ${error instanceof Error ? error.message : String(error)}`, "error");
      } finally { creating = false; }
    }
    ctx.on("workspace/loaded", () => void ensureTraits());
    void ensureTraits();
    ctx.entityPanel({
      id: "tree-graph", label: "Tree", order: 30,
      appliesTo: (entity) => !!treeTrait(entity, studioState.traitTemplates, TREE_GRAPH_KEY),
      component: TreeGraphPanel,
    });
  },
});
