import AppKit
import SwiftUI

// MARK: Downloads

struct DownloadSettings: View {
    @Environment(AppModel.self) private var app

    var body: some View {
        if let p = app.settings?.player {
            Form {
                Section {
                    LabeledContent {
                        HStack(spacing: 6) {
                            Button("Choose…") { choose(current: p.downloads_dir) }
                            Button("Show in Finder") { app.perform { try await API.openDownloadsFolder() } }
                        }
                    } label: {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Downloads folder")
                            Text(p.downloads_dir.isEmpty ? "Default (Movies/Patina)" : p.downloads_dir)
                                .font(.callout.monospaced())
                                .foregroundStyle(.secondary)
                                .textSelection(.enabled)
                                .lineLimit(2)
                                .truncationMode(.middle)
                        }
                    }
                    if !p.downloads_dir.isEmpty {
                        LabeledContent("") {
                            Button("Use Default Folder") { set("") }
                        }
                    }
                } footer: {
                    Text("Where movies, episodes and music you download for offline viewing are saved.")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            .formStyle(.grouped)
            .fixedSize(horizontal: false, vertical: true)
        }
    }

    private func set(_ dir: String) {
        app.updatePlayer(["downloads_dir": .from(dir)]) { $0.downloads_dir = dir }
    }

    private func choose(current: String) {
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.canCreateDirectories = true
        panel.allowsMultipleSelection = false
        panel.prompt = "Choose"
        panel.message = "Choose where Patina saves downloads."
        if !current.isEmpty { panel.directoryURL = URL(fileURLWithPath: current) }
        guard panel.runModal() == .OK, let url = panel.url else { return }
        set(url.path)
    }
}

// MARK: Integrations

struct IntegrationSettings: View {
    @Environment(AppModel.self) private var app

    var body: some View {
        if let p = app.settings?.player {
            Form {
                Section {
                    Toggle(isOn: settingsBinding(p.media_controls) { v in app.updatePlayer(["media_controls": .from(v)]) { $0.media_controls = v } }) {
                        SettingsLabel("System media controls", "Show what's playing in Control Center and respond to media keys.")
                    }
                }
                Section {
                    Toggle(isOn: settingsBinding(p.discord) { v in app.updatePlayer(["discord": .from(v)]) { $0.discord = v } }) {
                        SettingsLabel("Discord Rich Presence", "Show what you're watching or listening to on your Discord profile.")
                    }
                    if p.discord {
                        LabeledContent {
                            SettingsTextField(value: p.discord_app_id, prompt: "Application ID", width: 200, monospaced: true, trim: true) { v in
                                app.updatePlayer(["discord_app_id": .from(v)]) { $0.discord_app_id = v }
                            }
                        } label: {
                            SettingsLabel("Application ID", "Create an application in the Discord developer portal and paste its ID. Its name is what Discord shows, e.g. “Patina”.")
                        }
                        LabeledContent("") {
                            Button {
                                app.perform { try await API.openUrl("https://discord.com/developers/applications") }
                            } label: {
                                Label("Developer Portal", systemImage: "arrow.up.forward.square")
                            }
                        }
                    }
                }
            }
            .formStyle(.grouped)
            .fixedSize(horizontal: false, vertical: true)
        }
    }
}

// MARK: Appearance

struct AppearanceSettings: View {
    @Environment(AppModel.self) private var app
    /// While dragging, the size is previewed locally and saved on release.
    @State private var dragging: Double?

    var body: some View {
        if let ui = app.settings?.ui, let info = app.info {
            Form {
                Section {
                    LabeledContent {
                        HStack(spacing: 10) {
                            ForEach(info.accents) { a in swatch(a, on: ui.accent == a.id) }
                        }
                    } label: {
                        SettingsLabel("Accent", "Used sparingly: progress, watched marks and selection.")
                    }
                    LabeledContent {
                        HStack(spacing: 10) {
                            Slider(value: Binding(
                                get: { dragging ?? Double(ui.poster_size) },
                                set: { raw in
                                    // Steps of 4 without the tick marks a stepped slider draws.
                                    let v = (raw / 4).rounded() * 4
                                    dragging = v
                                    app.settings?.ui.poster_size = Int(v)
                                }
                            ), in: 120...240) { editing in
                                if !editing, let v = dragging {
                                    dragging = nil
                                    let size = Int(v)
                                    app.updateUi(["poster_size": .from(size)]) { $0.poster_size = size }
                                }
                            }
                            .frame(width: 200)
                            Text("\(Int(dragging ?? Double(ui.poster_size))) pt")
                                .monospacedDigit()
                                .foregroundStyle(.secondary)
                                .frame(width: 48, alignment: .trailing)
                        }
                    } label: {
                        Text("Poster size")
                    }
                    Toggle(isOn: settingsBinding(ui.hide_spoilers) { v in app.updateUi(["hide_spoilers": .from(v)]) { $0.hide_spoilers = v } }) {
                        SettingsLabel("Hide spoilers", "Blur thumbnails and summaries of episodes you haven't watched.")
                    }
                }
            }
            .formStyle(.grouped)
            .fixedSize(horizontal: false, vertical: true)
        }
    }

    private func swatch(_ a: Choice, on: Bool) -> some View {
        Button {
            app.updateUi(["accent": .from(a.id)]) { $0.accent = a.id }
        } label: {
            Circle()
                .fill(a.color.flatMap { Color(hex: $0) } ?? Theme.defaultAccent)
                .overlay(Circle().strokeBorder(Color.primary.opacity(0.15), lineWidth: 0.5))
                .frame(width: 18, height: 18)
                .padding(3)
                .overlay(Circle().strokeBorder(on ? Color.primary : .clear, lineWidth: 1.5))
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .help(a.label)
        .accessibilityLabel(a.label)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}
