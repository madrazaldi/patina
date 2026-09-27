import AppKit
import SwiftUI

/// Everything before the library (web/src/pages/Gate.svelte): splash,
/// sign-in, server choice, connecting and failure. When the server can't be
/// reached, downloaded items can still be played offline.
struct GateView: View {
    @Environment(AppModel.self) private var app
    /// Showing downloads instead of the failure screen.
    @State private var offline = false

    var body: some View {
        switch app.stage {
        case .failed, .connecting:
            if offline { GateOfflineView(offline: $offline) } else { GateColumn(offline: $offline) }
        default:
            GateColumn(offline: $offline)
        }
    }
}

/// The centred column with the wordmark and whatever the stage needs.
struct GateColumn: View {
    @Environment(AppModel.self) private var app
    @Binding var offline: Bool

    var body: some View {
        VStack(spacing: 22) {
            VStack(spacing: 14) {
                Logo(height: 48)
                Text("Patina")
                    .font(Theme.display(40))
                    .tracking(-0.4)
            }
            .padding(.bottom, 4)
            content
        }
        .frame(width: 340)
        .multilineTextAlignment(.center)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(40)
        .background(Color(nsColor: .windowBackgroundColor))
        .animation(.easeOut(duration: 0.15), value: stageKey)
    }

    private var stageKey: String {
        switch app.stage {
        case .booting: "booting"
        case .login: "login"
        case .pickServer: "pick"
        case .connecting: "connecting"
        case .ready: "ready"
        case .failed: "failed"
        }
    }

    @ViewBuilder private var content: some View {
        switch app.stage {
        case .booting, .ready:
            ProgressView().controlSize(.small)
        case .connecting(let name):
            ProgressView().controlSize(.small)
            Text("Connecting to \(name)…").foregroundStyle(.secondary)
        case .failed(let message):
            GateFailedView(message: message, offline: $offline)
        case .pickServer(let servers, let user):
            GateServerPicker(servers: servers, user: user)
        case .login:
            GateLoginView()
        }
    }
}

// MARK: Failure

struct GateFailedView: View {
    @Environment(AppModel.self) private var app
    let message: String
    @Binding var offline: Bool

    var body: some View {
        GateErrorBox(text: message)
        VStack(spacing: 8) {
            Button { gateRetry(app) } label: { Text("Try Again").frame(maxWidth: .infinity) }
                .buttonStyle(.borderedProminent)
                .keyboardShortcut(.defaultAction)
            if app.downloads.contains(where: { $0.state == .done }) {
                Button { offline = true } label: { Text("Play Downloads Offline").frame(maxWidth: .infinity) }
            }
            Button { app.perform { try await API.signOut() } } label: { Text("Sign Out").frame(maxWidth: .infinity) }
        }
        .controlSize(.large)
    }
}

@MainActor func gateRetry(_ app: AppModel) {
    app.perform { try await API.retry() }
}

/// Downloads without a server, with a slim bar to get back online.
struct GateOfflineView: View {
    @Environment(AppModel.self) private var app
    @Binding var offline: Bool

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                Logo(height: 18)
                Text("Patina").font(Theme.display(17))
                if case .connecting(let name) = app.stage {
                    Text("Connecting to \(name)…").foregroundStyle(.tertiary)
                } else {
                    Text("Offline").foregroundStyle(.tertiary)
                }
                Spacer()
                if case .connecting = app.stage {
                    ProgressView().controlSize(.small)
                } else {
                    Button("Try Again") { gateRetry(app) }
                    Button("Back") { offline = false }
                }
            }
            .padding(.horizontal, Theme.gutter)
            .frame(height: 44)
            Hairline()
            NavigationStack { DownloadsPage() }
            if app.nowPlaying != nil || app.queue != nil { NowPlayingBar() }
        }
    }
}

/// A quiet, left-aligned error message.
struct GateErrorBox: View {
    let text: String

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Image(systemName: "exclamationmark.circle").foregroundStyle(.red)
            Text(text).textSelection(.enabled).fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
        }
        .font(.callout)
        .multilineTextAlignment(.leading)
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.red.opacity(0.06), in: RoundedRectangle(cornerRadius: 6))
        .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(Color.red.opacity(0.18)))
    }
}
