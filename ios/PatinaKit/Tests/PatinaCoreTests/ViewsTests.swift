// Port of desktop/core/ops/views.test.ts.

import Foundation
import PlexKit
import Testing

@testable import PatinaCore

@Suite struct ViewsTests {
    typealias V = Ops.Views

    /// An entry as the desktop app writes it.
    func guiEntry(_ id: String, _ name: String, _ library: String) -> JSONValue {
        [
            "id": .string(id), "name": .string(name), "scope": "alice:home", "library": .string(library), "mediaType": 0,
            "sort": "addedAt", "desc": true, "unwatched": true, "toggles": ["hdr"],
            "picked": [["filter": "genre", "title": "Genre", "value": "10", "label": "Drama"]],
            "somethingNew": ["a": 1],
        ]
    }

    @Test func readsWhatTheDesktopAppWrote() {
        let stored: [JSONValue] = [guiEntry("v1", "Recent drama", "1"), guiEntry("v2", "Elsewhere", "2"), "garbage", ["name": "no id"]]
        let views = V.matching(stored, "alice:home", "1")
        #expect(views.count == 1)
        let v = views[0]
        #expect([v.id, v.name, v.sort] == ["v1", "Recent drama", "addedAt"])
        #expect(v.desc && v.unwatched && v.tui == nil)
        #expect(v.toggles == ["hdr"])
        #expect(v.picked == [V.Picked(filter: "genre", title: "Genre", value: "10", label: "Drama")])
        // Another account or server doesn't see it.
        #expect(V.matching(stored, "bob:home", "1").isEmpty)
        // Unknown fields survive a rewrite.
        let back = V.toJson(v)
        #expect(back["somethingNew"] == ["a": 1])
        #expect(back["mediaType"] == 0)
        #expect(back["tui"] == nil)
    }

    @Test func terminalViewsKeepTheDesktopShape() throws {
        var view = V.newSavedView("alice:home", "1", "  Unwatched HDR ")
        view.unwatched = true
        view.toggles = ["hdr"]
        view.tui = V.TuiExtras(hideWatched: true, filter: "dune")
        let json = V.toJson(view)
        #expect(json["name"] == "Unwatched HDR")
        for key in ["id", "name", "scope", "library", "mediaType", "sort", "desc", "unwatched", "toggles", "picked"] {
            #expect(json[key] != nil, "\(key)")
        }
        #expect(json["tui"] == ["hideWatched": true, "filter": "dune"])
        #expect(V.parse(json) == view)
        // And it round-trips through the config file.
        var settings = Settings()
        settings.ui.savedViews.append(json)
        let again = try Config.parseSettings(Config.settingsToJson(settings).serialized())
        #expect(V.matching(again.ui.savedViews, "alice:home", "1") == [view])
    }

    @Test func savingTheSameNameReplacesAndRemovingDeletes() {
        var stored: [JSONValue] = [guiEntry("v1", "Recent drama", "1")]
        var view = V.newSavedView("alice:home", "1", "recent DRAMA")
        view.sort = "titleSort"
        V.upsert(&stored, view, V.toJson(view))
        #expect(stored.count == 1)
        #expect(stored[0]["id"] == "v1")
        #expect(stored[0]["sort"] == "titleSort")
        let other = V.newSavedView("alice:home", "1", "Another")
        V.upsert(&stored, other, V.toJson(other))
        #expect(stored.count == 2)
        // A view of the same name in another library is a different view.
        let elsewhere = V.newSavedView("alice:home", "2", "Another")
        V.upsert(&stored, elsewhere, V.toJson(elsewhere))
        #expect(stored.count == 3)
        V.forget(&stored, other.id)
        #expect(stored.count == 2)
        V.forget(&stored, "missing")
        #expect(stored.count == 2)
    }

    @Test func scopeMatchesTheDesktopApps() {
        #expect(V.scopeOf(accountId: "acct", serverId: "srv") == "acct:srv")
    }
}
