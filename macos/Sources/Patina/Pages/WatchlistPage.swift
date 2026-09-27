import SwiftUI

/// The plex.tv watchlist. Each entry opens the server's own copy when there is
/// one; titles this server doesn't have are dimmed.
struct WatchlistPage: View {
    @Environment(AppModel.self) private var app

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                PageHeader("Watchlist", subtitle: "Synced with your Plex account. Titles this server doesn't have are dimmed.")
                Loading(id: [AnyHashable(app.revision), AnyHashable(app.scope)], { try await API.watchlist() }) { list in
                    if list.isEmpty {
                        EmptyState(title: "Your watchlist is empty", message: "Add movies and shows from their detail page.")
                    } else {
                        LazyVGrid(
                            columns: [GridItem(.adaptive(minimum: app.posterWidth, maximum: app.posterWidth * 1.4), spacing: 18, alignment: .top)],
                            alignment: .leading, spacing: 22
                        ) {
                            ForEach(Array(list.enumerated()), id: \.offset) { _, m in WatchlistEntry(item: m).id(m.id) }
                        }
                        .padding(.horizontal, Theme.gutter)
                    }
                }
            }
            .padding(.bottom, 40)
        }
        .navigationTitle("Watchlist")
    }
}

/// guid → the server's rating key (nil when the server doesn't have it),
/// remembered for the session so revisits don't ask again.
@MainActor
private enum WatchlistLocal {
    static var known: [String: String?] = [:]
    static var scope = ""
}

private struct WatchlistEntry: View {
    let item: Metadata

    @Environment(AppModel.self) private var app
    @State private var hovering = false
    /// nil while unknown; .some(nil) when the server doesn't have it.
    @State private var local: String??

    private var missing: Bool { if case .some(.none) = local { true } else { false } }
    private var localKey: String? { local ?? nil }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Color.clear
                .aspectRatio(2 / 3, contentMode: .fit)
                .overlay { Artwork(item.thumb, width: 300, height: 450, fallback: item.name) }
                .overlay(alignment: .topTrailing) {
                    if hovering {
                        Button { remove() } label: {
                            Image(systemName: "xmark")
                                .font(.system(size: 10, weight: .semibold))
                                .foregroundStyle(.white)
                                .frame(width: 22, height: 22)
                                .background(RoundedRectangle(cornerRadius: 3).fill(.black.opacity(0.75)))
                        }
                        .buttonStyle(.plain)
                        .help("Remove from Watchlist")
                        .padding(6)
                    }
                }
                .clipShape(RoundedRectangle(cornerRadius: Theme.radius))
                .overlay(RoundedRectangle(cornerRadius: Theme.radius).strokeBorder(Theme.hairline))
                .opacity(missing ? 0.4 : 1)
            VStack(alignment: .leading, spacing: 1) {
                Text(item.name).font(.system(size: 12, weight: .medium)).lineLimit(1)
                Text(missing ? "Not on this server" : item.subtitle)
                    .font(Theme.caption).foregroundStyle(.secondary).lineLimit(1)
            }
        }
        .contentShape(Rectangle())
        .onTapGesture { open() }
        .onHover { hovering = $0 }
        .contextMenu {
            Button("Show Details", systemImage: "info.circle") { open() }.disabled(localKey == nil)
            Divider()
            Button("Remove from Watchlist", systemImage: "bookmark.slash") { remove() }
        }
        .help(item.fullTitle)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityAction { open() }
        .task(id: item.guid) { await resolve() }
    }

    private func resolve() async {
        guard let guid = item.guid else { local = .some(nil); return }
        if WatchlistLocal.scope != app.scope { WatchlistLocal.known = [:]; WatchlistLocal.scope = app.scope }
        if let hit = WatchlistLocal.known[guid] { local = .some(hit); return }
        // Only cache real answers; a failed lookup retries next time the card appears.
        let rk: String?
        do { rk = try await API.findByGuid(guid) } catch { return }
        WatchlistLocal.known[guid] = .some(rk)
        local = .some(rk)
    }

    private func open() {
        if let rk = localKey { app.open(.item(rk)) } else { app.toast("Not available on this server") }
    }

    private func remove() {
        let id = item.ratingKey ?? ""
        app.perform("Removed from watchlist", refresh: true) { try await API.setWatchlisted(id, false) }
    }
}
