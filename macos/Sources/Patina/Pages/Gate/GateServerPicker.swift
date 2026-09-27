import AppKit
import SwiftUI

// MARK: Server choice

struct GateServerPicker: View {
    @Environment(AppModel.self) private var app
    let servers: [ServerView]
    let user: UserView
    @State private var picking: String?

    var body: some View {
        Text("Signed in as \(user.name). Choose a server.").foregroundStyle(.secondary)
        VStack(spacing: 0) {
            Hairline()
            ForEach(servers) { s in
                Button { pick(s) } label: { row(s) }
                    .buttonStyle(.plain)
                    .disabled(picking != nil)
                Hairline()
            }
        }
        .multilineTextAlignment(.leading)
        Button("Use Another Account") { app.perform { try await API.signOut() } }
            .buttonStyle(.link)
            .font(.callout)
    }

    private func row(_ s: ServerView) -> some View {
        HStack(spacing: 12) {
            Image(systemName: "server.rack")
                .foregroundStyle(.secondary)
                .frame(width: 18)
            VStack(alignment: .leading, spacing: 2) {
                Text(s.name).font(.body.weight(.medium))
                Text(detail(s)).font(.caption).foregroundStyle(.secondary).lineLimit(1)
            }
            Spacer(minLength: 8)
            if picking == s.id {
                ProgressView().controlSize(.small)
            } else {
                HStack(spacing: 5) {
                    Circle().fill(s.presence ? app.accent : Color.secondary.opacity(0.4)).frame(width: 6, height: 6)
                    Text(s.presence ? "Online" : "Offline?")
                }
                .font(.caption)
                .foregroundStyle(.secondary)
            }
        }
        .padding(.vertical, 10)
        .padding(.horizontal, 4)
        .contentShape(Rectangle())
    }

    private func detail(_ s: ServerView) -> String {
        var parts = [s.owned ? "Owned" : "Shared by \(s.sourceTitle ?? "someone")"]
        if let p = s.platform, !p.isEmpty { parts.append(p) }
        if let v = s.version, !v.isEmpty { parts.append("v\(v)") }
        return parts.joined(separator: " · ")
    }

    private func pick(_ s: ServerView) {
        picking = s.id
        Task {
            await app.act { try await API.pickServer(s.id) }
            picking = nil
        }
    }
}
