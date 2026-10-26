import path from "node:path";
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";
import { electrobunViteAliases } from "./.hutch/devkit/api/config/electrobun-vite.ts";

// `bun run build` writes the webview bundle to dist/; Electrobun copies it into the app (electrobun.config.ts).
// `electrobun/view` resolves into the devkit that `electrobun prepare` projects into .hutch/.
export default defineConfig({
  root: "web",
  plugins: [tailwindcss(), svelte({ configFile: "../svelte.config.js" })],
  resolve: {
    alias: [
      { find: "$lib", replacement: path.resolve(import.meta.dirname, "web/src/lib") },
      ...electrobunViteAliases(path.resolve(import.meta.dirname, ".hutch/devkit")),
    ],
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/crates/**", "**/target/**"] },
  },
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    target: "safari16",
  },
});
