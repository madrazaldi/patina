// Artwork goes through the `plximg` protocol served by crates/app/src/images.rs,
// which adds the server token and caches resized images on disk.

const base = navigator.userAgent.includes("Windows") ? "http://plximg.localhost/i" : "plximg://localhost/i";

/** URL for a server-relative or absolute image, resized to `w`×`h`. */
export function img(source: string | null | undefined, w: number, h: number): string | undefined {
  if (!source) return undefined;
  const q = new URLSearchParams({ u: source, w: String(w), h: String(h) });
  return `${base}?${q}`;
}
