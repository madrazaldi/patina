// Port of web/src/pages/Search.svelte: search as you type (debounced), grouped by type,
// with people and tags, or across every server of the account (results fill in as servers
// answer, with each server's state). ⌘F focuses the field.

import PatinaCore
import PlexKit
import SwiftUI

struct SearchScreen: View {
    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var results: (q: String, hubs: [Hub]?, error: String?)?
    @State private var retry = 0
    @State private var searchFocused = false
    /// Search every server of the account (a toggle kept for the session).
    @State private var searchAll = false
    /// Results of a search across every server; they grow as servers answer.
    @State private var everywhere: (q: String, all: Ops.XSearch.Results?)?
    /// The title whose copies are being chosen.
    @State private var chosen: Ops.XSearch.Group?

    private static let people = ["actor", "director", "writer", "producer", "person", "tag"]

    private var query: String { store.searchText.trimmingCharacters(in: .whitespacesAndNewlines) }
    /// Searching every server needs a second one on the account.
    private var all: Bool { searchAll && (store.session?.servers.count ?? 0) > 1 }

    var body: some View {
        @Bindable var store = store
        ScrollView {
            VStack(alignment: .leading, spacing: 28) {
                if (store.session?.servers.count ?? 0) > 1 {
                    Toggle(isOn: $searchAll) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Search All Servers").font(.subheadline.weight(.medium))
                            Text(all ? "Every library on every server of your account." : "Every library on this server.")
                                .font(.caption).foregroundStyle(Theme.secondaryText)
                        }
                    }
                    .tint(store.accent)
                    .padding(.horizontal, Theme.margin(sizeClass))
                }
                if all && query.count >= 2 {
                    AllServersResults(results: everywhere?.q == query ? everywhere?.all : nil) { g in open(g) }
                        .padding(.horizontal, Theme.margin(sizeClass))
                } else {
                    content
                }
            }
            .padding(.vertical, 8)
        }
        .scrollDismissesKeyboard(.immediately)
        .navigationTitle("Search")
        .searchable(
            text: $store.searchText, isPresented: $searchFocused, placement: .navigationBarDrawer(displayMode: .always),
            prompt: "Movies, shows, people, genres")
        .task(id: "\(query)|\(retry)|\(store.revision)|\(all)") { await search() }
        .sheet(item: Binding(get: { chosen.map(ChosenGroup.init) }, set: { if $0 == nil { chosen = nil } })) { c in
            CopySheet(group: c.group).presentationDetents([.medium, .large])
        }
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

    /// One copy on the connected server, in one file, opens directly; anything else asks which.
    private func open(_ g: Ops.XSearch.Group) {
        if let first = g.copies.first, g.copies.count == 1, first.versions.count <= 1, first.current {
            store.navigate(to: PlexUI.route(first.item))
        } else {
            chosen = g
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
        if all {
            if everywhere?.q != q { everywhere = (q, nil) }
            let found = await Commands.searchAllServers(store.app, query: q) { r in
                if !Task.isCancelled && query == q { everywhere = (q, r) }
            }
            if !Task.isCancelled && query == q { everywhere = (q, found) }
            return
        }
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

private struct ChosenGroup: Identifiable {
    let group: Ops.XSearch.Group
    var id: String { group.key }
}

/// Results of a search across servers: which servers answered, then one row per title with
/// the servers that have it.
private struct AllServersResults: View {
    let results: Ops.XSearch.Results?
    let open: (Ops.XSearch.Group) -> Void
    @Environment(AppStore.self) private var store

    var body: some View {
        if let found = results {
            VStack(alignment: .leading, spacing: 16) {
                FlowLayout(spacing: 6) {
                    ForEach(found.servers, id: \.id) { s in
                        let bad = s.state == .failed || s.state == .timedOut
                        HStack(spacing: 4) {
                            if s.state == .pending { ProgressView().controlSize(.mini) }
                            Text(s.name).fontWeight(.medium)
                            Text(Self.stateLabel(s)).foregroundStyle(bad ? Theme.danger : Theme.secondaryText)
                        }
                        .font(.caption)
                        .padding(.horizontal, 10).padding(.vertical, 5)
                        .background(Theme.raised, in: .capsule)
                        .help(s.error ?? "")
                    }
                }
                .accessibilityElement(children: .combine)
                .accessibilityLabel("Servers searched")
                if !found.groups.isEmpty {
                    LazyVStack(spacing: 0) {
                        ForEach(found.groups, id: \.key) { g in
                            row(g)
                            Divider().opacity(0.5)
                        }
                    }
                } else if found.done {
                    StatusView(
                        title: "No results", text: "None of your servers has it. Check the spelling or try another word.",
                        systemImage: "magnifyingglass")
                } else {
                    ProgressView().frame(maxWidth: .infinity).padding(.top, 30)
                }
            }
        } else {
            ProgressView().frame(maxWidth: .infinity).padding(.top, 40)
        }
    }

    private func row(_ g: Ops.XSearch.Group) -> some View {
        let first = g.copies.first
        return Button {
            open(g)
        } label: {
            HStack(spacing: 12) {
                Artwork(first.flatMap { Metadata.poster($0.item) }, width: 96, height: 144, server: first.flatMap { Metadata.serverId($0.item) })
                    .frame(width: 40, height: 60)
                    .clipShape(.rect(cornerRadius: 6))
                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 4) {
                        Text(g.title).font(.subheadline.weight(.semibold)).lineLimit(1)
                        if let y = g.year { Text("(\(String(y)))").font(.subheadline).foregroundStyle(Theme.secondaryText) }
                    }
                    Text(g.subtitle ?? g.kind.capitalized).font(.caption).foregroundStyle(Theme.secondaryText).lineLimit(1)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                VStack(alignment: .trailing, spacing: 4) {
                    ForEach(g.copies, id: \.serverId) { c in Badge(Self.badge(c), accent: c.current) }
                }
            }
            .padding(.vertical, 8)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("\(g.title)\(g.year.map { " (\($0))" } ?? ""), on \(g.copies.map(Self.badge).joined(separator: ", "))")
    }

    /// What a server's search came to (`stateLabel` of web/src/lib/xsearch.ts).
    static func stateLabel(_ s: Ops.XSearch.ServerStatus) -> String {
        switch s.state {
        case .pending: "searching…"
        case .done: s.count == 1 ? "1 title" : "\(s.count) titles"
        case .timedOut: "too slow to answer"
        case .offline: "offline"
        case .failed: "unavailable"
        }
    }

    /// The best resolution among a copy's files ("4K", "1080p"), if the server reports any.
    static func quality(_ c: Ops.XSearch.ServerCopy) -> String? {
        func rank(_ r: String) -> Int { r == "4K" ? 2160 : r == "SD" ? 480 : Int(r.prefix { $0.isNumber }) ?? 0 }
        return c.versions.compactMap(\.resolution).max { rank($0) < rank($1) }
    }

    /// "Alpha 1080p": a server with the quality of its best file.
    static func badge(_ c: Ops.XSearch.ServerCopy) -> String {
        [c.serverName, quality(c)].compactMap { $0 }.joined(separator: " ")
    }
}

/// Choose which server's copy of a title to play (CopyDialog.svelte).
private struct CopySheet: View {
    let group: Ops.XSearch.Group
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    /// Playing or opening is dropped if the account or server changed meanwhile.
    @State private var scope = ""

    var body: some View {
        NavigationStack {
            List {
                Section {
                    ForEach(group.copies, id: \.serverId) { copy in
                        let resumable = Metadata.progress(copy.item) != nil
                        VStack(alignment: .leading, spacing: 6) {
                            HStack {
                                Text(copy.serverName).font(.headline)
                                if copy.current { Badge("Connected", accent: true) }
                            }
                            if let l = copy.library { Text(l).font(.caption).foregroundStyle(Theme.secondaryText) }
                            ForEach(Array(copy.versions.enumerated()), id: \.offset) { _, v in
                                Text(v.label).font(.caption.monospacedDigit()).foregroundStyle(Theme.secondaryText)
                            }
                            if copy.versions.isEmpty {
                                Text("No file information").font(.caption).foregroundStyle(Theme.tertiaryText)
                            }
                            HStack {
                                Button(resumable ? "Resume" : "Play", systemImage: "play.fill") { use(copy, resume: true) }
                                    .buttonStyle(.borderedProminent)
                                    .foregroundStyle(.black)
                                if resumable {
                                    Button("Play from Start", systemImage: "arrow.counterclockwise") { use(copy, resume: false) }
                                        .buttonStyle(.bordered)
                                }
                                Button("Details", systemImage: "info.circle") {
                                    dismiss()
                                    if scope == store.scope { store.navigate(to: PlexUI.route(copy.item)) }
                                }
                                .buttonStyle(.bordered)
                                .labelStyle(.iconOnly)
                            }
                            .controlSize(.small)
                            .padding(.top, 2)
                        }
                        .padding(.vertical, 4)
                    }
                } footer: {
                    Text(group.copies.count == 1 ? "On one server." : "On \(group.copies.count) servers.")
                }
            }
            .navigationTitle("\(group.title)\(group.year.map { " (\($0))" } ?? "")")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
        }
        .onAppear { scope = store.scope }
    }

    private func use(_ copy: Ops.XSearch.ServerCopy, resume: Bool) {
        dismiss()
        if scope == store.scope { store.play(copy.item, .init(resume: resume)) }
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
