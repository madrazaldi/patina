// Port of desktop/plex/server/admin.ts.
// Preferences, accounts, activities, butler, updates, statistics and logs
// (port of crates/plex/src/server/admin.rs).

import Foundation

extension PlexServer {
    // ----- Preferences -----------------------------------------------------

    public func prefs() async throws -> [Setting] {
        try await get("/:/prefs").setting
    }

    public func setPref(_ id: String, value: String) async throws {
        try await put("/:/prefs", Params().set(id, value))
    }

    public func setPrefs(_ params: Params) async throws {
        try await put("/:/prefs", params)
    }

    // ----- Accounts, devices, clients --------------------------------------

    public func accounts() async throws -> [Account] {
        try await get("/accounts").account
    }

    public func devices() async throws -> [Device] {
        try await get("/devices").device
    }

    /// Players discovered on the local network (GDM).
    public func clients() async throws -> [ServerDevice] {
        try await get("/clients").server
    }

    public func servers() async throws -> [ServerDevice] {
        try await get("/servers").server
    }

    /// Media providers exposed by the server (library, live TV, ...), raw.
    public func mediaProviders() async throws -> JSONValue {
        try await raw("GET", "/media/providers")
    }

    // ----- Activities ------------------------------------------------------

    public func activities() async throws -> [Activity] {
        try await get("/activities").activity
    }

    public func cancelActivity(_ uuid: String) async throws {
        try await delete("/activities/\(uuid)")
    }

    // ----- Butler (scheduled maintenance) ----------------------------------

    public func butlerTasks() async throws -> [ButlerTask] {
        let v = try await raw("GET", "/butler")
        // `from_value(list).unwrap_or_default()`: anything that does not decode gives an empty list.
        guard case .array(let list)? = De.pointer(v, "/ButlerTasks/ButlerTask") else { return [] }
        return (try? list.map(ButlerTask.init(json:))) ?? []
    }

    public func startAllButlerTasks() async throws {
        try await post("/butler")
    }

    public func stopAllButlerTasks() async throws {
        try await delete("/butler")
    }

    public func startButlerTask(_ name: String) async throws {
        try await post("/butler/\(name)")
    }

    public func stopButlerTask(_ name: String) async throws {
        try await delete("/butler/\(name)")
    }

    // ----- Updates -----------------------------------------------------------

    public func updateStatus() async throws -> MediaContainer {
        try await get("/updater/status")
    }

    public func checkForUpdates(_ download: Bool) async throws {
        try await put("/updater/check", Params().flag("download", download))
    }

    /// Install a downloaded update now, or `tonight` during maintenance.
    public func applyUpdate(_ tonight: Bool, skip: Bool) async throws {
        try await put("/updater/apply", Params().flag("tonight", tonight).flag("skip", skip))
    }

    // ----- Statistics --------------------------------------------------------

    /// Plays per account/device, bucketed by the server-defined `timespan` code.
    public func statisticsMedia(_ timespan: Int) async throws -> MediaContainer {
        try await get("/statistics/media", Params().set("timespan", timespan))
    }

    /// CPU and memory utilisation samples.
    public func statisticsResources(_ timespan: Int) async throws -> [StatisticsResources] {
        try await get("/statistics/resources", Params().set("timespan", timespan)).statisticsResources
    }

    public func statisticsBandwidth(_ timespan: Int) async throws -> MediaContainer {
        try await get("/statistics/bandwidth", Params().set("timespan", timespan))
    }

    // ----- Logging -----------------------------------------------------------

    /// Write a line into the server log. `level`: 0 error .. 4 verbose.
    public func log(_ level: Int, message: String, source: String) async throws {
        try await empty("GET", "/log", Params().set("level", level).set("message", message).set("source", source))
    }

    /// Diagnostics archive with the server logs (zip bytes).
    public func downloadLogs() async throws -> Data {
        try await bytes("/diagnostics/logs").0
    }

    /// Download the server database backup (zip bytes).
    public func downloadDatabase() async throws -> Data {
        try await bytes("/diagnostics/databases").0
    }
}
