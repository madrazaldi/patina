// Port of desktop/core/ops/views.ts.
// Saved library views: named queries (type, sort, filters) of one library, kept per account
// and server in the settings under `ui.saved_views`, in the shape the desktop app writes.
//
// The entries are stored as the JSON the desktop app writes (`web/src/lib/types.ts`,
// `SavedView`). Fields this code doesn't know are kept as they are when an entry is
// rewritten, and entries that don't parse are left alone.

import Foundation
import PlexKit

extension Ops {
    public enum Views {
        /// A value filter ("Genre: Drama").
        public nonisolated struct Picked: Sendable, Equatable {
            public var filter: String
            public var title: String
            public var value: String
            public var label: String

            public init(filter: String, title: String, value: String, label: String) {
                self.filter = filter
                self.title = title
                self.value = value
                self.label = label
            }
        }

        /// What only the terminal client adds to a view; the other front ends ignore it.
        public nonisolated struct TuiExtras: Sendable, Equatable {
            public var hideWatched: Bool
            /// Text typed into the column's `/` filter.
            public var filter: String

            public init(hideWatched: Bool, filter: String) {
                self.hideWatched = hideWatched
                self.filter = filter
            }
        }

        /// Fields written by other versions or front ends are kept in `extra` (serde `flatten`).
        public nonisolated struct SavedView: Sendable, Equatable {
            public var id = ""
            public var name = ""
            /// `account:server`, see `scope`.
            public var scope = ""
            /// The library's section id.
            public var library = ""
            /// Plex media type listed; 0 for the library's default.
            public var mediaType = 0
            /// Plex sort key, e.g. `titleSort`; `desc` reverses it.
            public var sort = ""
            public var desc = false
            public var unwatched = false
            /// Boolean filters that are on, by key.
            public var toggles: [String] = []
            public var picked: [Picked] = []
            public var tui: TuiExtras?
            public var extra: [String: JSONValue] = [:]

            public init() {}
        }

        private nonisolated static let KNOWN: Set<String> = [
            "id", "name", "scope", "library", "mediaType", "sort", "desc", "unwatched", "toggles", "picked", "tui",
        ]

        /// A new view of `library` for `scope`, with a fresh id.
        public nonisolated static func newSavedView(_ scope: String, _ library: String, _ name: String) -> SavedView {
            var v = SavedView()
            v.id = UUID().uuidString.lowercased()
            v.name = name.trimmingCharacters(in: .whitespacesAndNewlines)
            v.scope = scope
            v.library = library
            v.sort = "titleSort"
            return v
        }

        /// The scope views are saved under: the same `accountId:serverId` the desktop app uses.
        public nonisolated static func scopeOf(accountId: String, serverId: String) -> String {
            "\(accountId):\(serverId)"
        }

        /// The current account and server's scope; `nil` when not connected.
        public static func scope(_ app: AppState) -> String? {
            app.session().map { scopeOf(accountId: Offline.account($0), serverId: $0.serverId) }
        }

        /// The views of `library` on the connected server, in the order saved.
        public static func list(_ app: AppState, _ library: String) -> [SavedView] {
            guard let sc = scope(app) else { return [] }
            return matching(app.settings().ui.savedViews, sc, library)
        }

        /// Save `view`. A view of the same library and name (in the same scope) is replaced.
        public static func save(_ app: AppState, _ view: SavedView) throws {
            if view.name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                throw CoreError("A saved view needs a name.")
            }
            let value = toJson(view)
            try app.tryUpdateSettings { s in upsert(&s.ui.savedViews, view, value) }
        }

        /// Delete the view with this id.
        public static func remove(_ app: AppState, _ id: String) throws {
            try app.tryUpdateSettings { s in forget(&s.ui.savedViews, id) }
        }

        /// `serde_json::to_value(&view)`: the known fields, `tui` only when present, then the extras.
        public nonisolated static func toJson(_ view: SavedView) -> JSONValue {
            var out: [String: JSONValue] = [
                "id": .string(view.id), "name": .string(view.name), "scope": .string(view.scope),
                "library": .string(view.library), "mediaType": JSONValue(view.mediaType), "sort": .string(view.sort),
                "desc": .bool(view.desc), "unwatched": .bool(view.unwatched),
                "toggles": .array(view.toggles.map(JSONValue.string)),
                "picked": .array(
                    view.picked.map {
                        ["filter": .string($0.filter), "title": .string($0.title), "value": .string($0.value), "label": .string($0.label)]
                    }),
            ]
            if let tui = view.tui { out["tui"] = ["hideWatched": .bool(tui.hideWatched), "filter": .string(tui.filter)] }
            for (k, v) in view.extra where !KNOWN.contains(k) { out[k] = v }
            return .object(out)
        }

        /// `serde_json::from_value::<SavedView>` (every field defaulted), `nil` when it does not fit.
        private nonisolated static func fromJson(_ v: JSONValue) -> SavedView? {
            guard case .object(let o) = v else { return nil }
            func str(_ o: [String: JSONValue], _ key: String) -> String?? {
                switch o[key] {
                case nil: .some("")
                case .string(let s)?: .some(s)
                default: .none
                }
            }
            func bool(_ o: [String: JSONValue], _ key: String) -> Bool? {
                switch o[key] {
                case nil: false
                case .bool(let b)?: b
                default: nil
                }
            }
            var out = SavedView()
            guard case let id?? = str(o, "id"), case let name?? = str(o, "name"), case let scope?? = str(o, "scope"),
                case let library?? = str(o, "library"), case let sort?? = str(o, "sort"), let desc = bool(o, "desc"),
                let unwatched = bool(o, "unwatched")
            else { return nil }
            (out.id, out.name, out.scope, out.library, out.sort, out.desc, out.unwatched) = (id, name, scope, library, sort, desc, unwatched)
            if let mediaType = o["mediaType"] {
                guard case .number(let n) = mediaType, let i = Int(exactly: n) else { return nil }
                out.mediaType = i
            }
            if let toggles = o["toggles"] {
                guard case .array(let a) = toggles else { return nil }
                let strings = a.compactMap(\.stringValue)
                guard strings.count == a.count else { return nil }
                out.toggles = strings
            }
            if let picked = o["picked"] {
                guard case .array(let a) = picked else { return nil }
                for p in a {
                    // `Picked` has no defaults: all four fields are required.
                    guard let filter = p["filter"]?.stringValue, let title = p["title"]?.stringValue,
                        let value = p["value"]?.stringValue, let label = p["label"]?.stringValue, p.objectValue != nil
                    else { return nil }
                    out.picked.append(Picked(filter: filter, title: title, value: value, label: label))
                }
            }
            if let tui = o["tui"], tui != .null {
                guard case .object(let t) = tui, let hideWatched = bool(t, "hideWatched"), case let filter?? = str(t, "filter")
                else { return nil }
                out.tui = TuiExtras(hideWatched: hideWatched, filter: filter)
            }
            for (k, val) in o where !KNOWN.contains(k) { out.extra[k] = val }
            return out
        }

        public nonisolated static func parse(_ v: JSONValue) -> SavedView? {
            guard let view = fromJson(v), !view.id.isEmpty else { return nil }
            return view
        }

        public nonisolated static func matching(_ stored: [JSONValue], _ scope: String, _ library: String) -> [SavedView] {
            stored.compactMap(parse).filter { $0.scope == scope && $0.library == library }
        }

        public nonisolated static func upsert(_ stored: inout [JSONValue], _ view: SavedView, _ value: JSONValue) {
            let at = stored.firstIndex { v in
                guard let old = parse(v) else { return false }
                return old.scope == view.scope && old.library == view.library
                    && Pick.asciiLower(old.name) == Pick.asciiLower(view.name)
            }
            guard let at else {
                stored.append(value)
                return
            }
            // Keep the id so anything pointing at the view still does.
            var value = value
            if case .object(var o) = value, let id = stored[at]["id"] {
                o["id"] = id
                value = .object(o)
            }
            stored[at] = value
        }

        public nonisolated static func forget(_ stored: inout [JSONValue], _ id: String) {
            stored.removeAll { $0.objectValue != nil && $0["id"] == .string(id) }
        }
    }
}
