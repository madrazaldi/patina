import SwiftUI

/// Home: a featured title, then every server hub as a row, in the order and
/// with the visibility the user chose (stored per account and server).
struct HomePage: View {
    @Environment(AppModel.self) private var app

    var body: some View {
        Loading(id: [AnyHashable(app.revision), AnyHashable(app.scope)], { try await API.homeHubs() }) { hubs in
            HomeContent(hubs: hubs.filter { !$0.metadata.isEmpty || !$0.directories.isEmpty })
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .navigationTitle("Home")
    }
}

/// Stable identity of a hub for the saved layout.
private func hubKey(_ h: Hub) -> String { h.hubIdentifier ?? h.key ?? h.title ?? "" }

/// Continue Watching / On Deck style hubs: backdrop cards, or a compact list.
private func isContinue(_ h: Hub) -> Bool {
    let id = (h.hubIdentifier ?? "").lowercased()
    return ["continue", "ondeck", "inprogress"].contains { id.contains($0) }
}

private struct HomeContent: View {
    let hubs: [Hub]

    @Environment(AppModel.self) private var app
    @State private var customizing = false

    private var layout: HomeLayout { app.settings?.ui.home_layouts[app.scope] ?? HomeLayout() }

    /// Every hub in the saved order; unknown hubs keep the server's order at the end.
    private var allRows: [Hub] {
        let order = layout.order
        return hubs.enumerated().sorted { a, b in
            let ai = order.firstIndex(of: hubKey(a.element)) ?? 100_000 + a.offset
            let bi = order.firstIndex(of: hubKey(b.element)) ?? 100_000 + b.offset
            return ai < bi
        }.map(\.element)
    }

    private var rows: [Hub] { allRows.filter { !layout.isHidden(hubKey($0)) } }

    var body: some View {
        let rows = rows
        Group {
            if rows.isEmpty {
                EmptyState(
                    title: "Nothing here yet",
                    message: hubs.isEmpty
                        ? "Your libraries are empty or still scanning."
                        : "Your Home rows are hidden. Use Customize Home to show them."
                )
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 34) {
                        FeaturedBanner(picks: Self.featured(rows))
                        ForEach(Array(rows.enumerated()), id: \.offset) { _, hub in
                            if isContinue(hub) && layout.compact_continue {
                                CompactContinue(hub: hub)
                            } else {
                                HubRow(hub: hub, useArt: isContinue(hub))
                            }
                        }
                    }
                    .padding(.top, 24)
                    .padding(.bottom, 40)
                }
            }
        }
        .toolbar {
            ToolbarItem {
                Button { customizing = true } label: {
                    Label("Customize Home", systemImage: "slider.horizontal.3")
                }
                .help("Customize Home")
            }
        }
        .sheet(isPresented: $customizing) {
            CustomizeHome(rows: allRows, layout: layout, save: save)
        }
    }

    private func save(_ l: HomeLayout) {
        var all = app.settings?.ui.home_layouts ?? [:]
        all[app.scope] = l
        app.updateUi(["home_layouts": .from(all)]) { $0.home_layouts = all }
    }

    /// Recent or in-progress movies and shows that have backdrop art.
    static func featured(_ hubs: [Hub]) -> [(Metadata, String)] {
        var out: [(Metadata, String)] = []
        for hub in hubs {
            let id = (hub.hubIdentifier ?? "").lowercased()
            let label: String
            if id.contains("continue") || id.contains("ondeck") { label = "Continue Watching" }
            else if id.contains("recent") { label = "Recently Added" }
            else { continue }
            for m in hub.metadata where out.count < 6 {
                guard m.art != nil, m.kind == "movie" || m.kind == "show",
                      !out.contains(where: { $0.0.ratingKey == m.ratingKey }) else { continue }
                out.append((m, label))
            }
        }
        return out
    }
}

// MARK: Featured

/// The featured title: artwork on the right, the copy beside it, stepping
/// through recent and in-progress titles.
private struct FeaturedBanner: View {
    let picks: [(Metadata, String)]

    @Environment(AppModel.self) private var app
    @State private var index = 0

    var body: some View {
        if !picks.isEmpty {
            let (item, label) = picks[index % picks.count]
            VStack(alignment: .leading, spacing: 0) {
                HStack(alignment: .center, spacing: 36) {
                    copy(item, label)
                        .frame(minWidth: 260, maxWidth: 380, alignment: .leading)
                    Color.clear
                        .aspectRatio(16 / 9, contentMode: .fit)
                        .overlay { Artwork(item.art, width: 1280, height: 720, fallback: nil) }
                        .clipShape(RoundedRectangle(cornerRadius: Theme.radius))
                        .overlay(RoundedRectangle(cornerRadius: Theme.radius).strokeBorder(Theme.hairline))
                        .frame(maxWidth: 820, maxHeight: 440)
                        .contentShape(Rectangle())
                        .onTapGesture { app.open(item) }
                        .contextMenu { ItemMenu(item: item) }
                        .id(item.ratingKey)
                        .transition(.opacity)
                }
                .padding(.horizontal, Theme.gutter)
                .padding(.bottom, 30)
                Hairline().padding(.horizontal, Theme.gutter)
            }
            .animation(.easeOut(duration: 0.25), value: index)
        }
    }

    private func copy(_ item: Metadata, _ label: String) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Eyebrow(label)
            Text(item.name)
                .font(Theme.display(40))
                .lineLimit(3)
                .minimumScaleFactor(0.7)
                .padding(.top, 10)
            let meta = Self.meta(item)
            if !meta.isEmpty {
                Text(meta).font(.callout).foregroundStyle(.secondary).padding(.top, 10)
            }
            if let summary = item.summary, !summary.isEmpty {
                Text(summary)
                    .font(.system(size: 13))
                    .foregroundStyle(.secondary)
                    .lineSpacing(3)
                    .lineLimit(4)
                    .padding(.top, 12)
            }
            HStack(spacing: 8) {
                Button { Actions.play(item) } label: {
                    Label(item.progress != nil ? "Resume" : "Play", systemImage: "play.fill")
                        .padding(.horizontal, 6)
                }
                .buttonStyle(.borderedProminent)
                Button("Details") { app.open(item) }
            }
            .controlSize(.large)
            .padding(.top, 20)
            if picks.count > 1 { pager.padding(.top, 22) }
        }
    }

    private var pager: some View {
        HStack(spacing: 2) {
            Button { index = (index + picks.count - 1) % picks.count } label: {
                Image(systemName: "chevron.left").frame(width: 22, height: 22).contentShape(Rectangle())
            }
            .help("Previous")
            Text(String(format: "%02d / %02d", index % picks.count + 1, picks.count))
                .font(.system(size: 11).monospacedDigit())
                .foregroundStyle(.secondary)
                .frame(minWidth: 52)
            Button { index = (index + 1) % picks.count } label: {
                Image(systemName: "chevron.right").frame(width: 22, height: 22).contentShape(Rectangle())
            }
            .help("Next")
        }
        .buttonStyle(.borderless)
        .foregroundStyle(.secondary)
    }

    static func meta(_ m: Metadata) -> String {
        var bits: [String] = []
        if let y = m.year { bits.append(String(y)) }
        if let r = m.contentRating { bits.append(r) }
        if m.kind == "show" {
            if let c = m.childCount { bits.append(Fmt.plural(c, "season")) }
        } else if let d = m.duration {
            bits.append(Fmt.duration(d))
        }
        let genres = m.genres.prefix(2).compactMap(\.tag).joined(separator: ", ")
        if !genres.isEmpty { bits.append(genres) }
        return bits.joined(separator: "  ·  ")
    }
}

// MARK: Compact Continue Watching

/// Continue Watching as a plain list: one line per title, click to resume.
private struct CompactContinue: View {
    let hub: Hub

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(hub.title ?? "").font(Theme.sectionTitle)
            VStack(spacing: 0) {
                ForEach(Array(hub.metadata.enumerated()), id: \.offset) { i, item in
                    if i > 0 { Hairline() }
                    CompactRow(item: item)
                }
            }
        }
        .padding(.horizontal, Theme.gutter)
    }
}

private struct CompactRow: View {
    let item: Metadata
    @Environment(AppModel.self) private var app
    @State private var hovering = false

    var body: some View {
        HStack(spacing: 16) {
            Group {
                if let show = item.grandparentTitle {
                    Text(show).foregroundStyle(.primary) + Text("  \(item.episodeCode) · \(item.name)").foregroundStyle(.secondary)
                } else {
                    Text(item.name)
                }
            }
            .lineLimit(1)
            Spacer(minLength: 12)
            if let left = item.remaining {
                Text(left).font(Theme.caption).foregroundStyle(.tertiary).monospacedDigit()
            }
            if let p = item.progress {
                ProgressView(value: p).progressViewStyle(.linear).tint(app.accent).frame(width: 60).controlSize(.mini)
            }
            Button("Resume") { Actions.play(item) }
                .buttonStyle(.borderless)
                .foregroundStyle(hovering ? .primary : .secondary)
        }
        .font(.system(size: 13))
        .padding(.vertical, 8)
        .padding(.horizontal, 6)
        .background(RoundedRectangle(cornerRadius: 4).fill(hovering ? Theme.placeholder : .clear))
        .contentShape(Rectangle())
        .onHover { hovering = $0 }
        .onTapGesture(count: 2) { Actions.play(item) }
        .onTapGesture { app.open(item) }
        .contextMenu { ItemMenu(item: item) }
        .help(item.fullTitle)
    }
}

// MARK: Customize

/// Reorder and hide Home rows, and switch Continue Watching to a list.
private struct CustomizeHome: View {
    let rows: [Hub]
    let layout: HomeLayout
    let save: (HomeLayout) -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var order: [String] = []
    @State private var hidden: Set<String> = []
    @State private var compact = false

    private var titles: [String: String] {
        Dictionary(rows.map { (hubKey($0), $0.title ?? hubKey($0)) }, uniquingKeysWith: { a, _ in a })
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Customize Home").font(Theme.display(20))
                Text("Drag to reorder. Uncheck a row to hide it.").font(.callout).foregroundStyle(.secondary)
            }
            .padding([.horizontal, .top], 20)
            .padding(.bottom, 14)

            List {
                ForEach(order, id: \.self) { key in
                    Toggle(isOn: Binding(
                        get: { !hidden.contains(key) },
                        set: { on in if on { hidden.remove(key) } else { hidden.insert(key) }; commit() }
                    )) {
                        Text(titles[key] ?? key)
                    }
                }
                .onMove { from, to in
                    order.move(fromOffsets: from, toOffset: to)
                    commit()
                }
            }
            .listStyle(.bordered(alternatesRowBackgrounds: true))
            .frame(minHeight: 220)
            .padding(.horizontal, 20)

            Toggle("Show Continue Watching as a compact list", isOn: Binding(
                get: { compact },
                set: { compact = $0; commit() }
            ))
                .padding(.horizontal, 20)
                .padding(.top, 14)

            HStack {
                Button("Reset Layout") {
                    order = rows.map(hubKey)
                    hidden = HomeLayout.hiddenByDefault
                    compact = false
                    save(HomeLayout())
                }
                Spacer()
                Button("Done") { dismiss() }
                    .keyboardShortcut(.defaultAction)
            }
            .padding(20)
        }
        .frame(width: 440, height: 520)
        .onAppear {
            order = rows.map(hubKey)
            hidden = Set(order.filter(layout.isHidden))
            compact = layout.compact_continue
        }
    }

    private func commit() {
        // Keep the choices for hubs that aren't showing right now.
        let hiddenExtra = layout.hidden.filter { !order.contains($0) }
        let shownExtra = layout.shown.filter { !order.contains($0) }
        let shown = order.filter { HomeLayout.hiddenByDefault.contains($0) && !hidden.contains($0) }
        save(HomeLayout(
            order: order,
            hidden: order.filter { hidden.contains($0) && !HomeLayout.hiddenByDefault.contains($0) } + hiddenExtra,
            shown: shown + shownExtra,
            compact_continue: compact
        ))
    }
}
