// Port of web/src/pages/Pick.svelte: "Pick something to watch", a few random candidates that
// fit what you feel like. The first is shown big with Play; the others follow as cards. The
// criteria are in a sheet and kept for the session, so coming back doesn't start over.

import PatinaCore
import PlexKit
import SwiftUI

struct PickScreen: View {
    /// The criteria as the user left them.
    struct Form: Equatable {
        /// `movie`, `show` or `any` (`Ops.Pick.Kind`).
        var kind = "movie"
        var section = ""
        var unwatched = true
        var maxMinutes: Double?
        var minRating: Double?
        var yearMin = ""
        var yearMax = ""
        var genres: [String] = []
        var contentRatings: [String] = []
    }

    static let runtimes: [Double] = [30, 60, 90, 120, 150, 180]
    static let ratings: [Double] = [5, 6, 7, 8]
    private static var remembered: (scope: String, form: Form)?

    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var options = Loader<Ops.Pick.Options>()
    @State private var form = Form()
    @State private var result: Ops.Pick.Result?
    @State private var error: String?
    @State private var busy = false
    @State private var editing = false
    /// Rating keys already offered for these criteria, so a reroll shows something new.
    @State private var seen: [String] = []
    @State private var token = 0
    @State private var restored = false

    private var query: Ops.Pick.Query {
        func year(_ v: String) -> Int? { Int(v.trimmingCharacters(in: .whitespaces)).flatMap { $0 > 0 ? $0 : nil } }
        return Ops.Pick.Query(
            kind: Ops.Pick.Kind(rawValue: form.kind) ?? .movie, unwatched: form.unwatched, maxMinutes: form.maxMinutes, genres: form.genres,
            contentRatings: form.contentRatings, sections: form.section.isEmpty ? [] : [form.section],
            yearMin: year(form.yearMin), yearMax: year(form.yearMax), minRating: form.minRating, count: 4)
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                Text("A few random candidates that fit what you feel like.")
                    .font(.subheadline).foregroundStyle(Theme.secondaryText)
                criteria
                if let error {
                    StatusView(error: error) { roll(again: false) }
                } else if let result {
                    if result.items.isEmpty {
                        StatusView(
                            title: "Nothing fits", text: "Loosen a filter, such as the runtime, rating or years.",
                            systemImage: "dice")
                    } else {
                        found(result)
                    }
                } else {
                    Skeleton().frame(height: 320).clipShape(.rect(cornerRadius: 18))
                }
            }
            .padding(.horizontal, Theme.margin(sizeClass))
            .padding(.bottom, 24)
            .opacity(busy && result != nil ? 0.6 : 1)
            .animation(.easeOut(duration: 0.2), value: busy)
        }
        .navigationTitle("Pick Something")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button("Options", systemImage: "slider.horizontal.3") { editing = true }
            }
        }
        .sheet(isPresented: $editing) {
            PickOptionsSheet(form: $form, options: options.value)
                .presentationDetents([.medium, .large])
        }
        .task {
            if !restored, let r = Self.remembered, r.scope == store.scope { form = r.form }
            restored = true
            await options.load { try await Commands.pickOptions(store.app) }
        }
        // New criteria start a fresh draw, shortly after the last change.
        .task(id: form) {
            guard restored else { return }
            Self.remembered = (store.scope, form)
            try? await Task.sleep(for: .milliseconds(250))
            if !Task.isCancelled { roll(again: false) }
        }
        .onChange(of: options.value?.sections.map(\.id)) { dropStaleSection() }
        .onChange(of: form.kind) { dropStaleSection() }
    }

    /// A library of the other kind no longer applies once the kind changes.
    private func dropStaleSection() {
        guard !form.section.isEmpty, let sections = options.value?.sections else { return }
        if !sections.contains(where: { $0.id == form.section && (form.kind == "any" || $0.kind == form.kind) }) {
            form.section = ""
        }
    }

    private func roll(again: Bool) {
        token += 1
        let mine = token
        var q = query
        q.exclude = again ? seen : []
        let scope = store.scope
        busy = true
        error = nil
        Task {
            do {
                let r = try await Commands.pickSomething(store.app, query: q)
                guard mine == token, scope == store.scope else { return }
                withAnimation(.spring(duration: 0.4)) { result = r }
                let keys = r.items.map { $0.ratingKey ?? "" }
                seen = again && !r.wrapped ? seen + keys : keys
                if again { Haptics.selection() }
            } catch {
                if mine == token, scope == store.scope { self.error = errorText(error) }
            }
            if mine == token { busy = false }
        }
    }

    /// The criteria in effect, as chips that open the options.
    private var criteria: some View {
        let f = form
        var parts = [f.kind == "movie" ? "Movies" : f.kind == "show" ? "Shows" : "Movies and shows"]
        if let s = options.value?.sections.first(where: { $0.id == f.section }) { parts.append(s.title) }
        if f.unwatched { parts.append("Unwatched") }
        if let m = f.maxMinutes { parts.append("Up to \(Int(m)) min") }
        if let r = f.minRating { parts.append("\(Int(r)).0+") }
        if !f.yearMin.isEmpty || !f.yearMax.isEmpty {
            parts.append("\(f.yearMin.isEmpty ? "…" : f.yearMin)–\(f.yearMax.isEmpty ? "…" : f.yearMax)")
        }
        parts += f.genres + f.contentRatings
        return Button {
            editing = true
        } label: {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) {
                    ForEach(parts, id: \.self) { p in
                        Text(p).font(.subheadline.weight(.medium))
                            .padding(.horizontal, 12).padding(.vertical, 6)
                            .background(Theme.raised, in: .capsule)
                    }
                }
            }
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Options: \(parts.joined(separator: ", "))")
    }

    @ViewBuilder
    private func found(_ r: Ops.Pick.Result) -> some View {
        let first = r.items[0]
        PickHero(item: first)
            .id(first.ratingKey)
            .transition(.asymmetric(insertion: .scale(scale: 0.96).combined(with: .opacity), removal: .opacity))
        HStack(spacing: 10) {
            Button {
                store.play(first)
            } label: {
                Label("Play", systemImage: "play.fill").frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .foregroundStyle(.black)
            .disabled(!PlexUI.canPlay(first))
            Button {
                roll(again: true)
            } label: {
                Label("Reroll", systemImage: "dice").frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .disabled(busy)
            .keyboardShortcut("r", modifiers: [])
        }
        .controlSize(.large)
        Group {
            if r.wrapped {
                Text("That was everything that fits (\(r.matches)); starting over.")
            } else {
                Text(
                    "\(r.matches) \(r.matches == 1 ? "title fits" : "titles fit")\(r.remaining > 0 ? ", \(r.remaining) not offered yet" : "")\(r.libraries.isEmpty ? "" : " · \(r.libraries.joined(separator: ", "))")"
                )
            }
        }
        .font(.footnote).foregroundStyle(Theme.secondaryText)
        if r.items.count > 1 {
            Text("Also Fits").font(.title3.weight(.semibold)).padding(.top, 4)
            LazyVGrid(columns: cardColumns(.poster, sizeClass), spacing: 22) {
                ForEach(r.items.dropFirst(), id: \.ratingKey) { Card($0) }
            }
        }
    }
}

/// The first candidate, big: backdrop, poster, title, facts and summary. Tapping opens it.
private struct PickHero: View {
    let item: Metadata
    @Environment(AppStore.self) private var store

    var body: some View {
        let m = item
        let facts = [
            m.year.map(String.init), m.duration.map { Format.duration(ms: Int64($0)) }, m.contentRating,
            m.audienceRating.map { "★ \(String(format: "%.1f", $0))" },
        ].compactMap { $0 }
        Button {
            store.navigate(to: PlexUI.route(m))
        } label: {
            ZStack(alignment: .bottomLeading) {
                Artwork(m.art ?? m.thumb, width: 1280, height: 720)
                    .frame(maxWidth: .infinity)
                    .frame(height: 340)
                    .overlay(
                        LinearGradient(
                            colors: [.clear, .black.opacity(0.55), .black.opacity(0.9)], startPoint: .top,
                            endPoint: .bottom))
                HStack(alignment: .bottom, spacing: 14) {
                    Artwork(Metadata.poster(m), width: 300, height: 450)
                        .frame(width: 96, height: 144)
                        .clipShape(.rect(cornerRadius: Theme.cornerRadius, style: .continuous))
                        .shadow(radius: 10)
                    VStack(alignment: .leading, spacing: 4) {
                        Text(Metadata.title(m)).font(.title2.weight(.bold)).lineLimit(2)
                        if !facts.isEmpty {
                            Text(facts.joined(separator: " · ")).font(.caption).foregroundStyle(Theme.secondaryText)
                        }
                        if !m.genre.isEmpty {
                            Text(m.genre.prefix(3).compactMap(\.tag).joined(separator: ", "))
                                .font(.caption).foregroundStyle(Theme.secondaryText)
                        }
                        if let s = m.summary, !s.isEmpty {
                            Text(s).font(.footnote).lineLimit(3).padding(.top, 2)
                        }
                    }
                    .multilineTextAlignment(.leading)
                }
                .padding(16)
            }
            .clipShape(.rect(cornerRadius: 18, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 18, style: .continuous).strokeBorder(Theme.hairline))
        }
        .buttonStyle(CardButtonStyle())
        .contextMenu { ItemMenu(item: m) }
    }
}

/// What to pick from: kind, library, unwatched, runtime, rating, years, genres and ratings.
private struct PickOptionsSheet: View {
    @Binding var form: PickScreen.Form
    let options: Ops.Pick.Options?
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Picker("Looking For", selection: $form.kind) {
                        Text("Movies").tag("movie")
                        Text("Shows").tag("show")
                        Text("Movies and Shows").tag("any")
                    }
                    Picker("Library", selection: $form.section) {
                        Text("Any Library").tag("")
                        ForEach((options?.sections ?? []).filter { form.kind == "any" || $0.kind == form.kind }, id: \.id) {
                            Text($0.title).tag($0.id)
                        }
                    }
                    Toggle("Only Unwatched", isOn: $form.unwatched)
                    Picker("Runtime", selection: $form.maxMinutes) {
                        Text("Any Length").tag(Double?.none)
                        ForEach(PickScreen.runtimes, id: \.self) { Text("Up to \(Int($0)) minutes").tag(Optional($0)) }
                    }
                    Picker("Audience Rating", selection: $form.minRating) {
                        Text("Any Rating").tag(Double?.none)
                        ForEach(PickScreen.ratings, id: \.self) { Text("\(Int($0)).0 or higher").tag(Optional($0)) }
                    }
                    LabeledContent("From Year") {
                        TextField("Any", text: $form.yearMin).keyboardType(.numberPad).multilineTextAlignment(.trailing)
                    }
                    LabeledContent("To Year") {
                        TextField("Any", text: $form.yearMax).keyboardType(.numberPad).multilineTextAlignment(.trailing)
                    }
                }
                if let genres = options?.genres, !genres.isEmpty {
                    Section("Genres, Any Of") { ChipPicker(values: genres, selection: $form.genres) }
                }
                if let ratings = options?.contentRatings, !ratings.isEmpty {
                    Section("Content Rating, Any Of") {
                        ChipPicker(values: ratings, selection: $form.contentRatings)
                    }
                }
                Section {
                    Button("Reset", role: .destructive) { form = PickScreen.Form() }
                }
            }
            .navigationTitle("What to Watch")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
    }
}

/// Toggle buttons that wrap, for a multiple choice.
private struct ChipPicker: View {
    let values: [String]
    @Binding var selection: [String]
    @Environment(AppStore.self) private var store

    var body: some View {
        FlowLayout(spacing: 6) {
            ForEach(values, id: \.self) { v in
                let on = selection.contains(v)
                Button {
                    Haptics.selection()
                    selection = on ? selection.filter { $0 != v } : selection + [v]
                } label: {
                    Text(v).font(.subheadline.weight(on ? .semibold : .regular))
                        .foregroundStyle(on ? Color.black : .primary)
                        .padding(.horizontal, 12).padding(.vertical, 6)
                        .background(on ? store.accent : Theme.raised, in: .capsule)
                }
                .buttonStyle(.plain)
                .accessibilityAddTraits(on ? .isSelected : [])
            }
        }
        .padding(.vertical, 4)
    }
}
