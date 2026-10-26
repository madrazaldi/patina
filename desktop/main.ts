// Patina's desktop shell: the Electrobun main process. The whole backend (Plex
// client, sign-in, mpv, downloads) runs in this process as TypeScript; the
// webview in web/ talks to it over Electrobun's browser RPC (see rpc.ts and
// web/src/lib/ipc.ts).

import { BrowserView, BrowserWindow, Updater, Utils } from "electrobun/main";
import { electrobunEventEmitter } from "electrobun/main/events";
import { logger } from "./core/log";
import * as images from "./core/images";
import { setDebugBuild } from "./core/ops/setup";
import * as player from "./core/player";
import * as session from "./core/session";
import { AppState } from "./core/state";
import type { PatinaRPC } from "./rpc";
import { WebviewSink, createBridge } from "./shell/bridge";
import { startImageServer } from "./shell/images-server";

const log = logger("patina");

process.on("unhandledRejection", (e) => log.error("unhandled rejection:", e));
process.on("uncaughtException", (e) => log.error("uncaught exception:", e));

// `electrobun dev` builds are the "debug builds": PATINA_ROUTE / PATINA_PLAY apply there.
const channel = await Updater.localInfo.channel();
setDebugBuild(channel === "dev" || process.env.PATINA_DEBUG_BUILD === "1");

const sink = new WebviewSink();
const app = new AppState(sink);
const imageServer = startImageServer(app);
setTimeout(() => images.pruneCache(), 0);

const bridge = createBridge(app, () => sink.view, {
  async openUrl(url) {
    if (!Utils.openExternal(url)) throw new Error(`Could not open ${url}`);
  },
  async openPath(path) {
    if (!Utils.openPath(path)) throw new Error(`Could not open ${path}`);
  },
});

const rpc = BrowserView.defineRPC<PatinaRPC>({
  // Commands such as complete_pin wait for the user; never time them out.
  maxRequestTime: Infinity,
  handlers: {
    requests: { invoke: (params: { cmd: string; args?: Record<string, unknown> }) => bridge.invoke(params) },
    messages: {},
  },
});
sink.view = {
  event: (name, payload) => rpc.send.event({ name, payload }),
  channel: (id, message) => rpc.send.channel({ id, message }),
};

// With `PATINA_DEV_URL=http://localhost:1420 bun run dev` (and `bun run dev:web` running) the page hot-reloads.
const url = process.env.PATINA_DEV_URL ?? "views://app/index.html";

const win = new BrowserWindow({
  title: "Patina",
  url,
  frame: { width: 1440, height: 900 },
  titleBarStyle: "hiddenInset",
  renderer: "cef",

  preload: `window.__PATINA__ = ${JSON.stringify({ images: imageServer.base })};`,
  rpc,
});
log.info(`window ${win.id} open (${channel}), artwork at ${imageServer.base.replace(/\/[0-9a-f]+$/, "/…")}`);

// Dev builds say which engine renders the page (CEF or the system webview).
if (channel === "dev") {
  setTimeout(async () => {
    try {
      const ua = await (rpc as any).request.evaluateJavascriptWithResponse({ script: "return navigator.userAgent" });
      log.info(`webview: ${ua}`);
    } catch (e) {
      log.warn("could not read the webview's user agent:", e);
    }
  }, 4000);
}

// Quitting: close mpv and let its last reports reach Plex before the process goes
// (the first before-quit is vetoed, then the app quits for real).
let closing = false;
electrobunEventEmitter.on("before-quit", (event: { response: unknown }) => {
  if (closing) return;
  closing = true;
  event.response = { allow: false };
  void Promise.race([player.shutdown(), Bun.sleep(8000)])
    .catch((e) => log.warn("shutdown:", e))
    .finally(() => {
      imageServer.stop();
      Utils.quit();
    });
});
for (const signal of ["SIGINT", "SIGTERM"] as const) process.on(signal, () => Utils.quit());

app.spawn(session.restore(app));
