// Port of desktop/core/smart.test.ts.

import Foundation
import PlexKit
import Testing

@testable import PatinaCore

private func policy(_ id: String, _ keep: Int, _ deleteWatched: Bool) -> Smart.Policy {
    Smart.Policy(
        id: id, account: "alice", serverId: "home", serverName: "Home", ratingKey: "show-\(id)", target: .show,
        title: id, keep: keep, deleteWatched: deleteWatched, enabled: true)
}

private func episodes(_ prefix: String, _ watched: Int, _ total: Int, _ size: Int) -> [Smart.Candidate] {
    (1...total).map { n in
        Smart.Candidate(ratingKey: "\(prefix)\(n)", id: "home:alice:\(prefix)\(n)", watched: n <= watched, size: size)
    }
}

private func held(_ prefix: String, _ numbers: [Int], _ owner: String?, _ size: Int) -> [Smart.Held] {
    numbers.map { Smart.Held(id: "home:alice:\(prefix)\($0)", policy: owner, size: size) }
}

private func added(_ plan: Smart.Plan) -> [String] {
    plan.add.map(\.ratingKey)
}

private func episode(_ season: Int, _ number: Int, _ key: String) -> Metadata {
    var m = Metadata(ratingKey: key)
    m.parentIndex = season
    m.index = number
    return m
}

@Suite struct SmartTests {
    @Test func keepsTheNextUnwatchedEpisodes() {
        let p = policy("a", 3, true)
        let fetched = [Smart.Fetched(policy: p, candidates: episodes("e", 1, 6, 100))]
        var plan = Smart.evaluate(fetched, [], ["a"], 0, nil)
        #expect(added(plan) == ["e2", "e3", "e4"])
        #expect(plan.remove == [])
        // Already downloaded episodes are not fetched again.
        plan = Smart.evaluate(fetched, held("e", [2, 3], "a", 100), ["a"], 0, nil)
        #expect(added(plan) == ["e4"])
    }

    @Test func removesWatchedDownloadsAndFetchesTheNextOnes() {
        let p = policy("a", 3, true)
        // e2 was watched since the last round: it goes, the window moves on to e5.
        let fetched = [Smart.Fetched(policy: p, candidates: episodes("e", 2, 6, 100))]
        let plan = Smart.evaluate(fetched, held("e", [2, 3, 4], "a", 100), ["a"], 0, nil)
        #expect(plan.remove == ["home:alice:e2"])
        #expect(added(plan) == ["e5"])
    }

    @Test func keepsWatchedDownloadsUnlessThePolicySaysOtherwise() {
        let p = policy("a", 2, false)
        let fetched = [Smart.Fetched(policy: p, candidates: episodes("e", 2, 6, 100))]
        let plan = Smart.evaluate(fetched, held("e", [1, 2, 3], "a", 100), ["a"], 0, nil)
        #expect(plan.remove == [])
        #expect(added(plan) == ["e4"])
    }

    @Test func downloadsThatFellOutOfTheWindowAreRemoved() {
        // Keep was lowered from 4 to 2: the two furthest episodes go.
        let p = policy("a", 2, false)
        let fetched = [Smart.Fetched(policy: p, candidates: episodes("e", 0, 6, 100))]
        let plan = Smart.evaluate(fetched, held("e", [1, 2, 3, 4], "a", 100), ["a"], 0, nil)
        #expect(plan.remove == ["home:alice:e3", "home:alice:e4"])
        #expect(plan.add == [])
    }

    @Test func manualDownloadsAreNeverRemoved() {
        let p = policy("a", 2, true)
        // e1 was watched and downloaded by hand: it stays. It does not count towards the window.
        let fetched = [Smart.Fetched(policy: p, candidates: episodes("e", 1, 5, 100))]
        let plan = Smart.evaluate(fetched, held("e", [1], nil, 100), ["a"], 0, nil)
        #expect(plan.remove == [])
        #expect(added(plan) == ["e2", "e3"])
    }

    @Test func aPolicyThatCouldNotBeFetchedRemovesNothing() {
        // No Fetched entry for "a" (the server was unreachable, or it is disabled).
        let plan = Smart.evaluate([], held("e", [1, 2], "a", 100), ["a"], 0, nil)
        #expect(plan == Smart.emptyPlan())
    }

    @Test func theBudgetLimitsWhatIsAdded() {
        let p = policy("a", 4, true)
        let fetched = [Smart.Fetched(policy: p, candidates: episodes("e", 0, 6, 100))]
        // 250 bytes: two fit, and the rest wait in order (no skipping ahead).
        var plan = Smart.evaluate(fetched, [], ["a"], 0, 250)
        #expect(added(plan) == ["e1", "e2"])
        #expect(plan.overBudget == 2)
        // Other servers' downloads count against the budget too.
        plan = Smart.evaluate(fetched, [], ["a"], 150, 250)
        #expect(added(plan) == ["e1"])
        // Existing downloads count; nothing is removed to make room.
        plan = Smart.evaluate(fetched, held("e", [1, 2], "a", 100), ["a"], 0, 250)
        #expect(added(plan) == [])
        #expect(plan.overBudget == 2)
        #expect(plan.remove == [])
    }

    @Test func removingWatchedFilesFreesBudgetForTheNextOnes() {
        let p = policy("a", 2, true)
        let fetched = [Smart.Fetched(policy: p, candidates: episodes("e", 1, 5, 100))]
        // e1 (watched) is removed, which leaves room for e3 within 200 bytes.
        let plan = Smart.evaluate(fetched, held("e", [1, 2], "a", 100), ["a"], 0, 200)
        #expect(plan.remove == ["home:alice:e1"])
        #expect(added(plan) == ["e3"])
        #expect(plan.overBudget == 0)
    }

    @Test func twoPoliciesShareTheBudgetAndDoNotFightOverFiles() {
        let a = policy("a", 2, true)
        let b = policy("b", 2, true)
        let fetched = [
            Smart.Fetched(policy: a, candidates: episodes("x", 0, 3, 100)),
            // The second policy lists the same episode x1 plus its own.
            Smart.Fetched(policy: b, candidates: episodes("x", 0, 1, 100) + episodes("y", 0, 3, 100)),
        ]
        var plan = Smart.evaluate(fetched, [], ["a", "b"], 0, nil)
        #expect(added(plan) == ["x1", "x2", "y1"])
        // A file wanted by b stays even though a no longer wants it.
        let aOnly = [Smart.Fetched(policy: a, candidates: episodes("x", 1, 3, 100)), fetched[1]]
        plan = Smart.evaluate(aOnly, held("x", [1], "a", 100), ["a", "b"], 0, nil)
        #expect(plan.remove == [])
    }

    @Test func downloadsOfADeletedPolicyAreAdoptedOrReleased() {
        let b = policy("b", 2, true)
        let fetched = [Smart.Fetched(policy: b, candidates: episodes("e", 0, 4, 100))]
        // "gone" no longer exists. e1 is wanted by b and adopted; e4 is nobody's and released.
        let files = held("e", [1], "gone", 100) + held("e", [4], "gone", 100)
        let plan = Smart.evaluate(fetched, files, ["b"], 0, nil)
        #expect(
            plan.assign == [
                Smart.Plan.Assignment(id: "home:alice:e1", policy: "b"),
                Smart.Plan.Assignment(id: "home:alice:e4", policy: nil),
            ])
        #expect(plan.remove == [])
    }

    @Test func onlyThisAccountAndServerAreConsidered() {
        let scope = Scope("alice", "home")
        func mk(_ server: String, _ account: String, _ size: Int) -> Download {
            Download(
                id: "\(server):\(account)", serverId: server, serverName: "", item: Metadata(), size: size, state: .done,
                account: account, policy: "a")
        }
        let all = [mk("home", "alice", 10), mk("home", "bob", 20), mk("away", "alice", 40)]
        let (heldList, outside) = Smart.split(all, scope)
        #expect(heldList.map(\.id) == ["home:alice"])
        #expect(outside == 60)

        var other = policy("a", 1, true)
        #expect(Smart.inScope(other, scope))
        other.serverId = "away"
        #expect(!Smart.inScope(other, scope))
        other.serverId = "home"
        other.account = "bob"
        #expect(!Smart.inScope(other, scope))
    }

    @Test func episodesAreWatchedInOrderWithoutSpecials() {
        let items = [episode(2, 1, "c"), episode(0, 1, "special"), episode(1, 2, "b"), episode(1, 1, "a")]
        let keys = { (v: [Metadata]) in v.map(Metadata.ratingKey) }
        #expect(keys(Smart.watchOrder(.show, items)) == ["a", "b", "c"])
        // Asking for the specials season itself keeps them.
        let specials = [episode(0, 2, "s2"), episode(0, 1, "s1")]
        #expect(keys(Smart.watchOrder(.season, specials)) == ["s1", "s2"])
        // A playlist keeps its own order.
        #expect(keys(Smart.watchOrder(.playlist, specials)) == ["s2", "s1"])
    }

    @Test func candidatesFollowServerAndOfflineWatches() {
        let scope = Scope("alice", "home")
        let media = [Media(part: [Part(size: 500)])]
        func ep(_ n: Int, _ key: String, file: Bool = true) -> Metadata {
            var m = episode(1, n, key)
            m.type = "episode"
            if file { m.media = media }
            return m
        }
        var seen = ep(1, "e1")
        seen.viewCount = 1
        let c = Smart.candidates([seen, ep(2, "e2"), ep(3, "e3"), ep(4, "e4", file: false)], scope, ["home:alice:e2"])
        #expect(c.map(\.ratingKey) == ["e1", "e2", "e3"])
        #expect(c.map(\.watched) == [true, true, false])
        #expect(c.map(\.size) == [500, 500, 500])
    }
}
