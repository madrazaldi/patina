import SwiftUI

/// Every playlist on the server, grouped by kind when there's more than one.
struct PlaylistsPage: View {
    @Environment(AppModel.self) private var app

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                PageHeader("Playlists", subtitle: "Right-click anything and choose Add to Playlist… to build one.")
                Loading(id: [AnyHashable(app.revision), AnyHashable(app.scope)], { try await API.playlists() }) { lists in
                    if lists.isEmpty {
                        EmptyState(title: "No playlists yet")
                    } else {
                        let groups = Self.groups(lists)
                        VStack(alignment: .leading, spacing: 30) {
                            ForEach(groups, id: \.0) { title, items in
                                VStack(alignment: .leading, spacing: 12) {
                                    if groups.count > 1 {
                                        Text(title).font(Theme.sectionTitle).padding(.horizontal, Theme.gutter)
                                    }
                                    CardGrid(items: items, shape: .square)
                                }
                            }
                        }
                    }
                }
            }
            .padding(.bottom, 40)
        }
        .navigationTitle("Playlists")
    }

    private static func groups(_ lists: [Metadata]) -> [(String, [Metadata])] {
        let order: [(String, String)] = [("video", "Video"), ("audio", "Music"), ("photo", "Photos")]
        var out = order.compactMap { type, title -> (String, [Metadata])? in
            let items = lists.filter { $0.playlistType == type }
            return items.isEmpty ? nil : (title, items)
        }
        let other = lists.filter { !["video", "audio", "photo"].contains($0.playlistType ?? "") }
        if !other.isEmpty { out.append(("Other", other)) }
        return out
    }
}
