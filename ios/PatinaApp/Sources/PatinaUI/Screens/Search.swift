// Port of web/src/pages/Search.svelte: search as you type (debounced), grouped by type,
// with people and tags. ⌘F focuses the field.

import PatinaCore
import PlexKit
import SwiftUI

struct SearchScreen: View {
    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var results: (q: String, hubs: [Hub]?, error: String?)?
    @State private var retry = 0
    @State private var searchFocused = false

    private static let people = ["actor", "director", "writer", "producer", "person", "tag"]

    private var query: String { store.searchText.trimmingCharacters(in: .whitespacesAndNewlines) }

    var body: some View {
        @Bindable var store = store
        ScrollView {
            VStack(alignment: .leading, spacing: 28) {
                // TODO: "Search all servers" (search_all_servers) is a later task; the toggle waits for it.
                if (store.session?.servers.count ?? 0) > 1 {
                    Toggle(isOn: .constant(false)) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Search All Servers").font(.subheadline.weight(.medium))
                            Text("Coming in a later update").font(.caption).foregroundStyle(Theme.tertiaryText)
                        }
                    }
                    .disabled(true)
                    .padding(.horizontal, Theme.margin(sizeClass))
                }
                content
            }
            .padding(.vertical, 8)
        }
        .scrollDismissesKeyboard(.immediately)
        .navigationTitle("Search")
        .searchable(
            text: $store.searchText, isPresented: $searchFocused, placement: .navigationBarDrawer(displayMode: .always),
            prompt: "Movies, shows, people, genres")
        .task(id: "\(query)|\(retry)|\(store.revision)") { await search() }
        .onChange(of: store.searchFocusRequest, initial: true) { _, n in
            if n > 0 { searchFocused = true }
        }
    }

    @ViewBuilder
    private var content: some View {
        if query.count < 2 {
            StatusView(
                title: "Find something to watch", text: "Movies, shows, episodes, music, actors, directors, genres…",
                systemImage: "magnifyingglass")
        } else if let r = results, r.q == query {
            if let error = r.error {
                StatusView(error: error) { retry += 1 }
            } else if let hubs = r.hubs, !hubs.isEmpty {
                ForEach(Array(hubs.enumerated()), id: \.offset) { _, hub in section(hub) }
            } else {
                StatusView(title: "No results", text: "Check the spelling or try another word.", systemImage: "magnifyingglass")
            }
        } else {
            LazyVGrid(columns: cardColumns(.poster, sizeClass), spacing: 22) {
                ForEach(0..<8, id: \.self) { _ in SkeletonCard() }
            }
            .padding(.horizontal, Theme.margin(sizeClass))
        }
    }

    private func section(_ hub: Hub) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            SectionHeader(hub.title ?? "")
            if !hub.metadata.isEmpty {
                let wide = hub.type == "episode" || hub.type == "clip"
                let shape: CardShape = wide ? .landscape : PlexUI.commonShape(hub.metadata)
                LazyVGrid(columns: cardColumns(shape, sizeClass), spacing: 20) {
                    ForEach(Array(hub.metadata.enumerated()), id: \.offset) { _, item in
                        Card(item, shape: shape)
                    }
                }
                .padding(.horizontal, Theme.margin(sizeClass))
            }
            if !hub.directory.isEmpty {
                let people = hub.directory.filter { Self.people.contains($0.type ?? "") }
                let tags = hub.directory.filter { !Self.people.contains($0.type ?? "") }
                if !people.isEmpty {
                    ScrollView(.horizontal) {
                        LazyHStack(alignment: .top, spacing: 16) {
                            ForEach(Array(people.enumerated()), id: \.offset) { _, d in person(d) }
                        }
                    }
                    .scrollIndicators(.hidden)
                    .contentMargins(.horizontal, Theme.margin(sizeClass), for: .scrollContent)
                }
                if !tags.isEmpty {
                    FlowLayout(spacing: 8) {
                        ForEach(Array(tags.enumerated()), id: \.offset) { _, d in
                            if let key = d.key {
                                NavigationLink(value: Route.browse(path: key, title: Directory.title(d))) {
                                    Text(Directory.title(d))
                                        .font(.subheadline.weight(.medium))
                                        .padding(.horizontal, 14)
                                        .padding(.vertical, 8)
                                        .background(Theme.raised, in: .capsule)
                                        .overlay(Capsule().strokeBorder(Theme.hairline))
                                }
                                .buttonStyle(CardButtonStyle())
                            }
                        }
                    }
                    .padding(.horizontal, Theme.margin(sizeClass))
                }
            }
        }
    }

    @ViewBuilder
    private func person(_ d: Directory) -> some View {
        if let key = d.key {
            let name = Directory.title(d)
            NavigationLink(value: Route.browse(path: key, title: name)) {
                VStack(spacing: 6) {
                    Avatar(name: name, thumb: d.thumb, size: 76)
                    Text(name).font(.caption.weight(.medium)).lineLimit(2).multilineTextAlignment(.center)
                    if let count = d.count {
                        Text("\(count) titles").font(.caption2).foregroundStyle(Theme.secondaryText)
                    }
                }
                .frame(width: 88)
            }
            .buttonStyle(CardButtonStyle())
        }
    }

    /// Debounced: a newer keystroke cancels the pending search (the task restarts).
    private func search() async {
        let q = query
        guard q.count >= 2 else {
            results = nil
            return
        }
        try? await Task.sleep(for: .milliseconds(220))
        if Task.isCancelled { return }
        do {
            let hubs = try await Commands.search(store.app, query: q)
            if !Task.isCancelled && query == q {
                withAnimation(.easeOut(duration: 0.2)) {
                    results = (q, hubs.filter { !Hub.isEmpty($0) }, nil)
                }
            }
        } catch is CancellationError {
        } catch {
            if query == q { results = (q, nil, errorText(error)) }
        }
    }
}

/// Lays out children in rows, wrapping (tags, chips).
struct FlowLayout: Layout {
    var spacing: CGFloat = 8

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? .infinity
        var x: CGFloat = 0
        var y: CGFloat = 0
        var row: CGFloat = 0
        var widest: CGFloat = 0
        for s in subviews {
            let size = Self.size(s, width)
            if x > 0 && x + size.width > width {
                y += row + spacing
                x = 0
                row = 0
            }
            x += size.width + spacing
            widest = max(widest, x - spacing)
            row = max(row, size.height)
        }
        return CGSize(width: min(widest, width), height: y + row)
    }

    /// A child's ideal size, but no wider than a row (long text wraps).
    private static func size(_ s: LayoutSubview, _ width: CGFloat) -> CGSize {
        let ideal = s.sizeThatFits(.unspecified)
        guard ideal.width > width, width.isFinite else { return ideal }
        return s.sizeThatFits(ProposedViewSize(width: width, height: nil))
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var x = bounds.minX
        var y = bounds.minY
        var row: CGFloat = 0
        for s in subviews {
            let size = Self.size(s, bounds.width)
            if x > bounds.minX && x + size.width > bounds.maxX {
                y += row + spacing
                x = bounds.minX
                row = 0
            }
            s.place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(size))
            x += size.width + spacing
            row = max(row, size.height)
        }
    }
}
