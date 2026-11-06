// How the bytes of one download move (no TypeScript twin: on the desktop `fetchOne` streams
// the response itself). `fetchOne` (Downloads.swift) decides what to fetch and saves what the
// server said; a `Transfer` fetches it into the `.part` file. Two kinds:
//
// * `ForegroundTransfer` streams the response as the TypeScript does (tests, and the app
//   when it is given its own URL session).
// * `BackgroundTransfer` (Downloads+Background.swift) hands the request to a background
//   `URLSession`, which the system continues while the app is suspended or not running.

import Foundation
import PlexKit
import Synchronization

extension Downloads {
    /// One file to fetch.
    nonisolated struct Job: Sendable {
        var id: String
        var server: PlexServer
        /// The part's key on the server.
        var key: String
        /// The finished file; the bytes collect in `<path>.part`.
        var path: URL
        /// Bytes of `.part` to keep (0: start over).
        var requested: Int
        /// The saved validator of those bytes, sent as `If-Range`.
        var validator: String?
        /// The part's size, the total when the server doesn't say.
        var partSize: Int?

        var partial: URL { URL(fileURLWithPath: path.path + ".part") }
    }

    /// What `fetchOne` does along the way.
    struct Hooks {
        /// The server's answer checked out: the body starts at `offset` of `total`, identified
        /// by `validator`. Saves that and returns whether the download is still wanted.
        let ready: (_ offset: Int, _ total: Int?, _ validator: String?) async throws -> Bool
        /// Bytes held so far, offset included.
        let progress: (Int) -> Void
        /// Whether the download is still wanted (pause and remove stop it).
        let active: () -> Bool
        /// The system holds the transfer now (or no longer).
        let handedOff: (Bool) -> Void
    }

    /// Where the transfer of the app goes: a background session in the app, a foreground stream
    /// when the environment brings its own URL session (tests) or off iOS.
    static func transfer(_ app: AppState) -> any Transfer {
        #if os(iOS)
            if app.environment.urlSession == nil { return BackgroundTransfer.shared(app.paths) }
        #endif
        return foreground
    }

    static let foreground = ForegroundTransfer()
}

/// Moves the bytes of one download into its `.part` file.
protocol Transfer: AnyObject {
    /// Fetch `job`: append to `.part` from the offset the server agreed to, or start it over.
    /// Returns the bytes in `.part` once the file is complete, `nil` when the download stopped
    /// being active (whatever allows resuming is kept).
    func fetch(_ job: Downloads.Job, _ hooks: Downloads.Hooks) async throws -> Int?
}

/// Streams the response into `.part` (the TypeScript `fetchOne` loop).
final class ForegroundTransfer: Transfer {
    func fetch(_ job: Downloads.Job, _ hooks: Downloads.Hooks) async throws -> Int? {
        var requested = job.requested
        let server = job.server
        var resp: PlexResponse
        do {
            resp = try await server.downloadRange(job.key, offset: requested, validator: job.validator)
        } catch let e as PlexError where e.kind == .status && e.status == 416 {
            requested = 0
            resp = try await server.downloadRange(job.key, offset: 0, validator: nil)
        }
        var answer = try Downloads.judge(resp.response, requested: requested, saved: job.validator, partSize: job.partSize) {
            Data()
        }
        if answer == .restart {
            resp.bytes.task.cancel()
            requested = 0
            resp = try await server.downloadRange(job.key, offset: 0, validator: nil)
            answer = try Downloads.judge(resp.response, requested: 0, saved: nil, partSize: job.partSize) { Data() }
        }
        guard case .accept(let offset, let total, let validator) = answer else { return nil }
        guard try await hooks.ready(offset, total, validator) else {
            resp.bytes.task.cancel()
            return nil
        }
        // create + write + truncate when starting over, append when resuming.
        let fd = open(job.partial.path, offset > 0 ? O_WRONLY | O_APPEND | O_CREAT : O_WRONLY | O_CREAT | O_TRUNC, 0o644)
        guard fd >= 0 else {
            resp.bytes.task.cancel()
            throw TransferError("writing the file: \(String(cString: strerror(errno)))", retry: false)
        }
        let pump = Pump(offset)
        let body = resp
        let reader = Task.detached { await pump.run(body, fd) }
        func stop() async {
            reader.cancel()
            body.bytes.task.cancel()
            await reader.value
        }
        var lastControl = ContinuousClock.now - .seconds(1)
        while true {
            let s = pump.state.withLock { $0 }
            if let error = s.error { throw error }
            if s.finished { break }
            if ContinuousClock.now - s.lastChunk >= .seconds(60) {
                await stop()
                throw TransferError("The download stalled. Resume to try again.", retry: false)
            }
            if ContinuousClock.now - lastControl >= .milliseconds(250) {
                lastControl = ContinuousClock.now
                if !hooks.active() {
                    await stop()
                    return nil
                }
            }
            hooks.progress(s.received)
            do {
                try await Task.sleep(for: .milliseconds(50))
            } catch {
                await stop()
                throw error
            }
        }
        let received = pump.state.withLock { $0.received }
        if let total, total != received { throw CoreError("Download ended before the complete file arrived.") }
        return received
    }
}

/// Reads a response body into a file off the main actor, and says how far it got.
nonisolated final class Pump: Sendable {
    struct State: Sendable {
        var received: Int
        var lastChunk = ContinuousClock.now
        var finished = false
        var error: (any Error)?
    }

    private struct WriteFailed: Error {
        let reason: String
    }

    let state: Mutex<State>

    init(_ offset: Int) {
        state = Mutex(State(received: offset))
    }

    @concurrent func run(_ resp: PlexResponse, _ fd: Int32) async {
        defer { close(fd) }
        var buf = [UInt8]()
        buf.reserveCapacity(1 << 16)
        func flush() throws(WriteFailed) {
            var at = 0
            while at < buf.count {
                let n = buf.withUnsafeBytes { write(fd, $0.baseAddress! + at, buf.count - at) }
                if n <= 0 { throw WriteFailed(reason: String(cString: strerror(errno))) }
                at += n
            }
            let n = buf.count
            state.withLock { $0.received += n }
            buf.removeAll(keepingCapacity: true)
        }
        do {
            do {
                for try await b in resp.bytes {
                    buf.append(b)
                    if buf.count & 0xFFF == 0 {
                        state.withLock { $0.lastChunk = ContinuousClock.now }
                        if buf.count >= 1 << 16 { try flush() }
                    }
                }
            } catch let e as WriteFailed {
                throw e
            } catch {
                // What arrived before the failure is good, and lets a retry resume after it.
                try flush()
                if !Task.isCancelled {
                    let e = TransferError("downloading: error decoding response body: \(errorText(error))", retry: true)
                    state.withLock { $0.error = e }
                }
                return
            }
            try flush()
            // fsync the data before it becomes the finished file.
            guard fsync(fd) == 0 else { throw WriteFailed(reason: String(cString: strerror(errno))) }
            state.withLock { $0.finished = true }
        } catch {
            let reason = (error as? WriteFailed)?.reason ?? errorText(error)
            state.withLock { $0.error = TransferError("writing the file: \(reason)", retry: false) }
        }
    }
}
