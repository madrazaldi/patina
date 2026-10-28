// Port of desktop/core/live.test.ts.

import PlexKit
import Testing

@testable import PatinaCore

@Suite struct LiveTests {
    @Test func changesMergeSectionsAndItems() {
        var c = Changes()
        c.add("1", "10")
        c.add("1", "11")
        c.add("2", nil)
        #expect(!c.all)
        #expect(c.sections == ["1", "2"])
        #expect(c.items == ["10", "11"])
        c.add(nil, "12")
        #expect(c.all)  // a change without a section could be anywhere
    }

    @Test func textReadsStringsAndNumbers() {
        let v: JSONValue = ["a": "1", "b": 2, "c": "", "d": nil]
        #expect(Live.text(v, "a") == "1")
        #expect(Live.text(v, "b") == "2")
        #expect(Live.text(v, "c") == nil)
        #expect(Live.text(v, "d") == nil)
        #expect(Live.text(v, "e") == nil)
    }

    @Test func entriesAcceptArraysSingleObjectsAndANotificationContainerWrapper() {
        #expect(Live.entries(["TimelineEntry": [["a": 1], ["a": 2]]], "TimelineEntry").count == 2)
        #expect(Live.entries(["NotificationContainer": ["TimelineEntry": ["a": 1]]], "TimelineEntry") == [["a": 1]])
        #expect(Live.entries(["other": 1], "TimelineEntry") == [])
        #expect(Live.entries(nil, "x") == [])
    }

    @Test func activitiesBecomeViews() throws {
        let a = try Activity(json: [
            "uuid": "u1", "type": "library.update.section", "title": "Scanning", "subtitle": "", "progress": 40,
            "cancellable": true, "Context": ["librarySectionID": 1],
        ])
        #expect(
            Live.activityView(a)
                == ServerActivity(
                    uuid: "u1", kind: "library.update.section", title: "Scanning", subtitle: nil, progress: 40,
                    cancellable: true, sectionId: "1"))
        #expect(Live.activityView(Activity()) == nil)
    }
}
