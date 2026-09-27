// C interface to Patina's backend (crates/ffi/src/lib.rs).
// Callbacks run on backend threads; strings and bytes they receive are only
// valid during the call.

#ifndef PATINA_H
#define PATINA_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef void (*patina_event_cb)(void *ctx, const char *name, const char *payload_json);
typedef void (*patina_reply_cb)(void *ctx, bool ok, const char *json);
typedef void (*patina_bytes_cb)(void *ctx, const uint8_t *data, size_t len);

/// Start the backend and restore the saved session. Call once, first.
void patina_start(void *ctx, patina_event_cb callback);

/// Call a command by name with a JSON object of arguments (or NULL).
void patina_call(const char *name, const char *args_json, void *ctx, patina_reply_cb callback);

/// Plex artwork through the backend's cache. No bytes (NULL, 0) on failure.
void patina_image(const char *source, uint32_t width, uint32_t height, void *ctx, patina_bytes_cb callback);

/// The saved artwork of a download.
void patina_download_image(const char *id, void *ctx, patina_bytes_cb callback);

/// Stop mpv. Call when the app quits.
void patina_shutdown(void);

#endif
