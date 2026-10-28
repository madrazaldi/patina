// Port of desktop/core/servers.ts.
// The other servers of the signed-in account: connecting to them on demand and keeping those
// connections, so items found on a second server keep being browsed, played and reported
// against that server (never the current one).

import Foundation
import PlexKit

public enum Servers {
    /// How long a connection to another server is reused before it is raced again.
    static let REUSE: TimeInterval = 15 * 60

    /// The connections made so far, per account and machine id. Process-wide in TypeScript;
    /// per app here, which is the same in the app and keeps tests apart.
    final class Connections {
        var map: [String: (at: Date, server: PlexServer)] = [:]
    }

    private static func connections(_ app: AppState) -> Connections {
        app.local(Connections.self) { Connections() }
    }

    // (account, machine id) pairs as one key.
    private static func keyOf(_ account: String, _ id: String) -> String {
        "\(account)\u{0}\(id)"
    }

    /// Whether `id` names the session's own server (or nothing at all).
    public static func isCurrent(_ app: AppState, _ id: String?) -> Bool {
        guard let id, !id.isEmpty else { return true }
        guard let s = app.session() else { return false }
        return id == s.serverId || s.server.machineIdentifier == id
    }

    /// The server with machine identifier `id`: the session's own, or another server of the
    /// account, connected to on first use. No id means the current server.
    public static func serverFor(_ app: AppState, _ id: String?) async throws -> PlexServer {
        if isCurrent(app, id) { return try app.server() }
        let machine = id ?? ""
        guard let session = app.session() else { throw CoreError("Not connected to a server.") }
        let key = keyOf(Offline.account(session), machine)
        if let known = connections(app).map[key], Date().timeIntervalSince(known.at) < REUSE { return known.server }
        guard let tv = session.tv else { throw CoreError("Other servers need a plex.tv sign-in.") }
        guard let resource = session.servers.first(where: { Resource.id($0) == machine }) else {
            throw CoreError("That server is no longer on this account.")
        }
        let server: PlexServer
        do {
            server = try await connectResource(tv, resource)
        } catch {
            throw context(error, "connecting to \(Resource.name(resource))")
        }
        connections(app).map[key] = (Date(), server)
        return server
    }

    /// The server an item lives on: its own tag, else the current one.
    public static func serverOf(_ app: AppState, _ item: Metadata) async throws -> PlexServer {
        try await serverFor(app, Metadata.serverId(item))
    }

    /// Forget a connection that stopped working so the next use connects again.
    public static func forget(_ app: AppState, _ id: String) {
        if let session = app.session() { connections(app).map[keyOf(Offline.account(session), id)] = nil }
    }

    /// Tag items with the server they came from, leaving the current server's items alone.
    public static func tag(_ app: AppState, _ items: inout [Metadata], _ server: String?) {
        let s = server.flatMap { !$0.isEmpty && !isCurrent(app, $0) ? $0 : nil }
        for i in items.indices { tagOne(&items[i], s) }
    }

    /// Tag an item and the items of its related hubs (shown on its page).
    private static func tagOne(_ item: inout Metadata, _ server: String?) {
        Metadata.setServerId(&item, server)
        guard var related = item.related else { return }
        for h in related.hub.indices {
            for m in related.hub[h].metadata.indices { Metadata.setServerId(&related.hub[h].metadata[m], server) }
        }
        item.related = related
    }

    /// A server's display name: the account's resource name, or the session's own.
    public static func nameOf(_ app: AppState, _ id: String?) -> String {
        guard let session = app.session() else { return "" }
        if isCurrent(app, id) { return session.serverName }
        return session.servers.first { Resource.id($0) == id }.map(Resource.name) ?? ""
    }

    /// Artwork of an item on another server, resized by that server.
    public static func image(_ app: AppState, _ id: String, _ source: String, _ w: Int, _ h: Int) async -> (
        Data, String
    )? {
        guard !source.isEmpty else { return nil }
        do {
            let server = try await serverFor(app, id)
            let (bytes, ctype) = try await server.image(source, width: w, height: h)
            if bytes.isEmpty { return nil }
            return (bytes, ctype.flatMap { $0.hasPrefix("image/") ? $0 : nil } ?? "image/jpeg")
        } catch {
            return nil
        }
    }
}
