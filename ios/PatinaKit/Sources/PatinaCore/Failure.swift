// Port of desktop/core/failure.ts.
// Playback failures: why the player (or the setup before it) gave up, and what can be tried next.
//
// The player reports a failure as one `PlaybackFailure` event carrying a readable reason and
// the actions that make sense for it. Front ends offer them and answer with `recover`. The
// request needed to try again stays here, so a front end only sends the choice.
//
// The failure waiting for a decision is kept per `AppState` (module-level in TypeScript).

import Foundation
import PlexKit

nonisolated private let log = logger("failure")

public nonisolated struct PlaybackFailure: Sendable, Equatable {
    /// Distinguishes this failure from earlier ones.
    public var id: Int
    public var item: Metadata
    /// A sentence for people.
    public var reason: String
    /// What the player (or the underlying error) said.
    public var detail: String?
    public var kind: Failure.Kind
    public var phase: Failure.Phase
    /// Where trying again resumes.
    public var positionMs: Int
    /// The version (media) that failed.
    public var mediaIndex: Int
    /// The quality preset that was in use.
    public var quality: String
    /// The next step down, when transcoding more aggressively is an option.
    public var lowerQuality: Failure.QualityChoice?
    public var canRetry: Bool
    /// Items after the failed one that stay queued.
    public var remaining: Int
}

/// An error with the error it came from (`new Error(text, { cause })`), so `explainError` can
/// look through it.
public nonisolated struct ChainedError: Error, LocalizedError {
    public let message: String
    public let cause: any Error

    public init(_ message: String, cause: any Error) {
        self.message = message
        self.cause = cause
    }

    public var errorDescription: String? { message }
}

/// `anyhow::Context` keeping the cause (`context` in player.ts).
public nonisolated func chained(_ e: any Error, _ message: String) -> ChainedError {
    ChainedError("\(message): \(errorText(e))", cause: e)
}

public enum Failure {
    public nonisolated enum Kind: String, Sendable, Equatable {
        /// The stream could not be loaded or was cut off: the connection or the server.
        case network
        /// The player could not make sense of the file.
        case format
        /// The player could not open an audio or video device.
        case output
        case other
    }

    /// Whether playback never got going or broke after it had.
    public nonisolated enum Phase: String, Sendable, Equatable {
        case start, playing
    }

    public nonisolated struct QualityChoice: Sendable, Equatable {
        public var id: String
        public var label: String
    }

    /// What to do about a failure.
    public nonisolated enum Recover: Sendable, Equatable {
        /// Try again from where it stopped.
        case retry
        /// Try another version (media) of the item.
        case version(Int)
        /// Transcode at the next lower quality.
        case lowerQuality
    }

    struct Failed {
        var failure: PlaybackFailure
        var request: Player.PlayRequest
        /// The account and server it failed on: its rating keys mean nothing elsewhere.
        var scope: Scope?
        var at: ContinuousClock.Instant
        var autoRetried: Bool
    }

    final class State {
        var failed: Failed?
        var nextId = 1
    }

    static func state(_ app: AppState) -> State {
        app.local(State.self) { State() }
    }

    /// The failure waiting for a decision, if any.
    public static func current(_ app: AppState) -> PlaybackFailure? {
        state(app).failed?.failure
    }

    /// Forget the failure (dismissed, or something else started playing).
    public static func clear(_ app: AppState) {
        let s = state(app)
        if s.failed != nil {
            s.failed = nil
            app.emit(.playbackFailure(nil))
        }
    }

    static func scopeOf(_ app: AppState) -> Scope? {
        app.session().map(Scope.of)
    }

    /// The account, profile or server changed: a failure from another one can't be acted on here.
    public static func sessionChanged(_ app: AppState, _ session: Session?) {
        let s = state(app)
        if let f = s.failed, !Scope.eq(f.scope, session.map(Scope.of)) {
            s.failed = nil
            app.emit(.playbackFailure(nil))
        }
    }

    /// Everything about a failure that the player knows.
    public struct Report {
        public var item: Metadata
        public var error: String?
        /// Set for failures from before the engine was involved, whose `error` is readable already.
        public var kind: Kind?
        /// Set for the engine's own failures; `error` is then what the engine said.
        public var engineKind: EngineFailureKind?
        public var phase: Phase
        public var positionMs: Int
        public var mediaIndex: Int
        public var quality: String
        public var remaining: Int
        /// How to start the same playback again (from the failed item on).
        public var request: Player.PlayRequest
        /// The account and server playback started on: its rating keys mean nothing elsewhere.
        public var scope: Scope?

        public init(
            item: Metadata, error: String?, kind: Kind? = nil, engineKind: EngineFailureKind? = nil, phase: Phase,
            positionMs: Int, mediaIndex: Int, quality: String, remaining: Int, request: Player.PlayRequest,
            scope: Scope?
        ) {
            self.item = item
            self.error = error
            self.kind = kind
            self.engineKind = engineKind
            self.phase = phase
            self.positionMs = positionMs
            self.mediaIndex = mediaIndex
            self.quality = quality
            self.remaining = remaining
            self.request = request
            self.scope = scope
        }
    }

    /// Publish a failure and keep what is needed to act on it.
    public static func record(_ app: AppState, _ report: Report) {
        // A failure that arrives after switching account or server can't be retried here.
        guard Scope.eq(report.scope, scopeOf(app)) else {
            log.info("dropping a playback failure from another account or server")
            return
        }
        // A failure from before the engine was involved already carries readable text; the
        // engine's is translated.
        let kind: Kind
        let reason: String
        let detail: String?
        let said = report.error.flatMap { $0.isEmpty ? nil : $0 }
        if let k = report.kind {
            (kind, reason, detail) = (k, report.error ?? "", nil)
        } else if let ek = report.engineKind {
            (kind, reason) = explainEngine(ek)
            detail = said
        } else {
            (kind, reason) = explain(report.error ?? "")
            detail = said
        }
        let video = ["movie", "episode", "clip", "trailer"].contains(Metadata.kind(report.item))
        let lower = video ? lowerQuality(report.quality) : nil
        let s = state(app)
        let failure = PlaybackFailure(
            id: s.nextId, item: report.item, reason: reason, detail: detail, kind: kind, phase: report.phase,
            positionMs: max(0, report.positionMs), mediaIndex: report.mediaIndex, quality: report.quality,
            lowerQuality: lower.map { QualityChoice(id: $0.id, label: $0.label) }, canRetry: !report.request.live,
            remaining: report.remaining)
        s.nextId += 1
        app.error("Couldn't play \(Metadata.fullTitle(failure.item)): \(failure.reason)")
        app.emit(.playbackFailure(failure))
        s.failed = Failed(failure: failure, request: report.request, scope: report.scope, at: .now, autoRetried: false)
    }

    /// Turn what the engine reported into a kind of failure and a sentence for people.
    public nonisolated static func explainEngine(_ kind: EngineFailureKind) -> (Kind, String) {
        switch kind {
        case .network:
            (
                .network,
                "The stream could not be loaded. The connection to the server may have dropped, or the server could not serve this file."
            )
        case .unsupported: (.format, "This file's format or codec can't be played on this device.")
        case .other: (.other, "The player stopped with an error.")
        }
    }

    /// Turn what the player said into a kind of failure and a sentence for people.
    public nonisolated static func explain(_ error: String) -> (Kind, String) {
        let e = error.lowercased()
        if e.contains("loading failed") {
            return (
                .network,
                "The stream could not be loaded. The connection to the server may have dropped, or the server could not serve this file."
            )
        } else if e.contains("unrecognized file format") || e.contains("unknown format") {
            return (.format, "The player does not recognise this file's format.")
        } else if e.contains("no audio or video") || e.contains("nothing to play") {
            return (.format, "The file has no audio or video that the player could play.")
        } else if e.contains("demux") || e.contains("decod") {
            return (.format, "The player could not read or decode this file.")
        } else if e.contains("audio output") {
            return (.output, "The player could not open the audio output. Check your sound device.")
        } else if e.contains("video output") {
            return (.output, "The player could not open the video output.")
        }
        return (.other, "The player stopped with an error.")
    }

    /// A transport failure (connect, timeout, request) or no reachable connection: the
    /// connection or the server.
    nonisolated static func isNetworkError(_ e: any Error) -> Bool {
        guard let e = e as? PlexError else { return false }
        return e.kind == .http || e.kind == .noConnection
    }

    /// Classify an error from before the engine was involved (building the queue, starting the player).
    public nonisolated static func explainError(_ e: any Error) -> (Kind, String) {
        // Walk the cause chain like `anyhow::Error::chain`.
        var kind = Kind.other
        var cause: (any Error)? = e
        var depth = 0
        while let c = cause, depth < 16 {
            if isNetworkError(c) {
                kind = .network
                break
            }
            cause = (c as? ChainedError)?.cause
            depth += 1
        }
        return (kind, errorText(e))
    }

    /// The preset one step below `current`, as a fallback for a stream that will not play or
    /// keep up. Direct play steps down to a 1080p transcode first; a 4K transcode is only
    /// offered by choice.
    public nonisolated static func lowerQuality(_ current: String) -> Quality? {
        let all = Config.QUALITIES
        if current == "original" || Config.quality(current) == nil { return all.first { $0.id == "20m-1080" } }
        guard let at = all.firstIndex(where: { $0.id == current }), at + 1 < all.count else { return nil }
        return all[at + 1]
    }

    /// Act on the failure shown to the user.
    public static func recover(_ app: AppState, _ action: Recover) throws {
        let s = state(app)
        guard let failed = s.failed else { throw CoreError("There is nothing to retry.") }
        s.failed = nil
        let failure = failed.failure
        var request = failed.request
        guard Scope.eq(failed.scope, scopeOf(app)) else {
            app.emit(.playbackFailure(nil))
            throw CoreError("The account or server changed since this failed, so it can't be retried.")
        }
        switch action {
        case .retry:
            break
        case .version(let index):
            guard index >= 0, index < max(failure.item.media.count, 1) else {
                throw CoreError("That version does not exist.")
            }
            request.mediaIndex = index
        case .lowerQuality:
            guard let q = failure.lowerQuality else { throw CoreError("There is no lower quality to try.") }
            request.quality = q.id
        }
        app.emit(.playbackFailure(nil))
        request.resume = true
        Player.playFrom(app, request, failure.positionMs)
    }

    /// The server answered again after a network failure: try once more, on our own.
    public static func onReconnect(_ app: AppState) {
        let s = state(app)
        guard let f = s.failed, f.failure.kind == .network, f.failure.canRetry, Scope.eq(f.scope, scopeOf(app)),
            !f.autoRetried, ContinuousClock.now - f.at < .seconds(15 * 60), !Player.isActive(app)
        else { return }
        s.failed?.autoRetried = true
        app.toast("Reconnected; resuming playback")
        try? recover(app, .retry)
    }
}
