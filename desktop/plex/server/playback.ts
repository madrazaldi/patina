// Timeline reporting and the universal transcoder (port of crates/plex/src/server/playback.rs).

import { randomUUID } from "node:crypto";

import { Params } from "../client";
import type { MediaContainer, TranscodeSession } from "../models";
import { CollectionsApi } from "./collections";

export type TimelineState = "playing" | "paused" | "buffering" | "stopped";

/** The `TimelineState` variants. */
export const TimelineState = {
  Playing: "playing",
  Paused: "paused",
  Buffering: "buffering",
  Stopped: "stopped",
  /** `TimelineState::as_str` */
  asStr(s: TimelineState): string {
    return s;
  },
} as const;

/**
 * A playback progress report. Plex uses these to track resume points,
 * mark items watched and show the session on the dashboard.
 */
export interface TimelineUpdate {
  ratingKey: string;
  key: string;
  state: TimelineState;
  timeMs: number;
  durationMs: number;
  playQueueItemId: number | null;
  /** The session id used for transcoding, if any. */
  session: string | null;
}

/** The fields of `TranscodeOptions`; every one is optional (defaults as in the Rust `Default`). */
export interface TranscodeOptionsInit {
  session?: string;
  mediaIndex?: number;
  partIndex?: number;
  maxVideoBitrate?: number | null;
  videoResolution?: string | null;
  videoQuality?: number | null;
  directStream?: boolean;
  directStreamAudio?: boolean;
  subtitles?: string;
  subtitleSize?: number | null;
  audioBoost?: number | null;
  offsetSecs?: number;
  protocol?: string;
}

/** Options for the universal transcoder. */
export class TranscodeOptions {
  session: string;
  mediaIndex: number;
  partIndex: number;
  /** Maximum video bitrate in kbps. */
  maxVideoBitrate: number | null;
  /** e.g. `1920x1080`. */
  videoResolution: string | null;
  videoQuality: number | null;
  directStream: boolean;
  directStreamAudio: boolean;
  /** `burn`, `sidecar`, `embedded`, `none` or `auto`. */
  subtitles: string;
  subtitleSize: number | null;
  audioBoost: number | null;
  offsetSecs: number;
  protocol: string;

  constructor(init: TranscodeOptionsInit = {}) {
    this.session = init.session ?? randomUUID();
    this.mediaIndex = init.mediaIndex ?? 0;
    this.partIndex = init.partIndex ?? 0;
    this.maxVideoBitrate = init.maxVideoBitrate ?? null;
    this.videoResolution = init.videoResolution ?? null;
    this.videoQuality = init.videoQuality ?? null;
    this.directStream = init.directStream ?? true;
    this.directStreamAudio = init.directStreamAudio ?? true;
    this.subtitles = init.subtitles ?? "auto";
    this.subtitleSize = init.subtitleSize ?? null;
    this.audioBoost = init.audioBoost ?? null;
    this.offsetSecs = init.offsetSecs ?? 0;
    this.protocol = init.protocol ?? "hls";
  }

  static from(o: TranscodeOptionsInit): TranscodeOptions {
    return o instanceof TranscodeOptions ? o : new TranscodeOptions(o);
  }
}

export class PlaybackApi extends CollectionsApi {
  /** Report playback progress. */
  timeline(u: TimelineUpdate): Promise<void> {
    const p = new Params()
      .set("ratingKey", u.ratingKey)
      .set("key", u.key)
      .set("state", TimelineState.asStr(u.state))
      .set("time", Math.max(u.timeMs, 0))
      .set("duration", Math.max(u.durationMs, 0))
      .set("identifier", "com.plexapp.plugins.library")
      .set("hasMDE", 1)
      .opt("playQueueItemID", u.playQueueItemId)
      .opt("X-Plex-Session-Identifier", u.session);
    return this.empty("GET", "/:/timeline", p);
  }

  private transcodeParams(path: string, init: TranscodeOptionsInit): Params {
    const o = TranscodeOptions.from(init);
    const info = this.clientInfo();
    return (
      new Params()
        .set("path", path)
        .set("mediaIndex", o.mediaIndex)
        .set("partIndex", o.partIndex)
        .set("protocol", o.protocol)
        .set("fastSeek", 1)
        .set("directPlay", 0)
        .set("directStream", o.directStream ? 1 : 0)
        .set("directStreamAudio", o.directStreamAudio ? 1 : 0)
        .set("subtitles", o.subtitles)
        .opt("subtitleSize", o.subtitleSize)
        .opt("audioBoost", o.audioBoost)
        .opt("maxVideoBitrate", o.maxVideoBitrate)
        .opt("videoBitrate", o.maxVideoBitrate)
        .opt("videoResolution", o.videoResolution)
        .opt("videoQuality", o.videoQuality)
        .set("offset", o.offsetSecs.toFixed(3))
        .set("copyts", 1)
        .set("mediaBufferSize", 102400)
        .set("session", o.session)
        .set("X-Plex-Session-Identifier", o.session)
        .set("X-Plex-Client-Identifier", info.identifier)
        .set("X-Plex-Product", info.product)
        .set("X-Plex-Version", info.version)
        .set("X-Plex-Device", info.device)
        // The "Chrome" profile gives broadly compatible HLS (h264/hevc + aac/ac3).
        .set("X-Plex-Platform", "Chrome")
        .set(
          "X-Plex-Client-Profile-Extra",
          "add-transcode-target-audio-codec(type=videoProfile&context=streaming&protocol=hls&audioCodec=ac3,eac3)",
        )
    );
  }

  /** Ask the server what it would do (direct play/stream/transcode) for these options. */
  transcodeDecision(path: string, o: TranscodeOptionsInit): Promise<MediaContainer> {
    return this.get("/video/:/transcode/universal/decision", this.transcodeParams(path, o));
  }

  /** HLS playlist URL for a transcoded stream of `path` (`/library/metadata/<id>`). */
  transcodeUrl(path: string, o: TranscodeOptionsInit): URL {
    return this.authedUrl("/video/:/transcode/universal/start.m3u8", this.transcodeParams(path, o));
  }

  /** Transcoded audio stream URL (for music). */
  audioTranscodeUrl(path: string, o: TranscodeOptionsInit): URL {
    const opts = TranscodeOptions.from(o);
    const p = this.transcodeParams(path, opts);
    p.entries = p.entries.filter(([k]) => k !== "protocol");
    p.push("protocol", "http");
    p.push("musicBitrate", opts.maxVideoBitrate ?? 320);
    return this.authedUrl("/music/:/transcode/universal/start.mp3", p);
  }

  /** Keep a transcode session alive while paused. */
  pingTranscode(session: string): Promise<void> {
    return this.empty("GET", "/video/:/transcode/universal/ping", new Params().set("session", session));
  }

  stopTranscode(session: string): Promise<void> {
    return this.empty("GET", "/video/:/transcode/universal/stop", new Params().set("session", session));
  }

  /** Active transcoder sessions. */
  async transcodeSessions(): Promise<TranscodeSession[]> {
    return (await this.get("/transcode/sessions", new Params())).TranscodeSession;
  }

  killTranscodeSession(key: string): Promise<void> {
    return this.delete(`/transcode/sessions/${key}`, new Params());
  }
}
