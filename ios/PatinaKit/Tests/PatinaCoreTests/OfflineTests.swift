// Port of desktop/core/offline.test.ts.

import Testing

@testable import PatinaCore

@Suite struct OfflineTests {
    func progress() -> Offline.Progress {
        Offline.Progress(
            id: "one", account: "alice", server: "home", ratingKey: "1", position: 10, duration: 100, watched: false,
            updated: 1, revision: "old", pending: true)
    }

    @Test func neverReplaysToADifferentUserOrServer() {
        let p = progress()
        #expect(Offline.belongs(p, "home", "alice"))
        #expect(!Offline.belongs(p, "home", "bob"))
        #expect(!Offline.belongs(p, "away", "alice"))
    }

    @Test func acknowledgingARequestDoesNotEraseNewerProgress() {
        let sent = progress()
        var list = [sent]
        list[0].revision = "new"
        Offline.acknowledge(&list, sent)
        #expect(list[0].pending)
        let newest = list[0]
        Offline.acknowledge(&list, newest)
        #expect(!list[0].pending)
    }
}
