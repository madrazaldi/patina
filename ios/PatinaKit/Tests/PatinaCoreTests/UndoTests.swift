// Port of desktop/core/undo.test.ts.

import PlexKit
import Testing

@testable import PatinaCore

private func item(_ rk: String, _ views: Int, _ offset: Int) -> Metadata {
    Metadata(ratingKey: rk, title: rk, viewCount: views, viewOffset: offset)
}

private func push(_ stack: inout Undo.Stack, _ scope: String, _ label: String) -> Int {
    stack.push(scope, label, .progress(server: nil, ratingKey: "1", offset: 5))
}

private func slots(_ ids: [Int]) -> [Undo.Slot] {
    ids.map { Undo.Slot(entryId: $0, ratingKey: "rk\($0)", title: "t\($0)") }
}

@Suite struct UndoTests {
    @Test func undoTakesTheNewestEntryFirst() {
        var s = Undo.Stack()
        let a = push(&s, "x", "a")
        let b = push(&s, "x", "b")
        #expect(s.last("x")?.id == b)
        #expect(s.take("x", nil)?.id == b)
        #expect(s.take("x", a)?.label == "a")
        #expect(s.take("x", nil) == nil)
    }

    @Test func aSpecificEntryCanBeTakenOutOfOrder() {
        var s = Undo.Stack()
        let a = push(&s, "x", "a")
        let b = push(&s, "x", "b")
        #expect(s.take("x", a)?.id == a)
        #expect(s.take("x", a) == nil)  // already undone
        #expect(s.count == 1)
        #expect(s.last("x")?.id == b)
    }

    @Test func entriesNeverCrossAccountOrServerScopes() {
        var s = Undo.Stack()
        let a = push(&s, "alice:server1", "a")
        #expect(s.last("alice:server2") == nil)
        #expect(s.take("bob:server1", a) == nil)
        // Taking from another scope also forgot the old one.
        #expect(s.isEmpty)
        _ = push(&s, "alice:server1", "a")
        _ = push(&s, "alice:server2", "b")
        #expect(s.count == 1)  // pushing in a new scope drops the old scope
        s.retainScope("alice:server1")
        #expect(s.isEmpty)
    }

    @Test func historyIsBounded() {
        var s = Undo.Stack()
        for i in 0..<(Undo.DEPTH + 10) { _ = push(&s, "x", String(i)) }
        #expect(s.count == Undo.DEPTH)
        #expect(s.last("x")?.label == String(Undo.DEPTH + 9))
        #expect(s.entries[0].label == "10")
    }

    @Test func aFailedUndoGoesBackOnTheStack() {
        var s = Undo.Stack()
        let a = push(&s, "x", "a")
        let e = s.take("x", nil)!
        s.restore(e)
        #expect(s.last("x")?.id == a)
    }

    @Test func onlyItemsWhoseStateChangesAreRemembered() {
        let items = [item("unwatched", 0, 0), item("partial", 0, 30_000), item("watched", 2, 0)]
        let markedWatched = Undo.changing(items, true)
        #expect(markedWatched.map(\.ratingKey) == ["unwatched", "partial"])
        // The resume position of the partly watched one is kept so undo can put it back.
        #expect(markedWatched[1].offset == 30_000)
        let markedUnwatched = Undo.changing(items, false)
        #expect(markedUnwatched.map(\.ratingKey) == ["partial", "watched"])
        #expect(Undo.changing([item("a", 0, 0)], false) == [])  // nothing changes, nothing to undo
    }

    @Test func restoringReplaysStateThenPosition() {
        let restore = [
            Undo.WatchState(ratingKey: "1", watched: false, offset: 30_000),
            Undo.WatchState(ratingKey: "2", watched: true, offset: 0),
            Undo.WatchState(ratingKey: "3", watched: false, offset: 0),
        ]
        #expect(
            Undo.callsFor(restore) == [
                .unscrobble(ratingKey: "1"), .progress(ratingKey: "1", ms: 30_000), .scrobble(ratingKey: "2"),
                .unscrobble(ratingKey: "3"),
            ])
    }

    @Test func locatingAnEntryFindsItsPredecessor() {
        let s = slots([10, 20, 30])
        let found = Undo.locate(s, 20)
        #expect(found?.0.entryId == 20)
        #expect(found?.1 == 10)
        #expect(Undo.locate(s, 10).map { $0.1 == nil } == true)  // first entry has no predecessor
        #expect(Undo.locate(s, 99) == nil)
    }

    @Test func aRestoredEntryGoesBackAfterItsPredecessor() {
        // The restored entry (40) is last; it used to follow 10.
        let now = slots([10, 30, 40])
        #expect(Undo.place(now, 10) == .some(10))
        #expect(Undo.place(now, nil) == .some(nil))  // it was first
        #expect(Undo.place(now, 99) == nil)  // the predecessor is gone, so it stays at the end
        // The entry itself is not its own predecessor.
        #expect(Undo.place(slots([40]), 40) == nil)
    }

    @Test func labelsNameASingleItemAndCountMany() {
        let one = [item("Heat", 0, 0)]
        #expect(Undo.watchLabel(one, 1, true) == "Marked Heat watched")
        #expect(Undo.watchLabel([], 1, false) == "Marked as unwatched")
        #expect(Undo.watchLabel(one, 7, true) == "Marked 7 items watched")
    }
}
