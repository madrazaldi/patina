import AppKit
import SwiftUI

@main
struct PatinaApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @State private var app = AppModel.shared

    var body: some Scene {
        Window("Patina", id: "main") {
            RootView()
                .environment(app)
                .tint(app.accent)
                .frame(minWidth: 960, minHeight: 620)
        }
        .defaultSize(width: 1360, height: 860)
        .commands { PatinaCommands(app: app) }

        SwiftUI.Settings {
            SettingsView()
                .environment(app)
                .tint(app.accent)
        }
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        MainActor.assumeIsolated {
            AppModel.shared.start()
            MediaRemote.shared.start()
        }
    }

    func applicationWillTerminate(_ notification: Notification) {
        Core.shutdown()
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}

/// Signed in: the shell; otherwise the sign-in and server screens.
struct RootView: View {
    @Environment(AppModel.self) private var app
    @Environment(\.openSettings) private var openSettings

    var body: some View {
        ZStack(alignment: .bottomTrailing) {
            if case .ready = app.stage, app.session != nil {
                Shell()
            } else {
                GateView()
            }
            Toasts()
        }
        .onChange(of: app.settingsRequest) { openSettings() }
    }
}

/// Menu bar commands: navigation and playback.
struct PatinaCommands: Commands {
    let app: AppModel

    var body: some Commands {
        CommandGroup(after: .sidebar) {
            Button("Back") {
                let place = app.selection ?? .home
                if !(app.paths[place] ?? []).isEmpty { app.paths[place]?.removeLast() }
            }
            .keyboardShortcut("[", modifiers: .command)
            Button("Home") { app.selection = .home; app.paths[.home] = [] }
                .keyboardShortcut("h", modifiers: [.command, .shift])
        }
        CommandMenu("Playback") {
            Button("Play/Pause") { app.perform { try await API.control(.togglePause) } }
                .keyboardShortcut("p", modifiers: [.command])
                .disabled(app.nowPlaying == nil)
            Button("Next") { app.perform { try await API.control(.next) } }
                .keyboardShortcut(.rightArrow, modifiers: [.command, .control])
                .disabled(app.nowPlaying == nil)
            Button("Previous") { app.perform { try await API.control(.previous) } }
                .keyboardShortcut(.leftArrow, modifiers: [.command, .control])
                .disabled(app.nowPlaying == nil)
            Divider()
            Button("Skip Forward 10 Seconds") { app.perform { try await API.control(.seekRelative(10)) } }
                .keyboardShortcut(.rightArrow, modifiers: [.command, .option])
                .disabled(app.nowPlaying == nil)
            Button("Skip Back 10 Seconds") { app.perform { try await API.control(.seekRelative(-10)) } }
                .keyboardShortcut(.leftArrow, modifiers: [.command, .option])
                .disabled(app.nowPlaying == nil)
            Divider()
            Button("Stop") { app.perform { try await API.control(.stop) } }
                .keyboardShortcut(".", modifiers: [.command])
                .disabled(app.nowPlaying == nil)
            Button("Show Queue") { app.showQueue.toggle() }
                .keyboardShortcut("u", modifiers: [.command, .option])
                .disabled(app.queue == nil)
        }
    }
}
