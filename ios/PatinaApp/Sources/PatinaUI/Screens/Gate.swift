// Port of web/src/pages/Gate.svelte: everything before the library. The splash, sign-in
// (plex.tv/link code, browser approval, email and password with 2FA, a server by URL and
// token), the server picker, and the connecting and failed states.

import PatinaCore
import PlexKit
import SwiftUI

struct Gate: View {
    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass

    enum Mode: Equatable {
        case choose
        case browser(url: String)
        case code(String)
        case password
        case manual
    }

    @State private var mode = Mode.choose
    @State private var busy = false
    @State private var error: String?
    @State private var login = ""
    @State private var password = ""
    @State private var code = ""
    @State private var url = "http://"
    @State private var token = ""
    @State private var pinTask: Task<Void, Never>?

    var body: some View {
        ZStack {
            backdrop
            ScrollView {
                VStack(spacing: 28) {
                    Spacer(minLength: sizeClass == .regular ? 80 : 40)
                    brand
                    VStack(spacing: 18) {
                        content
                        if let error {
                            Label(error, systemImage: "exclamationmark.triangle.fill")
                                .font(.footnote)
                                .foregroundStyle(Theme.danger)
                                .textSelection(.enabled)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .padding(12)
                                .background(Theme.danger.opacity(0.1), in: .rect(cornerRadius: 12, style: .continuous))
                        }
                    }
                    .padding(22)
                    .background(.ultraThinMaterial, in: .rect(cornerRadius: 24, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 24, style: .continuous).strokeBorder(Theme.hairline))
                    .animation(.spring(duration: 0.4), value: mode)
                    .animation(.spring(duration: 0.4), value: store.stage)
                    Spacer(minLength: 40)
                }
                .frame(maxWidth: 420)
                .padding(.horizontal, 20)
                .frame(maxWidth: .infinity)
            }
            .scrollBounceBehavior(.basedOnSize)
        }
    }

    // MARK: Pieces

    private var backdrop: some View {
        ZStack {
            Theme.background
            RadialGradient(
                colors: [store.accent.opacity(0.28), .clear], center: .init(x: 0.2, y: 0.05), startRadius: 10,
                endRadius: 520)
            RadialGradient(
                colors: [store.accent.opacity(0.12), .clear], center: .init(x: 0.95, y: 0.9), startRadius: 10,
                endRadius: 480)
        }
        .ignoresSafeArea()
    }

    private var brand: some View {
        VStack(spacing: 10) {
            RoundedRectangle(cornerRadius: 22, style: .continuous)
                .fill(LinearGradient(colors: [store.accent, store.accent.opacity(0.55)], startPoint: .topLeading, endPoint: .bottomTrailing))
                .frame(width: 76, height: 76)
                .overlay(Text("P").font(.system(size: 40, weight: .bold, design: .rounded)).foregroundStyle(.black.opacity(0.75)))
                .shadow(color: store.accent.opacity(0.35), radius: 24, y: 10)
            Text("Patina").font(.system(size: 40, weight: .bold, design: .rounded))
            Text("A Plex client for iPhone and iPad")
                .font(.subheadline)
                .foregroundStyle(Theme.secondaryText)
        }
    }

    @ViewBuilder
    private var content: some View {
        switch store.stage {
        case .booting, .ready:
            ProgressView().controlSize(.large).padding(.vertical, 20)
        case .connecting(let name):
            VStack(spacing: 14) {
                ProgressView().controlSize(.large)
                Text("Connecting to \(name)…").foregroundStyle(Theme.secondaryText)
            }
            .padding(.vertical, 12)
        case .failed(let message):
            VStack(spacing: 14) {
                Image(systemName: "bolt.horizontal.circle").font(.system(size: 40)).foregroundStyle(Theme.danger)
                Text(message)
                    .font(.callout)
                    .foregroundStyle(Theme.secondaryText)
                    .multilineTextAlignment(.center)
                    .textSelection(.enabled)
                primary("Try Again") {
                    Task { await Commands.retry(store.app) }
                }
                secondary("Sign Out") { store.signOut() }
            }
        case .pickServer(let servers, let user):
            serverPicker(servers, user)
        case .login:
            signIn
        }
    }

    @ViewBuilder
    private var signIn: some View {
        switch mode {
        case .choose:
            VStack(spacing: 12) {
                primary("Sign In with Plex", busy: busy) { startPin(strong: true) }
                secondary("Use a Link Code") { startPin(strong: false) }
                HStack(spacing: 18) {
                    link("Email and Password") { switchMode(.password) }
                    link("Connect to a Server") { switchMode(.manual) }
                }
                .padding(.top, 6)
            }
        case .browser(let url):
            VStack(spacing: 16) {
                ProgressView()
                Text("Approve Patina in the browser that just opened.")
                    .multilineTextAlignment(.center)
                    .foregroundStyle(Theme.secondaryText)
                HStack(spacing: 18) {
                    link("Open It Again") { store.perform { try await Commands.openUrl(store.host, url: url) } }
                    link("Cancel", action: cancelPin)
                }
            }
        case .code(let code):
            VStack(spacing: 16) {
                Text("Go to **plex.tv/link** on any device and enter")
                    .multilineTextAlignment(.center)
                    .foregroundStyle(Theme.secondaryText)
                Text(code)
                    .font(.system(size: 44, weight: .semibold, design: .monospaced))
                    .kerning(10)
                    .textSelection(.enabled)
                    .padding(.vertical, 14)
                    .frame(maxWidth: .infinity)
                    .background(Theme.raised.opacity(0.7), in: .rect(cornerRadius: 14, style: .continuous))
                    .contextMenu {
                        Button("Copy Code", systemImage: "doc.on.doc") { UIPasteboard.general.string = code }
                    }
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text("Waiting for approval…").font(.footnote).foregroundStyle(Theme.secondaryText)
                }
                HStack(spacing: 18) {
                    link("Open plex.tv/link") {
                        store.perform { try await Commands.openUrl(store.host, url: "https://plex.tv/link") }
                    }
                    link("Cancel", action: cancelPin)
                }
            }
        case .password:
            VStack(spacing: 10) {
                field("Email or username", text: $login, content: .username)
                SecureField("Password", text: $password)
                    .textContentType(.password)
                    .modifier(FieldStyle())
                field("Two-factor code (if enabled)", text: $code, content: .oneTimeCode)
                    .keyboardType(.numberPad)
                primary("Sign In", busy: busy) {
                    submit { try await Commands.passwordSignIn(store.app, login: login, password: password, code: code) }
                }
                .disabled(login.isEmpty || password.isEmpty)
                link("Back") { switchMode(.choose) }
            }
        case .manual:
            VStack(spacing: 10) {
                Text("Enter the server address and an X-Plex-Token. Watchlist and Plex Home need a plex.tv sign-in.")
                    .font(.footnote)
                    .foregroundStyle(Theme.secondaryText)
                    .frame(maxWidth: .infinity, alignment: .leading)
                field("http://192.168.1.10:32400", text: $url, content: .URL)
                    .keyboardType(.URL)
                field("X-Plex-Token", text: $token, content: .password)
                primary("Connect", busy: busy) {
                    submit { try await Commands.signInManual(store.app, url: url, token: token) }
                }
                .disabled(token.trimmingCharacters(in: .whitespaces).isEmpty)
                link("Back") { switchMode(.choose) }
            }
        }
    }

    private func serverPicker(_ servers: [ServerView], _ user: UserView) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 10) {
                Avatar(name: user.name, thumb: user.thumb, size: 36)
                VStack(alignment: .leading, spacing: 1) {
                    Text(user.name).font(.subheadline.weight(.semibold))
                    Text("Choose a server").font(.caption).foregroundStyle(Theme.secondaryText)
                }
            }
            ForEach(servers, id: \.id) { s in
                Button {
                    Haptics.tap()
                    Task { await Commands.pickServer(store.app, id: s.id) }
                } label: {
                    HStack(spacing: 12) {
                        Image(systemName: "server.rack")
                            .font(.title3)
                            .foregroundStyle(s.owned ? store.accent : Theme.secondaryText)
                            .frame(width: 32)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(s.name).font(.body.weight(.medium)).foregroundStyle(.primary)
                            Text(
                                [s.owned ? "Owned" : "Shared by \(s.sourceTitle ?? "someone")", s.platform, s.version.map { "v\($0)" }]
                                    .compactMap { $0 }.joined(separator: " · ")
                            )
                            .font(.caption)
                            .foregroundStyle(Theme.secondaryText)
                            .lineLimit(1)
                        }
                        Spacer()
                        Text(s.presence ? "Online" : "Offline?")
                            .font(.caption2.weight(.semibold))
                            .foregroundStyle(s.presence ? store.accent : Theme.tertiaryText)
                            .padding(.horizontal, 8)
                            .padding(.vertical, 3)
                            .background((s.presence ? store.accent : Theme.tertiaryText).opacity(0.14), in: .capsule)
                    }
                    .padding(12)
                    .background(Theme.raised.opacity(0.6), in: .rect(cornerRadius: 14, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(Theme.hairline))
                }
                .buttonStyle(CardButtonStyle())
            }
            link("Use Another Account") { store.signOut() }
                .frame(maxWidth: .infinity)
                .padding(.top, 4)
        }
    }

    // MARK: Controls

    private func primary(_ title: String, busy: Bool = false, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            ZStack {
                Text(title).opacity(busy ? 0 : 1)
                if busy { ProgressView().tint(.black) }
            }
            .font(.headline)
            .frame(maxWidth: .infinity, minHeight: 50)
        }
        .buttonStyle(.borderedProminent)
        .buttonBorderShape(.roundedRectangle(radius: 14))
        .foregroundStyle(.black)
        .disabled(self.busy)
    }

    private func secondary(_ title: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title).font(.headline).frame(maxWidth: .infinity, minHeight: 50)
        }
        .buttonStyle(.bordered)
        .buttonBorderShape(.roundedRectangle(radius: 14))
        .disabled(busy)
    }

    private func link(_ title: String, action: @escaping () -> Void) -> some View {
        Button(title, action: action)
            .font(.subheadline)
            .foregroundStyle(Theme.secondaryText)
    }

    private func field(_ placeholder: String, text: Binding<String>, content: UITextContentType) -> some View {
        TextField(placeholder, text: text)
            .textContentType(content)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .modifier(FieldStyle())
    }

    // MARK: Flows

    private func switchMode(_ m: Mode) {
        error = nil
        mode = m
    }

    private func startPin(strong: Bool) {
        error = nil
        busy = true
        pinTask = Task {
            do {
                let pin = try await Commands.startPin(store.app, store.host, strong: strong)
                busy = false
                mode = strong ? .browser(url: pin.url) : .code(pin.code)
                // Returns once signed in (the stage changes) or when cancelled.
                try await Commands.completePin(store.app, id: pin.id, attempt: pin.attempt)
            } catch is CancellationError {
            } catch {
                self.error = errorText(error)
                mode = .choose
            }
            busy = false
        }
    }

    private func cancelPin() {
        Commands.cancelPin(store.app)
        pinTask?.cancel()
        mode = .choose
    }

    private func submit(_ f: @escaping () async throws -> Void) {
        busy = true
        error = nil
        Task {
            do {
                try await f()
            } catch {
                self.error = errorText(error)
                Haptics.error()
            }
            busy = false
        }
    }
}

/// A text field on a raised surface.
struct FieldStyle: ViewModifier {
    func body(content: Content) -> some View {
        content
            .padding(.horizontal, 14)
            .frame(minHeight: 48)
            .background(Theme.raised.opacity(0.7), in: .rect(cornerRadius: 12, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(Theme.hairline))
    }
}

#Preview("Sign in") {
    Gate()
        .environment(AppStore.preview(stage: .login))
        .preferredColorScheme(.dark)
}

#Preview("Failed") {
    Gate()
        .environment(AppStore.preview(stage: .failed(message: "connecting to Tower: the server did not answer")))
        .preferredColorScheme(.dark)
}
