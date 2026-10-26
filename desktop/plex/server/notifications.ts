// Live server notifications over SSE (port of crates/plex/src/server/notifications.rs).

import { finishResponse, Params } from "../client";
import { PlexError } from "../error";
import { AdminApi } from "./admin";

/** A server-sent event from `/:/eventsource/notifications`. */
export interface Notification {
  /** `playing`, `activity`, `timeline`, `transcodeSession.update`, `status`, ... */
  event: string;
  data: unknown;
}

/** A year; `Http.send` clamps it to what timers can hold. */
const SSE_TIMEOUT_MS = 60 * 60 * 24 * 365 * 1000;

/** Parse the SSE framing of `resp` into notifications. Ends when the body ends or `signal` aborts. */
async function* events(resp: Response, signal: AbortSignal | undefined): AsyncGenerator<Notification, void, void> {
  const reader = resp.body?.getReader();
  if (!reader) return;
  const decoder = new TextDecoder("utf-8");
  let buf = "";
  let event = "";
  let data = "";
  try {
    for (;;) {
      // Drain complete lines already buffered.
      for (let pos = buf.indexOf("\n"); pos >= 0; pos = buf.indexOf("\n")) {
        const line = buf.slice(0, pos + 1).replace(/[\r\n]+$/, "");
        buf = buf.slice(pos + 1);
        if (line === "") {
          if (data === "") {
            event = "";
            continue;
          }
          const text = data;
          const name = event;
          data = "";
          event = "";
          let parsed: unknown;
          try {
            parsed = JSON.parse(text);
          } catch {
            parsed = text;
          }
          yield { event: name === "" ? "message" : name, data: parsed };
          continue;
        }
        if (line.startsWith("event:")) {
          event = line.slice("event:".length).trim();
        } else if (line.startsWith("data:")) {
          if (data !== "") data += "\n";
          data += line.slice("data:".length).trimStart();
        }
      }
      let chunk: Awaited<ReturnType<typeof reader.read>>;
      try {
        chunk = await reader.read();
      } catch (e) {
        if (signal?.aborted) return;
        throw PlexError.http(e instanceof Error ? e.message : String(e));
      }
      if (chunk.done) return;
      buf += decoder.decode(chunk.value, { stream: true });
    }
  } finally {
    reader.cancel().catch(() => undefined);
    finishResponse(resp);
  }
}

export class NotificationsApi extends AdminApi {
  /**
   * Live server notifications (playback state, scans, activities, library changes).
   * `filters` limits the event types; empty means all. The request is made (and its errors
   * raised) before this resolves; iterating the result reads the stream. Aborting `signal`
   * ends the iteration; a stream that breaks makes the iteration throw a `PlexError`.
   */
  async notifications(
    filters: readonly string[] = [],
    signal?: AbortSignal,
  ): Promise<AsyncGenerator<Notification, void, void>> {
    const p = filters.length === 0 ? new Params() : new Params().set("filters", filters.join(","));
    const rb = this.http
      .request("GET", this.url("/:/eventsource/notifications"), this._token, p)
      .header("Accept", "text/event-stream")
      .timeout(SSE_TIMEOUT_MS);
    if (signal) rb.abortWith(signal);
    const resp = await this.http.send(rb);
    return events(resp, signal);
  }
}
