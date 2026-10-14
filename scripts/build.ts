/**
 * scripts/build.ts
 * Builds the frontend client bundle and verifies assets.
 */

import * as fs from "node:fs";

console.log("Building generic storylet studio bundle...");
const result = await Bun.build({
  entrypoints: ["./src/web/client.ts"],
  outdir: "./public/dist",
  target: "browser",
  minify: true
});

if (!result.success) {
  console.error("Build failed:", result.logs);
  process.exit(1);
}

console.log(`Bundle created successfully: public/dist/client.js (${fs.statSync("./public/dist/client.js").size} bytes)`);
