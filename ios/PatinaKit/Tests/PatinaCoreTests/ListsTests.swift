// Port of desktop/core/ops/lists.test.ts.

import Testing

@testable import PatinaCore

/// A fake paged source of `n` numbers that serves at most `cap` per request.
private func source(_ n: Int, _ cap: Int, _ reportTotal: Bool) -> (Int, Int) async throws -> ([Int], Int?) {
    { start, size in
        let take = min(size, cap)
        return (Array(start..<max(start, min(n, start + take))), reportTotal ? n : nil)
    }
}

@Suite struct ListsTests {
    @Test func collectsEveryPage() async throws {
        let (items, total) = try await Ops.Lists.collectPages(1000, 100_000, source(2500, 1000, true))
        #expect(items.count == 2500 && total == 2500)
        #expect(items.enumerated().allSatisfy { $0.offset == $0.element })
    }

    @Test func exactMultiplesAndEmpty() async throws {
        var (items, total) = try await Ops.Lists.collectPages(1000, 100_000, source(2000, 1000, true))
        #expect(items.count == 2000 && total == 2000)
        (items, total) = try await Ops.Lists.collectPages(1000, 100_000, source(2000, 1000, false))
        #expect(items.count == 2000 && total == 2000)
        (items, total) = try await Ops.Lists.collectPages(1000, 100_000, source(0, 1000, true))
        #expect(items.count == 0 && total == 0)
    }

    @Test func serverPageCapBelowRequestedSize() async throws {
        // The server hands out 300 at a time even though 1000 were requested.
        let (items, total) = try await Ops.Lists.collectPages(1000, 100_000, source(1000, 300, true))
        #expect(items.count == 1000 && total == 1000)
    }

    @Test func limitTruncatesButKeepsTheTrueTotal() async throws {
        let (items, total) = try await Ops.Lists.collectPages(1000, 2500, source(10_000, 1000, true))
        #expect(items.count == 2500)
        #expect(total == 10_000)
    }

    @Test func errorsBubbleUp() async {
        await #expect(throws: CoreError("boom")) {
            try await Ops.Lists.collectPages(10, 100) { start, _ -> ([Int], Int?) in
                if start == 0 { return (Array(0..<10), 30) }
                throw CoreError("boom")
            }
        }
    }
}
