import SwiftUI

/// Set audio and subtitle tracks across a show or season. The matching is
/// done by the backend; this sheet collects the choices and shows the plan.
struct TrackSheet: View {
    let item: Metadata
    @Environment(AppModel.self) private var app
    @Environment(\.dismiss) private var dismiss
    @State private var loaded = false
    @State private var error: String?
    @State private var season: Int?
    @State private var reference: String?
    @State private var audio: Int64?
    @State private var subtitle = "keep"
    @State private var view: TrackView?
    @State private var applying = false

    private var show: (String?, String) {
        switch item.kind {
        case "show": (item.ratingKey, item.name)
        case "season": (item.parentRatingKey, item.parentTitle ?? "")
        default: (item.grandparentRatingKey, item.grandparentTitle ?? "")
        }
    }

    private struct Query: Hashable { var loaded: Bool; var season: Int?; var reference: String?; var audio: Int64?; var subtitle: String }
    private var query: Query { Query(loaded: loaded, season: season, reference: reference, audio: audio, subtitle: subtitle) }

    var body: some View {
        SheetFrame(
            title: "Audio & Subtitles",
            subtitle: "\(show.1): pick tracks in one episode and Patina selects the closest match in every other.",
            width: 560, height: 540
        ) {
            if let error {
                SheetMessage(text: error, error: true)
            } else if let view {
                if view.total == 0 {
                    SheetMessage(text: "This show has no episodes.")
                } else {
                    form(view)
                }
            } else {
                SheetMessage(text: "Reading tracks from every episode…", spinner: true)
            }
        } footer: {
            Spacer()
            Button("Cancel", role: .cancel) { dismiss() }
                .keyboardShortcut(.cancelAction)
                .disabled(applying)
            Button(view.map { $0.plan.episodes > 0 ? "Apply to \(Fmt.plural($0.plan.episodes, "episode"))" : "Apply" } ?? "Apply", action: apply)
                .keyboardShortcut(.defaultAction)
                .disabled(applying || (view?.plan.changes.isEmpty ?? true))
        }
        .interactiveDismissDisabled(applying)
        .task { await load() }
        .task(id: query) { await plan() }
    }

    private func form(_ v: TrackView) -> some View {
        Form {
            Section {
                Picker("Apply to", selection: Binding(get: { season }, set: { season = $0; reference = nil; resetTracks() })) {
                    Text("All seasons (\(Fmt.plural(v.total, "episode")))").tag(Int?.none)
                    ForEach(v.seasons) { s in Text("\(s.title) (\(Fmt.plural(s.count, "episode")))").tag(Int?.some(s.index)) }
                }
                Picker("Take tracks from", selection: Binding(get: { v.reference ?? "" }, set: { reference = $0; resetTracks() })) {
                    ForEach(v.scope) { ep in Text(ep.label).tag(ep.ratingKey) }
                }
            }
            Section {
                Picker("Audio", selection: $audio) {
                    Text("Don't change").tag(Int64?.none)
                    ForEach(v.audio) { s in Text(s.label).tag(Int64?.some(s.id)) }
                }
                .disabled(v.audio.isEmpty)
                Picker("Subtitles", selection: $subtitle) {
                    Text("Don't change").tag("keep")
                    Text("None (turn off)").tag("off")
                    ForEach(v.subtitles) { s in Text(s.label + (s.external ? " · external" : "")).tag(String(s.id)) }
                }
            }
            Section { summary(v) }
        }
        .formStyle(.grouped)
        .disabled(applying)
    }

    @ViewBuilder private func summary(_ v: TrackView) -> some View {
        if applying {
            let (done, total) = app.tracksProgress ?? (0, v.plan.changes.count)
            VStack(alignment: .leading, spacing: 6) {
                ProgressView(value: Double(done), total: Double(max(1, total))).tint(app.accent)
                Text("Updating \(done) of \(total)…").font(.callout.monospacedDigit()).foregroundStyle(.secondary)
            }
        } else if v.nothingChosen {
            Text("Choose an audio or subtitle track. Matches are found by language first, then title, codec and channels.")
                .font(.callout).foregroundStyle(.secondary)
        } else if v.plan.episodes == 0 && v.plan.noAudio.isEmpty && v.plan.noSubtitle.isEmpty {
            Text("Every episode already uses these tracks.").font(.callout)
        } else {
            Text("\(v.plan.episodes) of \(Fmt.plural(v.scope.count, "episode")) will change.").font(.callout)
        }
        if !applying {
            if !v.plan.noAudio.isEmpty {
                missing(v.plan.noAudio, what: "no audio matching “\(v.audioName ?? "")”")
            }
            if !v.plan.noSubtitle.isEmpty {
                missing(v.plan.noSubtitle, what: "no subtitles matching “\(v.subtitleName ?? "")”")
            }
        }
    }

    private func missing(_ episodes: [String], what: String) -> some View {
        let n = episodes.count
        return DisclosureGroup {
            VStack(alignment: .leading, spacing: 2) {
                ForEach(episodes, id: \.self) { Text($0).font(.caption).foregroundStyle(.secondary) }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        } label: {
            Label("\(Fmt.plural(n, "episode")) \(n == 1 ? "has" : "have") \(what), left unchanged", systemImage: "exclamationmark.triangle")
                .font(.callout)
        }
    }

    private func resetTracks() {
        audio = nil
        subtitle = "keep"
    }

    private func load() async {
        switch item.kind {
        case "season": season = item.index
        case "episode": season = item.parentIndex; reference = item.ratingKey
        default: break
        }
        guard let rk = show.0 else {
            error = "This item doesn't belong to a show."
            return
        }
        do {
            try await API.tracksLoad(show: rk)
            loaded = true
        } catch {
            self.error = error.localizedDescription
        }
    }

    private func plan() async {
        guard loaded, let rk = show.0 else { return }
        do {
            let v = try await API.tracksView(TrackQuery(show: rk, season: season, reference: reference, audio: audio, subtitle: subtitle))
            if !Task.isCancelled { view = v }
        } catch is CancellationError {
        } catch {
            self.error = error.localizedDescription
        }
    }

    private func apply() {
        guard let v = view, !v.plan.changes.isEmpty else { return }
        app.tracksProgress = (0, v.plan.changes.count)
        applying = true
        Task {
            // Refresh either way: a failed run may have applied some changes.
            defer { applying = false; app.tracksProgress = nil; app.refresh() }
            do {
                try await API.tracksApply(v.plan.changes)
                dismiss()
            } catch {
                app.error(error)
                // Some changes may have landed; reload and replan so a retry only sends the rest.
                if let rk = show.0, (try? await API.tracksLoad(show: rk)) != nil { await plan() }
            }
        }
    }
}
