import SwiftUI

/// Online subtitle search for a movie or episode, plus the external subtitles it already has.
struct SubtitleSheet: View {
    let item: Metadata
    @Environment(AppModel.self) private var app
    @Environment(\.dismiss) private var dismiss
    @State private var language = "en"
    @State private var hearingImpaired = false
    @State private var forced = false
    @State private var phase: Phase<[Stream]> = .loading
    @State private var generation = 0
    @State private var busy: String?
    @State private var existing: [Stream] = []
    @State private var deleting: Stream?
    @State private var ready = false

    /// [ISO 639-1, ISO 639-2 (as Plex reports stream languages), name]
    static let languages: [(String, String, String)] = [
        ("ar", "ara", "Arabic"), ("bg", "bul", "Bulgarian"), ("zh", "chi", "Chinese"), ("hr", "hrv", "Croatian"),
        ("cs", "cze", "Czech"), ("da", "dan", "Danish"), ("nl", "dut", "Dutch"), ("en", "eng", "English"),
        ("et", "est", "Estonian"), ("fi", "fin", "Finnish"), ("fr", "fre", "French"), ("de", "ger", "German"),
        ("el", "gre", "Greek"), ("he", "heb", "Hebrew"), ("hi", "hin", "Hindi"), ("hu", "hun", "Hungarian"),
        ("is", "ice", "Icelandic"), ("id", "ind", "Indonesian"), ("it", "ita", "Italian"), ("ja", "jpn", "Japanese"),
        ("ko", "kor", "Korean"), ("lv", "lav", "Latvian"), ("lt", "lit", "Lithuanian"), ("ms", "may", "Malay"),
        ("no", "nor", "Norwegian"), ("fa", "per", "Persian"), ("pl", "pol", "Polish"), ("pt", "por", "Portuguese"),
        ("ro", "rum", "Romanian"), ("ru", "rus", "Russian"), ("sr", "srp", "Serbian"), ("sk", "slo", "Slovak"),
        ("sl", "slv", "Slovenian"), ("es", "spa", "Spanish"), ("sv", "swe", "Swedish"), ("th", "tha", "Thai"),
        ("tr", "tur", "Turkish"), ("uk", "ukr", "Ukrainian"), ("vi", "vie", "Vietnamese"),
    ]
    /// ISO 639-2/T variants Plex sometimes uses.
    static let aliases = ["zho": "zh", "ces": "cs", "nld": "nl", "fra": "fr", "deu": "de", "ell": "el", "isl": "is", "msa": "ms", "fas": "fa", "ron": "ro", "slk": "sk"]

    static func iso1(_ code: String?) -> String? {
        guard let c = code?.lowercased() else { return nil }
        return languages.first { $0.0 == c || $0.1 == c }?.0 ?? aliases[c]
    }

    var body: some View {
        SheetFrame(title: "Subtitles", subtitle: item.fullTitle, width: 600, height: 580) {
            VStack(spacing: 0) {
                if !existing.isEmpty { existingSection }
                HStack(spacing: 12) {
                    Picker("Language", selection: $language) {
                        ForEach(Self.languages, id: \.0) { l in Text(l.2).tag(l.0) }
                    }
                    .frame(width: 220)
                    Toggle("Hearing impaired", isOn: $hearingImpaired)
                    Toggle("Forced only", isOn: $forced)
                    Spacer()
                }
                .toggleStyle(.checkbox)
                .padding(.horizontal, 20)
                .padding(.bottom, 10)
                Hairline()
                results
            }
        } footer: {
            Spacer()
            Button("Done") { dismiss() }
                .keyboardShortcut(.cancelAction)
                .disabled(busy != nil)
        }
        .interactiveDismissDisabled(busy != nil)
        .onChange(of: language) { if ready { search() } }
        .onChange(of: hearingImpaired) { if ready { search() } }
        .onChange(of: forced) { if ready { search() } }
        .confirmationDialog(
            "Delete “\(deleting?.label ?? "")”?",
            isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } }),
            presenting: deleting
        ) { s in
            Button("Delete", role: .destructive) { delete(s) }
            Button("Cancel", role: .cancel) {}
        } message: { _ in
            Text("The subtitle file is removed from the server.")
        }
        .task { await start() }
    }

    private var existingSection: some View {
        VStack(alignment: .leading, spacing: 6) {
            Eyebrow("External subtitles")
            ForEach(existing, id: \.id) { s in
                HStack {
                    Image(systemName: "captions.bubble").foregroundStyle(.secondary)
                    Text(s.label).lineLimit(1)
                    if let c = s.codec { Text(c.uppercased()).font(.caption).foregroundStyle(.secondary) }
                    Spacer()
                    Button { deleting = s } label: { Image(systemName: "trash") }
                        .buttonStyle(.borderless)
                        .help("Delete this subtitle")
                        .disabled(s.streamID == nil)
                }
                .font(.callout)
            }
        }
        .padding(.horizontal, 20)
        .padding(.bottom, 12)
    }

    @ViewBuilder private var results: some View {
        switch phase {
        case .loading: SheetMessage(text: "Searching subtitle providers…", spinner: true)
        case .failed(let e): SheetMessage(text: e, error: true)
        case .loaded(let list) where list.isEmpty: SheetMessage(text: "Nothing found for this language.")
        case .loaded(let list):
            List(Array(list.enumerated()), id: \.offset) { _, s in row(s) }
                .listStyle(.inset)
        }
    }

    private func row(_ s: Stream) -> some View {
        HStack(spacing: 10) {
            VStack(alignment: .leading, spacing: 2) {
                Text(s.displayTitle ?? s.title ?? "Subtitle").lineLimit(1).help(s.displayTitle ?? s.title ?? "")
                let sub = [s.providerTitle, s.title != nil && s.title != s.displayTitle ? s.title : nil].compactMap { $0 }.joined(separator: " · ")
                if !sub.isEmpty { Text(sub).font(.caption).foregroundStyle(.secondary).lineLimit(1) }
            }
            Spacer(minLength: 8)
            HStack(spacing: 6) {
                if s.perfectMatch { badge("Perfect match", accent: true) }
                if s.hearingImpaired { badge("SDH") }
                if s.forced { badge("Forced") }
                if let score = s.score { Text("\(Int(score))").font(.caption.monospacedDigit()).foregroundStyle(.secondary) }
            }
            if s.downloaded {
                Text("Downloaded").font(.caption).foregroundStyle(.secondary)
            } else if busy != nil && busy == s.key {
                ProgressView().controlSize(.small)
            } else {
                Button("Download") { fetch(s) }
                    .controlSize(.small)
                    .disabled(s.key == nil || busy != nil)
            }
        }
        .padding(.vertical, 2)
    }

    private func badge(_ text: String, accent: Bool = false) -> some View {
        Text(text)
            .font(.caption2.weight(.medium))
            .foregroundStyle(accent ? AnyShapeStyle(app.accent) : AnyShapeStyle(.secondary))
            .padding(.horizontal, 5).padding(.vertical, 1)
            .overlay(RoundedRectangle(cornerRadius: 3).strokeBorder(Theme.hairline))
    }

    private func start() async {
        var part = item.media.first?.parts.first
        if let full = try? await API.metadata(item.ratingKey ?? "") { part = full.media.first?.parts.first ?? part }
        existing = part?.subtitleStreams.filter(\.isExternal) ?? []
        if let part {
            language = Self.iso1(part.selectedSubtitle?.languageCode) ?? Self.iso1(part.selectedAudio?.languageCode) ?? "en"
        }
        ready = true
        search()
    }

    private func reloadExisting() async {
        guard let full = try? await API.metadata(item.ratingKey ?? "") else { return }
        existing = full.media.first?.parts.first?.subtitleStreams.filter(\.isExternal) ?? []
    }

    private func search() {
        generation += 1
        let run = generation
        phase = .loading
        let rk = item.ratingKey ?? "", lang = language, hi = hearingImpaired, fo = forced
        Task {
            do {
                let r = try await API.searchSubtitles(rk, language: lang, hearingImpaired: hi, forced: fo)
                if run == generation { phase = .loaded(r) }
            } catch {
                if run == generation { phase = .failed(error.localizedDescription) }
            }
        }
    }

    private func fetch(_ s: Stream) {
        guard let key = s.key, busy == nil else { return }
        busy = key
        let rk = item.ratingKey ?? ""
        Task {
            let ok = await app.act("Subtitle downloaded", refresh: true) { try await API.downloadSubtitle(rk, key: key) }
            busy = nil
            if ok { dismiss() }
        }
    }

    private func delete(_ s: Stream) {
        guard let id = s.streamID else { return }
        Task {
            if await app.act("Subtitle deleted", refresh: true, { try await API.deleteStream(id) }) {
                await reloadExisting()
            }
        }
    }
}
