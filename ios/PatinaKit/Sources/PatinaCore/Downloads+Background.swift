// Transfers in a background URLSession (no TypeScript twin; see Downloads+Transfer.swift).
//
// The system runs the transfer, also while the app is suspended or after it was terminated,
// and relaunches the app to deliver the result (`Downloads.handleBackgroundEvents`). So the
// delegate keeps nothing it can't find again: tasks carry the download id as their
// `taskDescription`, and what a task was asked for, its finished body and its resume data are
// staged in `<data>/transfers/<digest of id>.*`. The worker's `fetch` picks a staged result up
// whether or not it was waiting when it came in. Pausing cancels the task with resume data;
// resuming uses that data when there is some, else a range request with the validator, which
// is checked once the answer is in (`Downloads.judge`). Not unit-tested (background sessions
// need the app); the logic it shares with the foreground transfer is.

import Foundation
import PlexKit
import Synchronization

extension Downloads {
    /// For the app delegate's `application(_:handleEventsForBackgroundURLSession:completionHandler:)`:
    /// reconnects to the downloads session and calls `completion` once its events are delivered.
    /// Returns false (and leaves `completion` alone) for other sessions.
    @discardableResult
    public static func handleBackgroundEvents(identifier: String, completion: @escaping @Sendable () -> Void) -> Bool {
        guard identifier == BackgroundTransfer.identifier else { return false }
        BackgroundTransfer.shared(Paths.standard()).relay.onEventsDelivered(completion)
        return true
    }

    /// The staging files of a download's background transfer.
    nonisolated static func stage(_ paths: Paths, _ id: String) -> Stage {
        Stage(base: paths.data.appendingPathComponent("transfers").appendingPathComponent(Offline.digest(id)))
    }

    nonisolated struct Stage: Sendable {
        let base: URL
        /// What the task asked for (`Started`).
        var started: URL { URL(fileURLWithPath: base.path + ".task") }
        var resumeData: URL { URL(fileURLWithPath: base.path + ".resume") }
        /// The finished body, and the answer it came with (`Finished`).
        var body: URL { URL(fileURLWithPath: base.path + ".body") }
        var answer: URL { URL(fileURLWithPath: base.path + ".answer") }

        func clear() {
            for f in [started, resumeData, body, answer] { unlink(f.path) }
        }

        func clearResult() {
            unlink(body.path)
            unlink(answer.path)
        }
    }

    nonisolated struct Started: Codable, Sendable {
        var requested: Int
        var validator: String?
        /// Continues from resume data: the system checks the source and assembles the whole file.
        var resumed: Bool
    }

    nonisolated struct Finished: Codable, Sendable {
        var url: String
        var status: Int
        var headers: [String: String]

        var response: HTTPURLResponse? {
            URL(string: url).flatMap { HTTPURLResponse(url: $0, statusCode: status, httpVersion: nil, headerFields: headers) }
        }
    }
}

final class BackgroundTransfer: Transfer {
    nonisolated static let identifier = "io.github.madrazaldi.patina.downloads"
    private static var instance: BackgroundTransfer?

    /// The one session of the process (a background session's identifier must be unique).
    static func shared(_ paths: Paths) -> BackgroundTransfer {
        if let instance { return instance }
        let made = BackgroundTransfer(paths)
        instance = made
        return made
    }

    let paths: Paths
    let relay: Relay
    private let session: URLSession
    private var reconnected: Task<Void, Never>?

    private init(_ paths: Paths) {
        self.paths = paths
        relay = Relay(paths)
        let config = URLSessionConfiguration.background(withIdentifier: Self.identifier)
        config.sessionSendsLaunchEvents = true
        config.isDiscretionary = false
        config.urlCache = nil
        config.httpCookieStorage = nil
        config.httpShouldSetCookies = false
        session = URLSession(configuration: config, delegate: relay, delegateQueue: nil)
        // Transfers of a previous launch are still running: find them again.
        let session = session
        let relay = relay
        reconnected = Task {
            for task in await session.allTasks {
                guard let t = task as? URLSessionDownloadTask, let id = t.taskDescription,
                    Downloads.entry(paths, id) != nil
                else {
                    task.cancel()
                    continue
                }
                relay.track(id, t)
            }
        }
    }

    func fetch(_ job: Downloads.Job, _ hooks: Downloads.Hooks) async throws -> Int? {
        await reconnected?.value
        let stage = Downloads.stage(paths, job.id)
        try FileManager.default.createDirectory(
            at: stage.base.deletingLastPathComponent(), withIntermediateDirectories: true)

        func started() -> Downloads.Started? {
            try? Storage.readJson(stage.started) { Downloads.Started?.none }
        }
        // Where the bytes the task writes go in the file.
        var base = started().map { $0.resumed ? 0 : $0.requested } ?? 0

        func start(_ requested: Int, _ validator: String?) throws {
            let task: URLSessionDownloadTask
            let resumed: Bool
            if let data = try? Data(contentsOf: stage.resumeData), !data.isEmpty {
                unlink(stage.resumeData.path)
                task = session.downloadTask(withResumeData: data)
                resumed = true
            } else {
                var req = try job.server.downloadRequest(job.key, offset: requested, validator: validator)
                // An idle minute fails the task, like the foreground transfer's stall check.
                req.timeoutInterval = 60
                task = session.downloadTask(with: req)
                resumed = false
            }
            try Storage.write(
                stage.started,
                Storage.pretty(Downloads.Started(requested: requested, validator: validator, resumed: resumed)))
            base = resumed ? 0 : requested
            task.taskDescription = job.id
            relay.track(job.id, task)
            task.resume()
        }
        func pause() async {
            if let data = await relay.cancel(job.id, keepResumeData: true) { try? Storage.write(stage.resumeData, data) }
        }

        /// Where the body starts, judged from the answer the task got.
        func judge(_ resp: HTTPURLResponse, _ body: () -> Data) throws -> Downloads.Answer {
            let s = started()
            if s?.resumed == true {
                // The system matched the source itself; take its range as the one asked for.
                let start = resp.value(forHTTPHeaderField: "Content-Range").flatMap(Self.rangeStart) ?? 0
                return try Downloads.judge(
                    resp, requested: start, saved: Downloads.responseValidator(resp), partSize: job.partSize, body: body)
            }
            return try Downloads.judge(
                resp, requested: s?.requested ?? 0, saved: s?.validator, partSize: job.partSize, body: body)
        }

        var answered = false
        func restart() async throws {
            _ = await relay.cancel(job.id, keepResumeData: false)
            stage.clear()
            answered = false
            try start(0, nil)
        }

        if !relay.tracking(job.id) && !FileManager.default.fileExists(atPath: stage.answer.path) {
            stage.clearResult()
            try start(job.requested, job.validator)
        }
        hooks.handedOff(true)
        defer { hooks.handedOff(false) }
        var lastControl = ContinuousClock.now - .seconds(1)
        while true {
            if let finished = try? Storage.readJson(stage.answer, { Downloads.Finished?.none }), let resp = finished.response {
                relay.forget(job.id)
                let answer: Downloads.Answer
                do {
                    answer = try judge(resp) { (try? Data(contentsOf: stage.body)) ?? Data() }
                } catch {
                    stage.clear()
                    throw error
                }
                guard case .accept(let offset, let total, let validator) = answer else {
                    try await restart()
                    continue
                }
                if !answered {
                    answered = true
                    guard try await hooks.ready(offset, total, validator) else {
                        stage.clear()
                        return nil
                    }
                }
                let resumed = started()?.resumed ?? false
                try Self.place(stage.body, job.partial, append: !resumed && offset > 0)
                stage.clear()
                let received = (try? FileManager.default.attributesOfItem(atPath: job.partial.path)[.size] as? Int) ?? 0
                if let total, total != received { throw CoreError("Download ended before the complete file arrived.") }
                return received
            }
            if let error = relay.takeError(job.id) {
                relay.forget(job.id)
                throw Self.mapped(error)
            }
            if !relay.tracking(job.id) {
                // The task ended without a result we saw (it failed before this launch found it).
                try start(job.requested, job.validator)
            }
            if ContinuousClock.now - lastControl >= .milliseconds(250) {
                lastControl = ContinuousClock.now
                if !hooks.active() {
                    await pause()
                    return nil
                }
            }
            if !answered, let resp = relay.response(job.id) {
                do {
                    switch try judge(resp, { Data() }) {
                    case .restart:
                        try await restart()
                        continue
                    case .accept(let offset, let total, let validator):
                        answered = true
                        guard try await hooks.ready(offset, total, validator) else {
                            await pause()
                            return nil
                        }
                    }
                } catch let e as PlexError where e.kind == .status {
                    // An error answer: its body is in the result, which says it better.
                } catch {
                    await pause()
                    throw error
                }
            }
            hooks.progress(relay.received(job.id, offset: base))
            do {
                try await Task.sleep(for: .milliseconds(250))
            } catch {
                await pause()
                throw error
            }
        }
    }

    /// The first byte of a `Content-Range` (`bytes 500-999/1000`).
    nonisolated static func rangeStart(_ range: String) -> Int? {
        guard range.hasPrefix("bytes "), let dash = range.firstIndex(of: "-") else { return nil }
        return Int(range[range.index(range.startIndex, offsetBy: 6)..<dash])
    }

    /// Move a finished body into `.part`, or append it to what `.part` holds.
    nonisolated static func place(_ body: URL, _ partial: URL, append: Bool) throws {
        let fm = FileManager.default
        do {
            if !append {
                if fm.fileExists(atPath: partial.path) { try fm.removeItem(at: partial) }
                try fm.moveItem(at: body, to: partial)
                return
            }
            let out = try FileHandle(forWritingTo: partial)
            defer { try? out.close() }
            try out.seekToEnd()
            let input = try FileHandle(forReadingFrom: body)
            defer { try? input.close() }
            while let chunk = try input.read(upToCount: 1 << 20), !chunk.isEmpty { try out.write(contentsOf: chunk) }
            try out.synchronize()
        } catch {
            throw TransferError("writing the file: \(error.localizedDescription)", retry: false)
        }
    }

    /// Transport errors as the foreground transfer reports them.
    nonisolated static func mapped(_ error: any Error) -> any Error {
        guard let u = error as? URLError else { return error }
        if u.code == .timedOut { return TransferError("The download stalled. Resume to try again.", retry: false) }
        return TransferError("downloading: \(u.localizedDescription)", retry: true)
    }
}

/// The session delegate. Runs on the session's queue; what it learns goes in `flights`, and
/// finished bodies go to the stage before the system deletes them.
nonisolated final class Relay: NSObject, URLSessionDownloadDelegate, Sendable {
    struct Flight: @unchecked Sendable {
        var task: URLSessionDownloadTask
        var written = 0
        var response: HTTPURLResponse?
        var error: (any Error)?
    }

    let paths: Paths
    private let flights = Mutex<[String: Flight]>([:])
    private let eventsDelivered = Mutex<(@Sendable () -> Void)?>(nil)

    init(_ paths: Paths) {
        self.paths = paths
    }

    func track(_ id: String, _ task: URLSessionDownloadTask) {
        flights.withLock { $0[id] = Flight(task: task) }
    }

    func tracking(_ id: String) -> Bool {
        flights.withLock { $0[id] != nil }
    }

    func forget(_ id: String) {
        flights.withLock { _ = $0.removeValue(forKey: id) }
    }

    func response(_ id: String) -> HTTPURLResponse? {
        flights.withLock { $0[id]?.response }
    }

    func takeError(_ id: String) -> (any Error)? {
        flights.withLock { f in
            defer { f[id]?.error = nil }
            return f[id]?.error
        }
    }

    /// Bytes held: what the task wrote, after `offset` bytes of `.part` it continues.
    func received(_ id: String, offset: Int) -> Int {
        offset + flights.withLock { $0[id]?.written ?? 0 }
    }

    /// Stop the task; its resume data when asked for and the server allows resuming.
    func cancel(_ id: String, keepResumeData: Bool) async -> Data? {
        guard let task = flights.withLock({ $0.removeValue(forKey: id)?.task }) else { return nil }
        guard keepResumeData else {
            task.cancel()
            return nil
        }
        return await withCheckedContinuation { done in task.cancel { done.resume(returning: $0) } }
    }

    func onEventsDelivered(_ completion: @escaping @Sendable () -> Void) {
        eventsDelivered.withLock { $0 = completion }
    }

    func urlSession(
        _ session: URLSession, downloadTask: URLSessionDownloadTask, didWriteData bytesWritten: Int64,
        totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64
    ) {
        guard let id = downloadTask.taskDescription else { return }
        let response = downloadTask.response as? HTTPURLResponse
        flights.withLock { f in
            f[id]?.written = Int(totalBytesWritten)
            if let response { f[id]?.response = response }
        }
    }

    func urlSession(
        _ session: URLSession, downloadTask: URLSessionDownloadTask, didResumeAtOffset fileOffset: Int64,
        expectedTotalBytes: Int64
    ) {
        guard let id = downloadTask.taskDescription else { return }
        flights.withLock { $0[id]?.written = Int(fileOffset) }
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {
        guard let id = downloadTask.taskDescription else { return }
        // `location` is deleted when this returns.
        let stage = Downloads.stage(paths, id)
        let resp = downloadTask.response as? HTTPURLResponse
        do {
            try FileManager.default.createDirectory(
                at: stage.base.deletingLastPathComponent(), withIntermediateDirectories: true)
            unlink(stage.body.path)
            try FileManager.default.moveItem(at: location, to: stage.body)
            var headers: [String: String] = [:]
            for (k, v) in resp?.allHeaderFields ?? [:] {
                if let k = k as? String, let v = v as? String { headers[k] = v }
            }
            let finished = Downloads.Finished(
                url: resp?.url?.absoluteString ?? downloadTask.originalRequest?.url?.absoluteString ?? "",
                status: resp?.statusCode ?? 0, headers: headers)
            try Storage.write(stage.answer, Storage.pretty(finished))
        } catch {
            flights.withLock { $0[id]?.error = TransferError("writing the file: \(errorText(error))", retry: false) }
        }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: (any Error)?) {
        guard let id = task.taskDescription, let error else { return }
        let ns = error as NSError
        if let data = ns.userInfo[NSURLSessionDownloadTaskResumeData] as? Data {
            try? Storage.write(Downloads.stage(paths, id).resumeData, data)
        }
        // Cancelled by `cancel`, which already stopped tracking it.
        if ns.domain == NSURLErrorDomain && ns.code == NSURLErrorCancelled { return }
        flights.withLock { $0[id]?.error = error }
    }

    func urlSessionDidFinishEvents(forBackgroundURLSession session: URLSession) {
        let completion = eventsDelivered.withLock { f in
            defer { f = nil }
            return f
        }
        DispatchQueue.main.async { completion?() }
    }
}
