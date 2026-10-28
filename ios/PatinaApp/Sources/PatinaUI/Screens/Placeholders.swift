// PLACEHOLDERS: the screens outside the first UI task. Each is replaced by its port in a
// later task (the downloads, admin/discovery and playback agents' screens); `RouteDestination`
// already routes to them, so the replacement keeps the type's name or changes that one line.

import PatinaCore
import SwiftUI

/// A screen that isn't built yet: its title, what it will do, and the web page it ports.
struct PlaceholderScreen: View {
    var title: String
    var systemImage: String
    var text: String
    /// The web page the screen will port.
    var port: String

    var body: some View {
        ScrollView {
            ContentUnavailableView {
                Label(title, systemImage: systemImage)
            } description: {
                Text(text)
            } actions: {
                Text("Coming in a later update · \(port)")
                    .font(.caption)
                    .foregroundStyle(Theme.tertiaryText)
            }
            .padding(.top, 80)
        }
        .navigationTitle(title)
    }
}

/// TODO(placeholder): web/src/pages/Pick.svelte.
struct PickPlaceholder: View {
    var body: some View {
        PlaceholderScreen(
            title: "Pick Something", systemImage: "dice",
            text: "Let Patina choose something to watch, by mood, length and what you haven't seen.",
            port: "Pick.svelte")
    }
}

/// TODO(placeholder): web/src/pages/Watchlist.svelte.
struct WatchlistPlaceholder: View {
    var body: some View {
        PlaceholderScreen(
            title: "Watchlist", systemImage: "bookmark", text: "Your plex.tv watchlist, and where to find each title.",
            port: "Watchlist.svelte")
    }
}

/// TODO(placeholder): web/src/pages/Downloads.svelte (downloads agent).
struct DownloadsPlaceholder: View {
    var body: some View {
        PlaceholderScreen(
            title: "Downloads", systemImage: "arrow.down.circle",
            text: "Movies, episodes and music saved on this device for offline playback.", port: "Downloads.svelte")
    }
}

/// TODO(placeholder): web/src/pages/LiveTV.svelte.
struct LiveTvPlaceholder: View {
    var body: some View {
        PlaceholderScreen(
            title: "Live TV", systemImage: "dot.radiowaves.left.and.right",
            text: "The guide of your DVR, and your recordings.", port: "LiveTV.svelte")
    }
}

/// TODO(placeholder): web/src/pages/Activity.svelte.
struct ActivityPlaceholder: View {
    var body: some View {
        PlaceholderScreen(
            title: "Activity", systemImage: "waveform.path.ecg",
            text: "What is playing on the server, its background tasks and the watch history.", port: "Activity.svelte")
    }
}

/// TODO(placeholder): web/src/pages/Server.svelte.
struct ServerPlaceholder: View {
    var body: some View {
        PlaceholderScreen(
            title: "Server", systemImage: "server.rack",
            text: "Libraries, users, settings, updates and statistics of your server.", port: "Server.svelte")
    }
}

/// TODO(placeholder): web/src/pages/Settings.svelte.
struct SettingsPlaceholder: View {
    var body: some View {
        PlaceholderScreen(
            title: "Settings", systemImage: "gearshape",
            text: "Playback, appearance, downloads and your Seerr instance.", port: "Settings.svelte")
    }
}

/// TODO(placeholder): web/src/pages/Requests.svelte.
struct RequestsPlaceholder: View {
    var body: some View {
        PlaceholderScreen(
            title: "Requests", systemImage: "tray", text: "Discover titles and request them with Seerr.",
            port: "Requests.svelte")
    }
}

/// TODO(placeholder): web/src/pages/SeerrTitle.svelte.
struct SeerrTitlePlaceholder: View {
    var mediaType: SeerrMediaType
    var tmdbId: Int

    var body: some View {
        PlaceholderScreen(
            title: mediaType == .movie ? "Movie" : "Show", systemImage: "tray",
            text: "Details of TMDB title \(tmdbId), and requesting it with Seerr.", port: "SeerrTitle.svelte")
    }
}

/// TODO(placeholder): web/src/components/CommandPalette.svelte. ⌘K opens this (`store.palette`).
struct CommandPalettePlaceholder: View {
    var body: some View {
        ContentUnavailableView(
            "Command Palette", systemImage: "command",
            description: Text("Jump anywhere and run any action from the keyboard. Coming in a later update."))
    }
}
