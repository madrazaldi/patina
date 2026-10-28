// Port of web/src/pages/Browse.svelte: any server listing by key ("See all" of a hub, a
// genre, an actor), loaded a page at a time as it scrolls.

import PatinaCore
import PlexKit
import SwiftUI

/// The columns of a grid of cards, by shape and size class.
func cardColumns(_ shape: CardShape, _ sizeClass: UserInterfaceSizeClass?) -> [GridItem] {
    let regular = sizeClass == .regular
    let minimum: CGFloat =
        switch shape {
        case .poster: regular ? 140 : 104
        case .landscape: regular ? 260 : 160
        case .square: regular ? 150 : 110
        }
    return [GridItem(.adaptive(minimum: minimum), spacing: regular ? 18 : 12, alignment: .top)]
}

struct BrowseScreen: View {
    let path: String
    let title: String

    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var items: [Metadata] = []
    @State private var done = false
    @State private var loading = false
    @State private var error: String?

    private static let page = 100

    var body: some View {
        let shape: CardShape = items.first.map { PlexUI.shape($0) == .landscape ? .landscape : PlexUI.commonShape(items) } ?? .poster
        ScrollView {
            if let error, !items.isEmpty {
                StaleBanner(error: error, retry: load).padding(.horizontal, Theme.margin(sizeClass))
            }
            if items.isEmpty {
                if let error {
                    StatusView(error: error, retry: load)
                } else if loading || !done {
                    skeleton
                } else {
                    StatusView(title: "Nothing here", systemImage: "square.grid.2x2")
                }
            } else {
                LazyVGrid(columns: cardColumns(shape, sizeClass), spacing: 22) {
                    ForEach(Array(items.enumerated()), id: \.offset) { i, item in
                        Card(item, shape: shape)
                            .onAppear { if i >= items.count - 12 { load() } }
                    }
                }
                .padding(.horizontal, Theme.margin(sizeClass))
                if loading { ProgressView().padding() }
            }
        }
        .navigationTitle(title)
        .refreshable { await reload() }
        .task { if items.isEmpty { load() } }
    }

    private var skeleton: some View {
        LazyVGrid(columns: cardColumns(.poster, sizeClass), spacing: 22) {
            ForEach(0..<12, id: \.self) { _ in SkeletonCard() }
        }
        .padding(.horizontal, Theme.margin(sizeClass))
    }

    private func load() {
        guard !loading, !done || items.isEmpty else { return }
        if done && !items.isEmpty { return }
        loading = true
        error = nil
        Task { await fetch(from: items.count) }
    }

    private func reload() async {
        done = false
        loading = true
        await fetch(from: 0)
    }

    private func fetch(from start: Int) async {
        do {
            let mc = try await Commands.listing(store.app, path: path, start: start, size: Self.page)
            if start == 0 { items = [] }
            items.append(contentsOf: mc.metadata)
            // Some endpoints ignore paging; stop when a page comes back short.
            done = mc.metadata.count < Self.page || (mc.totalSize.map { items.count >= $0 } ?? false)
        } catch {
            self.error = errorText(error)
        }
        loading = false
    }
}
