// A bench for the player on its own (DEBUG only; no desktop twin): the engine, the surface, the
// gestures and the controls wired to a URL, without the core or the rest of the app. It also
// shows how a screen composes them.

#if DEBUG
import PatinaCore
import SwiftUI

public struct PlayerLab: View {
    @State private var urlText: String
    @State private var subtitleText: String
    @State private var player: AetherPlaybackEngine?
    @State private var error: String?
    private let autoplay: Bool

    /// - Parameters:
    ///   - url: What to play; `python3 -m http.server` in a folder of test media serves it to the simulator.
    ///   - subtitles: An SRT, VTT or ASS sidecar, loaded as an external track and shown from the start.
    ///   - autoplay: Start at once instead of showing the form.
    public init(url: URL? = nil, subtitles: URL? = nil, autoplay: Bool = false) {
        _urlText = State(initialValue: url?.absoluteString ?? "http://127.0.0.1:8000/lab.mp4")
        _subtitleText = State(initialValue: subtitles?.absoluteString ?? "")
        self.autoplay = autoplay && url != nil
    }

    public var body: some View {
        Group {
            if let player, let url = URL(string: urlText) {
                PlayerLabScreen(player: player, url: url, subtitles: URL(string: subtitleText)) {
                    player.stop()
                    self.player = nil
                }
            } else {
                form
            }
        }
        .preferredColorScheme(.dark)
        .task {
            if autoplay { open() }
        }
    }

    private var form: some View {
        NavigationStack {
            Form {
                Section("Media") {
                    TextField("URL", text: $urlText)
                        .textContentType(.URL)
                        .keyboardType(.URL)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                    TextField("Subtitle sidecar (optional)", text: $subtitleText)
                        .keyboardType(.URL)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                }
                Section {
                    Button("Play", systemImage: "play.fill", action: open)
                        .disabled(URL(string: urlText) == nil)
                }
                if let error {
                    Section { Text(error).foregroundStyle(.red) }
                }
            }
            .navigationTitle("Player Lab")
        }
    }

    private func open() {
        do {
            player = try AetherPlaybackEngine()
        } catch {
            self.error = error.localizedDescription
        }
    }
}

/// The player screen, as the app's will be: surface, gestures, chrome, menus and prompts.
private struct PlayerLabScreen: View {
    let player: AetherPlaybackEngine
    let url: URL
    let subtitles: URL?
    let close: () -> Void

    @State private var interaction = PlayerInteraction()
    @State private var fill = PlayerVideoFill.fit
    @State private var subtitleStyle = SubtitleStyle()
    @State private var rateBeforeBoost: Double?
    @State private var events: [String] = []
    @State private var showsDiagnostics = false
    @State private var stopAfter = false
    @State private var dismissedPrompt = false

    var body: some View {
        let state = PlayerTransportState(player: player, hasPrevious: true, hasNext: true, markers: markers)
        let actions = actions
        ZStack {
            PlayerSurface(player: player, fill: fill, subtitleStyle: subtitleStyle,
                          subtitleLift: interaction.chromeVisible ? 96 : 0)
                .ignoresSafeArea()
            PlayerGestureLayer(state: state, actions: actions, interaction: interaction, fill: $fill)
                .ignoresSafeArea()
            PlayerControlsOverlay(title: "Player Lab", subtitle: url.lastPathComponent, state: state, actions: actions, interaction: interaction) {
                menus
            } accessory: {
                prompt(state)
            }
            if showsDiagnostics {
                diagnostics
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
        .animation(.spring(response: 0.4, dampingFraction: 0.85), value: promptKind(state))
        .task { await load() }
        .onAppear {
            player.pictureInPicture.onRestore = {}
            player.onEvent = { event in
                events.append(String(describing: event))
                if events.count > 12 { events.removeFirst(events.count - 12) }
            }
            player.onRemoteCommand = { command in
                switch command {
                case .play: player.play()
                case .pause: player.pause()
                case .togglePause: player.togglePause()
                case .seek(let seconds): Task { await player.seek(toSeconds: seconds) }
                case .skip(let seconds): actions.skip(seconds, in: state)
                case .next, .previous, .stop: break
                }
            }
        }
    }

    private var actions: PlayerTransportActions {
        let player = player
        return PlayerTransportActions(
            togglePause: { player.togglePause() },
            seek: { seconds in Task { await player.seek(toSeconds: seconds) } },
            previous: { Task { await player.seek(toSeconds: 0) } },
            next: { Task { await player.seek(toSeconds: max(0, (player.duration ?? 0) - 15)) } },
            close: close,
            speedBoost: { on in
                if on {
                    rateBeforeBoost = player.rate
                    player.setRate(2)
                } else {
                    player.setRate(rateBeforeBoost ?? 1)
                    rateBeforeBoost = nil
                }
            },
            thumbnail: { seconds in await player.scrubThumbnail(at: seconds) }
        )
    }

    /// Made-up markers, so the bar, the snapping and the pills have something to show.
    private var markers: [PlayerMarkerRange] {
        guard let duration = player.duration, duration > 90 else { return [] }
        return [
            PlayerMarkerRange(kind: .intro, start: 5, end: 30),
            PlayerMarkerRange(kind: .credits, start: duration - 20, end: duration),
        ]
    }

    private enum Prompt: Equatable { case none, skipIntro, upNext }

    private func promptKind(_ state: PlayerTransportState) -> Prompt {
        guard !dismissedPrompt, let intro = markers.first, let credits = markers.last else { return .none }
        if (intro.start..<intro.end).contains(state.position) { return .skipIntro }
        if state.position >= credits.start { return .upNext }
        return .none
    }

    @ViewBuilder
    private func prompt(_ state: PlayerTransportState) -> some View {
        switch promptKind(state) {
        case .skipIntro:
            PlayerSkipPill("Skip Intro") {
                Task { await player.seek(toSeconds: markers[0].end) }
            }
        case .upNext:
            let credits = markers[1]
            let progress = (state.position - credits.start) / (credits.end - credits.start)
            UpNextCard(
                title: "The Next Episode", subtitle: "S1 · E2",
                countdown: stopAfter ? nil : progress,
                secondsLeft: stopAfter ? nil : Int((credits.end - state.position).rounded(.up)),
                onPlayNow: { Task { await player.seek(toSeconds: 0) } },
                onStopAfter: stopAfter ? nil : { stopAfter = true }
            ) {
                LinearGradient(colors: [.indigo, .teal], startPoint: .topLeading, endPoint: .bottomTrailing)
            }
        case .none:
            EmptyView()
        }
    }

    @ViewBuilder
    private var menus: some View {
        if !player.subtitleTracks.isEmpty {
            Menu {
                Picker("Subtitles", selection: Binding(
                    get: { player.subtitle.map(EngineMapping.subtitleTrackID) ?? -1 },
                    set: { id in player.selectSubtitle(id < 0 ? nil : EngineMapping.subtitle(trackID: id)) }
                )) {
                    Text("Off").tag(-1)
                    ForEach(player.subtitleTracks) { track in
                        Text(track.title).tag(track.id)
                    }
                }
                Picker("Size", selection: $subtitleStyle.scale) {
                    Text("Small").tag(0.8)
                    Text("Medium").tag(1.0)
                    Text("Large").tag(1.3)
                }
                Picker("Style", selection: $subtitleStyle.background) {
                    Text("Outline").tag(SubtitleStyle.Background.outline)
                    Text("Box").tag(SubtitleStyle.Background.box)
                }
            } label: {
                PlayerIcon(systemName: "captions.bubble")
            }
        }
        Menu {
            Picker("Speed", selection: Binding(get: { player.rate }, set: { player.setRate($0) })) {
                ForEach([0.5, 0.75, 1, 1.25, 1.5, 2], id: \.self) { rate in
                    Text(rate.formatted(.number.precision(.fractionLength(0...2))) + "×").tag(rate)
                }
            }
        } label: {
            PlayerIcon(systemName: "gauge.with.dots.needle.67percent")
        }
        Menu {
            Picker("Picture", selection: $fill.animation(.spring(response: 0.4, dampingFraction: 0.85))) {
                ForEach(PlayerVideoFill.allCases, id: \.self) { fill in
                    Label(fill.label, systemImage: fill.systemImage).tag(fill)
                }
            }
            if player.pictureInPicture.isPossible {
                Button("Picture in Picture", systemImage: "pip.enter") { player.pictureInPicture.start() }
            }
            Toggle("Diagnostics", systemImage: "waveform.path.ecg", isOn: $showsDiagnostics)
            Toggle("Hide Prompts", systemImage: "rectangle.badge.xmark", isOn: $dismissedPrompt)
        } label: {
            PlayerIcon(systemName: "ellipsis")
        }
    }

    private var diagnostics: some View {
        let d = player.diagnostics
        return VStack(alignment: .leading, spacing: 3) {
            Text("phase \(String(describing: player.phase))")
            Text("route \(d.route ?? "–") · decoder \(d.decoder ?? "–")")
            Text("\(d.videoSize ?? "–") \(d.videoFormat ?? "") · dropped \(d.droppedFrames.map(String.init) ?? "–")")
            Text("buffered \(d.bufferedSeconds.map { String(format: "%.1f s", $0) } ?? "–") · rate \(player.rate, format: .number)")
            Divider().overlay(.white.opacity(0.3))
            ForEach(Array(events.enumerated()), id: \.offset) { _, event in
                Text(event).lineLimit(1)
            }
        }
        .font(.caption2.monospaced())
        .foregroundStyle(.white)
        .padding(10)
        .frame(maxWidth: 420, alignment: .leading)
        .background(.black.opacity(0.6), in: .rect(cornerRadius: 10))
    }

    private func load() async {
        let sidecars = subtitles.map {
            [ExternalSubtitle(url: $0, title: "English (sidecar)", language: "en", format: $0.pathExtension)]
        } ?? []
        let request = EngineLoad(
            url: url, subtitle: sidecars.isEmpty ? nil : .external(0), externalSubtitles: sidecars,
            nowPlaying: NowPlayingCard(title: url.deletingPathExtension().lastPathComponent, subtitle: "Player Lab")
        )
        player.setRemoteQueue(hasPrevious: false, hasNext: false)
        try? await player.load(request)
    }
}

// MARK: - Previews

private struct ControlsPreview: View {
    @State private var interaction = PlayerInteraction()
    @State private var state = PlayerTransportState(
        position: 754, duration: 2710, buffered: 1100, isPaused: true, hasPrevious: true, hasNext: true,
        chapters: [420, 1260, 1980], markers: [
            PlayerMarkerRange(kind: .intro, start: 40, end: 130),
            PlayerMarkerRange(kind: .credits, start: 2580, end: 2710),
        ]
    )
    var showsUpNext = false

    var body: some View {
        let actions = PlayerTransportActions(
            togglePause: { state.isPaused.toggle() },
            seek: { state.position = $0 },
            previous: {}, next: {}, close: {}
        )
        ZStack {
            LinearGradient(colors: [.orange, .purple, .black], startPoint: .topLeading, endPoint: .bottomTrailing)
                .ignoresSafeArea()
            PlayerGestureLayer(state: state, actions: actions, interaction: interaction, fill: .constant(.fit))
                .ignoresSafeArea()
            PlayerControlsOverlay(title: "The Expanse", subtitle: "S2 · E5 · Home", state: state, actions: actions, interaction: interaction) {
                PlayerIcon(systemName: "captions.bubble")
                PlayerIcon(systemName: "ellipsis")
            } accessory: {
                if showsUpNext {
                    UpNextCard(title: "Paradigm Shift", subtitle: "S2 · E6", countdown: 0.4, secondsLeft: 9, onPlayNow: {}, onStopAfter: {}) {
                        LinearGradient(colors: [.indigo, .teal], startPoint: .top, endPoint: .bottom)
                    }
                } else {
                    PlayerSkipPill("Skip Intro") {}
                }
            }
        }
        .preferredColorScheme(.dark)
    }
}

#Preview("Controls", traits: .landscapeLeft) {
    ControlsPreview()
}

#Preview("Controls, portrait") {
    ControlsPreview()
}

#Preview("Up next", traits: .landscapeLeft) {
    ControlsPreview(showsUpNext: true)
}

#Preview("Scrub bubble", traits: .landscapeLeft) {
    ScrubBubble(
        scrub: .init(origin: 600, target: 684, precision: .half, snapped: true, raw: 684, duration: 2710),
        thumbnail: nil, showsThumbnail: true
    )
    .padding()
    .background(.gray)
}

#Preview("Lab") {
    PlayerLab()
}
#endif
