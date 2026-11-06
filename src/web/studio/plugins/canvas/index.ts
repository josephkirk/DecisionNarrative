import { definePlugin } from "../runtime.ts";
import VectorCanvas from "./VectorCanvas.vue";
import ModifyCanvasInspector from "./ModifyCanvasInspector.vue";
import CanvasPropertySummary from "./CanvasPropertySummary.vue";
import { CANVAS_PROPERTY_KEY } from "./model.ts";
import { modifyCanvasNode } from "./modifyCanvasNode.ts";

export { MODIFY_CANVAS } from "./modifyCanvasNode.ts";

export const vectorCanvasPlugin = definePlugin({
  name: "vector-canvas",
  apply(ctx) {
    // UI attached to an entity.
    ctx.entityPanel({
      id: "vector-canvas",
      label: "Map Canvas",
      order: 10,
      appliesTo: (entity) => entity.id !== "world",
      component: VectorCanvas,
    });

    // UI attached to a property.
    ctx.propertyEditor({
      id: "vector-canvas-doc",
      matches: (property) => property.key === CANVAS_PROPERTY_KEY,
      component: CanvasPropertySummary,
    });

    // Graph node that modifies its input and returns core nodes as output.
    ctx.nodeType({ ...modifyCanvasNode, inspector: ModifyCanvasInspector });
  },
});
