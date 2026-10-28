// Port of desktop/core/controls.test.ts. The tracker follows the engine's events instead of
// mpv's properties.

import Foundation
import PlexKit
import Testing

@testable import PatinaCore

@Suite struct ControlsTests {
    @Test func recognizesLocalAddresses() {
        for host in [
            "localhost", "127.0.0.1", "192.168.1.20", "10.0.0.2", "172.16.5.5", "nas.local", "nas", "[::1]",
            "192-168-1-20.abc123.plex.direct", "fd12::1",
        ] {
            #expect(Controls.isLocalHost(host), "\(host)")
        }
        for host in ["8.8.8.8", "plex.example.com", "203-0-113-9.abc123.plex.direct", "2001:db8::1"] {
            #expect(!Controls.isLocalHost(host), "\(host)")
        }
    }

    @Test func repeatCyclesThroughEveryMode() {
        #expect(Controls.nextRepeat(.off) == .all)
        #expect(Controls.nextRepeat(.all) == .one)
        #expect(Controls.nextRepeat(.one) == .off)
    }

    @Test func followsTheEngineAndFlagsUrgentChanges() {
        let app = TestEnv().app()
        defer { app.dispose() }
        let t = Controls.Tracker(Controls.ConnectionInfo(kind: "local", address: nil))
        t.volume = 80
        t.muted = true
        #expect(t.onDiagnostics(EngineDiagnostics(buffering: true, bufferedSeconds: 12.34, decoder: "VideoToolbox", droppedFrames: 3)))
        // Only the buffer level and frame count moved: that can wait.
        #expect(!t.onDiagnostics(EngineDiagnostics(buffering: true, bufferedSeconds: 8, decoder: "VideoToolbox", droppedFrames: 4)))
        #expect(t.onDiagnostics(EngineDiagnostics(buffering: false, bufferedSeconds: 12.34, decoder: "VideoToolbox", droppedFrames: 3)))
        let v = t.view(app)
        #expect(v.volume == 80 && v.muted)
        #expect(v.diagnostics.bufferedSecs == 12.3)
        #expect(v.diagnostics.droppedFrames == 3)
        #expect(v.diagnostics.hwdec == "VideoToolbox")
        #expect(!v.diagnostics.buffering)
        t.newItem([])
        #expect(t.view(app).diagnostics.droppedFrames == 0)
        #expect(t.view(app).diagnostics.connection?.kind == "local")
    }

    @Test func chaptersComeFromTheFileOrElsePlex() {
        let app = TestEnv().app()
        defer { app.dispose() }
        let t = Controls.Tracker(Controls.offlineConnection())
        t.newItem([Chapter(tag: "Opening", startTimeOffset: 0), Chapter(tag: nil, startTimeOffset: 60_000)])
        t.onTime(70_000)
        var v = t.view(app)
        #expect(v.chapters.map(\.title) == ["Opening", "Chapter 2"])
        #expect(v.chapter == 1)
        t.onChapters([
            EngineChapter(title: "A", startSeconds: 0), EngineChapter(title: "", startSeconds: 30.5),
            EngineChapter(title: "C", startSeconds: 90),
        ])
        t.onTime(95_000)
        v = t.view(app)
        #expect(v.chapters.count == 3)
        #expect(v.chapters[1] == Controls.ChapterView(title: "Chapter 2", timeMs: 30_500))
        #expect(v.chapter == 2)
    }

    @Test func connectionKindComesFromTheAccountsConnections() throws {
        let server = try PlexServer(info: testInfo, baseUrl: "https://1-2-3-4.abc.plex.direct:32400", token: "t")
        var r = Resource(name: "Home", clientIdentifier: "home", provides: "server", owned: true, presence: true)
        r.connections = [Connection(protocol: "https", address: "1.2.3.4", port: 32400, uri: "https://1-2-3-4.abc.plex.direct:32400", local: false, relay: true)]
        let session = Session(tv: nil, server: server, serverName: "Home", serverId: "home", user: nil, owned: true, servers: [r])
        #expect(Controls.connectionInfo(session) == Controls.ConnectionInfo(kind: "relay", address: "1-2-3-4.abc.plex.direct:32400"))
        let bare = Session(tv: nil, server: server, serverName: "Home", serverId: "home", user: nil, owned: true, servers: [])
        #expect(Controls.connectionInfo(bare).kind == "remote")
    }
}
