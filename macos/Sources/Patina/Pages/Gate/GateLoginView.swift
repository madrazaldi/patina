import AppKit
import SwiftUI

// MARK: Sign-in

enum GateLoginMode: Equatable {
    case choose
    case browser(url: String)
    case code(String)
    case password
    case manual
}

struct GateLoginView: View {
    @Environment(AppModel.self) private var app
    @State private var mode: GateLoginMode = .choose
    @State private var busy = false
    @State private var error: String?
    /// Bumped on every PIN start or cancel, so a stale wait can't touch the screen.
    @State private var attempt = 0

    @State private var login = ""
    @State private var password = ""
    @State private var code = ""
    @State private var url = "http://localhost:32400"
    @State private var token = ""
    @FocusState private var focus: Field?

    private enum Field { case login, url }

    var body: some View {
        Group {
            switch mode {
            case .choose: choose
            case .browser(let link): browser(link)
            case .code(let pin): linkCode(pin)
            case .password: passwordForm
            case .manual: manualForm
            }
        }
        if let error { GateErrorBox(text: error) }
    }

    // Choosing a method

    private var choose: some View {
        VStack(spacing: 22) {
            Text("A Plex client that plays everything in mpv, with your own mpv configuration.")
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            VStack(spacing: 8) {
                Button { startPin(strong: true) } label: { Text("Sign In with Plex").frame(maxWidth: .infinity) }
                    .buttonStyle(.borderedProminent)
                    .keyboardShortcut(.defaultAction)
                Button { startPin(strong: false) } label: { Text("Use a Link Code").frame(maxWidth: .infinity) }
            }
            .controlSize(.large)
            .disabled(busy)
            HStack(spacing: 18) {
                Button("Email and Password") { go(.password) }
                Button("Connect to a Server Directly") { go(.manual) }
            }
            .buttonStyle(.link)
            .font(.callout)
        }
    }

    // plex.tv in the browser

    private func browser(_ link: String) -> some View {
        VStack(spacing: 18) {
            ProgressView().controlSize(.small)
            Text("Approve Patina in the browser window that just opened.")
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            HStack(spacing: 18) {
                Button("Open It Again") { app.perform { try await API.openUrl(link) } }
                Button("Cancel") { cancel() }
                    .keyboardShortcut(.cancelAction)
            }
            .buttonStyle(.link)
            .font(.callout)
        }
    }

    // plex.tv/link code

    private func linkCode(_ pin: String) -> some View {
        VStack(spacing: 14) {
            HStack(spacing: 4) {
                Text("Go to").foregroundStyle(.secondary)
                Button("plex.tv/link") { app.perform { try await API.openUrl("https://plex.tv/link") } }
                    .buttonStyle(.link)
                Text("on any device and enter").foregroundStyle(.secondary)
            }
            Text(pin)
                .font(.system(size: 40, weight: .regular, design: .monospaced))
                .tracking(8)
                .textSelection(.enabled)
                .padding(.vertical, 4)
            HStack(spacing: 18) {
                Button("Copy Code") {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(pin, forType: .string)
                    app.toast("Code copied")
                }
                Button("Cancel") { cancel() }
                    .keyboardShortcut(.cancelAction)
            }
            .buttonStyle(.link)
            .font(.callout)
            HStack(spacing: 6) {
                ProgressView().controlSize(.mini)
                Text("Waiting for approval…")
            }
            .font(.caption)
            .foregroundStyle(.tertiary)
        }
    }

    // Email and password

    private var passwordForm: some View {
        VStack(spacing: 18) {
            VStack(spacing: 8) {
                TextField("Email or username", text: $login)
                    .textContentType(.username)
                    .focused($focus, equals: .login)
                SecureField("Password", text: $password)
                    .textContentType(.password)
                TextField("Two-factor code (if enabled)", text: $code)
                    .textContentType(.oneTimeCode)
            }
            .textFieldStyle(.roundedBorder)
            .multilineTextAlignment(.leading)
            .controlSize(.large)
            .onSubmit(signIn)
            Button(action: signIn) {
                HStack(spacing: 6) {
                    if busy { ProgressView().controlSize(.small) }
                    Text("Sign In")
                }
                .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .keyboardShortcut(.defaultAction)
            .disabled(busy || login.isEmpty || password.isEmpty)
            backButton
        }
        .onAppear { focus = .login }
    }

    private func signIn() {
        guard !busy, !login.isEmpty, !password.isEmpty else { return }
        let (l, p, c) = (login, password, code.trimmingCharacters(in: .whitespaces))
        submit { try await API.passwordSignIn(login: l, password: p, code: c.isEmpty ? nil : c) }
    }

    // Direct server connection

    private var manualForm: some View {
        VStack(spacing: 18) {
            Text("Enter the server address and an X-Plex-Token. Watchlist and Plex Home need a plex.tv sign-in.")
                .font(.callout)
                .foregroundStyle(.tertiary)
                .fixedSize(horizontal: false, vertical: true)
            VStack(spacing: 8) {
                TextField("http://192.168.1.10:32400", text: $url)
                    .textContentType(.URL)
                    .focused($focus, equals: .url)
                SecureField("X-Plex-Token", text: $token)
            }
            .textFieldStyle(.roundedBorder)
            .multilineTextAlignment(.leading)
            .controlSize(.large)
            .onSubmit(connect)
            Button(action: connect) {
                HStack(spacing: 6) {
                    if busy { ProgressView().controlSize(.small) }
                    Text("Connect")
                }
                .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .keyboardShortcut(.defaultAction)
            .disabled(!canConnect)
            backButton
        }
        .onAppear { focus = .url }
    }

    private var canConnect: Bool {
        !busy && !token.trimmingCharacters(in: .whitespaces).isEmpty && !url.trimmingCharacters(in: .whitespaces).isEmpty
    }

    private func connect() {
        guard canConnect else { return }
        let (u, t) = (url, token)
        submit { try await API.signInManual(url: u, token: t) }
    }

    private var backButton: some View {
        Button("Back") { go(.choose) }
            .buttonStyle(.link)
            .font(.callout)
            .keyboardShortcut(.cancelAction)
            .disabled(busy)
    }

    // Actions

    private func go(_ m: GateLoginMode) {
        error = nil
        mode = m
    }

    private func startPin(strong: Bool) {
        error = nil
        busy = true
        attempt += 1
        let mine = attempt
        Task {
            do {
                let pin = try await API.startPin(strong: strong)
                guard mine == attempt else { return }
                busy = false
                mode = strong ? .browser(url: pin.url) : .code(pin.code)
                // Returns once signed in (the stage changes) or when cancelled.
                try await API.completePin(id: pin.id, attempt: pin.attempt)
            } catch {
                guard mine == attempt else { return }
                self.error = error.localizedDescription
                mode = .choose
            }
            if mine == attempt { busy = false }
        }
    }

    private func cancel() {
        attempt += 1
        busy = false
        mode = .choose
        Task { try? await API.cancelPin() }
    }

    private func submit(_ work: @escaping () async throws -> Void) {
        busy = true
        error = nil
        Task {
            do { try await work() } catch { self.error = error.localizedDescription }
            busy = false
        }
    }
}
