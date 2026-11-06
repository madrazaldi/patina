// Port of web/src/pages/Watchlist.svelte: the plex.tv watchlist. Each entry links to the
// server's own copy when there is one; titles the server doesn't have are dimmed.

import PatinaCore
import PlexKit
import SwiftUI

struct WatchlistScreen: View {
    /// How many "is it on this server?" lookups run at once.
    static let lookups = 5

    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var list = Loader<[Metadata]>()
    /// guid → what the server said: a rating key, `.some(nil)` when it doesn't have the title,
    /// missing while unknown. A failed lookup is not an answer: it stays unknown (so the entry
    /// isn't dimmed) and can be retried.
    @State private var local: [String: String?] = [:]
    /// guid → the revision its answer was obtained at; an older answer is looked up again.
    @State private var answered: [String: Int] = [:]
    @State private var failed: Set<String> = []

    private var guids: [String] {
        var seen = Set<String>()
        return (list.value ?? []).compactMap(\.guid).filter { seen.insert($0).inserted }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("Synced with your Plex account. Titles this server doesn't have are dimmed.")
                    .font(.subheadline).foregroundStyle(Theme.secondaryText)
                if let error = list.error, list.value != nil { StaleBanner(error: error) { list.reload() } }
                let failures = (list.value ?? []).filter { $0.guid.map(failed.contains) ?? false }.count
                if failures > 0 {
                    StaleBanner(
                        error: "Couldn't check \(Format.plural(failures, "title")) against this server, so \(failures == 1 ? "it isn't" : "they aren't") dimmed."
                    ) { Task { await check() } }
                }
                if let items = list.value {
                    if items.isEmpty {
                        StatusView(
                            title: "Your watchlist is empty", text: "Add movies and shows from their detail page.",
                            systemImage: "bookmark")
                    } else {
                        LazyVGrid(columns: cardColumns(.poster, sizeClass), spacing: 22) {
                            ForEach(items, id: \.ratingKey) { m in entry(m) }
                        }
                    }
                } else if let error = list.error {
                    StatusView(error: error) { list.reload() }
                } else {
                    LazyVGrid(columns: cardColumns(.poster, sizeClass), spacing: 22) {
                        ForEach(0..<8, id: \.self) { _ in SkeletonCard(shape: .poster) }
                    }
                }
            }
            .padding(.horizontal, Theme.margin(sizeClass))
            .padding(.bottom, 24)
        }
        .navigationTitle("Watchlist")
        .refreshable { await list.refresh() }
        .task(id: store.revision) { await list.load(at: store.revision) { try await Commands.watchlist(store.app) } }
        // Library changes (a scan finishing, a title added or removed) and reconnecting both
        // make old answers unreliable.
        .task(id: "\(guids.count):\(store.revision):\(store.connected):\(store.scope)") { await check() }
    }

    /// Look up every entry whose answer is missing or from before the latest library change,
    /// a few at a time.
    private func check() async {
        let revision = store.revision
        let wanted = guids.filter { answered[$0] != revision }
        let app = store.app!
        let scope = store.scope
        for start in stride(from: 0, to: wanted.count, by: Self.lookups) {
            let batch = wanted[start..<min(start + Self.lookups, wanted.count)]
            // The lookups of a batch run together.
            let tasks = batch.map { guid in
                Task { () -> (String, String?, Bool) in
                    do {
                        return (guid, try await Commands.findByGuid(app, guid: guid), true)
                    } catch {
                        return (guid, nil, false)
                    }
                }
            }
            var answers: [(String, String?, Bool)] = []
            for t in tasks { answers.append(await t.value) }
            guard scope == store.scope, !Task.isCancelled else { return }
            for (guid, answer, ok) in answers {
                if ok {
                    local[guid] = .some(answer)
                    answered[guid] = revision
                    failed.remove(guid)
                } else {
                    answered[guid] = nil
                    failed.insert(guid)
                }
            }
        }
    }

    private func entry(_ m: Metadata) -> some View {
        let answer = m.guid.flatMap { local[$0] }
        let missing = answer != nil && answer! == nil
        return Button {
            open(m)
        } label: {
            VStack(alignment: .leading, spacing: 4) {
                Artwork(m.thumb, width: 300, height: 450, fallback: Metadata.title(m))
                    .aspectRatio(2 / 3, contentMode: .fit)
                    .clipShape(.rect(cornerRadius: Theme.cornerRadius, style: .continuous))
                    .overlay(
                        RoundedRectangle(cornerRadius: Theme.cornerRadius, style: .continuous).strokeBorder(Theme.hairline)
                    )
                    .opacity(missing ? 0.4 : 1)
                    .overlay(alignment: .topTrailing) {
                        if case .some(.some) = answer {
                            Image(systemName: "checkmark")
                                .font(.caption2.weight(.bold)).foregroundStyle(.black)
                                .padding(5).background(store.accent, in: .circle).padding(6)
                                .accessibilityLabel("On this server")
                        }
                    }
                Text(Metadata.title(m)).font(.subheadline.weight(.medium)).lineLimit(1)
                Text(missing ? "Not on this server" : PlexUI.subtitle(m))
                    .font(.caption).foregroundStyle(Theme.secondaryText).lineLimit(1)
            }
        }
        .buttonStyle(CardButtonStyle())
        .contextMenu {
            if case .some(.some(let rk)) = answer {
                Button("Open", systemImage: "arrow.up.right") { store.navigate(to: .item(ratingKey: rk)) }
            }
            Button("Remove from Watchlist", systemImage: "bookmark.slash", role: .destructive) { remove(m) }
        }
    }

    private func open(_ m: Metadata) {
        guard let guid = m.guid, let answer = local[guid] else {
            store.toast(m.guid == nil ? "Not available on this server" : "Still checking this server")
            return
        }
        if let rk = answer { store.navigate(to: .item(ratingKey: rk)) } else { store.toast("Not available on this server") }
    }

    private func remove(_ m: Metadata) {
        store.perform("Removed from watchlist", refresh: true) {
            try await Commands.setWatchlisted(store.app, discoverId: m.ratingKey ?? "", on: false)
        }
    }
}
