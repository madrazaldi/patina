import AppKit
import SwiftUI

// MARK: Playback

struct PlaybackSettings: View {
    @Environment(AppModel.self) private var app

    var body: some View {
        if let p = app.settings?.player, let info = app.info {
            Form {
                Section {
                    Picker(selection: settingsBinding(p.quality) { v in app.updatePlayer(["quality": .from(v)]) { $0.quality = v } }) {
                        ForEach(info.qualities) { q in Text(q.label).tag(q.id) }
                    } label: {
                        SettingsLabel("Streaming quality", "Original plays the file directly. Lower qualities ask the server to transcode, which helps on slow connections.")
                    }
                    Picker(selection: settingsBinding(p.watched_threshold) { v in app.updatePlayer(["watched_threshold": .from(v)]) { $0.watched_threshold = v } }) {
                        ForEach([0.8, 0.85, 0.9, 0.95], id: \.self) { t in Text("\(Int((t * 100).rounded()))%").tag(t) }
                    } label: {
                        SettingsLabel("Mark as watched after", "How much of an item must play before it counts as watched.")
                    }
                }
                Section {
                    Toggle(isOn: settingsBinding(p.continuous) { v in app.updatePlayer(["continuous": .from(v)]) { $0.continuous = v } }) {
                        SettingsLabel("Autoplay next episode", "Queue the following episodes when you play one.")
                    }
                    Toggle(isOn: settingsBinding(p.sync_streams) { v in app.updatePlayer(["sync_streams": .from(v)]) { $0.sync_streams = v } }) {
                        SettingsLabel("Sync audio and subtitle tracks", "Start with the tracks selected in Plex, and remember tracks you switch to in mpv.")
                    }
                }
                Section("Intros and credits") {
                    Toggle(isOn: settingsBinding(p.auto_skip_intro) { v in app.updatePlayer(["auto_skip_intro": .from(v)]) { $0.auto_skip_intro = v } }) {
                        SettingsLabel("Skip intros automatically", "Uses Plex intro markers. Otherwise press the skip key when prompted.")
                    }
                    Toggle(isOn: settingsBinding(p.auto_skip_credits) { v in app.updatePlayer(["auto_skip_credits": .from(v)]) { $0.auto_skip_credits = v } }) {
                        SettingsLabel("Skip credits automatically", "Jumps past credits, or straight to the next episode.")
                    }
                    LabeledContent {
                        SettingsTextField(value: p.skip_key, prompt: "Tab", width: 120) { v in
                            app.updatePlayer(["skip_key": .from(v)]) { $0.skip_key = v }
                        }
                    } label: {
                        SettingsLabel("Skip key", "mpv key name that skips an intro or credits. It is bound only while Patina controls mpv.")
                    }
                }
            }
            .formStyle(.grouped)
            .fixedSize(horizontal: false, vertical: true)
        }
    }
}

// MARK: mpv

struct MpvSettings: View {
    @Environment(AppModel.self) private var app
    @State private var probe: (ok: Bool, text: String)?
    @State private var probing = 0

    var body: some View {
        if let p = app.settings?.player, let info = app.info {
            Form {
                Section {
                    Text("Everything plays in mpv, with your normal mpv configuration, scripts and key bindings.")
                        .foregroundStyle(.secondary)
                }
                Section {
                    LabeledContent("mpv executable") {
                        HStack(spacing: 6) {
                            SettingsTextField(value: p.mpv_path, prompt: "Auto-detect", width: 220) { v in
                                app.updatePlayer(["mpv_path": .from(v)]) { $0.mpv_path = v }
                                test(after: .milliseconds(300))
                            }
                            Button("Choose…") { chooseMpv() }
                        }
                    }
                    LabeledContent {
                        Button("Test") { test() }
                    } label: {
                        HStack(alignment: .firstTextBaseline, spacing: 6) {
                            if let probe {
                                Image(systemName: probe.ok ? "checkmark.circle" : "xmark.circle")
                                    .foregroundStyle(probe.ok ? AnyShapeStyle(.secondary) : AnyShapeStyle(.red))
                                Text(probe.text)
                                    .foregroundStyle(probe.ok ? AnyShapeStyle(.secondary) : AnyShapeStyle(.red))
                                    .textSelection(.enabled)
                                    .lineLimit(3)
                            } else {
                                ProgressView().controlSize(.mini)
                                Text("Checking…").foregroundStyle(.secondary)
                            }
                        }
                        .font(.callout)
                    }
                }
                Section {
                    LabeledContent {
                        SettingsTextField(value: p.extra_args, prompt: "--fs --profile=hq", width: 240, monospaced: true) { v in
                            app.updatePlayer(["extra_args": .from(v)]) { $0.extra_args = v }
                        }
                    } label: {
                        SettingsLabel("Extra arguments", "Appended to the command line. Your mpv.conf is always loaded.")
                    }
                    LabeledContent {
                        Button("Open Folder") { app.perform { try await API.openMpvConfig() } }
                    } label: {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Configuration")
                            Text(info.mpvConfigDir)
                                .font(.callout.monospaced())
                                .foregroundStyle(.secondary)
                                .textSelection(.enabled)
                            Text(info.mpvConfExists ? "mpv.conf found" : "No mpv.conf yet")
                                .font(.callout)
                                .foregroundStyle(.tertiary)
                        }
                    }
                }
            }
            .formStyle(.grouped)
            .fixedSize(horizontal: false, vertical: true)
            .task { if probe == nil { test() } }
        }
    }

    private func test(after delay: Duration = .zero) {
        probing += 1
        let mine = probing
        probe = nil
        Task {
            if delay > .zero { try? await Task.sleep(for: delay) }
            let result: (Bool, String)
            do { result = (true, try await API.probeMpv()) } catch { result = (false, error.localizedDescription) }
            if mine == probing { probe = result }
        }
    }

    private func chooseMpv() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.treatsFilePackagesAsDirectories = true
        panel.message = "Choose the mpv executable, e.g. inside mpv.app/Contents/MacOS."
        panel.prompt = "Choose"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        var path = url.path
        // Picking mpv.app means the binary inside it.
        if url.pathExtension == "app" { path = url.appendingPathComponent("Contents/MacOS/mpv").path }
        app.updatePlayer(["mpv_path": .from(path)]) { $0.mpv_path = path }
        test(after: .milliseconds(300))
    }
}
