// Artwork goes through the `plximg` protocol served by crates/app/src/images.rs,
// which adds the server token and caches resized images on disk.

const root = navigator.userAgent.includes("Windows") ? "http://plximg.localhost" : "plximg://localhost";
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
