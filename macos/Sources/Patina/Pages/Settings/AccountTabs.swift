import AppKit
import SwiftUI

// MARK: Account

struct AccountSettings: View {
    @Environment(AppModel.self) private var app
    @State private var users: [HomeUser] = []
    @State private var pinFor: HomeUser?
    @State private var pin = ""
    @State private var confirmSignOut = false

    var body: some View {
        let user = app.session?.user
        Form {
            Section {
                if let user {
                    LabeledContent {
                        Button("Sign Out…", role: .destructive) { confirmSignOut = true }
                    } label: {
                        HStack(spacing: 10) {
                            Avatar(path: user.thumb, name: user.name, size: 36)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(user.name).font(.body.weight(.medium))
                                Text([user.email, user.plexPass ? "Plex Pass" : nil].compactMap { $0 }.joined(separator: " · "))
                                    .font(.callout)
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                } else if app.session != nil {
                    LabeledContent {
                        Button("Sign In with Plex") { app.perform { try await API.signOut() } }
                    } label: {
                        SettingsLabel("Direct server connection", "Not signed in to plex.tv.")
                    }
                } else {
                    Text("Not signed in.").foregroundStyle(.secondary)
                }
                if let s = app.session {
                    LabeledContent("Server", value: s.serverName)
                }
            }
            if users.count > 1 {
                Section("Plex Home") {
                    ForEach(users) { u in
                        let active = u.uuid != nil && u.uuid == user?.uuid
                        LabeledContent {
                            if active {
                                Text("Current").foregroundStyle(.secondary)
                            } else {
                                Button("Switch") {
                                    if u.protected { pin = ""; pinFor = u } else { switchTo(u, pin: nil) }
                                }
                            }
                        } label: {
                            HStack(spacing: 8) {
                                Avatar(path: u.thumb, name: u.title ?? u.username ?? "User", size: 22)
                                Text(u.title ?? u.username ?? "User")
                                if u.protected {
                                    Image(systemName: "lock").font(.caption).foregroundStyle(.tertiary).help("Protected by a PIN")
                                }
                            }
                        }
                    }
                }
            }
        }
        .formStyle(.grouped)
        .fixedSize(horizontal: false, vertical: true)
        .task(id: app.scope) {
            users = app.hasTv ? ((try? await API.homeUsers()) ?? []) : []
        }
        .confirmationDialog("Sign out of Plex?", isPresented: $confirmSignOut) {
            Button("Sign Out", role: .destructive) { app.perform { try await API.signOut() } }
        }
        .alert("Enter PIN", isPresented: Binding(get: { pinFor != nil }, set: { if !$0 { pinFor = nil } })) {
            SecureField("PIN", text: $pin)
            Button("Switch") { if let u = pinFor { switchTo(u, pin: pin) } }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("\(pinFor?.title ?? pinFor?.username ?? "This")'s profile is protected.")
        }
    }

    private func switchTo(_ u: HomeUser, pin: String?) {
        pinFor = nil
        guard let uuid = u.uuid else { return }
        app.resetNavigation()
        app.perform { try await API.switchUser(uuid, pin: pin) }
    }
}

// MARK: About

struct AboutSettings: View {
    @Environment(AppModel.self) private var app

    var body: some View {
        if let info = app.info {
            Form {
                Section {
                    HStack(spacing: 12) {
                        Logo(height: 32)
                        Text("Patina").font(Theme.display(26))
                        Text(info.version).monospacedDigit().foregroundStyle(.secondary)
                        Spacer()
                    }
                    .padding(.vertical, 4)
                    Text("A Plex client that plays everything in mpv, with your own mpv configuration.")
                        .foregroundStyle(.secondary)
                }
                Section {
                    LabeledContent {
                        Button("Show in Finder") {
                            NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: info.configPath)])
                        }
                    } label: {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Settings file")
                            Text(info.configPath)
                                .font(.callout.monospaced())
                                .foregroundStyle(.secondary)
                                .textSelection(.enabled)
                        }
                    }
                }
            }
            .formStyle(.grouped)
            .fixedSize(horizontal: false, vertical: true)
        }
    }
}
