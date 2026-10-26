// Errors of the Plex client (port of crates/plex/src/error.rs). The `message` of a
// `PlexError` is exactly the Rust `Display` text, because it is shown to the user.

import type { DecodeFailure } from "./de";

export type PlexErrorKind = "http" | "url" | "unauthorized" | "status" | "decode" | "noConnection" | "other";

/** The tail added to a status error: the first 200 chars of a non-HTML body. */
function snippet(body: string): string {
  const text = body.trim();
  if (text === "" || text.startsWith("<")) return "";
  const cut = Array.from(text).slice(0, 200).join("");
  return `: ${cut}`;
}

export class PlexError extends Error {
  readonly kind: PlexErrorKind;
  /** HTTP status of the failure, when there was one (401 for `unauthorized`). */
  readonly status: number | undefined;
  /** `status` errors: the request method. */
  readonly method: string | undefined;
  /** `status` and `decode` errors: the request path. */
  readonly path: string | undefined;
  /** `status` errors: the response body. */
  readonly body: string | undefined;

  private constructor(
    kind: PlexErrorKind,
    message: string,
    extra: { status?: number; method?: string; path?: string; body?: string } = {},
  ) {
    super(message);
    this.name = "PlexError";
    this.kind = kind;
    this.status = extra.status;
    this.method = extra.method;
    this.path = extra.path;
    this.body = extra.body;
  }

  /** `Error::Http`: "network error: {0}". */
  static http(detail: string): PlexError {
    return new PlexError("http", `network error: ${detail}`);
  }

  /** `Error::Url`: "invalid URL: {0}". */
  static url(detail: string): PlexError {
    return new PlexError("url", `invalid URL: ${detail}`);
  }

  /** `Error::Unauthorized`. */
  static unauthorized(): PlexError {
    return new PlexError("unauthorized", "not authorized (token missing, expired or lacking access)", { status: 401 });
  }

  /** `Error::Status`: "{method} {path} failed with HTTP {status}[: body]". */
  static statusError(method: string, path: string, status: number, body: string): PlexError {
    return new PlexError("status", `${method} ${path} failed with HTTP ${status}${snippet(body)}`, {
      status,
      method,
      path,
      body,
    });
  }

  /** `Error::Decode`: "could not decode response from {path}: {source}". */
  static decode(path: string, source: DecodeFailure | string): PlexError {
    const text = typeof source === "string" ? source : source.message;
    return new PlexError("decode", `could not decode response from ${path}: ${text}`, { path });
  }

  /** `Error::NoConnection`. */
  static noConnection(server: string): PlexError {
    return new PlexError("noConnection", `no reachable connection for server "${server}"`);
  }

  /** `Error::Other`. */
  static other(message: string): PlexError {
    return new PlexError("other", message);
  }

  isNotFound(): boolean {
    return this.status === 404;
  }

  static is(e: unknown): e is PlexError {
    return e instanceof PlexError;
  }
}
