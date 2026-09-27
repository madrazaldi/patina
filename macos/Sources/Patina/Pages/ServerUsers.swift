import SwiftUI

// MARK: Users & devices

struct ServerUsersTab: View {
    let data: UsersDevices

    var body: some View {
        Form {
            Section("Accounts") {
                ForEach(data.accounts.filter { ($0.accountID ?? 0) > 0 }) { a in
                    LabeledContent {
                        EmptyView()
                    } label: {
                        Text(a.name ?? "")
                        Text("Audio: \(a.defaultAudioLanguage ?? "auto") · Subtitles: \(a.defaultSubtitleLanguage ?? "auto")")
                    }
                }
            }
            Section("Friends & Shared Users") {
                if data.friends.isEmpty {
                    Text("This server isn't shared with anyone.").foregroundStyle(.secondary)
                }
                ForEach(Array(data.friends.enumerated()), id: \.offset) { _, f in
                    let name = [f.title, f.username, f.email].compactMap { $0 }.first { !$0.isEmpty } ?? "User"
                    let detail = [f.username != name ? f.username : nil, f.email].compactMap { $0 }.joined(separator: " · ")
                    HStack(spacing: 10) {
                        Avatar(path: f.thumb, name: name, size: 28)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(name)
                            if !detail.isEmpty { Text(detail).font(Theme.caption).foregroundStyle(.secondary).textSelection(.enabled) }
                        }
                        Spacer()
                        if f.home { ServerTag("Home") }
                        if f.restricted { ServerTag("Restricted") }
                        if let s = f.status, s != "accepted" { ServerTag(s.capitalized) }
                    }
                }
            }
            Section("Players on the Network") {
                if data.clients.isEmpty {
                    Text("No players are advertising on the local network right now.").foregroundStyle(.secondary)
                }
                ForEach(Array(data.clients.enumerated()), id: \.offset) { _, c in
                    LabeledContent {
                        Text(c.address ?? "").textSelection(.enabled).monospacedDigit()
                    } label: {
                        Text(c.name ?? "")
                        if let p = c.product { Text(p) }
                    }
                }
            }
            Section("Devices That Have Connected") {
                if data.devices.isEmpty {
                    Text("No devices have connected yet.").foregroundStyle(.secondary)
                }
                ForEach(Array(data.devices.reversed().enumerated()), id: \.offset) { _, d in
                    LabeledContent {
                        Text(d.createdAt.map { "First seen \(Fmt.date($0))" } ?? "").foregroundStyle(.secondary)
                    } label: {
                        Text(d.name ?? "")
                        if let p = d.platform, !p.isEmpty { Text(p) }
                    }
                }
            }
        }
        .formStyle(.grouped)
        .scrollContentBackground(.hidden)
    }
}
