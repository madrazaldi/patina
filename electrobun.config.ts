import { readFileSync } from "node:fs";
import type { ElectrobunConfig } from "electrobun";

// One version for the whole repo: [workspace.package] in Cargo.toml (tools/release.sh bumps it).
const version = readFileSync(new URL("./Cargo.toml", import.meta.url), "utf8").match(/^version = "([^"]+)"/m)?.[1] ?? "0.0.0";

export default {
  app: {
    name: "Patina",
    identifier: "dev.patina.app",
    version,
    description: "A Plex client that plays everything in mpv",
  },
  build: {
    mainProcess: "bun",
    bun: { entrypoint: "desktop/main.ts" },
    // The webview bundle (`bun run build` writes it to dist/).
    copy: {
      "dist/index.html": "views/app/index.html",
      "dist/assets": "views/app/assets",
    },
    watchIgnore: ["dist/**", "crates/**", "target/**"],
    // Chromium (CEF) bundled on every platform instead of the system webview, so the UI renders the same everywhere.
    mac: { bundleCEF: true, defaultRenderer: "cef", icons: "desktop/icons/icon.iconset", codesign: false, notarize: false },
    linux: { bundleCEF: true, defaultRenderer: "cef", icon: "desktop/icons/icon.png" },
    win: { bundleCEF: true, defaultRenderer: "cef", icon: "desktop/icons/icon.ico" },
  },
  runtime: { exitOnLastWindowClosed: true },
} satisfies ElectrobunConfig;
