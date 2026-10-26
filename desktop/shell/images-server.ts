// Artwork for the webview. The page loads `<base>/i?u=<path>&w=&h=&s=` URLs,
// served through the core's artwork cache so the token never reaches the DOM.
// `<base>/d?id=<download>` serves the saved artwork of a download, which also
// works offline. This replaces the `plximg://` scheme of the Tauri shell:
// a loopback HTTP server, with a random path prefix so other local pages
// can't use it as a proxy to the server.

import { randomBytes } from "node:crypto";
import * as images from "../core/images";
import * as servers from "../core/servers";
import type { AppState } from "../core/state";

export interface ImageServer {
  /** `http://127.0.0.1:<port>/<secret>`; the page appends `/i?...` or `/d?...`. */
  base: string;
  stop(): void;
}

export function startImageServer(app: AppState): ImageServer {
  const secret = randomBytes(16).toString("hex");
  const server = Bun.serve({
    hostname: "127.0.0.1",
    port: 0,
    fetch: (request) => handle(app, secret, request),
  });
  return { base: `http://127.0.0.1:${server.port}/${secret}`, stop: () => void server.stop(true) };
}

const notFound = () => new Response(null, { status: 404 });

export async function handle(app: AppState, secret: string, request: Request): Promise<Response> {
  const url = new URL(request.url);
  const prefix = `/${secret}`;
  if (!url.pathname.startsWith(`${prefix}/`)) return notFound();
  const route = url.pathname.slice(prefix.length);
  const q = url.searchParams;

  if (route === "/d") {
    const found = images.downloadPoster(q.get("id") ?? "");
    return found ? new Response(found[0] as BodyInit, { headers: { "Content-Type": found[1] } }) : notFound();
  }
  if (route !== "/i") return notFound();

  const source = q.get("u") ?? "";
  // `s` names another server of the account that the artwork comes from.
  const on = q.get("s") || undefined;
  const w = Number.parseInt(q.get("w") ?? "", 10) || 300;
  const h = Number.parseInt(q.get("h") ?? "", 10) || 450;
  const found = on && !servers.isCurrent(app, on) ? await servers.image(app, on, source, w, h) : await images.fetch(source, w, h);
  if (!found) return notFound();
  return new Response(found[0] as BodyInit, {
    headers: {
      "Content-Type": found[1],
      "Cache-Control": "public, max-age=604800",
      "Access-Control-Allow-Origin": "*",
    },
  });
}
