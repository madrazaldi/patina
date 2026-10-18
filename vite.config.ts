import path from "node:path";
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";

// The Tauri CLI runs `bun run dev` / `bun run build` from the repo root.
export default defineConfig({
  root: "web",
  plugins: [tailwindcss(), svelte({ configFile: "../svelte.config.js" })],
  resolve: { alias: { $lib: path.resolve(import.meta.dirname, "web/src/lib") } },
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
