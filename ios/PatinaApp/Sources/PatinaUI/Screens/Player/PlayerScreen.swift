// The full-screen player: the iOS counterpart of web/src/components/NowPlaying.svelte and
// PlayerPanel.svelte (on the desktop mpv drew the picture and its own OSD). It wires the
// PatinaPlayer pieces (surface, gestures, chrome, prompts) to the core's NowPlaying, queue,
// controls, failure and tracks. Video gets the picture with the chrome over it, music the
// "now playing" layout (MusicPlayer.swift). Dragging down closes it to the mini player.

import PatinaCore
import PatinaPlayer
import PlexKit
import SwiftUI

struct PlayerScreen: View {
    @Environment(AppStore.self) private var store
    /// How far the screen is pulled down to close it.
    @State private var drag: CGFloat = 0

    var body: some View {
        GeometryReader { geo in
            content
                .offset(y: drag)
                .background(.black.opacity(1 - min(1, drag / max(geo.size.height, 1)) * 1.4))
        }
        .ignoresSafeArea()
    }

    @ViewBuilder
    private var content: some View {
        if let engine = store.engine, let np = store.nowPlaying {
            if PlexUI.isAudio(np.item) {
                MusicPlayer(engine: engine, np: np, drag: $drag, close: close)
            } else {
                VideoPlayer(engine: engine, np: np, drag: $drag, close: close)
            }
        } else if let failure = store.failure {
            PlaybackFailureView(failure: failure, close: close)
                .modifier(PlayerDismissGesture(offset: $drag, close: close))
        } else {
            Color.black
        }
    }

    private func close() {
        store.playerOpen = false
    }
}

/// Pull down to close the player; it springs back when let go early.
struct PlayerDismissGesture: ViewModifier {
    @Binding var offset: CGFloat
    let close: () -> Void

    func body(content: Content) -> some View {
        content.gesture(
            DragGesture(minimumDistance: 14, coordinateSpace: .global)
                .onChanged { g in
                    // Only downwards; the first points of the drag are slack, so it doesn't jump.
                    offset = max(0, g.translation.height - 14)
                }
                .onEnded { g in
                    if g.translation.height > 140 || g.predictedEndTranslation.height > 480 {
                        Haptics.tap()
                        close()
                    } else {
                        withAnimation(.spring(response: 0.35, dampingFraction: 0.85)) { offset = 0 }
                    }
                })
    }
}

// MARK: Video

struct VideoPlayer: View {
    let engine: AetherPlaybackEngine
    let np: NowPlaying
    @Binding var drag: CGFloat
    let close: () -> Void

    @Environment(AppStore.self) private var store
    @State private var interaction = PlayerInteraction()
    @State private var fill = PlayerVideoFill.fit
    @AppStorage("player.subtitleScale") private var subtitleScale = 1.0
    @AppStorage("player.subtitleBackground") private var subtitleBackground = SubtitleStyle.Background.outline
    @State private var panel: PlayerPanel?
    @State private var diagnostics = false
    /// Where the "Up next" countdown started.
    @State private var upNextFrom: Double?

    var body: some View {
        let state = transport
        let actions = actions(state)
        let (title, subtitle) = PlexUI.playingLines(np.item)
        ZStack {
            PlayerSurface(
                player: engine, fill: fill,
                subtitleStyle: SubtitleStyle(scale: subtitleScale, background: subtitleBackground),
                subtitleLift: interaction.chromeVisible ? 96 : 0
            )
            .ignoresSafeArea()
            PlayerGestureLayer(state: state, actions: actions, interaction: interaction, fill: $fill)
                .ignoresSafeArea()
            // The picture's vertical drags are volume and brightness, so pulling down to close
            // starts along the top edge.
            VStack {
                Color.clear
                    .frame(height: 96)
                    .contentShape(.rect)
                    .onTapGesture { interaction.toggleChrome() }
                    .modifier(PlayerDismissGesture(offset: $drag, close: close))
                Spacer()
            }
            .ignoresSafeArea()
            PlayerControlsOverlay(title: title, subtitle: subtitle, state: state, actions: actions, interaction: interaction) {
                menus
            } accessory: {
                prompt(state)
            }
            if diagnostics {
                DiagnosticsPanel(engine: engine, np: np)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                    .padding(.top, 70)
                    .padding(.leading, 20)
                    .allowsHitTesting(false)
            }
        }
        .background(.black)
        .playerKeyboardShortcuts(state: state, actions: actions, interaction: interaction)
        .statusBarHidden(!interaction.chromeVisible)
        .persistentSystemOverlays(.hidden)
        .defersSystemGestures(on: .vertical)
        .animation(.spring(response: 0.4, dampingFraction: 0.85), value: np.marker)
        .animation(.spring(response: 0.4, dampingFraction: 0.85), value: np.ending)
        .sheet(item: $panel) { p in
            PlayerPanelSheet(panel: p)
                .environment(store)
                .tint(store.accent)
                .preferredColorScheme(.dark)
        }
        .onChange(of: np.ending, initial: true) { upNextFrom = np.ending ? engine.position : nil }
        .onAppear { Orientation.landscape(true) }
        .onDisappear { Orientation.landscape(false) }
    }

    // MARK: State

    private var transport: PlayerTransportState {
        var s = PlayerTransportState(
            player: engine, hasPrevious: np.queuePos > 0 || engine.position > 3, hasNext: np.queuePos + 1 < np.queueLen,
            markers: PlayerScreenModel.markers(np.item))
        if let chapters = store.playback?.chapters, !chapters.isEmpty {
            s.chapters = chapters.map { Double($0.timeMs) / 1000 }
        }
        if np.live {
            s.isLive = true
            s.canSeek = false
        }
        return s
    }

    private func actions(_ state: PlayerTransportState) -> PlayerTransportActions {
        let app = store.app!
        let engine = engine
        let store = store
        return PlayerTransportActions(
            togglePause: { Commands.control(app, command: .togglePause) },
            seek: { Commands.control(app, command: .seekTo(seconds: $0)) },
            previous: { Commands.control(app, command: .previous) },
            next: { Commands.control(app, command: .next) },
            close: close,
            // Held for 2x: straight to the engine, so the speed the core keeps is untouched.
            speedBoost: { on in engine.setRate(on ? 2 : store.playback?.speed ?? 1) },
            thumbnail: state.supportsThumbnails ? { await engine.scrubThumbnail(at: $0) } : nil
        )
    }

    // MARK: Prompts

    @ViewBuilder
    private func prompt(_ state: PlayerTransportState) -> some View {
        if let marker = np.marker {
            PlayerSkipPill(PlayerScreenModel.skipTitle(marker, np: np)) {
                Haptics.tap()
                Commands.skipMarker(store.app)
            }
        } else if np.ending, let next = store.upNext {
            let left = max(0, (state.duration ?? 0) - state.position)
            let from = upNextFrom ?? state.position
            let total = max(1, (state.duration ?? 0) - from)
            let (title, subtitle) = PlexUI.playingLines(next)
            UpNextCard(
                title: Metadata.kind(next) == "episode" ? Metadata.title(next) : title,
                subtitle: Metadata.kind(next) == "episode" ? "\(next.grandparentTitle ?? "") · \(Metadata.episodeCode(next))" : subtitle,
                countdown: np.stopAfter ? nil : min(1, max(0, 1 - left / total)),
                secondsLeft: np.stopAfter ? nil : Int(left.rounded(.up)),
                onPlayNow: { Commands.control(store.app, command: .next) },
                onStopAfter: np.stopAfter ? nil : { Commands.queue(store.app, command: .stopAfter(on: true)) }
            ) {
                Artwork(next.thumb ?? Metadata.poster(next), width: 320, height: 180, server: Metadata.serverId(next))
            }
        }
    }

    // MARK: Menus

    @ViewBuilder
    private var menus: some View {
        if !engine.audioTracks.isEmpty || !engine.subtitleTracks.isEmpty {
            Menu {
                TrackMenu(engine: engine)
                Section("Subtitle Style") {
                    Picker("Size", systemImage: "textformat.size", selection: $subtitleScale) {
                        Text("Small").tag(0.8)
                        Text("Medium").tag(1.0)
                        Text("Large").tag(1.3)
                        Text("Extra Large").tag(1.6)
                    }
                    Picker("Style", systemImage: "character.textbox", selection: $subtitleBackground) {
                        Text("Outline").tag(SubtitleStyle.Background.outline)
                        Text("Box").tag(SubtitleStyle.Background.box)
                    }
                }
            } label: {
                PlayerIcon(systemName: "captions.bubble")
            }
            .accessibilityLabel("Audio and Subtitles")
        }
        Menu {
            SpeedPicker(engine: engine)
        } label: {
            PlayerIcon(systemName: "gauge.with.dots.needle.67percent")
        }
        .accessibilityLabel("Speed")
        Menu {
            Picker("Picture", systemImage: fill.systemImage, selection: $fill.animation(.spring(response: 0.4, dampingFraction: 0.85))) {
                ForEach(PlayerVideoFill.allCases, id: \.self) { f in
                    Label(f.label, systemImage: f.systemImage).tag(f)
                }
            }
            if engine.pictureInPicture.isPossible {
                Button("Picture in Picture", systemImage: "pip.enter") { engine.pictureInPicture.start() }
            }
            PlaybackMenuItems(np: np, panel: $panel)
            Toggle("Diagnostics", systemImage: "waveform.path.ecg", isOn: $diagnostics)
            Section {
                Button("Stop Playback", systemImage: "stop.fill", role: .destructive) {
                    Commands.control(store.app, command: .stop)
                }
            }
        } label: {
            PlayerIcon(systemName: "ellipsis")
        }
        .accessibilityLabel("More")
    }
}

// MARK: Shared pieces

/// The audio and subtitle pickers (the desktop's track menus, here over the engine's tracks).
struct TrackMenu: View {
    let engine: AetherPlaybackEngine
    @Environment(AppStore.self) private var store

    var body: some View {
        if !engine.audioTracks.isEmpty {
            Picker(
                "Audio", systemImage: "speaker.wave.2",
                selection: Binding(
                    get: { engine.audioTrackID ?? -1 },
                    set: { Commands.selectTrack(store.app, choice: .audio($0)) })
            ) {
                ForEach(engine.audioTracks) { t in
                    Text(t.title).tag(t.id)
                }
            }
        }
        if !engine.subtitleTracks.isEmpty {
            Picker(
                "Subtitles", systemImage: "captions.bubble",
                selection: Binding(
                    get: { engine.subtitle.map(AetherPlaybackEngine.trackID(of:)) ?? -1 },
                    set: { id in
                        Commands.selectTrack(
                            store.app, choice: .subtitle(id < 0 ? nil : AetherPlaybackEngine.subtitle(trackID: id)))
                    })
            ) {
                Text("Off").tag(-1)
                ForEach(engine.subtitleTracks) { t in
                    Text(t.isForced ? "\(t.title) (Forced)" : t.title).tag(t.id)
                }
            }
        }
    }
}

struct SpeedPicker: View {
    let engine: AetherPlaybackEngine
    @Environment(AppStore.self) private var store

    var body: some View {
        let speeds = [0.5, 0.75, 1, 1.25, 1.5, 1.75, 2].filter { $0 <= max(engine.maxRate, 1) }
        Picker(
            "Speed", systemImage: "gauge.with.dots.needle.67percent",
            selection: Binding(
                get: { store.playback?.speed ?? 1 },
                set: { Commands.tune(store.app, command: .speed($0)) })
        ) {
            ForEach(speeds, id: \.self) { rate in
                Text(rate.formatted(.number.precision(.fractionLength(0...2))) + "×").tag(rate)
            }
        }
        .pickerStyle(.inline)
    }
}

/// Chapters, queue, repeat, stop-after and the sleep timer: the entries of PlayerPanel.svelte.
struct PlaybackMenuItems: View {
    let np: NowPlaying
    @Binding var panel: PlayerPanel?
    @Environment(AppStore.self) private var store

    var body: some View {
        let app = store.app!
        Section {
            if let chapters = store.playback?.chapters, !chapters.isEmpty {
                Button("Chapters", systemImage: "list.bullet.rectangle") { panel = .chapters }
            }
            if store.queue != nil {
                Button("Queue", systemImage: "list.number") { panel = .queue }
            }
            Picker(
                "Repeat", systemImage: "repeat",
                selection: Binding(
                    get: { store.playback?.repeat ?? .off },
                    set: { Commands.tune(app, command: .repeat($0)) })
            ) {
                Text("Off").tag(Controls.Repeat.off)
                Text("Repeat All").tag(Controls.Repeat.all)
                Text("Repeat One").tag(Controls.Repeat.one)
            }
            Toggle(
                "Stop After This", systemImage: "stop.circle",
                isOn: Binding(get: { np.stopAfter }, set: { Commands.queue(app, command: .stopAfter(on: $0)) }))
            SleepMenu()
        }
    }
}

struct SleepMenu: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let app = store.app!
        let sleep = store.playback?.sleep
        Menu {
            Button("Off") { Commands.tune(app, command: .sleepOff) }
            ForEach(Controls.SLEEP_PRESETS, id: \.self) { m in
                Button("\(m) Minutes") { Commands.tune(app, command: .sleepMinutes(m)) }
            }
            Button {
                Commands.tune(app, command: .sleepEndOfItem)
            } label: {
                if sleep?.mode == "endOfItem" { Label("End of Item", systemImage: "checkmark") } else { Text("End of Item") }
            }
        } label: {
            Label(PlayerScreenModel.sleepLabel(sleep), systemImage: "moon.zzz")
        }
    }
}

/// Small derivations from the core's state, shared by the video and music layouts.
enum PlayerScreenModel {
    /// The intro and credits of an item, for the scrub bar.
    static func markers(_ item: Metadata) -> [PlayerMarkerRange] {
        item.marker.compactMap { m in
            guard let s = m.startTimeOffset, let e = m.endTimeOffset, e > s else { return nil }
            let kind: PlayerMarkerRange.Kind = m.type == "intro" ? .intro : m.type == "credits" ? .credits : .other
            return PlayerMarkerRange(kind: kind, start: Double(s) / 1000, end: Double(e) / 1000)
        }
    }

    static func skipTitle(_ marker: SkipMarker, np: NowPlaying) -> String {
        if marker.kind == "intro" { return "Skip Intro" }
        if marker.skipsToNext, !np.stopAfter, np.queuePos + 1 < np.queueLen {
            return Metadata.kind(np.item) == "episode" ? "Next Episode" : "Play Next"
        }
        return "Skip Credits"
    }

    static func sleepLabel(_ sleep: Controls.SleepState?) -> String {
        guard let sleep else { return "Sleep Timer" }
        if sleep.mode == "endOfItem" { return "Sleep: End of Item" }
        guard let ends = sleep.endsAtMs else { return "Sleep Timer" }
        let left = max(1, Int((Double(ends) / 1000 - Date().timeIntervalSince1970) / 60 + 0.99))
        return "Sleep: \(left) min"
    }
}

extension AppStore {
    /// What plays after the current item, for "Up next".
    var upNext: Metadata? {
        guard let np = nowPlaying, let q = queue, let i = q.items.firstIndex(where: { $0.uid == np.uid }),
            i + 1 < q.items.count
        else { return nil }
        return q.items[i + 1].item
    }

    /// The download playing offline, for its saved artwork.
    func localDownload(_ np: NowPlaying) -> Download? {
        guard np.local else { return nil }
        return downloads.first { $0.item.ratingKey == np.item.ratingKey && $0.poster != nil }
    }
}

/// iPhone: video plays in landscape; the rest of the app turns back upright.
@MainActor
enum Orientation {
    static func landscape(_ on: Bool) {
        guard UIDevice.current.userInterfaceIdiom == .phone else { return }
        PatinaAppDelegate.orientations = on ? .landscape : .all
        guard let scene = UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }).first else { return }
        for window in scene.windows {
            var vc = window.rootViewController
            while let c = vc {
                c.setNeedsUpdateOfSupportedInterfaceOrientations()
                vc = c.presentedViewController
            }
        }
        scene.requestGeometryUpdate(.iOS(interfaceOrientations: on ? .landscape : .portrait))
    }
}
