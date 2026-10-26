// Media, parts and streams (port of crates/plex/src/models/media.rs).

import { boolean, list, obj, optF64, optI64, optString } from "../de";
import { streamType } from "../media-type";

/**
 * One version of an item (a file or set of files).
 */
export interface Media {
  id: number | null;
  duration: number | null;
  bitrate: number | null;
  width: number | null;
  height: number | null;
  aspectRatio: number | null;
  audioChannels: number | null;
  audioCodec: string | null;
  videoCodec: string | null;
  videoResolution: string | null;
  container: string | null;
  videoFrameRate: string | null;
  videoProfile: string | null;
  audioProfile: string | null;
  title: string | null;
  editionTitle: string | null;
  optimizedForStreaming: boolean;
  has64bitOffsets: boolean;
  selected: boolean;
  Part: Part[];
}

export function normalizeMedia(v: unknown): Media {
  const o = obj(v, "Media");
  return {
    id: optI64(o.id),
    duration: optI64(o.duration),
    bitrate: optI64(o.bitrate),
    width: optI64(o.width),
    height: optI64(o.height),
    aspectRatio: optF64(o.aspectRatio),
    audioChannels: optI64(o.audioChannels),
    audioCodec: optString(o.audioCodec),
    videoCodec: optString(o.videoCodec),
    videoResolution: optString(o.videoResolution),
    container: optString(o.container),
    videoFrameRate: optString(o.videoFrameRate),
    videoProfile: optString(o.videoProfile),
    audioProfile: optString(o.audioProfile),
    title: optString(o.title),
    editionTitle: optString(o.editionTitle),
    optimizedForStreaming: boolean(o.optimizedForStreaming),
    has64bitOffsets: boolean(o.has64bitOffsets),
    selected: boolean(o.selected),
    Part: list(o.Part, normalizePart, "Part"),
  };
}

/** `Media::default()`, with fields overridden by `over`. */
export function defaultMedia(over: Partial<Media> = {}): Media {
  return { ...normalizeMedia({}), ...over };
}

/**
 * A single file of a [`Media`].
 */
export interface Part {
  id: number | null;
  key: string | null;
  duration: number | null;
  file: string | null;
  size: number | null;
  container: string | null;
  videoProfile: string | null;
  audioProfile: string | null;
  indexes: string | null;
  decision: string | null;
  hasThumbnail: boolean;
  selected: boolean;
  Stream: Stream[];
}

export function normalizePart(v: unknown): Part {
  const o = obj(v, "Part");
  return {
    id: optI64(o.id),
    key: optString(o.key),
    duration: optI64(o.duration),
    file: optString(o.file),
    size: optI64(o.size),
    container: optString(o.container),
    videoProfile: optString(o.videoProfile),
    audioProfile: optString(o.audioProfile),
    indexes: optString(o.indexes),
    decision: optString(o.decision),
    hasThumbnail: boolean(o.hasThumbnail),
    selected: boolean(o.selected),
    Stream: list(o.Stream, normalizeStream, "Stream"),
  };
}

/** `Part::default()`, with fields overridden by `over`. */
export function defaultPart(over: Partial<Part> = {}): Part {
  return { ...normalizePart({}), ...over };
}

/**
 * A video, audio, subtitle or lyrics stream inside a part.
 */
export interface Stream {
  id: number | null;
  streamType: number | null;
  default: boolean;
  selected: boolean;
  forced: boolean;
  hearingImpaired: boolean;
  codec: string | null;
  /** Container stream index (matches mpv's `ff-index`). Absent for sidecar files. */
  index: number | null;
  bitrate: number | null;
  language: string | null;
  languageCode: string | null;
  languageTag: string | null;
  title: string | null;
  displayTitle: string | null;
  extendedDisplayTitle: string | null;
  /** Download path for sidecar subtitles. */
  key: string | null;
  format: string | null;
  channels: number | null;
  samplingRate: number | null;
  audioChannelLayout: string | null;
  profile: string | null;
  width: number | null;
  height: number | null;
  frameRate: number | null;
  bitDepth: number | null;
  colorPrimaries: string | null;
  colorTrc: string | null;
  colorSpace: string | null;
  DOVIPresent: boolean;
  DOVIProfile: number | null;
  decision: string | null;
  location: string | null;
  providerTitle: string | null;
  score: number | null;
  perfectMatch: boolean;
  downloaded: boolean;
}

export function normalizeStream(v: unknown): Stream {
  const o = obj(v, "Stream");
  return {
    id: optI64(o.id),
    streamType: optI64(o.streamType),
    default: boolean(o.default),
    selected: boolean(o.selected),
    forced: boolean(o.forced),
    hearingImpaired: boolean(o.hearingImpaired),
    codec: optString(o.codec),
    index: optI64(o.index),
    bitrate: optI64(o.bitrate),
    language: optString(o.language),
    languageCode: optString(o.languageCode),
    languageTag: optString(o.languageTag),
    title: optString(o.title),
    displayTitle: optString(o.displayTitle),
    extendedDisplayTitle: optString(o.extendedDisplayTitle),
    key: optString(o.key),
    format: optString(o.format),
    channels: optI64(o.channels),
    samplingRate: optI64(o.samplingRate),
    audioChannelLayout: optString(o.audioChannelLayout),
    profile: optString(o.profile),
    width: optI64(o.width),
    height: optI64(o.height),
    frameRate: optF64(o.frameRate),
    bitDepth: optI64(o.bitDepth),
    colorPrimaries: optString(o.colorPrimaries),
    colorTrc: optString(o.colorTrc),
    colorSpace: optString(o.colorSpace),
    DOVIPresent: boolean(o.DOVIPresent),
    DOVIProfile: optI64(o.DOVIProfile),
    decision: optString(o.decision),
    location: optString(o.location),
    providerTitle: optString(o.providerTitle),
    score: optI64(o.score),
    perfectMatch: boolean(o.perfectMatch),
    downloaded: boolean(o.downloaded),
  };
}

/** `Stream::default()`, with fields overridden by `over`. */
export function defaultStream(over: Partial<Stream> = {}): Stream {
  return { ...normalizeStream({}), ...over };
}


export function channelLabel(channels: number): string {
  switch (channels) {
    case 1:
      return "Mono";
    case 2:
      return "Stereo";
    case 3:
      return "2.1";
    case 6:
      return "5.1";
    case 7:
      return "6.1";
    case 8:
      return "7.1";
    default:
      return `${channels}ch`;
  }
}

export namespace Media {
  /** Human label such as "4K HEVC · DV · TrueHD 7.1". */
  export function label(m: Media): string {
    const bits: string[] = [];
    const t = m.editionTitle ?? m.title;
    if (t !== null) bits.push(t);
    const r = m.videoResolution;
    if (r !== null) {
      if (r === "4k" || r === "4K") bits.push("4K");
      else if (r === "sd" || r === "SD") bits.push("SD");
      else if (/^[0-9]*$/.test(r)) bits.push(`${r}p`);
      else bits.push(r.toUpperCase());
    }
    if (m.videoCodec !== null) bits.push(m.videoCodec.toUpperCase());
    const video = m.Part.length > 0 ? Part.videoStream(m.Part[0]!) : null;
    const hdr = video !== null ? Stream.hdrLabel(video) : null;
    if (hdr !== null) bits.push(hdr);
    if (m.audioCodec !== null) {
      const ch = m.audioChannels !== null ? channelLabel(m.audioChannels) : "";
      bits.push(`${m.audioCodec.toUpperCase()} ${ch}`.trim());
    }
    return bits.join(" · ");
  }
}

export namespace Part {
  export function streamsOf(p: Part, kind: number): Stream[] {
    return p.Stream.filter((s) => s.streamType === kind);
  }

  export function videoStream(p: Part): Stream | null {
    return streamsOf(p, streamType.VIDEO)[0] ?? null;
  }

  export function audioStreams(p: Part): Stream[] {
    return streamsOf(p, streamType.AUDIO);
  }

  export function subtitleStreams(p: Part): Stream[] {
    return streamsOf(p, streamType.SUBTITLE);
  }

  export function selectedAudio(p: Part): Stream | null {
    const audio = audioStreams(p);
    return audio.find((s) => s.selected) ?? audio[0] ?? null;
  }

  export function selectedSubtitle(p: Part): Stream | null {
    return subtitleStreams(p).find((s) => s.selected) ?? null;
  }

  /**
   * The stream in this part that best corresponds to `template`, which
   * usually comes from another episode. Language must match (and for
   * subtitles, forced-ness too). Ties go to the stream that comes first.
   */
  export function matchingStream(p: Part, template: Stream): Stream | null {
    const kind = template.streamType;
    if (kind === null) return null;
    let best: { score: number; stream: Stream } | null = null;
    for (const s of streamsOf(p, kind)) {
      const score = Stream.similarity(template, s);
      if (score === null) continue;
      if (best === null || score > best.score) best = { score, stream: s };
    }
    return best?.stream ?? null;
  }
}

export namespace Stream {
  export function label(s: Stream): string {
    return s.extendedDisplayTitle ?? s.displayTitle ?? s.title ?? s.language ?? `Track ${s.index ?? 0}`;
  }

  /** Short label: "English (AC3 5.1)" for audio, "English (SRT)" for subtitles. */
  export function shortLabel(s: Stream): string {
    return s.displayTitle ?? label(s);
  }

  export function isExternal(s: Stream): boolean {
    return s.key !== null && s.index === null;
  }

  /** Language identity used for matching: the code, else the tag, else the name. */
  function languageKey(s: Stream): string | null {
    const v = s.languageCode ?? s.languageTag ?? s.language;
    return v === null ? null : v.replace(/[A-Z]/g, (c) => c.toLowerCase());
  }

  /**
   * How closely `other` resembles this stream, or `null` if it can't stand in
   * for it at all (a different type or language, or forced vs. full subtitles).
   */
  export function similarity(s: Stream, other: Stream): number | null {
    if (s.streamType !== other.streamType || languageKey(s) !== languageKey(other)) return null;
    const subtitle = s.streamType === streamType.SUBTITLE;
    if (subtitle && s.forced !== other.forced) return null;
    const same = (a: string | null, b: string | null): boolean =>
      a !== null && b !== null && a.toLowerCase() === b.toLowerCase();
    let score = 1;
    if (same(s.title, other.title)) score += 8;
    if (same(s.extendedDisplayTitle, other.extendedDisplayTitle)) score += 4;
    else if (same(s.displayTitle, other.displayTitle)) score += 2;
    if (same(s.codec, other.codec)) score += 3;
    if (s.channels !== null && s.channels === other.channels) score += 3;
    if (s.hearingImpaired === other.hearingImpaired) score += 2;
    if (isExternal(s) === isExternal(other)) score += 1;
    return score;
  }

  export function hdrLabel(s: Stream): string | null {
    if (s.DOVIPresent) return "Dolby Vision";
    switch (s.colorTrc) {
      case "smpte2084":
        return "HDR10";
      case "arib-std-b67":
        return "HLG";
      default:
        return null;
    }
  }
}
