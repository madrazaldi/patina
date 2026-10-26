// Artwork comes from the image server in the desktop shell (desktop/images-server.ts),
// which adds the server token and caches resized images on disk. The shell tells the
// page where it listens (`window.__PATINA__.images`) before the page loads. Outside the
// shell (the Playwright fixture) the URLs point nowhere, which is fine: nothing loads.

const root = (typeof window !== "undefined" && window.__PATINA__?.images) || "plximg://localhost";
const base = `${root}/i`;

/** URL for a server-relative or absolute image, resized to `w`×`h`. `server` is the machine id of another account server the image lives on. */
export function img(source: string | null | undefined, w: number, h: number, server?: string | null): string | undefined {
  if (!source) return undefined;
  const q = new URLSearchParams({ u: source, w: String(w), h: String(h) });
  if (server) q.set("s", server);
  return `${base}?${q}`;
}

/** Saved artwork of a download (works offline). */
export const downloadImg = (id: string) => `${root}/d?${new URLSearchParams({ id })}`;
