// Port of desktop/core/images.test.ts. The image server is a `MockPMS` route instead of
// `Bun.serve`, and the fetches name their server rather than setting the global one, so
// parallel suites don't see it.

import CryptoKit
import Foundation
import PlexKit
import Synchronization
import Testing

@testable import PatinaCore

/// Run `f` for `0..<n` at once on the main actor (`Promise.all`), the results in order.
private func all<T: Sendable>(_ n: Int, _ f: @escaping (Int) async -> T) async -> [T] {
    let tasks = (0..<n).map { i in Task { await f(i) } }
    var results: [T] = []
    for t in tasks { results.append(await t.value) }
    return results
}

@Suite struct ImagesTests {
    @Test func simultaneousRequestsShareOneFetch() async {
        let flights = Images.SingleFlight<Int>()
        var started = 0
        let results = await all(20) { _ in
            await flights.run("poster") {
                started += 1
                try await Task.sleep(for: .milliseconds(50))
                return 7
            }
        }
        #expect(results.allSatisfy { $0 == 7 })
        #expect(started == 1)
        #expect(flights.running == 0)  // a finished key must be forgotten
        // A later request starts again.
        #expect(await flights.run("poster") { 8 } == 8)
    }

    @Test func differentKeysRunIndependentlyAndWorkSurvivesAbandonment() async throws {
        let flights = Images.SingleFlight<Int>()
        var done = 0
        // Nobody waits for "a": dropping the caller must not cancel the work.
        let a = Task {
            await flights.run("a") {
                try await Task.sleep(for: .milliseconds(30))
                done += 1
                return 1
            }
        }
        a.cancel()
        #expect(await flights.run("b") { 2 } == 2)
        try await Task.sleep(for: .milliseconds(100))
        #expect(done == 1)
        #expect(flights.running == 0)
    }

    @Test func aFailingTaskIsReportedAndForgotten() async {
        struct Boom: Error {}
        let flights = Images.SingleFlight<Int>()
        #expect(await flights.run("x") { throw Boom() } == nil)
        #expect(flights.running == 0)  // a failure must not wedge the key
        #expect(await flights.run("x") { 3 } == 3)
    }

    @Test func fetchesAreBounded() async {
        var running = 0
        var peak = 0
        let permits = Images.Semaphore(3)
        let flights = Images.SingleFlight<Int>()
        _ = await all(12) { i in
            await flights.run("k\(i)") {
                let release = await permits.acquire()
                running += 1
                peak = max(peak, running)
                try await Task.sleep(for: .milliseconds(20))
                running -= 1
                release()
                return i
            }
        }
        #expect(peak == 3)
    }

    func entry(_ dir: URL, _ name: String, _ bytes: Int, _ used: Date) throws {
        let sub = dir.appendingPathComponent(String(name.prefix(2)))
        try FileManager.default.createDirectory(at: sub, withIntermediateDirectories: true)
        let path = sub.appendingPathComponent(name)
        try Data(repeating: 1, count: bytes).write(to: path)
        // A new file's access time is now; set both so the entry looks last used at `used`.
        let t = used.timeIntervalSince1970
        var times = [timeval(tv_sec: Int(t), tv_usec: 0), timeval(tv_sec: Int(t), tv_usec: 0)]
        #expect(utimes(path.path, &times) == 0)
    }

    func names(_ dir: URL) -> [String] {
        let fm = FileManager.default
        return ((try? fm.contentsOfDirectory(atPath: dir.path)) ?? [])
            .flatMap { (try? fm.contentsOfDirectory(atPath: dir.appendingPathComponent($0).path)) ?? [] }
            .sorted()
    }

    @Test func pruneDropsOldEntriesAndDeadTemporaryFiles() throws {
        let tmp = TempDir("patina-images")
        let now = Date()
        let day: TimeInterval = 86_400
        try entry(tmp.url, "aa-fresh", 10, now - day)
        try entry(tmp.url, "bb-stale", 10, now - 40 * day)
        try entry(tmp.url, "cc-dead.tmp-1", 10, now - 2 * day)
        try entry(tmp.url, "dd-writing.tmp-2", 10, now)
        let pruned = Images.pruneDir(tmp.url, now, 30 * day, Int.max)
        #expect(pruned == Images.Pruned(files: 2, bytes: 20))
        #expect(names(tmp.url) == ["aa-fresh", "dd-writing.tmp-2"])
    }

    @Test func pruneEnforcesTheSizeBudgetOldestFirst() throws {
        let tmp = TempDir("patina-images")
        let now = Date()
        for i in 0..<10 {
            // e0 is the most recently used, e9 the least.
            try entry(tmp.url, "e\(i)", 100, now - 3600 * Double(i + 1))
        }
        // 1000 bytes against a 500 budget trims to 90% of it (450): the six least recently used go.
        let pruned = Images.pruneDir(tmp.url, now, 86_400 * 30, 500)
        #expect(pruned.bytes == 600)
        #expect(names(tmp.url) == ["e0", "e1", "e2", "e3"])
        // Under budget: nothing changes.
        #expect(Images.pruneDir(tmp.url, now, 86_400 * 30, 500) == Images.Pruned())
    }

    @Test func atomicWritesLeaveNoTemporaryFiles() async throws {
        let tmp = TempDir("patina-images")
        let path = tmp.url.appendingPathComponent("ab").appendingPathComponent("abcdef")
        try await Images.writeAtomic(path, Data("first".utf8))
        try await Images.writeAtomic(path, Data("second".utf8))
        #expect(try String(contentsOf: path, encoding: .utf8) == "second")
        #expect(names(tmp.url) == ["abcdef"])
        // An unwritable destination cleans up after itself.
        let blocked = path.appendingPathComponent("child")
        await #expect(throws: (any Error).self) { try await Images.writeAtomic(blocked, Data("x".utf8)) }
        #expect(names(tmp.url) == ["abcdef"])
    }

    /// When each request began.
    final class Starts: Sendable {
        let list = Mutex<[ContinuousClock.Instant]>([])
    }

    /// Answers every photo request with a small PNG after `delay`, logging when each began.
    func slowImageServer(_ delay: Double) -> (MockPMS, Starts) {
        let mock = MockPMS()
        let starts = Starts()
        let body = Data([0x89, 0x50, 0x4E, 0x47, 1, 2, 3, 4])
        mock.on { _, req in
            guard req.path == "/photo/:/transcode" else { return nil }
            starts.list.withLock { $0.append(ContinuousClock.now) }
            return MockResponse(headers: ["Content-Type": "image/png"], body: body, delay: delay)
        }
        return (mock, starts)
    }

    @Test func fetchSharesRequestsBoundsConcurrencyAndCaches() async throws {
        let env = TestEnv()
        let app = env.app()
        let delay = 0.15
        let (mock, starts) = slowImageServer(delay)
        let server = try PlexServer(info: testInfo, baseUrl: mock.url, token: "token", session: MockPMS.session)
        let requests = { starts.list.withLock { $0.count } }

        // Thirty simultaneous requests for one image make one request.
        let same = "/library/metadata/1/thumb/1"
        let results = await all(30) { _ in await Images.fetch(app, server, same, 300, 450) }
        #expect(results.allSatisfy { $0 != nil && $0!.0.count == 8 && $0!.1 == "image/png" })
        #expect(requests() == 1)
        // And it is cached, so asking again makes none.
        #expect(await Images.fetch(app, server, same, 300, 450) != nil)
        #expect(requests() == 1)
        let file = try #require(await Images.cachedFile(app, server, same, 300, 450))
        #expect(try Data(contentsOf: file).count == 8)

        // A grid's worth of different images never has more than MAX_FETCHES in flight.
        let many = (0..<(4 * Images.MAX_FETCHES)).map { "/library/metadata/\(100 + $0)/thumb/1" }
        let found = await all(many.count) { await Images.fetch(app, server, many[$0], 300, 450) != nil }
        #expect(found.allSatisfy { $0 })
        #expect(requests() == 1 + many.count)
        // Each request is in flight for `delay` from when it began.
        let times = starts.list.withLock { Array($0.dropFirst()) }
        let window = Duration.milliseconds(Int(delay * 1000) - 10)
        let peak = times.map { t in times.filter { $0 >= t && $0 - t < window }.count }.max() ?? 0
        #expect(peak <= Images.MAX_FETCHES)
        #expect(peak > 1)  // fetches should still overlap
    }

    @Test func emptyCacheFilesAreMisses() async throws {
        let tmp = TempDir("patina-images")
        let path = tmp.file("empty")
        try Data().write(to: path)
        #expect(await Images.readCached(path) == nil)
        try Data([0x89, 0x50, 0x4E, 0x47, 0, 0]).write(to: path)
        let hit = try #require(await Images.readCached(path))
        #expect(hit.0.count == 6 && hit.1 == "image/png")
    }

    @Test func theCacheKeyIsTheSha256OfScopeSourceWidthHeightInATwoLevelFolder() {
        let paths = Paths.under(URL(fileURLWithPath: "/tmp/x"))
        let p = Images.cachePath(paths, nil, "/a", 1, 2)
        let hex = SHA256.hash(data: Data("|/a|1|2".utf8)).map { String(format: "%02x", $0) }.joined()
        #expect(p.path.hasSuffix("/\(hex.prefix(2))/\(hex)"))
    }
}
