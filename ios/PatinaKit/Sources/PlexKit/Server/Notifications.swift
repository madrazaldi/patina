// Port of desktop/plex/server/notifications.ts.
// Live server notifications over SSE (port of crates/plex/src/server/notifications.rs).

import Foundation

/// A server-sent event from `/:/eventsource/notifications` (`Notification` in TypeScript;
/// renamed so it does not clash with Foundation's).
public struct PlexNotification: Sendable, Hashable {
    /// `playing`, `activity`, `timeline`, `transcodeSession.update`, `status`, ...
    public var event: String
    public var data: JSONValue

    public init(event: String, data: JSONValue) {
        self.event = event
        self.data = data
    }
}

/// A year: long enough to never end a healthy stream.
let sseTimeout: TimeInterval = 60 * 60 * 24 * 365

/// The SSE framing: fed one line at a time (without the newline), it returns a notification
/// when a blank line ends an event.
struct SSEParser {
    private var event = ""
    private var data = ""

    mutating func line(_ bytes: [UInt8]) -> PlexNotification? {
        var raw = bytes
        while let last = raw.last, last == 0x0A || last == 0x0D { raw.removeLast() }
        let line = String(decoding: raw, as: UTF8.self)
        if line.isEmpty {
            if data.isEmpty {
                event = ""
                return nil
            }
            let text = data
            let name = event
            data = ""
            event = ""
            let parsed = (try? JSONValue.parse(text)) ?? .string(text)
            return PlexNotification(event: name.isEmpty ? "message" : name, data: parsed)
        }
        if line.hasPrefix("event:") {
            event = line.dropFirst("event:".count).trimmingCharacters(in: .whitespacesAndNewlines)
        } else if line.hasPrefix("data:") {
            if !data.isEmpty { data += "\n" }
            data += String(line.dropFirst("data:".count).drop(while: { $0.isWhitespace }))
        }
        return nil
    }
}

extension PlexServer {
    /// Live server notifications (playback state, scans, activities, library changes).
    /// `filters` limits the event types; empty means all. The request is made (and its errors
    /// thrown) before this returns; iterating the result reads the stream. Cancelling the
    /// iterating task (or dropping the stream) closes the connection; a stream that breaks
    /// makes the iteration throw a `PlexError`.
    public func notifications(_ filters: [String] = []) async throws -> AsyncThrowingStream<PlexNotification, Error> {
        let p = filters.isEmpty ? Params() : Params().set("filters", filters.joined(separator: ","))
        let rb = try requestBuilder("GET", "/:/eventsource/notifications", p)
            .header("Accept", "text/event-stream")
            .timeout(sseTimeout)
        let resp = try await http.open(rb)
        return AsyncThrowingStream { continuation in
            let task = Task {
                var parser = SSEParser()
                var line: [UInt8] = []
                do {
                    for try await b in resp.bytes {
                        if b == 0x0A {
                            if let n = parser.line(line) { continuation.yield(n) }
                            line.removeAll(keepingCapacity: true)
                        } else {
                            line.append(b)
                        }
                    }
                    continuation.finish()
                } catch {
                    if Task.isCancelled {
                        continuation.finish()
                    } else {
                        continuation.finish(throwing: PlexError.http(error.localizedDescription))
                    }
                }
            }
            continuation.onTermination = { _ in task.cancel() }
        }
    }
}
