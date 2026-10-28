// Port of the `Route` type of web/src/lib/router.svelte.ts. Screens link to each other
// with these values; `RouteDestination` turns one into its screen, so no screen has to
// import another. History and scroll restoration are NavigationStack's.

import Foundation

public enum SeerrMediaType: String, Hashable, Sendable, Codable {
    case movie
    case tv
}

public enum Route: Hashable, Sendable {
    case home
    case library(id: String)
    /// `server` is the machine id of another server of the account (search all servers).
    case item(ratingKey: String, server: String? = nil)
    /// Any server listing by key: a hub's "See all", a genre or actor.
    case browse(path: String, title: String)
    case search
    case pick
    case playlists
    case playlist(id: String)
    case watchlist
    case downloads
    case livetv
    case activity
    case server
    case settings
    case requests
    case seerr(mediaType: SeerrMediaType, tmdbId: Int)

    /// "/item/100" style paths (`PATINA_ROUTE` in debug builds).
    public static func parse(_ path: String) -> Route? {
        let parts = path.split(separator: "/", omittingEmptySubsequences: false).map(String.init)
        let a = parts.count > 1 ? parts[1] : ""
        let b = parts.count > 2 ? parts[2] : ""
        switch a {
        case "": return .home
        case "library": return b.isEmpty ? nil : .library(id: b)
        case "item": return b.isEmpty ? nil : .item(ratingKey: b)
        case "playlist": return b.isEmpty ? nil : .playlist(id: b)
        case "seerr":
            guard let type = SeerrMediaType(rawValue: b), parts.count > 3, let id = Int(parts[3]), id > 0 else { return nil }
            return .seerr(mediaType: type, tmdbId: id)
        case "search": return .search
        case "pick": return .pick
        case "playlists": return .playlists
        case "watchlist": return .watchlist
        case "downloads": return .downloads
        case "livetv": return .livetv
        case "activity": return .activity
        case "server": return .server
        case "settings": return .settings
        case "requests": return .requests
        default: return nil
        }
    }
}
