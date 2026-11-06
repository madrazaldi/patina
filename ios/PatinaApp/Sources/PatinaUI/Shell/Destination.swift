// The places of the sidebar (iPad) and the tab bar (iPhone), each with its own navigation
// stack. The entries are those of web/src/components/Shell.svelte.

import Foundation
import PlexKit

public enum Destination: Hashable, Sendable {
    case home
    case search
    case pick
    case playlists
    case watchlist
    case downloads
    case livetv
    case requests
    case activity
    case server
    case settings
    case library(id: String)
    /// iPhone: the list of libraries.
    case libraries
    /// iPhone: everything that has no tab.
    case more

    /// The route shown at the root of the destination's stack.
    public var root: Route? {
        switch self {
        case .home: .home
        case .search: .search
        case .pick: .pick
        case .playlists: .playlists
        case .watchlist: .watchlist
        case .downloads: .downloads
        case .livetv: .livetv
        case .requests: .requests
        case .activity: .activity
        case .server: .server
        case .settings: .settings
        case .library(let id): .library(id: id)
        case .libraries, .more: nil
        }
    }

    public var title: String {
        switch self {
        case .home: "Home"
        case .search: "Search"
        case .pick: "Pick Something"
        case .playlists: "Playlists"
        case .watchlist: "Watchlist"
        case .downloads: "Downloads"
        case .livetv: "Live TV"
        case .requests: "Requests"
        case .activity: "Activity"
        case .server: "Server"
        case .settings: "Settings"
        case .library: "Library"
        case .libraries: "Libraries"
        case .more: "More"
        }
    }

    public var systemImage: String {
        switch self {
        case .home: "house"
        case .search: "magnifyingglass"
        case .pick: "dice"
        case .playlists: "music.note.list"
        case .watchlist: "bookmark"
        case .downloads: "arrow.down.circle"
        case .livetv: "dot.radiowaves.left.and.right"
        case .requests: "tray"
        case .activity: "waveform.path.ecg"
        case .server: "server.rack"
        case .settings: "gearshape"
        case .library: "folder"
        case .libraries: "square.stack"
        case .more: "ellipsis.circle"
        }
    }

    /// Icon of a library section by type (`SECTION_ICONS`).
    public static func icon(_ section: Directory) -> String {
        switch section.type {
        case "movie": "film"
        case "show": "tv"
        case "artist": "music.note"
        case "photo": "photo"
        default: "folder"
        }
    }

    /// The destinations the iPhone's More tab lists, in order.
    static let moreEntries: [Destination] = [.pick, .playlists, .watchlist, .livetv, .requests, .activity, .server, .settings]

    /// Where `route` lives when it is one of the top-level places: the destination and the
    /// stack to show in it. `nil` for anything that is pushed onto the current stack.
    static func place(_ route: Route, compact: Bool) -> (Destination, [Route])? {
        let top: Destination? =
            switch route {
            case .home: .home
            case .search: .search
            case .pick: .pick
            case .playlists: .playlists
            case .watchlist: .watchlist
            case .downloads: .downloads
            case .livetv: .livetv
            case .requests: .requests
            case .activity: .activity
            case .server: .server
            case .settings: .settings
            case .library(let id): .library(id: id)
            case .item, .browse, .playlist, .seerr: nil
            }
        guard let top else { return nil }
        guard compact else { return (top, []) }
        switch top {
        case .home, .search, .downloads: return (top, [])
        case .library: return (.libraries, [route])
        default: return (.more, [route])
        }
    }
}
