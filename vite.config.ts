import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// The Tauri CLI runs `bun run dev` / `bun run build` from the repo root.
export default defineConfig({
  root: "web",
  plugins: [svelte({ configFile: "../svelte.config.js" })],
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
