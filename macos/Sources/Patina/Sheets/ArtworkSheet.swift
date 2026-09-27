import SwiftUI

/// Choose a poster or background from the server's candidates.
struct ArtworkSheet: View {
    let item: Metadata
    /// "poster" or "art".
    let kind: String
    @Environment(AppModel.self) private var app
    @Environment(\.dismiss) private var dismiss
    @State private var phase: Phase<[ArtworkChoice]> = .loading
    @State private var busy: String?

    private var art: Bool { kind == "art" }
    private var square: Bool { !art && ["artist", "album"].contains(item.kind) }
    private var cell: CGFloat { art ? 200 : 118 }
    private var ratio: CGFloat { art ? 9.0 / 16.0 : square ? 1 : 1.5 }

    var body: some View {
        SheetFrame(title: art ? "Choose Background" : "Choose Poster", subtitle: item.fullTitle, width: 700, height: 560) {
            switch phase {
            case .loading: SheetMessage(text: "", spinner: true)
            case .failed(let e): SheetMessage(text: e, error: true)
            case .loaded(let choices) where choices.isEmpty:
                SheetMessage(text: "No \(art ? "backgrounds" : "posters") available.")
            case .loaded(let choices):
                ScrollView {
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: cell, maximum: cell + 40), spacing: 14)], spacing: 16) {
                        ForEach(choices) { choice in tile(choice) }
                    }
                    .padding(20)
                }
            }
        } footer: {
            Text("Click a picture to use it.").font(.caption).foregroundStyle(.secondary)
            Spacer()
            Button("Cancel", role: .cancel) { dismiss() }
                .keyboardShortcut(.cancelAction)
                .disabled(busy != nil)
        }
        .interactiveDismissDisabled(busy != nil)
        .task {
            do { phase = .loaded(try await API.artwork(item.ratingKey ?? "", kind: kind)) } catch { phase = .failed(error.localizedDescription) }
        }
    }

    private func tile(_ a: ArtworkChoice) -> some View {
        let url = a.key ?? a.ratingKey
        let w = Int(cell * 2), h = Int(cell * ratio * 2)
        return Button { pick(a) } label: {
            VStack(alignment: .leading, spacing: 5) {
                Artwork(a.thumb ?? a.key, width: w, height: h)
                    .aspectRatio(1 / ratio, contentMode: .fit)
                    .clipShape(RoundedRectangle(cornerRadius: Theme.radius))
                    .overlay {
                        RoundedRectangle(cornerRadius: Theme.radius)
                            .strokeBorder(a.selected ? app.accent : Theme.hairline, lineWidth: a.selected ? 2 : 1)
                    }
                    .overlay {
                        if busy != nil && busy == url {
                            ZStack { Color.black.opacity(0.35); ProgressView().controlSize(.small) }
                                .clipShape(RoundedRectangle(cornerRadius: Theme.radius))
                        }
                    }
                HStack(spacing: 4) {
                    if a.selected { Image(systemName: "checkmark").foregroundStyle(app.accent) }
                    Text(a.selected ? "Current" : (a.provider ?? "")).lineLimit(1)
                }
                .font(.caption)
                .foregroundStyle(.secondary)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(busy != nil || url == nil)
        .help(a.provider ?? "")
    }

    private func pick(_ a: ArtworkChoice) {
        guard let url = a.key ?? a.ratingKey, busy == nil else { return }
        busy = url
        let rk = item.ratingKey ?? "", kind = kind
        Task {
            let ok = await app.act(art ? "Background updated" : "Poster updated", refresh: true) {
                try await API.setArtwork(rk, kind: kind, url: url)
            }
            busy = nil
            if ok { dismiss() }
        }
    }
}
