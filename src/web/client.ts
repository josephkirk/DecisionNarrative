import { DesignerStudioController } from "./designer.ts";

window.addEventListener("DOMContentLoaded", async () => {
  const studio = new DesignerStudioController();
  await studio.init();
});
