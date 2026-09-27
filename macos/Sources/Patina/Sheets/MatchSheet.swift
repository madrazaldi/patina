import SwiftUI

/// "Fix Match": search the agent's matches by title and year, apply one, or unmatch.
struct MatchSheet: View {
    let item: Metadata
    @Environment(AppModel.self) private var app
    @Environment(\.dismiss) private var dismiss
    @State private var title = ""
    @State private var year = ""
    @State private var phase: Phase<[MatchResult]> = .loading
    @State private var generation = 0
    @State private var busy = false
    @State private var confirmUnmatch = false

    private var square: Bool { ["artist", "album"].contains(item.kind) }

    var body: some View {
        SheetFrame(title: "Fix Match", subtitle: item.fullTitle, width: 560, height: 560) {
            VStack(spacing: 0) {
                HStack(spacing: 8) {
                    TextField("Title", text: $title)
                        .onSubmit { search(manual: true) }
                    TextField("Year", text: $year)
                        .frame(width: 70)
                        .onSubmit { search(manual: true) }
                    Button("Search") { search(manual: true) }
                        .disabled(title.trimmingCharacters(in: .whitespaces).isEmpty)
                }
                .textFieldStyle(.roundedBorder)
                .padding(.horizontal, 20)
                .padding(.bottom, 12)
                Hairline()
                results
            }
        } footer: {
            Button("Unmatch…", role: .destructive) { confirmUnmatch = true }
                .disabled(busy)
            Spacer()
            if busy { ProgressView().controlSize(.small) }
            Button("Cancel", role: .cancel) { dismiss() }
                .keyboardShortcut(.cancelAction)
                .disabled(busy)
        }
        .interactiveDismissDisabled(busy)
        .confirmationDialog("Unmatch this item?", isPresented: $confirmUnmatch) {
            Button("Unmatch", role: .destructive, action: unmatch)
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Its metadata and artwork are cleared until it's matched again.")
        }
        .task {
            title = item.name
            year = item.year.map(String.init) ?? ""
            search(manual: false)
        }
    }

    @ViewBuilder private var results: some View {
        switch phase {
        case .loading: SheetMessage(text: "", spinner: true)
        case .failed(let e): SheetMessage(text: e, error: true)
        case .loaded(let list) where list.isEmpty:
            SheetMessage(text: "No matches. Try a different title or year.")
        case .loaded(let list):
            List(list) { r in
                Button { apply(r) } label: { row(r) }
                    .buttonStyle(.plain)
                    .disabled(busy || r.guid == nil)
            }
            .listStyle(.inset)
        }
    }

    private func row(_ r: MatchResult) -> some View {
        HStack(alignment: .top, spacing: 10) {
            let w: CGFloat = 40, h: CGFloat = square ? 40 : 60
            Artwork(r.thumb, width: Int(w * 2), height: Int(h * 2))
                .frame(width: w, height: h)
                .clipShape(RoundedRectangle(cornerRadius: Theme.radius))
            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 6) {
                    Text(r.name ?? "").fontWeight(.medium)
                    if let y = r.year { Text("(\(String(y)))").foregroundStyle(.secondary) }
                    if r.matched {
                        Text("Current")
                            .font(.caption2.weight(.semibold))
                            .foregroundStyle(app.accent)
                    }
                }
                .lineLimit(1)
                if let s = r.summary, !s.isEmpty {
                    Text(s).font(.caption).foregroundStyle(.secondary).lineLimit(2)
                }
            }
            Spacer(minLength: 8)
            if let score = r.score {
                Text("\(Int(score))%").font(.caption.monospacedDigit()).foregroundStyle(.secondary)
            }
        }
        .padding(.vertical, 3)
        .contentShape(Rectangle())
    }

    private func search(manual: Bool) {
        generation += 1
        let run = generation
        phase = .loading
        let q = manual ? title.trimmingCharacters(in: .whitespaces) : ""
        let y = manual ? Int(year.trimmingCharacters(in: .whitespaces)) : nil
        let rk = item.ratingKey ?? ""
        Task {
            do {
                let r = try await API.matches(rk, title: q.isEmpty ? nil : q, year: y)
                if run == generation { phase = .loaded(r) }
            } catch {
                if run == generation { phase = .failed(error.localizedDescription) }
            }
        }
    }

    private func apply(_ r: MatchResult) {
        guard let guid = r.guid, !busy else { return }
        busy = true
        let rk = item.ratingKey ?? ""
        Task {
            let ok = await app.act("Matched to “\(r.name ?? "")”", refresh: true) {
                try await API.applyMatch(rk, guid: guid, name: r.name)
            }
            busy = false
            if ok { dismiss() }
        }
    }

    private func unmatch() {
        busy = true
        let rk = item.ratingKey ?? ""
        Task {
            let ok = await app.act("Unmatched", refresh: true) { try await API.unmatch(rk) }
            busy = false
            if ok { dismiss() }
        }
    }
}
