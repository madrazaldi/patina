// Port of desktop/core/recovery.test.ts.

import PlexKit
import Testing

@testable import PatinaCore

@Suite struct RecoveryTests {
    func cp(_ position: Int, _ duration: Int, _ updated: Int) -> Checkpoint {
        Checkpoint(account: "a", server: "s", ratingKey: "1", position: position, duration: duration, updated: updated)
    }

    @Test func aCheckpointIsUsedUntilTheServerHasSeenTheItemLater() {
        let c = cp(120_000, 600_000, 1000)
        #expect(Recovery.usable(c, nil, 1100) == 120_000)
        #expect(Recovery.usable(c, 900, 1100) == 120_000)  // the server last saw it before the checkpoint
        #expect(Recovery.usable(c, 1000, 1100) == 120_000)
        #expect(Recovery.usable(c, 1001, 1100) == nil)  // watched since, maybe elsewhere
    }

    @Test func finishedOrStaleCheckpointsAreIgnored() {
        #expect(Recovery.usable(cp(0, 600_000, 1000), nil, 1100) == nil)
        #expect(Recovery.usable(cp(590_000, 600_000, 1000), nil, 1100) == nil)
        #expect(Recovery.usable(cp(5000, 600_000, 1000), nil, 1000 + Recovery.RECOVERY_WINDOW + 1) == nil)
    }

    @Test func longQueuesKeepThePlayingItemInTheWindow() {
        #expect(Recovery.window(10, 4) == (0..<10, 4))
        var (range, current) = Recovery.window(1000, 500)
        #expect(range.count == Recovery.MAX_QUEUE_ITEMS)
        #expect(range.lowerBound + current == 500)
        (range, current) = Recovery.window(1000, 999)
        #expect(range.upperBound == 1000)
        #expect(range.lowerBound + current == 999)
        #expect(Recovery.window(1000, 0).0.lowerBound == 0)
    }

    @Test func slimItemsKeepWhatIsNeededToListAndRequeue() {
        let full = Metadata(
            ratingKey: "5", key: "/library/metadata/5", type: "episode", title: "Pilot", summary: "long", index: 1,
            parentIndex: 1, grandparentTitle: "Show")
        let s = Recovery.slim(full)
        #expect(Metadata.fullTitle(s) == Metadata.fullTitle(full))
        #expect(s.summary == nil)
        #expect(Metadata.isPlayable(s))
    }

    @Test func checkpointsAndQueuesAreScopedAndSaved() throws {
        let env = TestEnv()
        let here = Scope("alice", "home")
        try Recovery.saveCheckpoint(env.paths, here, "1", 5000, 60_000)
        #expect(Recovery.checkpoint(env.paths, here, "1")?.position == 5000)
        #expect(Recovery.checkpoint(env.paths, Scope("bob", "home"), "1") == nil)
        try Recovery.clearCheckpoint(env.paths, here, "1")
        #expect(Recovery.checkpoint(env.paths, here, "1") == nil)

        let queue = SavedQueue(
            account: "alice", server: "home", savedAt: Offline.now(), kind: "video",
            items: [Recovery.slim(Metadata(ratingKey: "5", type: "movie", title: "Heat"))], current: 0, positionMs: 61_000,
            quality: nil)
        try Recovery.saveQueue(env.paths, here, queue)
        #expect(Recovery.loadQueue(env.paths, here) == queue)
        #expect(Recovery.loadQueue(env.paths, Scope("alice", "away")) == nil)
        #expect(Recovery.savedQueueView(queue)?.positionMs == 61_000)
        try Recovery.clearQueue(env.paths, here)
        #expect(Recovery.loadQueue(env.paths, here) == nil)
    }
}
