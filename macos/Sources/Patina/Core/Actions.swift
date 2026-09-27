import Foundation

// Item actions shared by cards, menus and detail pages (web/src/lib/actions.svelte.ts).

@MainActor
enum Actions {
    static var app: AppModel { AppModel.shared }

    /// Hand an item (or container) to mpv.
    static func play(_ item: Metadata, _ options: PlayOptions = PlayOptions()) {
        app.perform { try await API.play(item, options) }
    }

    static func setWatched(_ item: Metadata, _ watched: Bool) {
        app.perform(watched ? "Marked as watched" : "Marked as unwatched", refresh: true) {
            try await API.setWatched(item.ratingKey ?? "", watched)
        }
    }

    /// Queue an item (or everything in a container) next, or at the end. Starts playback if idle.
    static func enqueue(_ item: Metadata, next: Bool) {
        app.perform { try await API.queue(.add(item, next: next)) }
    }

    /// Download an item, or every episode/track below it, for offline playback.
    static func download(_ item: Metadata) {
        Task {
            do {
                let n = try await API.download(item.ratingKey ?? "")
                app.toast(n == 0 ? "Already downloaded" : n == 1 ? "Downloading…" : "Downloading \(n) items…")
            } catch {
                app.error(error)
            }
        }
    }

    /// The download of an item on the current server, if any.
    static func downloaded(_ item: Metadata) -> Download? {
        app.downloads.first {
            $0.serverId == app.session?.serverId
                && ($0.account.isEmpty || $0.account == app.session?.accountId)
                && $0.item.ratingKey == item.ratingKey
        }
    }

    /// Ask, then delete an item's files (or a collection).
    static func delete(_ item: Metadata, then: (() -> Void)? = nil) {
        let rk = item.ratingKey ?? ""
        let collection = item.kind == "collection"
        app.confirm(
            "Delete “\(item.name)”?",
            message: collection
                ? "The collection is removed; its items stay in the library."
                : "This deletes the media files from disk. It can't be undone.",
            action: "Delete", destructive: true
        ) {
            let ok = await app.act("Deleted", refresh: true) {
                if collection { try await API.deleteCollection(rk) } else { try await API.deleteItem(rk) }
            }
            if ok { then?() }
        }
    }

    static func openInPlexWeb(_ item: Metadata) {
        guard let machine = app.session?.machineIdentifier, let rk = item.ratingKey else { return }
        let key = "/library/metadata/\(rk)".addingPercentEncoding(withAllowedCharacters: .alphanumerics) ?? ""
        app.perform { try await API.openUrl("https://app.plex.tv/desktop#!/server/\(machine)/details?key=\(key)") }
    }
}
