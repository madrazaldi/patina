// Port of desktop/plex/error.ts.
// Errors of the Plex client (port of crates/plex/src/error.rs). The `message` of a
// `PlexError` is exactly the Rust `Display` text, because it is shown to the user.

import Foundation

/// The tail added to a status error: the first 200 chars of a non-HTML body.
private func snippet(_ body: String) -> String {
    let text = body.trimmingCharacters(in: .whitespacesAndNewlines)
    if text.isEmpty || text.hasPrefix("<") { return "" }
    var cut = String.UnicodeScalarView()
    cut.append(contentsOf: text.unicodeScalars.prefix(200))
    return ": \(String(cut))"
}

public struct PlexError: Error, LocalizedError, Sendable, Hashable {
    public enum Kind: String, Sendable, Hashable {
        case http, url, unauthorized, status, decode, noConnection, other
    }

    public let kind: Kind
    public let message: String
    /// HTTP status of the failure, when there was one (401 for `unauthorized`).
    public let status: Int?
    /// `status` errors: the request method.
    public let method: String?
    /// `status` and `decode` errors: the request path.
    public let path: String?
    /// `status` errors: the response body.
    public let body: String?

    private init(
        _ kind: Kind, _ message: String, status: Int? = nil, method: String? = nil, path: String? = nil,
        body: String? = nil
    ) {
        self.kind = kind
        self.message = message
        self.status = status
        self.method = method
        self.path = path
        self.body = body
    }

    public var errorDescription: String? { message }

    /// `Error::Http`: "network error: {0}".
    public static func http(_ detail: String) -> PlexError {
        PlexError(.http, "network error: \(detail)")
    }

    /// `Error::Url`: "invalid URL: {0}".
    public static func url(_ detail: String) -> PlexError {
        PlexError(.url, "invalid URL: \(detail)")
    }

    /// `Error::Unauthorized`.
    public static func unauthorized() -> PlexError {
        PlexError(.unauthorized, "not authorized (token missing, expired or lacking access)", status: 401)
    }

    /// `Error::Status`: "{method} {path} failed with HTTP {status}[: body]".
    public static func statusError(_ method: String, _ path: String, _ status: Int, _ body: String) -> PlexError {
        PlexError(
            .status, "\(method) \(path) failed with HTTP \(status)\(snippet(body))", status: status, method: method,
            path: path, body: body)
    }

    /// `Error::Decode`: "could not decode response from {path}: {source}".
    public static func decode(_ path: String, _ source: DecodeFailure) -> PlexError {
        decode(path, source.message)
    }

    public static func decode(_ path: String, _ source: String) -> PlexError {
        PlexError(.decode, "could not decode response from \(path): \(source)", path: path)
    }

    /// `Error::NoConnection`.
    public static func noConnection(_ server: String) -> PlexError {
        PlexError(.noConnection, "no reachable connection for server \"\(server)\"")
    }

    /// `Error::Other`.
    public static func other(_ message: String) -> PlexError {
        PlexError(.other, message)
    }

    public func isNotFound() -> Bool {
        status == 404
    }
}
