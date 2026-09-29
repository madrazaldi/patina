// Helpers over Plex metadata, mirroring the methods on the Rust models
// (crates/plex/src/models). Values from Rust may be `null`, so compare with `!= null`.

import type { Media, Metadata, Part, Stream } from "./types";

export const AUDIO = 2;
export const SUBTITLE = 3;
const VIDEO = 1;

export const kind = (m: Metadata) => m.type ?? "";
export const title = (m: Metadata) => m.title ?? "";

export function isPlayable(m: Metadata) {
  const k = kind(m);
  return (
    ["movie", "episode", "track", "clip", "trailer", "photo"].includes(k) ||
    (m.Media.length > 0 && !["show", "season", "artist", "album"].includes(k))
  );
}

export const isContainer = (m: Metadata) =>
  ["show", "season", "artist", "album", "collection", "playlist", "photoalbum"].includes(kind(m));

/** Something a Play button makes sense for. */
export const canPlay = (m: Metadata) => isPlayable(m) || isContainer(m);

export function isWatched(m: Metadata) {
  if (m.leafCount != null && m.leafCount > 0 && m.viewedLeafCount != null) return m.viewedLeafCount >= m.leafCount;
  return (m.viewCount ?? 0) > 0 && (m.viewOffset ?? 0) === 0;
}

export function unwatchedLeaves(m: Metadata) {
  return m.leafCount == null ? null : m.leafCount - (m.viewedLeafCount ?? 0);
}

/** Resume progress in 0..1, if partially watched. */
export function progress(m: Metadata): number | null {
  const offset = m.viewOffset ?? 0;
  const duration = m.duration ?? 0;
  return duration > 0 && offset > 0 ? Math.min(1, offset / duration) : null;
}

/** Best image for a poster-shaped card. */
export function poster(m: Metadata) {
  switch (kind(m)) {
    case "episode":
      return m.grandparentThumb ?? m.parentThumb ?? m.thumb;
    case "season":
      return m.thumb ?? m.parentThumb;
    case "playlist":
      return m.composite ?? m.thumb;
    default:
      return m.thumb ?? m.composite ?? m.parentThumb;
  }
}

/** Best background art. */
export function backdrop(m: Metadata) {
  return m.art ?? m.grandparentArt ?? m.parentArt ?? m.Image?.find((i) => i.type === "background")?.url;
}

/** "S01E02" for episodes. */
export function episodeCode(m: Metadata) {
  const pad = (n: number) => String(n).padStart(2, "0");
  if (m.parentIndex != null && m.index != null) return `S${pad(m.parentIndex)}E${pad(m.index)}`;
  if (m.index != null) return `E${pad(m.index)}`;
  return "";
}

/** Title with show/season context, e.g. "Show — S01E02 · Pilot". */
export function fullTitle(m: Metadata) {
  switch (kind(m)) {
    case "episode":
      return `${m.grandparentTitle ?? ""} — ${episodeCode(m)} · ${title(m)}`;
    case "season":
      return `${m.parentTitle ?? ""} — ${title(m)}`;
    case "track":
      return `${m.grandparentTitle ?? ""} — ${title(m)}`;
    default:
      return m.year != null ? `${title(m)} (${m.year})` : title(m);
  }
}

/** The plex.tv discover id from a `plex://type/<id>` guid. */
export function discoverId(m: Metadata) {
  const g = m.guid;
  if (!g?.startsWith("plex://")) return null;
  return g.split("/").pop() || null;
}

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;

/** Secondary line under a card title. */
export function subtitle(m: Metadata): string {
  switch (kind(m)) {
    case "episode": {
      const show = m.grandparentTitle ?? "";
      return show ? `${show} · ${episodeCode(m)}` : episodeCode(m);
    }
    case "season":
      return m.leafCount != null ? plural(m.leafCount, "episode") : (m.parentTitle ?? "");
    case "show":
      return m.childCount != null ? plural(m.childCount, "season") : String(m.year ?? "");
    case "album":
      return m.parentTitle ?? String(m.year ?? "");
    case "track":
      return m.grandparentTitle ?? "";
    case "playlist":
      return plural(m.leafCount ?? 0, "item");
    case "collection":
      return m.childCount != null ? plural(m.childCount, "item") : "";
    default:
      return m.year != null ? String(m.year) : "";
  }
}

/** Title and subtitle for "now playing"-style rows. */
export function playingLines(m: Metadata): [string, string] {
  switch (kind(m)) {
    case "episode":
      return [m.grandparentTitle ?? "", `${episodeCode(m)} · ${title(m)}`];
    case "track":
      return [title(m), m.grandparentTitle ?? ""];
    default:
      return [title(m), m.year != null ? String(m.year) : ""];
  }
}

export type Shape = "poster" | "landscape" | "square";

export function shapeFor(m: Metadata): Shape {
  const k = kind(m);
  if (k === "episode" || k === "clip") return "landscape";
  if (["album", "artist", "track", "playlist", "photo", "photoalbum"].includes(k)) return "square";
  return "poster";
}

/** One shape for a row of mixed items, so the cards line up: theirs if they
 *  agree, else posters (episodes show their show's poster). */
export function commonShape(items: Metadata[]): Shape {
  const shapes = new Set(items.map(shapeFor));
  if (shapes.size === 1) return [...shapes][0];
  return shapes.has("poster") || shapes.size === 0 ? "poster" : "square";
}

/** Playlist type an item can be added to. */
export function playlistKind(m: Metadata) {
  const k = kind(m);
  if (["track", "album", "artist"].includes(k)) return "audio";
  if (["photo", "photoalbum"].includes(k)) return "photo";
  return "video";
}

/** Whether the bulk audio/subtitle dialog applies. */
export const supportsTracks = (m: Metadata) => ["show", "season", "episode"].includes(kind(m));

/** Things that have a watched state worth toggling. */
export const hasWatchState = (m: Metadata) =>
  !["playlist", "photo", "photoalbum", "artist", "album", "track", "collection"].includes(kind(m));

// Media & streams -------------------------------------------------------------

export function channelLabel(channels: number) {
  return (
    ({ 1: "Mono", 2: "Stereo", 3: "2.1", 6: "5.1", 7: "6.1", 8: "7.1" } as Record<number, string>)[channels] ??
    `${channels}ch`
  );
}

const streamsOf = (p: Part, type: number) => p.Stream.filter((s) => s.streamType === type);
export const audioStreams = (p: Part) => streamsOf(p, AUDIO);
export const subtitleStreams = (p: Part) => streamsOf(p, SUBTITLE);
export const selectedAudio = (p: Part) => audioStreams(p).find((s) => s.selected) ?? audioStreams(p)[0];
export const selectedSubtitle = (p: Part) => subtitleStreams(p).find((s) => s.selected);

export function hdrLabel(s: Stream) {
  if (s.DOVIPresent) return "Dolby Vision";
  if (s.colorTrc === "smpte2084") return "HDR10";
  if (s.colorTrc === "arib-std-b67") return "HLG";
  return null;
}

export function streamLabel(s: Stream) {
  return s.extendedDisplayTitle ?? s.displayTitle ?? s.title ?? s.language ?? `Track ${s.index ?? 0}`;
}

export const isExternal = (s: Stream) => s.key != null && s.index == null;

/** "4K · HEVC · Dolby Vision · TRUEHD 7.1" */
export function mediaLabel(media: Media) {
  const bits: string[] = [];
  const name = media.editionTitle ?? media.title;
  if (name) bits.push(name);
  const r = media.videoResolution;
  if (r) bits.push(/^4k$/i.test(r) ? "4K" : /^sd$/i.test(r) ? "SD" : /^\d+$/.test(r) ? `${r}p` : r.toUpperCase());
  if (media.videoCodec) bits.push(media.videoCodec.toUpperCase());
  const video = media.Part[0]?.Stream.find((s) => s.streamType === VIDEO);
  const hdr = video && hdrLabel(video);
  if (hdr) bits.push(hdr);
  if (media.audioCodec) {
    const ch = media.audioChannels != null ? channelLabel(media.audioChannels) : "";
    bits.push(`${media.audioCodec.toUpperCase()} ${ch}`.trim());
  }
  return bits.join(" · ");
}
