import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: "./tests", fullyParallel: true, use: { baseURL: "http://127.0.0.1:1421", viewport: { width: 1280, height: 900 } },
  webServer: { command: "bun x vite --host 127.0.0.1 --port 1421", url: "http://127.0.0.1:1421/test/", reuseExistingServer: !process.env.CI },
});
