// Port of desktop/core/report.test.ts.

import Foundation
import PlexKit
import Testing

@testable import PatinaCore

private func update(_ key: String, _ state: TimelineState, _ timeMs: Int) -> Report.Op {
    .timeline(
        TimelineUpdate(ratingKey: key, key: "/library/metadata/\(key)", state: state, timeMs: timeMs, durationMs: 100_000))
}

private func times(_ p: Report.Pending<String>) -> [String] {
    p.ops().compactMap { op in
        if case .timeline(let u) = op { return "\(u.ratingKey) \(u.timeMs) \(u.state.rawValue)" }
        return nil
    }
}

private func pending() -> Report.Pending<String> {
    Report.Pending { $0 == $1 }
}

/// Records what reaches "Plex"; each request takes `delay` ms, and the first `fail` of them
/// fail. Like a URLSession request, one ends (throwing) as soon as it is cancelled.
private final class Fake {
    var seen: [Report.Op] = []
    var hang = false
    let delay: Int
    var fail: Int

    init(_ delay: Int, _ fail: Int) {
        self.delay = delay
        self.fail = fail
    }

    var sink: Report.Sink<String> {
        Report.Sink { [self] _, op in
            try await Task.sleep(for: hang ? .seconds(3600) : .milliseconds(delay))
            if fail > 0 {
                fail -= 1
                throw CoreError("boom")
            }
            seen.append(op)
        }
    }

    func reporter(_ config: Report.Config) -> Report.Reporter<String> {
        Report.Reporter.spawn(sink, config, eq: ==)
    }
}

private let fast = Report.Config(timeoutMs: 200, attempts: 4, backoffMs: 10)

private func isStop(_ op: Report.Op, at time: Int? = nil) -> Bool {
    if case .timeline(let u) = op { return u.state == .stopped && (time == nil || u.timeMs == time) }
    return false
}

@Suite struct ReportTests {
    @Test func progressCollapsesToTheNewestCheckpoint() {
        var p = pending()
        for t in 0..<50 { p.push("s", update("1", .playing, t * 1000)) }
        p.push("s", update("1", .paused, 49_500))
        #expect(times(p) == ["1 49500 paused"])
    }

    @Test func differentItemsAndServersDoNotMerge() {
        var p = pending()
        p.push("a", update("1", .playing, 1))
        p.push("a", update("2", .playing, 2))
        p.push("b", update("1", .playing, 3))
        #expect(p.count == 3)
    }

    @Test func theFinalStopReplacesProgressAndIsKept() {
        var p = pending()
        p.push("s", update("1", .playing, 10_000))
        p.push("s", update("1", .stopped, 90_000))
        p.push("s", .scrobble(ratingKey: "1"))
        #expect(times(p) == ["1 90000 stopped"])
        #expect(p.ops()[1] == .scrobble(ratingKey: "1"))
    }

    @Test func progressOfANewPlaybackNeverMergesIntoAnEarlierStop() {
        var p = pending()
        p.push("s", update("1", .stopped, 90_000))
        p.push("s", update("1", .playing, 0))
        p.push("s", update("1", .playing, 5_000))
        #expect(times(p) == ["1 90000 stopped", "1 5000 playing"])
    }

    @Test func streamChoicesMergePerPart() {
        var p = pending()
        p.push("s", .streams(partId: 7, audio: 1, subtitle: nil))
        p.push("s", .streams(partId: 7, audio: nil, subtitle: 5))
        p.push("s", .streams(partId: 7, audio: 2, subtitle: nil))
        p.push("s", .streams(partId: 8, audio: 9, subtitle: nil))
        #expect(p.ops() == [.streams(partId: 7, audio: 2, subtitle: 5), .streams(partId: 8, audio: 9, subtitle: nil)])
    }

    @Test func transcodePingsAreDeduplicatedAndDroppedWhenStopping() {
        var p = pending()
        for _ in 0..<5 { p.push("s", .pingTranscode(session: "x")) }
        #expect(p.count == 1)
        p.push("s", .stopTranscode(session: "x"))
        #expect(p.ops() == [.stopTranscode(session: "x")])
        p.push("s", .pingTranscode(session: "x"))
        #expect(p.count == 1)
    }

    @Test func aSlowServerCostsTheCallerNothingAndCollapsesTheBacklog() async throws {
        let fake = Fake(150, 0)
        let reporter = fake.reporter(fast)
        reporter.send("s", update("1", .playing, 0))
        // The first request is now in flight and takes 150ms.
        try await Task.sleep(for: .milliseconds(20))
        let began = ContinuousClock.now
        for t in 1..<200 { reporter.send("s", update("1", .playing, t * 1000)) }
        reporter.send("s", update("1", .stopped, 200_000))
        reporter.send("s", .scrobble(ratingKey: "1"))
        #expect(ContinuousClock.now - began < .milliseconds(50))
        #expect(await reporter.flush(5000))
        let seen = fake.seen
        #expect(seen.count <= 4)
        #expect(seen.filter { isStop($0) }.count == 1)
        #expect(isStop(seen[seen.count - 2], at: 200_000))
        #expect(seen.last == .scrobble(ratingKey: "1"))
        // Nothing older than the stop was sent after it.
        let stopAt = seen.firstIndex { isStop($0) }!
        #expect(seen[stopAt...].allSatisfy { !Report.isProgress($0) })
    }

    @Test func theFinalStopSurvivesFailures() async {
        let fake = Fake(1, 2)
        let reporter = fake.reporter(fast)
        reporter.send("s", update("1", .stopped, 42_000))
        #expect(await reporter.flush(5000))
        #expect(fake.seen.count == 1)
        #expect(isStop(fake.seen[0], at: 42_000))
    }

    @Test func failedProgressIsNotRetriedAndNotResentOverNewerState() async throws {
        let fake = Fake(1, 1)
        let reporter = fake.reporter(fast)
        reporter.send("s", update("1", .playing, 1_000))
        try await Task.sleep(for: .milliseconds(50))
        reporter.send("s", update("1", .playing, 2_000))
        #expect(await reporter.flush(5000))
        #expect(fake.seen == [update("1", .playing, 2_000)])
    }

    @Test func aHangingServerIsCutOffSoLaterReportsGetThrough() async {
        let hang = Fake(0, 0)
        hang.hang = true
        let reporter = hang.reporter(Report.Config(timeoutMs: 30, attempts: 2, backoffMs: 5))
        reporter.send("s", update("1", .stopped, 1))
        reporter.send("s", update("2", .stopped, 2))
        let began = ContinuousClock.now
        #expect(await reporter.flush(5000))
        #expect(ContinuousClock.now - began < .seconds(2))
    }

    @Test func aRequestCutOffNeverLandsAfterNewerReports() async throws {
        let fake = Fake(100, 0)
        let reporter = fake.reporter(Report.Config(timeoutMs: 30, attempts: 1, backoffMs: 5))
        reporter.send("s", update("1", .playing, 1_000))
        try await Task.sleep(for: .milliseconds(10))
        reporter.send("s", update("2", .stopped, 2_000))
        #expect(await reporter.flush(5000))
        try await Task.sleep(for: .milliseconds(150))
        // Both were cut off (each takes 100ms); neither reached the server late.
        #expect(fake.seen.isEmpty)
    }

    @Test func flushTimesOutWhileTheServerIsSlow() async {
        let fake = Fake(300, 0)
        let reporter = fake.reporter(Report.Config(timeoutMs: 5000, attempts: 4, backoffMs: 10))
        reporter.send("s", update("1", .stopped, 1))
        #expect(await reporter.flush(20) == false)
    }
}
