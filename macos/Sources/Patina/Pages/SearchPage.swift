import SwiftUI

/// Search every library as you type (the sidebar search field). Results come
/// grouped by type; people and tags open a listing of their titles.
struct SearchPage: View {
    @Environment(AppModel.self) private var app
    @State private var results: (query: String, phase: Phase<[Hub]>)?
    @State private var attempt = 0

    private var query: String { app.searchText.trimmingCharacters(in: .whitespacesAndNewlines) }

    var body: some View {
        let query = query
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                PageHeader(query.isEmpty ? "Search" : "“\(query)”", subtitle: "Every library on this server.")
                content(query)
            }
            .padding(.bottom, 40)
        }
        .navigationTitle("Search")
        .task(id: "\(query)\u{0}\(attempt)") {
            guard query.count >= 2 else { results = nil; return }
            // Debounce: a newer keystroke cancels this task.
            try? await Task.sleep(for: .milliseconds(220))
            guard !Task.isCancelled else { return }
            do {
                let hubs = try await API.search(query)
                guard !Task.isCancelled else { return }
                results = (query, .loaded(hubs.filter { !$0.metadata.isEmpty || !$0.directories.isEmpty }))
            } catch is CancellationError {
            } catch {
                guard !Task.isCancelled else { return }
                results = (query, .failed(error.localizedDescription))
            }
        }
    }

    @ViewBuilder private func content(_ query: String) -> some View {
        if query.count < 2 {
            EmptyState(title: "Find something to watch", message: "Movies, shows, episodes, music, actors, directors, genres…")
        } else if let results, results.query == query {
            switch results.phase {
            case .loading: EmptyView()
            case .failed(let message): ErrorState(message: message) { attempt += 1 }
            case .loaded(let hubs) where hubs.isEmpty:
                EmptyState(title: "No results", message: "Check the spelling or try another word.")
            case .loaded(let hubs):
                VStack(alignment: .leading, spacing: 34) {
                    ForEach(Array(hubs.enumerated()), id: \.offset) { _, hub in
                        SearchSection(hub: hub)
                    }
                }
            }
        } else {
            ProgressView().controlSize(.small).frame(maxWidth: .infinity, minHeight: 200)
        }
    }
}

/// One group of results: cards for items, faces for people, chips for tags.
private struct SearchSection: View {
    let hub: Hub

    private static let people: Set<String> = ["actor", "director", "writer", "producer", "person", "tag"]

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(hub.title ?? "").font(Theme.sectionTitle).padding(.horizontal, Theme.gutter)
            if !hub.metadata.isEmpty {
                let wide = hub.type == "episode" || hub.type == "clip"
                CardGrid(items: hub.metadata, shape: wide ? .landscape : nil)
            }
            if !hub.directories.isEmpty {
                let people = hub.directories.filter { Self.people.contains($0.type ?? "") }
                let tags = hub.directories.filter { !Self.people.contains($0.type ?? "") }
                if !people.isEmpty {
                    ScrollView(.horizontal, showsIndicators: false) {
                        LazyHStack(alignment: .top, spacing: 10) {
                            ForEach(Array(people.enumerated()), id: \.offset) { _, d in PersonTile(directory: d) }
                        }
                        .padding(.horizontal, Theme.gutter)
                    }
                }
                if !tags.isEmpty {
                    SearchChips(tags: tags).padding(.horizontal, Theme.gutter)
                }
            }
        }
    }
}

@MainActor private func openTag(_ d: Directory, _ app: AppModel) {
    guard let key = d.key else { return }
    app.open(.browse(path: key, title: d.title ?? d.tag ?? ""))
}

private struct PersonTile: View {
    let directory: Directory
    @Environment(AppModel.self) private var app
    @State private var hovering = false

    private var name: String { directory.title ?? directory.tag ?? "" }

    var body: some View {
        Button { openTag(directory, app) } label: {
            VStack(spacing: 3) {
                Avatar(path: directory.thumb, name: name, size: 72)
                    .overlay(Circle().strokeBorder(Theme.hairline))
                    .padding(.bottom, 5)
                Text(name)
                    .font(.system(size: 12, weight: .medium))
                    .underline(hovering)
                    .lineLimit(2)
                    .multilineTextAlignment(.center)
                if let count = directory.count {
                    Text(Fmt.plural(count, "title")).font(Theme.caption).foregroundStyle(.secondary)
                }
            }
            .frame(width: 96)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .help("Show titles with \(name)")
    }
}

/// Genres, collections and other tags as a wrapping row of chips.
private struct SearchChips: View {
    let tags: [Directory]
    @Environment(AppModel.self) private var app

    var body: some View {
        SearchFlow(spacing: 8) {
            ForEach(Array(tags.enumerated()), id: \.offset) { _, d in
                Button(d.title ?? d.tag ?? "") { openTag(d, app) }
                    .buttonStyle(.bordered)
                    .controlSize(.regular)
            }
        }
    }
}

/// Lays children out left to right, wrapping onto new lines.
private struct SearchFlow: Layout {
    var spacing: CGFloat = 8

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? .infinity
        var x: CGFloat = 0, y: CGFloat = 0, line: CGFloat = 0, widest: CGFloat = 0
        for s in subviews {
            let size = s.sizeThatFits(.unspecified)
            if x > 0 && x + size.width > width { x = 0; y += line + spacing; line = 0 }
            x += size.width + spacing
            widest = max(widest, x - spacing)
            line = max(line, size.height)
        }
        return CGSize(width: min(widest, width), height: y + line)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var x = bounds.minX, y = bounds.minY, line: CGFloat = 0
        for s in subviews {
            let size = s.sizeThatFits(.unspecified)
            if x > bounds.minX && x + size.width > bounds.maxX { x = bounds.minX; y += line + spacing; line = 0 }
            s.place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(size))
            x += size.width + spacing
            line = max(line, size.height)
        }
    }
}
