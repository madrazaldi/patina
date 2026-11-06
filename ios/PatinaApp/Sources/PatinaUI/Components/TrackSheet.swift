// Port of web/src/components/TrackDialog.svelte: set audio and subtitle tracks for many
// episodes at once. The matching is done in the core (`Tracks`); this sheet only collects
// the choices, shows the plan, and applies it with progress.

import PatinaCore
import PlexKit
import SwiftUI

struct TrackSheet: View {
    let item: Metadata

    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var loaded = false
    @State private var error: String?
    @State private var season: Int?
    @State private var reference: String?
    @State private var audio: Int?
    @State private var subtitle = "keep"
    /// Also save the choices as this show's playback preferences.
    @State private var remember = false
    @State private var view: Tracks.View?
    @State private var progress: (Int, Int)?
    /// The account and server the sheet opened on: writes for any other one are dropped.
    @State private var scope = ""

    private var busy: Bool { progress != nil }

    /// The show's rating key and title.
    private var show: (String?, String) {
        switch Metadata.kind(item) {
        case "show": (item.ratingKey, Metadata.title(item))
        case "season": (item.parentRatingKey, item.parentTitle ?? "")
        default: (item.grandparentRatingKey, item.grandparentTitle ?? "")
        }
    }

    private var query: Tracks.Query? {
        show.0.map { Tracks.Query(show: $0, season: season, reference: reference, audio: audio, subtitle: subtitle) }
    }

    private var canApply: Bool {
        guard let view else { return false }
        return !view.plan.changes.isEmpty || (remember && !view.nothingChosen)
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Text("\(show.1): pick tracks in one episode and Patina selects the closest match in every other.")
                        .foregroundStyle(Theme.secondaryText)
                }
                if let error {
                    Section { Text(error).foregroundStyle(Theme.danger) }
                } else if let view {
                    if view.total == 0 {
                        Section { Text("This show has no episodes.").foregroundStyle(Theme.secondaryText) }
                    } else {
                        choices(view)
                        summary(view)
                    }
                } else {
                    Section {
                        HStack(spacing: 10) {
                            ProgressView()
                            Text("Reading tracks from every episode…").foregroundStyle(Theme.secondaryText)
                        }
                    }
                }
            }
            .disabled(busy)
            .navigationTitle("Audio & Subtitles")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() }.disabled(busy) }
                ToolbarItem(placement: .confirmationAction) {
                    Button(applyTitle) { apply() }.disabled(busy || !canApply)
                }
            }
        }
        .interactiveDismissDisabled(busy)
        // Load the show's episodes when the sheet opens.
        .task {
            scope = store.scope
            let k = Metadata.kind(item)
            season = k == "season" ? item.index : k == "episode" ? item.parentIndex : nil
            reference = k == "episode" ? item.ratingKey : nil
            guard let rk = show.0 else {
                error = "This item doesn't belong to a show."
                return
            }
            do {
                try await Commands.tracksLoad(store.app, show: rk)
                loaded = true
            } catch {
                self.error = errorText(error)
            }
        }
        // Re-plan whenever a choice changes.
        .onChange(of: query, initial: true) { replan() }
        .onChange(of: loaded) { replan() }
    }

    private var applyTitle: String {
        if let view, view.plan.episodes > 0 { return "Apply to \(Format.plural(view.plan.episodes, "Episode"))" }
        return remember && canApply ? "Remember" : "Apply"
    }

    private func replan() {
        guard loaded, let query, scope == store.scope else { return }
        do { view = try Commands.tracksView(store.app, query: query) } catch { self.error = errorText(error) }
    }

    private func resetTracks() {
        audio = nil
        subtitle = "keep"
    }

    @ViewBuilder
    private func choices(_ view: Tracks.View) -> some View {
        Section {
            Picker(
                "Apply To",
                selection: Binding(
                    get: { season },
                    set: {
                        season = $0
                        reference = nil
                        resetTracks()
                    })
            ) {
                Text("All Seasons (\(Format.plural(view.total, "episode")))").tag(Int?.none)
                ForEach(view.seasons, id: \.index) { s in
                    Text("\(s.title) (\(Format.plural(s.count, "episode")))").tag(Optional(s.index))
                }
            }
            Picker(
                "Take Tracks From",
                selection: Binding(
                    get: { view.reference ?? "" },
                    set: {
                        reference = $0
                        resetTracks()
                    })
            ) {
                ForEach(view.scope, id: \.ratingKey) { ep in Text(ep.label).tag(ep.ratingKey) }
            }
            Picker("Audio", selection: $audio) {
                Text("Don't Change").tag(Int?.none)
                ForEach(view.audio, id: \.id) { s in Text(s.label).tag(Optional(s.id)) }
            }
            .disabled(view.audio.isEmpty)
            Picker("Subtitles", selection: $subtitle) {
                Text("Don't Change").tag("keep")
                Text("None (Turn Off)").tag("off")
                ForEach(view.subtitles, id: \.id) { s in
                    Text("\(s.label)\(s.external ? " · external" : "")").tag(String(s.id))
                }
            }
        }
        Section {
            Toggle("Remember for This Show", isOn: $remember).disabled(view.nothingChosen)
        } footer: {
            Text("Use these languages for new episodes too, and whenever the show starts playing.")
        }
    }

    @ViewBuilder
    private func summary(_ view: Tracks.View) -> some View {
        Section {
            if let (done, total) = progress {
                ProgressView(value: Double(done), total: Double(max(1, total))).tint(store.accent)
                Text("Updating \(done) of \(total)…").foregroundStyle(Theme.secondaryText)
            } else if view.nothingChosen {
                Text(
                    "Choose an audio or subtitle track. Matches are found by language first, then title, codec and channels."
                )
                .foregroundStyle(Theme.secondaryText)
            } else if view.plan.episodes == 0 && view.plan.noAudio.isEmpty && view.plan.noSubtitle.isEmpty {
                Text("Every episode already uses these tracks.").foregroundStyle(Theme.secondaryText)
            } else {
                Text("\(view.plan.episodes) of \(Format.plural(view.scope.count, "episode")) will change.")
            }
            if !busy && !view.plan.noAudio.isEmpty {
                misses(view.plan.noAudio, "no audio matching “\(view.audioName ?? "")”")
            }
            if !busy && !view.plan.noSubtitle.isEmpty {
                misses(view.plan.noSubtitle, "no subtitles matching “\(view.subtitleName ?? "")”")
            }
        }
    }

    private func misses(_ episodes: [String], _ what: String) -> some View {
        DisclosureGroup {
            ForEach(episodes, id: \.self) { Text($0).font(.caption) }
        } label: {
            Label(
                "\(Format.plural(episodes.count, "episode")) \(episodes.count == 1 ? "has" : "have") \(what), left unchanged",
                systemImage: "exclamationmark.triangle"
            )
            .font(.footnote)
            .foregroundStyle(Theme.danger)
        }
    }

    private func apply() {
        guard let view, let query else { return }
        guard scope == store.scope else { return dismiss() }
        let changes = view.plan.changes
        progress = (0, changes.count)
        Task {
            do {
                // Before applying: applying drops the loaded tracks the rules are read from.
                if remember { try Commands.tracksRemember(store.app, query: query) }
                if !changes.isEmpty {
                    try await Commands.tracksApply(store.app, changes: changes) { done, total in progress = (done, total) }
                }
                Haptics.success()
            } catch {
                store.error(error)
            }
            progress = nil
            store.refresh()
            dismiss()
        }
    }
}
