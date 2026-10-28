// Helpers shared by the PatinaCore tests (no TypeScript twin: the TS tests point
// XDG_CONFIG_HOME and PATINA_DATA_DIR at temporary directories instead).

import Foundation
import PlexKit
import Testing

@testable import PatinaCore

/// A temporary directory for one test's files, removed when the value goes away.
final class TempDir {
    let url: URL

    init(_ prefix: String = "patina") {
        url = FileManager.default.temporaryDirectory.appendingPathComponent("\(prefix)-\(UUID().uuidString)")
        try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    }

    deinit {
        try? FileManager.default.removeItem(at: url)
    }

    func file(_ name: String) -> URL {
        url.appendingPathComponent(name)
    }
}

/// An isolated environment: temporary directories, in-memory secrets, Plex traffic routed to
/// `MockPMS`, and no background poller unless asked for.
final class TestEnv {
    let dir = TempDir("patina-core")
    let secrets = MemorySecretStore()
    let environment: AppEnvironment

    init(tick: Duration? = nil) {
        environment = AppEnvironment(
            paths: .under(dir.url), secrets: secrets, platform: "iOS", platformVersion: "18.0", device: "iPhone",
            deviceName: "Test iPhone", urlSession: MockPMS.session, tickInterval: tick)
    }

    var paths: Paths { environment.paths }

    /// A new app on this environment (another launch of the app, when called twice).
    func app(_ sink: any EventSink = NullSink()) -> AppState {
        AppState(sink: sink, environment: environment)
    }
}

/// Collects every event (`{ emit: (e) => seen.push(e) }`).
final class Recorder: EventSink {
    var events: [CoreEvent] = []

    func emit(_ event: CoreEvent) {
        events.append(event)
    }
}

struct TimedOut: Error, CustomStringConvertible {
    let what: String
    var description: String { "timed out waiting for \(what)" }
}

/// Wait (polling) until `done` holds.
func until(_ what: String, timeout: Duration = .seconds(10), _ done: () -> Bool) async throws {
    let start = ContinuousClock.now
    while !done() {
        if ContinuousClock.now - start > timeout { throw TimedOut(what: what) }
        try await Task.sleep(for: .milliseconds(20))
    }
}

/// A Plex client info for tests that build sessions by hand.
let testInfo = ClientInfo(identifier: "test", product: "Patina", version: "test")
