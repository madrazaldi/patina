// Port of desktop/plex/connect.ts.
// Picking the best connection of a server resource (port of crates/plex/src/connect.rs).

import Foundation

/// A candidate URI for reaching a server, ranked by preference.
public struct ConnectionCandidate: Sendable, Hashable {
    public var uri: String
    public var local: Bool
    public var relay: Bool
    /// Lower is better.
    public var rank: Int

    public init(uri: String, local: Bool, relay: Bool, rank: Int) {
        self.uri = uri
        self.local = local
        self.relay = relay
        self.rank = rank
    }

    public static func candidates(_ resource: Resource) -> [ConnectionCandidate] {
        var out: [ConnectionCandidate] = []
        func push(_ uri: String, _ local: Bool, _ relay: Bool, _ rank: Int) {
            if !out.contains(where: { $0.uri == uri }) { out.append(ConnectionCandidate(uri: uri, local: local, relay: relay, rank: rank)) }
        }
        for c in resource.connections {
            guard let uri = c.uri else { continue }
            let https = uri.hasPrefix("https")
            let rank = c.relay ? 5 : c.local ? (https ? 0 : 1) : (https ? 2 : 3)
            push(uri, c.local, c.relay, rank)
            // plex.direct names need working DNS rebinding; offer the raw IP too.
            if c.local && !c.relay && !resource.httpsRequired, let address = c.address, let port = c.port {
                let host = c.ipv6 ? "[\(address)]" : address
                push("http://\(host):\(port)", true, false, 4)
            }
        }
        // A stable sort, like Array.prototype.sort.
        return out.enumerated().sorted { ($0.element.rank, $0.offset) < ($1.element.rank, $1.offset) }.map(\.element)
    }
}

private enum Attempt: Sendable {
    case connected(rank: Int, server: PlexServer)
    case failed(rank: Int, error: Error)
    case graceOver
}

/// Race every connection of a server resource and return a client bound to
/// the best one that answers. Local direct connections win over remote, and
/// relays are the last resort.
public func connectResource(_ tv: PlexTv, _ resource: Resource) async throws -> PlexServer {
    guard let token = resource.accessToken ?? tv.token else { throw PlexError.unauthorized() }
    let candidates = ConnectionCandidate.candidates(resource)
    guard let bestRank = candidates.first?.rank else { throw PlexError.noConnection(Resource.name(resource)) }

    let http = tv.http
    var best: (rank: Int, server: PlexServer)?
    var lastErr: Error?
    await withTaskGroup(of: Attempt.self) { group in
        for c in candidates {
            group.addTask {
                do {
                    let server = try PlexServer.withHttp(http, c.uri, token)
                    let identity = try await withDeadline(6) { try await server.identity() }
                    return .connected(rank: c.rank, server: server.withMachineIdentifier(identity.machineIdentifier))
                } catch is TimedOut {
                    return .failed(rank: c.rank, error: PlexError.other("timed out"))
                } catch {
                    return .failed(rank: c.rank, error: error)
                }
            }
        }
        var pending = candidates.count
        var graceStarted = false
        loop: while pending > 0, let attempt = await group.next() {
            switch attempt {
            case .graceOver:
                break loop
            case .connected(let rank, let server):
                pending -= 1
                if !graceStarted {
                    // Once something works, give better-ranked candidates a short grace period.
                    graceStarted = true
                    group.addTask {
                        try? await Task.sleep(for: .milliseconds(1200))
                        return .graceOver
                    }
                }
                if best == nil || rank < best!.rank { best = (rank, server) }
                if rank == bestRank { break loop }
            case .failed(_, let error):
                pending -= 1
                Log.debug("connection attempt failed: \(error.localizedDescription)")
                lastErr = error
            }
        }
        // Dropping the pending futures cancels them in Rust; here the leftovers are cancelled.
        group.cancelAll()
    }
    if let best { return best.server }
    if let e = lastErr as? PlexError, e.kind == .unauthorized { throw PlexError.unauthorized() }
    throw PlexError.noConnection(Resource.name(resource))
}
