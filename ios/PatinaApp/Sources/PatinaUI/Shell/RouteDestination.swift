// The screen of every `Route` (the `{#if route.name === ...}` chain of web/src/App.svelte).
// A new screen registers here: give it a `Route` case (Route.swift), map it below, and if it
// is a top-level place, a `Destination` (Destination.swift).

import PatinaCore
import SwiftUI

struct RouteDestination: View {
    let route: Route
    /// The root of a destination's stack (no zoom transition into it).
    var isRoot = false

    @Environment(\.zoomNamespace) private var zoom

    var body: some View {
        screen
            .modifier(ZoomDestination(route: route, namespace: isRoot ? nil : zoom))
    }

    @ViewBuilder
    private var screen: some View {
        switch route {
        case .home: HomeScreen()
        case .library(let id): LibraryScreen(id: id)
        case .item(let ratingKey, let server): ItemScreen(ratingKey: ratingKey, server: server)
        case .browse(let path, let title): BrowseScreen(path: path, title: title)
        case .search: SearchScreen()
        case .playlists: PlaylistsScreen()
        case .playlist(let id): PlaylistScreen(id: id)
        case .pick: PickPlaceholder()
        case .watchlist: WatchlistPlaceholder()
        case .downloads: DownloadsPlaceholder()
        case .livetv: LiveTvPlaceholder()
        case .activity: ActivityPlaceholder()
        case .server: ServerPlaceholder()
        case .settings: SettingsPlaceholder()
        case .requests: RequestsPlaceholder()
        case .seerr(let mediaType, let tmdbId): SeerrTitlePlaceholder(mediaType: mediaType, tmdbId: tmdbId)
        }
    }
}

/// Items and playlists zoom out of the card that opened them.
private struct ZoomDestination: ViewModifier {
    let route: Route
    let namespace: Namespace.ID?

    func body(content: Content) -> some View {
        switch route {
        case .item, .playlist:
            if let namespace {
                content.navigationTransition(.zoom(sourceID: route, in: namespace))
            } else {
                content
            }
        default:
            content
        }
    }
}
