// Port of desktop/plex/server/livetv.ts.
// Live TV & DVR (port of crates/plex/src/server/livetv.rs). These endpoints vary a lot
// between tuner setups and server versions, so they return raw JSON for the caller to pick apart.

import Foundation

private func trimTrailingSlashes(_ s: String) -> String {
    var s = s
    while s.hasSuffix("/") { s.removeLast() }
    return s
}

extension PlexServer {
    /// Configured DVRs, each with its tuner devices, lineup and channel mappings.
    public func dvrs() async throws -> JSONValue {
        try await raw("GET", "/livetv/dvrs")
    }

    /// Channels of a DVR lineup. `epg` is the EPG provider path, e.g. `/tv.plex.providers.epg.cloud:2`.
    public func lineupChannels(_ epg: String, lineup: String) async throws -> JSONValue {
        try await raw("GET", "\(trimTrailingSlashes(epg))/lineups/dvr/channels", Params().set("lineup", lineup))
    }

    /// Guide listings between two unix timestamps.
    public func epgGrid(_ epg: String, begins: Int, ends: Int) async throws -> JSONValue {
        let p = Params().set("type", "1,4").set("beginsAt<", ends).set("endsAt>", begins)
        return try await raw("GET", "\(trimTrailingSlashes(epg))/grid", p)
    }

    /// Start watching a channel. The response describes the new live session.
    public func tune(_ dvrId: String, channel: String) async throws -> JSONValue {
        // `url::form_urlencoded::byte_serialize`
        let path = "/livetv/dvrs/\(dvrId)/channels/\(formEncodeComponent(channel))/tune"
        return try await raw("POST", path, Params().set("autoPreview", 1))
    }

    /// Recording rules, with their upcoming grabs.
    public func subscriptions() async throws -> JSONValue {
        try await raw("GET", "/media/subscriptions", Params().set("includeGrabs", 1))
    }

    /// Upcoming recordings.
    public func scheduledRecordings() async throws -> JSONValue {
        try await raw("GET", "/media/subscriptions/scheduled")
    }

    /// The recording template for a guide item (its default rule parameters).
    public func subscriptionTemplate(_ guid: String) async throws -> JSONValue {
        try await raw("GET", "/media/subscriptions/template", Params().set("guid", guid))
    }

    /// Create a recording rule. `params` usually come from `subscriptionTemplate`.
    public func createSubscription(_ params: Params) async throws -> JSONValue {
        try await raw("POST", "/media/subscriptions", params)
    }

    public func deleteSubscription(_ id: String) async throws {
        try await delete("/media/subscriptions/\(id)")
    }
}
