// Port of web/src/lib/actions.svelte.ts: item actions shared by cards, menus and detail
// pages.

import PatinaCore
import PlexKit
import SwiftUI

extension AppStore {
    /// Hand an item (or a container) to the player.
    public func play(_ item: Metadata, _ options: PendingCommands.PlayOptions = .init()) {
        Haptics.tap()
        perform { try await PendingCommands.play(self.app, item: item, options: options) }
    }

    /// The core announces the change with a toast that offers Undo, and refreshes the views.
    public func setWatched(_ item: Metadata, _ watched: Bool) {
        Haptics.success()
        perform {
            try await Commands.setWatched(
                self.app, server: Metadata.serverId(item), ratingKey: Metadata.ratingKey(item), watched: watched)
        }
    }

    /// Queue an item (or everything in a container) next, or at the end. Starts playback if idle.
    public func enqueue(_ item: Metadata, next: Bool) {
        Haptics.tap()
        perform { try await PendingCommands.queueAdd(self.app, item: item, next: next) }
    }

    /// Download an item, or every episode/track below it, for offline playback.
    public func download(_ item: Metadata) {
        Task {
            do {
                let n = try await PendingCommands.download(app, ratingKey: Metadata.ratingKey(item))
                toast(n == 0 ? "Already downloaded" : n == 1 ? "Downloading…" : "Downloading \(n) items…")
                Haptics.success()
            } catch {
                self.error(error)
            }
        }
    }

    /// The download of an item on the current server, if any.
    public func downloaded(_ item: Metadata) -> Download? {
        downloads.first {
            $0.serverId == session?.serverId && ($0.account.isEmpty || $0.account == session?.accountId)
                && $0.item.ratingKey == item.ratingKey
        }
    }

    /// Delete an item (its files) or a collection, after asking. Returns whether it went.
    @discardableResult
    public func delete(_ item: Metadata) async -> Bool {
        let rk = Metadata.ratingKey(item)
        let scope = self.scope
        let collection = Metadata.kind(item) == "collection"
        let ok = await confirm(
            "Delete “\(Metadata.title(item))”?",
            text: collection
                ? "The collection is removed; its items stay in the library."
                : "This deletes the media files from disk. It can't be undone.",
            action: "Delete", danger: true)
        // The rating key belongs to the server the menu opened on.
        guard ok, scope == self.scope else { return false }
        let done = await act("Deleted", refresh: true) {
            if collection {
                try await PendingCommands.deleteCollection(self.app, id: rk)
            } else {
                try await Commands.deleteItem(self.app, ratingKey: rk)
            }
        }
        if done, case .item(let key, _)? = paths[selection]?.last, key == rk {
            paths[selection]?.removeLast()
        }
        return done
    }

    /// The item's page in Plex Web.
    public func plexWebUrl(_ item: Metadata) -> URL? {
        guard let machine = session?.machineIdentifier else { return nil }
        let key = "/library/metadata/\(Metadata.ratingKey(item))"
        let encoded = key.addingPercentEncoding(withAllowedCharacters: .alphanumerics) ?? key
        return URL(string: "https://app.plex.tv/desktop#!/server/\(machine)/details?key=\(encoded)")
    }
}
