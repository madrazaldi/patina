// Port of the playback cases of desktop/test/player.test.ts (which drive a real mpv against
// tools/mock_pms.py): here a `FakeEngine` plays the engine's part against `MockPMS`. Also the
// iOS-only parts of the port: the queue moving on by itself, repeat, markers and track choices.
//
// Serialized: the reports of every playback go through the one app-wide `Report` worker, and
// the failing-reports case keeps it retrying for a while, so it runs last.

import Foundation
import PlexKit
import Testing

@testable import PatinaCore

private func session(_ mock: MockPMS) throws -> Session {
    Session(
        tv: nil, server: try PlexServer(info: testInfo, baseUrl: mock.url, token: "alice", session: MockPMS.session),
        serverName: mock.name, serverId: mock.machine, user: nil, owned: true, servers: [])
}

/// `POST /playQueues`, which mock_pms.py leaves out: the items of the URI, with the following
/// episodes of a single episode when `continuous` is asked for.
private func servePlayQueues(_ mock: MockPMS) {
    mock.on { mock, req in
        guard req.method == "POST", req.path == "/playQueues", let uri = req.q("uri") else { return nil }
        mock.record("create_playqueue", req)
        let keys = (uri.split(separator: "/").last ?? "").split(separator: ",").map(String.init)
        var items = mock.state.withLock { s in keys.flatMap { mock.leaves($0, &s) } }
        if req.q("continuous") == "1", items.count == 1, let rk = items[0]["ratingKey"]?.stringValue,
            let season = items[0]["parentRatingKey"]?.stringValue
        {
            let all = mock.state.withLock { mock.children(season, &$0) }
            items = Array(all.drop { $0["ratingKey"]?.stringValue != rk })
        }
        for i in items.indices { items[i]["playQueueItemID"] = JSONValue(5000 + i) }
        let selected =
            req.q("key").flatMap { key in items.first { $0["key"]?.stringValue == key }?["playQueueItemID"] }
            ?? JSONValue(5000)
        return .json(
            mc(["playQueueID": 77, "playQueueSelectedItemID": selected, "Metadata": .array(items.map(JSONValue.object))]))
    }
}

/// An app connected to `mock` with a `FakeEngine` attached.
@MainActor
private final class Rig {
    let env = TestEnv()
    let mock: MockPMS
    let seen = Recorder()
    let engine = FakeEngine()
    let app: AppState

    init(_ mock: MockPMS = MockPMS(), settings: (inout Settings) -> Void = { _ in }) throws {
        self.mock = mock
        servePlayQueues(mock)
        app = env.app(seen)
        app.updateSettings(settings)
        app.setSession(try session(mock), [])
        engine.mock = mock
        Player.attach(app, engine)
    }

    func item(_ rk: String) async throws -> Metadata {
        try #require(try await app.server().metadata(rk))
    }

    /// Wait for load `n` (1-based) and open it.
    func opened(_ n: Int, duration: Double = 180) async throws {
        try await until("load \(n)") { engine.loads.count >= n }
        engine.open(duration: duration)
        try await until("load \(n) to be followed") { app.nowPlaying() != nil && Controls.playbackState(app) != nil }
    }

    func stopped() async throws {
        Player.control(app, .stop)
        try await until("playback to end") { app.nowPlaying() == nil && !Player.isActive(app) }
    }

    func logged(_ what: String, where test: ([String: String]) -> Bool = { _ in true }) -> Bool {
        mock.events().contains { $0["what"] == what && test($0) }
    }

    func done() async {
        await Player.shutdown(app)
        Live.stop(app)
        app.dispose()
        mock.stop()
    }
}

@Suite(.serialized) struct PlayerIntegrationTests {
    @Test func streamsPrefsAndTrackChoicesGoToTheEngineAndToPlex() async throws {
        let rig = try Rig()
        let app = rig.app
        let item = try await rig.item("100")
        Player.play(app, Player.PlayRequest(item: item, resume: false))
        try await rig.opened(1)
        let load = rig.engine.loads[0]
        // Direct play: the file itself, with the token in the headers rather than the URL.
        #expect(load.url.path == "/library/parts/100/1700000000/file.mkv")
        #expect(load.url.query?.contains("X-Plex-Token") != true)
        #expect(load.httpHeaders["X-Plex-Token"] == "alice")
        #expect(load.startSeconds == 0)
        // Plex's selection (Japanese audio, English subtitles), by container stream index.
        #expect(load.audioStreamIndex == 2)
        #expect(load.subtitle == .stream(3))
        #expect(load.nowPlaying.title == item.title)
        #expect(app.nowPlaying()?.transcoding == false)
        #expect(app.snapshot().queue?.items.count == 1)

        // Choosing a track in the player saves it to Plex.
        Player.selectTrack(app, .audio(1))
        Player.selectTrack(app, .subtitle(nil))
        try await until("the choice to reach Plex") {
            rig.logged("select_streams") { $0["audioStreamID"] == "1002" }
                && rig.logged("select_streams") { $0["subtitleStreamID"] == "0" }
        }
        #expect(rig.engine.audio == 1 && rig.engine.subtitle == nil)
        try await rig.stopped()

        // The automatic preferences are applied to the next playback (English audio, no subtitles).
        try Prefs.save(app, nil, nil, Prefs.Rules(audioLanguages: ["jpn"], subtitles: .off))
        let other = try await rig.item("102")
        Player.play(app, Player.PlayRequest(item: other, resume: false))
        try await rig.opened(2)
        #expect(rig.engine.loads[1].audioStreamIndex == 2)
        #expect(rig.engine.loads[1].subtitle == nil)
        try await until("the automatic choice to reach Plex") {
            rig.logged("select_streams") { $0["subtitleStreamID"] == "0" && $0["audioStreamID"] == nil }
        }
        try await rig.stopped()
        await rig.done()
    }

    @Test func pauseQueueEditsAndStoppingDoNotWaitForPlex() async throws {
        let rig = try Rig(MockPMS(reportDelay: 4))
        let app = rig.app
        let first = try await rig.item("100")
        let second = try await rig.item("101")
        Player.play(app, Player.PlayRequest(item: first, resume: false))
        try await rig.opened(1)
        rig.engine.time(2)
        try await until("the position") { (app.nowPlaying()?.positionMs ?? 0) >= 2000 }

        var began = ContinuousClock.now
        Player.control(app, .pause)
        try await until("pause", timeout: .seconds(3)) { app.nowPlaying()?.paused == true }
        #expect(ContinuousClock.now - began < .milliseconds(1500))

        began = .now
        Player.queue(app, .add(item: second, next: false))
        try await until("the queue edit", timeout: .seconds(3)) { app.snapshot().queue?.items.count == 2 }
        #expect(ContinuousClock.now - began < .milliseconds(1500))

        Player.control(app, .play)
        began = .now
        Player.control(app, .next)
        try await rig.opened(2)
        #expect(app.nowPlaying()?.queuePos == 1)
        #expect(ContinuousClock.now - began < .milliseconds(1500))

        began = .now
        try await rig.stopped()
        #expect(ContinuousClock.now - began < .seconds(4))
        #expect(rig.engine.stops >= 1)
        await Report.flush(40_000)
        func delivered(_ key: String) -> Bool {
            rig.logged("timeline") { $0["ratingKey"] == key && $0["state"] == "stopped" }
        }
        #expect(delivered("100") && delivered("101"))
        // Progress is collapsed rather than queued up behind a slow server.
        #expect(rig.mock.logged().filter { $0 == "timeline" }.count <= 8)
        await rig.done()
    }

    @Test func theQueueStaysOnItsServerAfterASwitch() async throws {
        let alphaMock = MockPMS(name: "Alpha", machine: "alpha-queue", seed: 0)
        let betaMock = MockPMS(name: "Beta", machine: "beta-queue", seed: 1)
        defer { betaMock.stop() }
        let rig = try Rig(alphaMock)
        let app = rig.app
        let first = try await rig.item("100")
        var more = try await rig.item("101")
        Player.play(app, Player.PlayRequest(item: first, resume: false))
        try await rig.opened(1)

        // Beta is now the connected server; its "101" has no tag, like Alpha's untagged items did.
        app.setSession(try session(betaMock), [])
        let other = try #require(try await app.server().metadata("101"))
        #expect(Metadata.serverId(other) == nil)
        #expect(other.title != more.title)
        Player.queue(app, .add(item: other, next: false))
        // Alpha's own item, found from Beta, carries Alpha's tag and is still welcome. Commands
        // are handled in order, so once it is queued Beta's has been decided too.
        var tagged = [more]
        Servers.tag(app, &tagged, "alpha-queue")
        more = tagged[0]
        Player.queue(app, .add(item: more, next: false))
        try await until("Alpha's item to be queued") { app.snapshot().queue?.items.last?.item.title == more.title }
        #expect(app.snapshot().queue?.items.count == 2)
        #expect(rig.seen.events.contains { if case .toast(let t) = $0 { t.error && t.text.contains("one server") } else { false } })
        try await rig.stopped()
        await rig.done()
    }

    @Test func aTranscodeGetsTheAutomaticTrackChoiceFirst() async throws {
        let rig = try Rig()
        let app = rig.app
        try Prefs.save(app, nil, nil, Prefs.Rules(audioLanguages: ["eng"], subtitles: .off))
        let item = try await rig.item("101")
        Player.play(app, Player.PlayRequest(item: item, resume: false, quality: "20m-1080"))
        try await rig.opened(1)
        let load = rig.engine.loads[0]
        #expect(load.url.path == "/video/:/transcode/universal/start.m3u8")
        let query = URLComponents(url: load.url, resolvingAgainstBaseURL: false)?.queryItems ?? []
        #expect(query.contains(URLQueryItem(name: "protocol", value: "hls")))
        #expect(query.contains(URLQueryItem(name: "maxVideoBitrate", value: "20000")))
        #expect(query.contains(URLQueryItem(name: "subtitles", value: "burn")))
        // The server picks (and burns in) the tracks of a transcode.
        #expect(load.audioStreamIndex == nil && load.subtitle == nil)
        #expect(app.nowPlaying()?.transcoding == true)
        // The choice was saved before the transcode was asked for.
        #expect(
            rig.mock.events().contains {
                $0["what"] == "select_streams" && $0["audioStreamID"] == "1012" && $0["subtitleStreamID"] == "0"
            })
        #expect(rig.engine.loggedAtLoad[0].contains("select_streams"))
        // Stopping ends the transcode.
        try await rig.stopped()
        await Report.flush(10_000)
        #expect(rig.mock.events().contains { $0["what"]?.hasPrefix("unhandled GET /video/:/transcode/universal/stop") == true })
        await rig.done()
    }

    @Test func aFailureCanBeRetriedAndControlsAndTheSavedQueueWork() async throws {
        let rig = try Rig()
        let app = rig.app
        let item = try await rig.item("100")

        // A file the engine cannot open: the failure arrives with a reason, before anything loaded.
        Player.play(app, Player.PlayRequest(item: item, resume: false))
        try await until("the load") { rig.engine.loads.count == 1 }
        rig.engine.fail("Unsupported codec", .unsupported)
        try await until("the failure") { Failure.current(app) != nil }
        let f = try #require(Failure.current(app))
        #expect(f.phase == .start)
        #expect(f.kind == .format)
        #expect(f.detail == "Unsupported codec")
        #expect(!f.reason.isEmpty && f.canRetry)
        #expect(f.positionMs == 0)
        #expect(f.lowerQuality?.id == "20m-1080")
        #expect(rig.seen.events.contains { if case .playbackFailure(let x) = $0 { x?.item.ratingKey == "100" } else { false } })
        try await until("playback to end") { app.nowPlaying() == nil && !Player.isActive(app) }

        // Retry: playback starts and the failure is gone.
        try Failure.recover(app, .retry)
        try await rig.opened(2)
        rig.engine.time(3)
        try await until("playback after retry") { (app.nowPlaying()?.positionMs ?? 0) >= 3000 }
        #expect(Failure.current(app) == nil)

        // Controls and diagnostics.
        Player.tune(app, .volume(40))
        Player.tune(app, .speed(1.5))
        Player.tune(app, .repeat(.all))
        Player.tune(app, .sleepMinutes(30))
        rig.engine.onEvent?(.diagnostics(EngineDiagnostics(bufferedSeconds: 12.34, decoder: "VideoToolbox")))
        try await until("the controls to apply") {
            guard let s = Controls.playbackState(app) else { return false }
            return s.volume == 40 && s.speed == 1.5 && s.repeat == .all && s.sleep != nil
                && s.diagnostics.hwdec == "VideoToolbox"
        }
        let state = try #require(Controls.playbackState(app))
        #expect(state.sleep?.mode == "minutes")
        #expect(state.diagnostics.connection?.kind == "local")
        #expect(state.diagnostics.bufferedSecs == 12.3)
        #expect(rig.engine.volume == 0.4 && rig.engine.rate == 1.5)
        // The engine plays at most 2x: the state shows what plays.
        Player.tune(app, .speed(3))
        try await until("the clamped speed") { Controls.playbackState(app)?.speed == 2 }
        Player.tune(app, .speed(1))
        Player.tune(app, .sleepEndOfItem)
        try await until("end-of-item sleep") { app.nowPlaying()?.stopAfter == true }
        Player.tune(app, .sleepOff)
        try await until("sleep off") { app.nowPlaying()?.stopAfter == false }
        Player.tune(app, .mute(true))
        try await until("mute") { Controls.playbackState(app)?.muted == true }
        #expect(rig.engine.muted)
        Player.tune(app, .repeat(.off))

        // The queue is saved while playing, and offered again after stopping.
        try await until("a saved queue") { (Ops.Playback.savedQueue(app)?.positionMs ?? 0) >= 3000 }
        try await rig.stopped()
        let saved = try #require(Ops.Playback.savedQueue(app))
        #expect(saved.current.ratingKey == "100")
        // Volume and mute carry over to the next playback; the sleep timer does not.
        #expect(Controls.playbackState(app) == nil)

        try Ops.Playback.resumeSaved(app)
        try await rig.opened(3)
        #expect(rig.engine.loads[2].startSeconds == Double(saved.positionMs) / 1000)
        #expect(Controls.playbackState(app)?.volume == 40)
        #expect(Controls.playbackState(app)?.muted == true)
        #expect(Controls.playbackState(app)?.sleep == nil)
        // Plex was told the position when playback stopped, so it is not a recovered one.
        #expect(Controls.playbackState(app)?.diagnostics.recoveredFromMs == nil)

        // Stopping and forgetting clears the offer.
        try await rig.stopped()
        Ops.Playback.forgetSaved(app)
        #expect(Ops.Playback.savedQueue(app) == nil)
        await rig.done()
    }

    @Test func markersUpNextAndSkipping() async throws {
        let rig = try Rig()
        let app = rig.app
        let episode = try await rig.item("1011")
        Player.play(app, Player.PlayRequest(item: episode, resume: false))
        try await rig.opened(1)
        // The play queue continues with the season's other episodes.
        #expect(app.snapshot().queue?.items.map(\.item.ratingKey) == ["1011", "1012", "1013"])
        #expect(rig.engine.remote?.hasNext == true && rig.engine.remote?.hasPrevious == false)

        // The intro is offered, then skipped by seeking past it.
        rig.engine.time(6)
        try await until("the intro") { app.nowPlaying()?.marker != nil }
        #expect(app.nowPlaying()?.marker == SkipMarker(kind: "intro", startMs: 5000, endMs: 25000, skipsToNext: false))
        Player.skipMarker(app)
        try await until("the skip") { rig.engine.seeks == [25] }
        try await until("the marker to go") { app.nowPlaying()?.marker == nil }

        // In the final credits: "Up next", and skipping them plays the next episode.
        rig.engine.time(170)
        try await until("the credits") { app.nowPlaying()?.ending == true }
        #expect(app.nowPlaying()?.marker?.kind == "credits")
        #expect(app.nowPlaying()?.marker?.skipsToNext == true)
        Player.skipMarker(app)
        try await rig.opened(2)
        #expect(app.nowPlaying()?.item.ratingKey == "1012")
        #expect(app.nowPlaying()?.ending == false)
        // Skipping the final credits counts as watching to the end.
        await Report.flush(10_000)
        #expect(rig.logged("scrobble") { $0["key"] == "1011" })

        // Previous goes back; the lock screen's commands act like the controls.
        rig.engine.onRemoteCommand?(.previous)
        try await rig.opened(3)
        #expect(app.nowPlaying()?.item.ratingKey == "1011")
        rig.engine.onRemoteCommand?(.pause)
        try await until("the remote pause") { app.nowPlaying()?.paused == true }
        try await rig.stopped()

        // Automatic skipping.
        app.updateSettings { $0.player.autoSkipIntro = true }
        Player.play(app, Player.PlayRequest(item: episode, resume: false))
        try await rig.opened(4)
        rig.engine.seeks = []
        rig.engine.time(6)
        try await until("the automatic skip") { rig.engine.seeks == [25] }
        try await rig.stopped()
        await rig.done()
    }

    @Test func theEndOfAnItemFollowsRepeatStopAfterAndTheSleepTimer() async throws {
        let rig = try Rig()
        let app = rig.app
        let episode = try await rig.item("1011")
        Player.play(app, Player.PlayRequest(item: episode, resume: false))
        try await rig.opened(1)

        // The item ends: the next one plays, and the finished one is watched.
        rig.engine.time(179)
        rig.engine.end()
        try await rig.opened(2)
        #expect(app.nowPlaying()?.item.ratingKey == "1012")
        await Report.flush(10_000)
        #expect(rig.logged("scrobble") { $0["key"] == "1011" })

        // Repeat one plays it again.
        Player.tune(app, .repeat(.one))
        rig.engine.end()
        try await rig.opened(3)
        #expect(app.nowPlaying()?.item.ratingKey == "1012")

        // "Stop after this" wins over repeating.
        Player.queue(app, .stopAfter(on: true))
        try await until("stop after this") { app.nowPlaying()?.stopAfter == true }
        #expect(app.snapshot().queue?.stopAfter == true)
        rig.engine.end()
        try await until("playback to end") { app.nowPlaying() == nil && !Player.isActive(app) }

        // Repeat all wraps around at the end of the queue.
        Player.tune(app, .repeat(.all))
        let last = try await rig.item("1013")
        Player.play(app, Player.PlayRequest(item: last, resume: false))
        try await rig.opened(4)
        #expect(app.snapshot().queue?.items.count == 1)
        rig.engine.end()
        try await rig.opened(5)
        #expect(app.nowPlaying()?.item.ratingKey == "1013")
        Player.tune(app, .repeat(.off))

        // The sleep timer's "end of item" stops when the item ends.
        Player.tune(app, .sleepEndOfItem)
        try await until("end-of-item sleep") { app.nowPlaying()?.stopAfter == true }
        rig.engine.end()
        try await until("playback to end") { app.nowPlaying() == nil && !Player.isActive(app) }
        // A queue played to its end is not offered again.
        #expect(Ops.Playback.savedQueue(app) == nil)
        await rig.done()
    }

    @Test func failuresMoveOnUntilEverythingFailed() async throws {
        let rig = try Rig()
        let app = rig.app
        let episode = try await rig.item("1011")
        Player.play(app, Player.PlayRequest(item: episode, resume: false))
        try await until("the load") { rig.engine.loads.count == 1 }
        // The first one fails before it opened: the next one is tried.
        rig.engine.fail("connection reset", .network)
        try await rig.opened(2)
        #expect(app.nowPlaying()?.item.ratingKey == "1012")
        #expect(Failure.current(app)?.kind == .network)
        #expect(Failure.current(app)?.remaining == 2)
        // A failure in the middle of an item retries from where it stopped.
        rig.engine.time(42)
        try await until("the position") { app.nowPlaying()?.positionMs == 42_000 }
        rig.engine.fail("connection reset", .network)
        try await until("the next item") { rig.engine.loads.count == 3 }
        let f = try #require(Failure.current(app))
        #expect(f.item.ratingKey == "1012" && f.phase == .playing && f.positionMs == 42_000)
        // The last one fails too: playback is over.
        rig.engine.fail("connection reset", .network)
        try await until("playback to end") { app.nowPlaying() == nil && !Player.isActive(app) }
        #expect(Failure.current(app)?.item.ratingKey == "1013")
        // The server answering again retries on its own, once.
        app.setConnected(false)
        app.setConnected(true)
        try await until("the automatic retry") { rig.engine.loads.count == 4 }
        #expect(rig.engine.loads[3].url.path.contains("/library/parts/1013/"))
        try await rig.stopped()
        await rig.done()
    }

    @Test func downloadsPlayOfflineWithLocalProgress() async throws {
        let rig = try Rig()
        let app = rig.app
        let dir = TempDir("patina-files")
        func download(_ key: String) -> Download {
            let file = dir.file("\(key).m4a")
            FileManager.default.createFile(atPath: file.path, contents: Data([1, 2, 3]))
            return Download(
                id: "mock:alice:\(key)", serverId: "mock", serverName: "Mock",
                item: Metadata(ratingKey: key, type: "track", title: key == "tone" ? "Tone" : "Next", duration: 20000),
                file: file.path, state: .done, account: "alice")
        }
        let d = download("tone")
        let next = download("next")
        try Offline.record(app.paths, d, 8000, 20000, false, 0.9)
        try Offline.record(app.paths, next, 4000, 20000, false, 0.9)
        Player.playFiles(app, [d, next])
        try await until("the first file") { rig.engine.loads.count == 1 }
        // Resumed from the saved position, from the file itself.
        #expect(rig.engine.loads[0].startSeconds == 8)
        #expect(rig.engine.loads[0].url.isFileURL)
        #expect(rig.engine.loads[0].audioOnly)
        rig.engine.open(duration: 20)
        rig.engine.time(10)
        try await until("the position") { app.nowPlaying()?.positionMs == 10_000 }
        #expect(app.nowPlaying()?.local == true)
        Player.control(app, .next)
        try await until("the next file") { rig.engine.loads.count == 2 }
        #expect(rig.engine.loads[1].startSeconds == 4)
        try await until("the new entry") { app.nowPlaying()?.queuePos == 1 }
        #expect(Offline.position(app.paths, d) == 10_000)
        rig.engine.open(duration: 20)
        rig.engine.time(19)
        rig.engine.end()
        try await until("completion to be saved") { app.nowPlaying() == nil && Offline.position(app.paths, next) == 0 }
        #expect(!Player.isActive(app))
        await rig.done()
    }

    @Test func commandsWithoutAPlayer() async throws {
        let rig = try Rig()
        let app = rig.app
        #expect(Commands.playbackState(app) == nil)
        #expect(Commands.playbackFailure(app) == nil)
        #expect(Commands.savedQueue(app) == nil)
        Commands.control(app, command: .pause)
        Commands.tune(app, command: .cycleRepeat)
        Commands.skipMarker(app)
        #expect(throws: CoreError.self) { try Commands.recoverPlayback(app, action: .retry) }
        #expect(throws: CoreError.self) { try Commands.resumeSaved(app) }

        // The profile's entry is stored with `show: nil` (as the TUI writes it); the UI's save replaces it.
        try Prefs.save(app, nil, nil, Prefs.Rules(audioLanguages: ["eng"]))
        try Commands.setPlaybackPrefs(app, show: nil, title: nil, rules: Prefs.Rules.from(["audioLanguages": ["jpn"]]))
        #expect(try Commands.playbackPrefs(app).profile.audioLanguages == ["jpn"])
        try Commands.setPlaybackPrefs(app, rules: Prefs.Rules.from([:]))
        #expect(try Commands.playbackPrefs(app).profile.audioLanguages == nil)
        #expect(try Commands.playbackPrefsShows(app).isEmpty)

        // The whole-show track dialog, and remembering its choice.
        try await Commands.tracksLoad(app, show: "500")
        let view = try Commands.tracksView(app, query: Tracks.Query(show: "500", audio: nil, subtitle: "keep"))
        #expect(view.total == 6)
        #expect(view.seasons.map(\.count) == [3, 3])
        let audio = try #require(view.audio.first { $0.label.contains("English") })
        let planned = try Commands.tracksView(app, query: Tracks.Query(show: "500", audio: audio.id, subtitle: "keep"))
        #expect(planned.plan.episodes == 5)  // S02E03 has English selected already
        try Commands.tracksRemember(app, query: Tracks.Query(show: "500", audio: audio.id, subtitle: "off"))
        let shows = try Commands.playbackPrefsShows(app)
        #expect(shows.map(\.ratingKey) == ["500"])
        #expect(shows.first?.rules.audioLanguages == ["eng"] && shows.first?.rules.subtitles == .off)
        var progress: [(Int, Int)] = []
        try await Commands.tracksApply(app, changes: planned.plan.changes) { progress.append(($0, $1)) }
        #expect(progress.last.map { $0 == (5, 5) } == true)
        #expect(app.tracks == nil)

        // The track's LRC stream (mock_pms.py serves these; the Swift mock leaves them out).
        rig.mock.on { _, req in
            guard req.path == "/library/streams/31015" else { return nil }
            return .text("[ti:Tone]\n[00:05.00]then a voice\n[00:00.00]The line goes quiet")
        }
        let lyrics = try #require(try await Commands.lyrics(app, ratingKey: "3101"))
        #expect(lyrics.synced)
        #expect(lyrics.lines.map(\.timeMs) == [0, 5000])
        #expect(lyrics.lines.first?.text == "The line goes quiet")
        await rig.done()
    }

    @Test func failingReportsDoNotBlockPlayback() async throws {
        let rig = try Rig(MockPMS(reportStatus: 503))
        let app = rig.app
        let first = try await rig.item("100")
        Player.play(app, Player.PlayRequest(item: first, resume: false))
        try await rig.opened(1)
        rig.engine.time(2)
        let began = ContinuousClock.now
        Player.control(app, .pause)
        try await until("pause", timeout: .seconds(3)) { app.nowPlaying()?.paused == true }
        try await rig.stopped()
        #expect(ContinuousClock.now - began < .seconds(4))
        try await until("a failed report", timeout: .seconds(20)) {
            rig.mock.logged().contains { $0.hasPrefix("failed report") }
        }
        await rig.done()
    }
}
