import AppKit
import SwiftUI

/// The Settings window (web/src/pages/Settings.svelte), as native tabs of
/// grouped forms. Every change saves straight away through
/// `app.updatePlayer` / `app.updateUi`; text fields save after a pause.
struct SettingsView: View {
    @Environment(AppModel.self) private var app
    @AppStorage("settingsTab") private var tab = "playback"

    var body: some View {
        Group {
            if app.settings != nil, app.info != nil {
                TabView(selection: $tab) {
                    PlaybackSettings().tabItem { Label("Playback", systemImage: "play.rectangle") }.tag("playback")
                    MpvSettings().tabItem { Label("mpv", systemImage: "terminal") }.tag("mpv")
                    DownloadSettings().tabItem { Label("Downloads", systemImage: "arrow.down.circle") }.tag("downloads")
                    IntegrationSettings().tabItem { Label("Integrations", systemImage: "puzzlepiece.extension") }.tag("integrations")
                    AppearanceSettings().tabItem { Label("Appearance", systemImage: "paintpalette") }.tag("appearance")
                    AccountSettings().tabItem { Label("Account", systemImage: "person.crop.circle") }.tag("account")
                    AboutSettings().tabItem { Label("About", systemImage: "info.circle") }.tag("about")
                }
            } else {
                ProgressView().controlSize(.small).frame(width: 560, height: 240)
            }
        }
        .frame(width: 560)
    }
}
