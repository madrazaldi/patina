// Port of desktop/core/report.ts.
// An ordered worker for everything the player tells Plex.
//
// Timeline reports, scrobbles, stream choices and play-queue edits used to be awaited while
// handling the player's events, so one slow or failing request (the shared HTTP timeout is
// 60 seconds) delayed queue edits and playback updates. The player now only enqueues an
// `Op`; one task sends them in order and keeps the backlog small while Plex is slow:
//
// * progress checkpoints for the same item collapse into the newest one;
// * the final `stopped` report replaces the pending progress it supersedes and is never
//   dropped (it, scrobbles and stream choices are retried);
// * a progress report is never sent after a newer report of the same item was queued, and
//   requests that hang are cut off (cancelled) after a timeout.
//
// On iOS the app also calls `Report.flush` when it moves to the background, so the last
// reports go out before the process is suspended.

import Foundation
import PlexKit

nonisolated private let log = logger("report")

public enum Report {
    /// Edits mirrored to the server's play queue.
    public nonisolated enum QueueOp: Sendable, Equatable {
        case remove(queue: Int, item: Int)
        case move(queue: Int, item: Int, after: Int?)
        case shuffle(queue: Int)
        case unshuffle(queue: Int)
    }

    /// One thing to tell the server.
    public nonisolated enum Op: Sendable, Equatable {
        case timeline(TimelineUpdate)
        case scrobble(ratingKey: String)
        case streams(partId: Int, audio: Int?, subtitle: Int?)
        case playQueue(QueueOp)
        case pingTranscode(session: String)
        case stopTranscode(session: String)
    }

    /// The item a timeline report or scrobble is about.
    nonisolated static func subject(_ op: Op) -> String? {
        switch op {
        case .timeline(let u): u.ratingKey
        case .scrobble(let rk): rk
        default: nil
        }
    }

    public nonisolated static func isProgress(_ op: Op) -> Bool {
        if case .timeline(let u) = op { return u.state != .stopped }
        return false
    }

    /// Worth retrying: losing it would leave the server with a wrong resume point or watch state.
    nonisolated static func isImportant(_ op: Op) -> Bool {
        switch op {
        case .timeline(let u): u.state == .stopped
        case .scrobble, .streams, .playQueue, .stopTranscode: true
        case .pingTranscode: false
        }
    }

    /// An `Op` for one server (`T`; a `PlexServer` in the app).
    public nonisolated struct Job<T: Sendable>: Sendable {
        public var target: T
        public var op: Op
        public var attempts: Int
        /// When the job may be tried again.
        public var notBefore: ContinuousClock.Instant?
    }

    /// The backlog, with coalescing applied as jobs are added.
    public nonisolated struct Pending<T: Sendable> {
        private var jobs: [Job<T>] = []
        /// `PartialEq` on the target.
        private let eq: (T, T) -> Bool

        public init(_ eq: @escaping (T, T) -> Bool) {
            self.eq = eq
        }

        public var count: Int { jobs.count }
        public var isEmpty: Bool { jobs.isEmpty }

        public func ops() -> [Op] {
            jobs.map(\.op)
        }

        public mutating func push(_ target: T, _ op: Op) {
            let job = Job(target: target, op: op, attempts: 0, notBefore: nil)
            switch op {
            case .timeline:
                // Fold into the last pending job about this item when that is progress: only
                // the newest checkpoint (or the final stop) matters.
                let last = jobs.lastIndex { eq($0.target, target) && subject($0.op) == subject(op) }
                if let last, isProgress(jobs[last].op) {
                    jobs[last] = Job(target: target, op: op, attempts: 0, notBefore: jobs[last].notBefore)
                } else {
                    jobs.append(job)
                }
            case .streams(let partId, let audio, let subtitle):
                let same = jobs.firstIndex {
                    if case .streams(partId, _, _) = $0.op { return eq($0.target, target) }
                    return false
                }
                if let same, case .streams(_, let a, let s) = jobs[same].op {
                    jobs[same].op = .streams(partId: partId, audio: audio ?? a, subtitle: subtitle ?? s)
                } else {
                    jobs.append(job)
                }
            case .pingTranscode(let session):
                let queued = jobs.contains {
                    guard eq($0.target, target) else { return false }
                    switch $0.op {
                    case .pingTranscode(session), .stopTranscode(session): return true
                    default: return false
                    }
                }
                if !queued { jobs.append(job) }
            case .stopTranscode(let session):
                jobs.removeAll {
                    if case .pingTranscode(session) = $0.op { return eq($0.target, target) }
                    return false
                }
                jobs.append(job)
            case .scrobble, .playQueue:
                jobs.append(job)
            }
        }

        public mutating func pop() -> Job<T>? {
            jobs.isEmpty ? nil : jobs.removeFirst()
        }

        /// Requeue a failed job at the front, to keep the order it was sent in.
        public mutating func retry(_ job: Job<T>, _ delayMs: Int) {
            var job = job
            job.attempts += 1
            job.notBefore = .now + .milliseconds(delayMs)
            jobs.insert(job, at: 0)
        }

        public func nextReady() -> ContinuousClock.Instant? {
            jobs.first?.notBefore
        }
    }

    /// How the worker reaches the server: returns when delivered, throws the reason otherwise.
    /// A request the worker gives up on is cancelled (Rust drops the future), so a late answer
    /// can't land after newer reports.
    public nonisolated struct Sink<T: Sendable>: Sendable {
        public let send: @MainActor @Sendable (T, Op) async throws -> Void

        public init(_ send: @escaping @MainActor @Sendable (T, Op) async throws -> Void) {
            self.send = send
        }
    }

    /// Tuning, so tests can run quickly.
    public nonisolated struct Config: Sendable {
        /// Longest a single request may take before it is abandoned.
        public var timeoutMs: Int
        /// Attempts made at an important job (the first included).
        public var attempts: Int
        public var backoffMs: Int

        public init(timeoutMs: Int, attempts: Int, backoffMs: Int) {
            self.timeoutMs = timeoutMs
            self.attempts = attempts
            self.backoffMs = backoffMs
        }
    }

    public static func defaultConfig() -> Config {
        Config(timeoutMs: 10_000, attempts: 4, backoffMs: 2000)
    }

    private enum Outcome: Sendable {
        case ok
        case timedOut
        case failed(String)
    }

    /// The sending side and its worker. Cheap to use from anywhere; never blocks.
    public final class Reporter<T: Sendable> {
        private var pending: Pending<T>
        private let sink: Sink<T>
        private let config: Config
        private var closed = false
        /// Flushes waiting for the backlog to drain.
        private var waiters: [Int: CheckedContinuation<Bool, Never>] = [:]
        private var nextWaiter = 0
        /// The worker, parked until something arrives (or a backoff ends).
        private var wake: CheckedContinuation<Void, Never>?
        private var wakeGeneration = 0
        private var worker: Task<Void, Never>?

        private init(_ sink: Sink<T>, _ config: Config, _ eq: @escaping (T, T) -> Bool) {
            pending = Pending(eq)
            self.sink = sink
            self.config = config
        }

        /// Start a worker.
        public static func spawn(_ sink: Sink<T>, _ config: Config = Report.defaultConfig(), eq: @escaping (T, T) -> Bool)
            -> Reporter<T>
        {
            let r = Reporter(sink, config, eq)
            r.worker = Task { await r.run() }
            return r
        }

        public func send(_ target: T, _ op: Op) {
            guard !closed else { return }
            pending.push(target, op)
            signal()
        }

        /// Wait until everything sent so far was delivered or given up on; false if `timeoutMs`
        /// came first.
        public func flush(_ timeoutMs: Int) async -> Bool {
            if closed { return true }
            nextWaiter += 1
            let id = nextWaiter
            let timer = Task {
                try? await Task.sleep(for: .milliseconds(timeoutMs))
                waiters.removeValue(forKey: id)?.resume(returning: false)
            }
            let delivered = await withCheckedContinuation { c in
                waiters[id] = c
                signal()
            }
            timer.cancel()
            return delivered
        }

        /// Stop the worker once it has handled what was sent (dropping the last sender in Rust).
        public func close() {
            closed = true
            signal()
        }

        private func signal() {
            if let c = wake {
                wake = nil
                c.resume()
            }
        }

        /// Park until `signal()`, or until `deadline` when there is one.
        private func park(until deadline: ContinuousClock.Instant?) async {
            wakeGeneration += 1
            let generation = wakeGeneration
            await withCheckedContinuation { c in
                wake = c
                if let deadline {
                    Task {
                        try? await Task.sleep(until: deadline)
                        if wakeGeneration == generation { signal() }
                    }
                }
            }
        }

        private func run() async {
            while true {
                if pending.isEmpty {
                    let done = waiters
                    waiters = [:]
                    for c in done.values { c.resume(returning: true) }
                    if closed { return }
                    await park(until: nil)
                    continue
                }
                // A retried job waits out its backoff, still absorbing new jobs.
                if let at = pending.nextReady(), at > .now {
                    if closed {
                        try? await Task.sleep(until: at)
                    } else {
                        await park(until: at)
                        continue
                    }
                }
                guard let job = pending.pop() else { continue }
                let error: String
                switch await attempt(job) {
                case .ok: continue
                case .timedOut: error = "no answer within \(Double(config.timeoutMs) / 1000)s"
                case .failed(let text): error = text
                }
                if isImportant(job.op) && job.attempts + 1 < config.attempts {
                    log.debug("plex report failed, will retry: \(error)")
                    pending.retry(job, config.backoffMs * (1 << job.attempts))
                } else {
                    log.debug("plex report dropped: \(error)")
                }
            }
        }

        /// Send one job, cut off after the timeout. A request that is cut off is cancelled and
        /// waited for, so it can't reach the server after what comes next.
        private func attempt(_ job: Job<T>) async -> Outcome {
            let send = sink.send
            let timeoutMs = config.timeoutMs
            return await withTaskGroup(of: Outcome?.self) { group in
                group.addTask {
                    do {
                        try await send(job.target, job.op)
                        return .ok
                    } catch {
                        return .failed(errorText(error))
                    }
                }
                group.addTask {
                    do {
                        try await Task.sleep(for: .milliseconds(timeoutMs))
                        return .timedOut
                    } catch {
                        return nil
                    }
                }
                let first = await group.next() ?? nil
                group.cancelAll()
                return first ?? .timedOut
            }
        }
    }

    /// The app's sink: talks to the Plex server a job was queued for. Cancelling the request
    /// is the TypeScript `withSignal`.
    static let plexSink = Sink<PlexServer> { server, op in
        switch op {
        case .timeline(let u):
            try await server.timeline(u)
        case .scrobble(let rk):
            try await server.scrobble(rk)
        case .streams(let partId, let audio, let subtitle):
            try await server.selectStreams(partId, audio: audio, subtitle: subtitle, allParts: true)
        case .playQueue(.remove(let queue, let item)):
            _ = try await server.removeFromPlayQueue(queue, itemId: item)
        case .playQueue(.move(let queue, let item, let after)):
            _ = try await server.movePlayQueueItem(queue, itemId: item, after: after)
        case .playQueue(.shuffle(let queue)):
            _ = try await server.shufflePlayQueue(queue)
        case .playQueue(.unshuffle(let queue)):
            _ = try await server.unshufflePlayQueue(queue)
        case .pingTranscode(let session):
            try await server.pingTranscode(session)
        case .stopTranscode(let session):
            try await server.stopTranscode(session)
        }
    }

    private static var REPORTER: Reporter<PlexServer>?

    /// Wait for the app-wide reporter to deliver what it has (nothing to do if it never
    /// started). The app calls this when it moves to the background and before it exits.
    @discardableResult
    public static func flush(_ timeoutMs: Int = 10_000) async -> Bool {
        guard let r = REPORTER else { return true }
        return await r.flush(timeoutMs)
    }

    /// The app-wide reporter. One worker for all playback keeps reports in order across
    /// consecutive playbacks (a retry never overtakes the stop before it).
    public static func reporter() -> Reporter<PlexServer> {
        if let r = REPORTER { return r }
        let r = Reporter.spawn(plexSink, defaultConfig(), eq: sameServer)
        REPORTER = r
        return r
    }
}
