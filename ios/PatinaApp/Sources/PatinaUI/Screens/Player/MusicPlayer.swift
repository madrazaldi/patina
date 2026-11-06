// The "now playing" layout of music: the album art (or the lyrics) over its own blurred
// colours, the scrub bar, the transport, and the queue, repeat, shuffle, sleep and AirPlay.
// The desktop's NowPlaying.svelte bar with its queue and lyrics panels, as a full screen.

import PatinaCore
import PatinaPlayer
import PlexKit
import SwiftUI

struct MusicPlayer: View {
    let engine: AetherPlaybackEngine
    let np: NowPlaying
    @Binding var drag: CGFloat
    let close: () -> Void

    @Environment(AppStore.self) private var store
    @State private var interaction = PlayerInteraction()
    @State private var lyrics = false
    @State private var panel: PlayerPanel?

    var body: some View {
        GeometryReader { geo in
            let wide = geo.size.width > geo.size.height * 1.1
            VStack(spacing: 0) {
                topBar
                if wide {
                    HStack(spacing: 40) {
                        stage.frame(maxWidth: geo.size.height * 0.75)
                        controls.frame(maxWidth: 520)
                    }
                    .frame(maxHeight: .infinity)
                } else {
                    stage
                        .frame(maxHeight: .infinity)
                        .padding(.vertical, 12)
                    controls
                }
            }
            .padding(.horizontal, 24)
            .padding(.top, geo.safeAreaInsets.top + 6)
            .padding(.bottom, max(geo.safeAreaInsets.bottom, 16) + 8)
        }
        .background { backdrop }
        .foregroundStyle(.white)
        .modifier(PlayerDismissGesture(offset: $drag, close: close))
        .sheet(item: $panel) { p in
            PlayerPanelSheet(panel: p)
                .environment(store)
                .tint(store.accent)
                .preferredColorScheme(.dark)
        }
    }

    // MARK: Pieces

    private var art: some View {
        Group {
            if let d = store.localDownload(np) {
                DownloadArtwork(id: d.id, source: Metadata.poster(np.item))
            } else {
                Artwork(Metadata.poster(np.item), width: 600, height: 600, server: Metadata.serverId(np.item))
            }
        }
    }

    private var backdrop: some View {
        ZStack {
            Color.black
            art
                .blur(radius: 60, opaque: true)
                .saturation(1.4)
                .opacity(0.55)
            LinearGradient(colors: [.black.opacity(0.1), .black.opacity(0.7)], startPoint: .top, endPoint: .bottom)
        }
        .ignoresSafeArea()
    }

    private var topBar: some View {
        HStack {
            Button(action: close) {
                Image(systemName: "chevron.down")
                    .font(.title3.weight(.semibold))
                    .frame(width: 44, height: 44)
            }
            .accessibilityLabel("Close")
            Spacer()
            Capsule().fill(.white.opacity(0.35)).frame(width: 40, height: 5)
            Spacer()
            Menu {
                if !engine.audioTracks.isEmpty { TrackMenu(engine: engine) }
                SpeedPicker(engine: engine)
                Section {
                    if let album = np.item.parentRatingKey {
                        Button("Go to Album", systemImage: "square.stack") { go(.item(ratingKey: album)) }
                    }
                    if let artist = np.item.grandparentRatingKey {
                        Button("Go to Artist", systemImage: "music.mic") { go(.item(ratingKey: artist)) }
                    }
                }
                Section {
                    Button("Stop Playback", systemImage: "stop.fill", role: .destructive) {
                        Commands.control(store.app, command: .stop)
                    }
                }
            } label: {
                Image(systemName: "ellipsis")
                    .font(.title3.weight(.semibold))
                    .frame(width: 44, height: 44)
            }
            .accessibilityLabel("More")
        }
        .buttonStyle(.plain)
    }

    /// The art, or the lyrics in its place.
    @ViewBuilder
    private var stage: some View {
        if lyrics {
            LyricsView(ratingKey: Metadata.ratingKey(np.item), position: engine.position)
                .transition(.opacity)
        } else {
            art
                .aspectRatio(1, contentMode: .fit)
                .clipShape(.rect(cornerRadius: 14, style: .continuous))
                .shadow(color: .black.opacity(0.45), radius: 30, y: 16)
                .scaleEffect(np.paused ? 0.86 : 1)
                .animation(.spring(response: 0.45, dampingFraction: 0.7), value: np.paused)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .transition(.opacity)
        }
    }

    private var controls: some View {
        let app = store.app!
        let state = PlayerTransportState(
            player: engine, hasPrevious: true, hasNext: np.queuePos + 1 < np.queueLen)
        let actions = PlayerTransportActions(
            togglePause: { Commands.control(app, command: .togglePause) },
            seek: { Commands.control(app, command: .seekTo(seconds: $0)) },
            close: close)
        let (title, subtitle) = PlexUI.playingLines(np.item)
        let repeatMode = store.playback?.repeat ?? .off
        let shuffled = store.queue?.shuffled ?? false
        return VStack(spacing: 22) {
            VStack(alignment: .leading, spacing: 3) {
                Text(title).font(.title2.bold()).lineLimit(1)
                Text([subtitle, np.item.parentTitle ?? ""].filter { !$0.isEmpty }.joined(separator: " · "))
                    .font(.body)
                    .foregroundStyle(.white.opacity(0.7))
                    .lineLimit(1)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            PlayerScrubBar(state: state, actions: actions, interaction: interaction)
            HStack {
                toggle("shuffle", on: shuffled, label: "Shuffle") {
                    Commands.queue(app, command: .shuffle(on: !shuffled))
                }
                Spacer()
                transport("backward.fill", label: "Previous") { Commands.control(app, command: .previous) }
                Spacer()
                Button {
                    Haptics.tap()
                    Commands.control(app, command: .togglePause)
                } label: {
                    Image(systemName: np.paused ? "play.fill" : "pause.fill")
                        .font(.system(size: 44))
                        .contentTransition(.symbolEffect(.replace))
                        .frame(width: 72, height: 72)
                }
                .accessibilityLabel(np.paused ? "Play" : "Pause")
                Spacer()
                transport("forward.fill", label: "Next") { Commands.control(app, command: .next) }
                    .disabled(!state.hasNext && repeatMode == .off)
                Spacer()
                toggle(repeatMode == .one ? "repeat.1" : "repeat", on: repeatMode != .off, label: "Repeat") {
                    Commands.tune(app, command: .cycleRepeat)
                }
            }
            .buttonStyle(.plain)
            HStack {
                toggle("quote.bubble", on: lyrics, label: "Lyrics") {
                    withAnimation(.easeInOut(duration: 0.25)) { lyrics.toggle() }
                }
                Spacer()
                AirPlayButton(video: false)
                    .frame(width: 44, height: 44)
                Spacer()
                Menu {
                    SleepMenu()
                    Toggle(
                        "Stop After This", systemImage: "stop.circle",
                        isOn: Binding(get: { np.stopAfter }, set: { Commands.queue(app, command: .stopAfter(on: $0)) }))
                } label: {
                    Image(systemName: store.playback?.sleep != nil || np.stopAfter ? "moon.zzz.fill" : "moon.zzz")
                        .font(.title3)
                        .foregroundStyle(store.playback?.sleep != nil || np.stopAfter ? store.accent : .white.opacity(0.75))
                        .frame(width: 44, height: 44)
                }
                .accessibilityLabel("Sleep Timer")
                Spacer()
                toggle("list.bullet", on: false, label: "Queue") { panel = .queue }
            }
            .buttonStyle(.plain)
        }
    }

    private func transport(_ symbol: String, label: String, action: @escaping () -> Void) -> some View {
        Button {
            Haptics.tap()
            action()
        } label: {
            Image(systemName: symbol).font(.system(size: 30)).frame(width: 56, height: 56)
        }
        .accessibilityLabel(label)
    }

    private func toggle(_ symbol: String, on: Bool, label: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.title3)
                .foregroundStyle(on ? store.accent : .white.opacity(0.75))
                .frame(width: 44, height: 44)
        }
        .accessibilityLabel(label)
        .accessibilityAddTraits(on ? .isSelected : [])
    }

    private func go(_ route: Route) {
        close()
        store.navigate(to: route)
    }
}

// MARK: Lyrics

/// Port of web/src/components/Lyrics.svelte: synced lyrics follow the song (tap a line to go
/// there); plain ones scroll.
struct LyricsView: View {
    let ratingKey: String
    let position: Double
    @Environment(AppStore.self) private var store
    @State private var loaded = Loader<Lyrics.Lyrics?>()

    var body: some View {
        Group {
            if let found = loaded.value {
                if let lyrics = found, !lyrics.lines.isEmpty {
                    lines(lyrics)
                } else {
                    message("No lyrics for this track.")
                }
            } else if loaded.error != nil {
                message("Couldn't load the lyrics.")
            } else {
                ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .task(id: ratingKey) {
            let (app, key) = (store.app!, ratingKey)
            loaded = Loader()
            await loaded.load { try await Commands.lyrics(app, ratingKey: key) }
        }
    }

    private func lines(_ lyrics: Lyrics.Lyrics) -> some View {
        let ms = Int(position * 1000)
        let current = lyrics.synced ? lyrics.lines.lastIndex { ($0.timeMs ?? .max) <= ms } : nil
        return ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    ForEach(Array(lyrics.lines.enumerated()), id: \.offset) { i, line in
                        Text(line.text.isEmpty ? "♪" : line.text)
                            .font(.title2.bold())
                            .foregroundStyle(.white.opacity(!lyrics.synced || i == current ? 1 : 0.35))
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .id(i)
                            .onTapGesture {
                                if let t = line.timeMs { Commands.control(store.app, command: .seekTo(seconds: Double(t) / 1000)) }
                            }
                    }
                }
                .padding(.vertical, 40)
            }
            .scrollIndicators(.hidden)
            .mask(LinearGradient(stops: [.init(color: .clear, location: 0), .init(color: .black, location: 0.1), .init(color: .black, location: 0.85), .init(color: .clear, location: 1)], startPoint: .top, endPoint: .bottom))
            .onChange(of: current) {
                guard let current else { return }
                withAnimation(.spring(response: 0.5, dampingFraction: 0.9)) { proxy.scrollTo(current, anchor: .center) }
            }
        }
    }

    private func message(_ text: String) -> some View {
        Text(text)
            .font(.headline)
            .foregroundStyle(.white.opacity(0.6))
            .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
