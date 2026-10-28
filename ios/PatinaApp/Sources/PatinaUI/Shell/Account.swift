// The account and server switcher of web/src/components/Shell.svelte's sidebar footer, and
// the Plex Home user switcher of web/src/pages/Settings.svelte, with a PIN pad for protected
// profiles.

import PatinaCore
import PlexKit
import SwiftUI

struct AccountMenu: View {
    @Environment(AppStore.self) private var store
    @State private var confirmSignOut = false

    var body: some View {
        let session = store.session
        let name = session?.user?.name ?? "Server owner"
        Menu {
            if let session, session.servers.count > 1 {
                Section("Switch Server") {
                    ForEach(session.servers, id: \.id) { s in
                        Button {
                            Haptics.selection()
                            store.switchServer(s.id)
                        } label: {
                            if s.id == session.serverId {
                                Label(s.name, systemImage: "checkmark")
                            } else {
                                Text(s.name)
                            }
                        }
                    }
                }
            }
            Section {
                if store.hasTv {
                    Button("Switch User…", systemImage: "person.2") { store.sheet = .switchUser }
                }
                Button("Sign Out", systemImage: "rectangle.portrait.and.arrow.right", role: .destructive) {
                    confirmSignOut = true
                }
            }
        } label: {
            HStack(spacing: 10) {
                Avatar(name: name, thumb: session?.user?.thumb, size: 36)
                VStack(alignment: .leading, spacing: 1) {
                    Text(name).font(.subheadline.weight(.semibold)).foregroundStyle(.primary).lineLimit(1)
                    Text(session?.serverName ?? "").font(.caption).foregroundStyle(Theme.secondaryText).lineLimit(1)
                }
                Spacer(minLength: 4)
                Image(systemName: "chevron.up.chevron.down")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(Theme.tertiaryText)
            }
            .padding(.vertical, 8)
            .padding(.horizontal, 8)
            .contentShape(.rect)
        }
        .confirmationDialog("Sign out of Patina?", isPresented: $confirmSignOut, titleVisibility: .visible) {
            Button("Sign Out", role: .destructive) { store.signOut() }
        } message: {
            Text("Downloads stay on this device.")
        }
    }
}

/// A round picture of a person, or their initials.
struct Avatar: View {
    var name: String
    var thumb: String?
    var size: CGFloat = 36
    /// plex.tv avatars are absolute URLs; people in a library are server paths.
    var server: String?

    var body: some View {
        ZStack {
            Circle().fill(Theme.raised)
            Text(Format.initials(name))
                .font(.system(size: size * 0.36, weight: .semibold))
                .foregroundStyle(Theme.secondaryText)
            if let thumb, !thumb.isEmpty {
                Artwork(thumb, width: Int(size * 2), height: Int(size * 2), server: server)
                    .clipShape(.circle)
            }
        }
        .frame(width: size, height: size)
        .overlay(Circle().strokeBorder(Theme.hairline))
    }
}

/// Plex Home: switch to another profile of the account.
struct SwitchUserSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var users: [HomeUser]?
    @State private var error: String?
    @State private var pinFor: HomeUser?
    @State private var busy = false

    var body: some View {
        NavigationStack {
            Group {
                if let pinFor {
                    PinPad(user: pinFor, busy: busy) { pin in
                        Task { await switchTo(pinFor, pin: pin) }
                    } cancel: {
                        withAnimation(.spring) { self.pinFor = nil }
                    }
                    .transition(.move(edge: .trailing).combined(with: .opacity))
                } else if let error {
                    StatusView(error: error) { load() }
                } else if let users {
                    if users.isEmpty {
                        StatusView(
                            title: "No Plex Home", text: "This account has no other profiles.", systemImage: "person.2")
                    } else {
                        ScrollView {
                            LazyVGrid(columns: [GridItem(.adaptive(minimum: 104), spacing: 20)], spacing: 24) {
                                ForEach(users, id: \.uuid) { u in profile(u) }
                            }
                            .padding(24)
                        }
                    }
                } else {
                    ProgressView()
                }
            }
            .navigationTitle(pinFor == nil ? "Switch User" : "")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() }.disabled(busy) }
            }
            .task { load() }
        }
        .presentationDetents([.medium, .large])
    }

    private func profile(_ u: HomeUser) -> some View {
        let current = u.uuid != nil && u.uuid == store.session?.user?.uuid
        return Button {
            guard !current else { return }
            if u.protected {
                withAnimation(.spring) { pinFor = u }
            } else {
                Task { await switchTo(u, pin: nil) }
            }
        } label: {
            VStack(spacing: 8) {
                Avatar(name: HomeUser.name(u), thumb: u.thumb, size: 76)
                    .overlay {
                        if current { Circle().strokeBorder(store.accent, lineWidth: 3) }
                    }
                HStack(spacing: 4) {
                    Text(HomeUser.name(u)).font(.subheadline.weight(.medium)).lineLimit(1)
                    if u.protected { Image(systemName: "lock.fill").font(.caption2).foregroundStyle(Theme.tertiaryText) }
                }
                if current {
                    Text("Current").font(.caption).foregroundStyle(store.accent)
                } else if u.admin {
                    Text("Admin").font(.caption).foregroundStyle(Theme.tertiaryText)
                }
            }
        }
        .buttonStyle(CardButtonStyle())
        .disabled(busy)
    }

    private func load() {
        error = nil
        Task {
            do {
                users = try await Commands.homeUsers(store.app)
            } catch {
                self.error = errorText(error)
            }
        }
    }

    private func switchTo(_ u: HomeUser, pin: String?) async {
        guard let uuid = u.uuid else { return }
        busy = true
        let ok = await store.switchUser(uuid, pin: pin)
        busy = false
        if ok {
            Haptics.success()
            dismiss()
        } else if pin != nil {
            Haptics.error()
        }
    }
}

/// Four digits for a protected profile.
struct PinPad: View {
    let user: HomeUser
    var busy: Bool
    let submit: (String) -> Void
    let cancel: () -> Void

    @Environment(AppStore.self) private var store
    @State private var digits = ""

    var body: some View {
        VStack(spacing: 22) {
            Avatar(name: HomeUser.name(user), thumb: user.thumb, size: 64)
            Text("Enter the PIN for \(HomeUser.name(user))").font(.headline)
            HStack(spacing: 16) {
                ForEach(0..<4, id: \.self) { i in
                    Circle()
                        .fill(i < digits.count ? store.accent : .clear)
                        .overlay(Circle().strokeBorder(i < digits.count ? store.accent : Theme.tertiaryText, lineWidth: 1.5))
                        .frame(width: 14, height: 14)
                }
            }
            .overlay { if busy { ProgressView().offset(y: 28) } }
            Grid(horizontalSpacing: 22, verticalSpacing: 14) {
                ForEach([["1", "2", "3"], ["4", "5", "6"], ["7", "8", "9"], ["", "0", "⌫"]], id: \.self) { row in
                    GridRow {
                        ForEach(row, id: \.self) { key in pinKey(key) }
                    }
                }
            }
            Button("Back", action: cancel).font(.subheadline)
        }
        .padding(24)
        .disabled(busy)
        .onChange(of: busy) { _, now in
            // A wrong PIN: start over.
            if !now { digits = "" }
        }
    }

    @ViewBuilder
    private func pinKey(_ key: String) -> some View {
        if key.isEmpty {
            Color.clear.frame(width: 68, height: 68)
        } else {
            Button {
                Haptics.tap()
                if key == "⌫" {
                    if !digits.isEmpty { digits.removeLast() }
                } else if digits.count < 4 {
                    digits.append(key)
                    if digits.count == 4 { submit(digits) }
                }
            } label: {
                Group {
                    if key == "⌫" {
                        Image(systemName: "delete.left").font(.title3)
                    } else {
                        Text(key).font(.title.weight(.medium).monospacedDigit())
                    }
                }
                .frame(width: 68, height: 68)
                .background(key == "⌫" ? Color.clear : Theme.raised, in: .circle)
            }
            .buttonStyle(CardButtonStyle())
            .accessibilityLabel(key == "⌫" ? "Delete" : key)
        }
    }
}

#Preview("PIN pad") {
    var user = HomeUser()
    user.title = "Robin"
    user.protected = true
    return PinPad(user: user, busy: false) { _ in } cancel: {}
        .environment(AppStore.preview())
        .preferredColorScheme(.dark)
}
