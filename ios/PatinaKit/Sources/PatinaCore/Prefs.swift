// Port of desktop/core/prefs.ts.
// Automatic playback preferences: which audio and subtitle tracks and which version of a
// title to use, remembered per profile and per show.
//
// Rules are stored in `playback-prefs.json` next to the config file, scoped by account and
// server (a profile on one server never affects another). A show's rules override the
// profile's, field by field; a field a show leaves unset inherits the profile's value.
//
// When playback starts the rules are matched against the streams of the file
// (`chooseStreams`) and the versions of the title (`pickVersion`). Both are pure functions,
// so they apply to newly added episodes too: nothing is stored per episode.

import Foundation
import PlexKit

nonisolated private let log = logger("prefs")

public enum Prefs {
    // MARK: Rules

    /// When subtitles are turned on. `keep` leaves the subtitle track as Plex has it;
    /// `foreign` is only for dialogue you can't follow: forced subtitles while the audio is in
    /// one of your languages, full subtitles when it is not.
    public nonisolated enum SubtitleMode: String, Codable, Sendable, CaseIterable {
        case keep, off, always, foreign
    }

    /// Which kind of subtitle track is wanted: `auto` full subtitles (forced ones when there
    /// are no others), `prefer` forced first then full, `avoid` never a forced track, `only`
    /// only forced tracks.
    public nonisolated enum ForcedMode: String, Codable, Sendable, CaseIterable {
        case auto, prefer, avoid, only
    }

    public nonisolated enum Resolution: String, Codable, Sendable, CaseIterable {
        case any, uhd = "4k", fhd = "1080", hd = "720", sd

        /// 1 (SD) to 4 (4K); `nil` for "any".
        var rank: Int? {
            switch self {
            case .any: nil
            case .uhd: 4
            case .fhd: 3
            case .hd: 2
            case .sd: 1
            }
        }

        public var label: String {
            switch self {
            case .any: "Any"
            case .uhd: "4K"
            case .fhd: "1080p"
            case .hd: "720p"
            case .sd: "SD"
            }
        }
    }

    public nonisolated enum HdrMode: String, Codable, Sendable, CaseIterable {
        case any, prefer, avoid
    }

    /// What a profile or show asks for. Every field is optional: `nil` means "not set" (a show
    /// then inherits the profile's value). Encoded with explicit nulls, as the Rust core wrote
    /// it; decoded leniently (`#[serde(default)]`): missing or invalid fields are unset.
    public nonisolated struct Rules: Codable, Sendable, Equatable {
        /// Languages in order of preference, as ISO codes or names ("eng", "ja", "French").
        public var audioLanguages: [String]?
        /// Subtitle languages; the audio languages when empty.
        public var subtitleLanguages: [String]?
        public var subtitles: SubtitleMode?
        public var forced: ForcedMode?
        /// Skip commentary and audio-description tracks.
        public var avoidCommentary: Bool?
        public var resolution: Resolution?
        /// Preferred video codec ("hevc", "h264", "av1", ...); empty for any.
        public var codec: String?
        public var hdr: HdrMode?

        public init(
            audioLanguages: [String]? = nil, subtitleLanguages: [String]? = nil, subtitles: SubtitleMode? = nil,
            forced: ForcedMode? = nil, avoidCommentary: Bool? = nil, resolution: Resolution? = nil,
            codec: String? = nil, hdr: HdrMode? = nil
        ) {
            self.audioLanguages = audioLanguages
            self.subtitleLanguages = subtitleLanguages
            self.subtitles = subtitles
            self.forced = forced
            self.avoidCommentary = avoidCommentary
            self.resolution = resolution
            self.codec = codec
            self.hdr = hdr
        }

        enum CodingKeys: String, CodingKey {
            case audioLanguages, subtitleLanguages, subtitles, forced, avoidCommentary, resolution, codec, hdr
        }

        public init(from decoder: any Decoder) throws {
            guard let c = try? decoder.container(keyedBy: CodingKeys.self) else {
                self.init()
                return
            }
            func strings(_ k: CodingKeys) -> [String]? {
                (try? c.decodeIfPresent([JSONValue].self, forKey: k))?.compactMap(\.stringValue)
            }
            func one<T: RawRepresentable>(_ k: CodingKeys) -> T? where T.RawValue == String {
                (try? c.decodeIfPresent(String.self, forKey: k)).flatMap { $0.flatMap(T.init(rawValue:)) }
            }
            self.init(
                audioLanguages: strings(.audioLanguages), subtitleLanguages: strings(.subtitleLanguages),
                subtitles: one(.subtitles), forced: one(.forced),
                avoidCommentary: (try? c.decodeIfPresent(Bool.self, forKey: .avoidCommentary)) ?? nil,
                resolution: one(.resolution), codec: (try? c.decodeIfPresent(String.self, forKey: .codec)) ?? nil,
                hdr: one(.hdr))
        }

        public func encode(to encoder: any Encoder) throws {
            var c = encoder.container(keyedBy: CodingKeys.self)
            try c.encode(audioLanguages, forKey: .audioLanguages)
            try c.encode(subtitleLanguages, forKey: .subtitleLanguages)
            try c.encode(subtitles, forKey: .subtitles)
            try c.encode(forced, forKey: .forced)
            try c.encode(avoidCommentary, forKey: .avoidCommentary)
            try c.encode(resolution, forKey: .resolution)
            try c.encode(codec, forKey: .codec)
            try c.encode(hdr, forKey: .hdr)
        }

        /// `Rules.from(v)`: what the UI sends (absent fields) or what was stored.
        public static func from(_ v: JSONValue) -> Rules {
            guard let data = try? JSONEncoder().encode(v), let r = try? JSONDecoder().decode(Rules.self, from: data)
            else { return Rules() }
            return r
        }

        public var isEmpty: Bool {
            self == Rules()
        }

        /// These rules, with `base` filling every field this leaves unset.
        public func over(_ base: Rules) -> Rules {
            Rules(
                audioLanguages: audioLanguages ?? base.audioLanguages,
                subtitleLanguages: subtitleLanguages ?? base.subtitleLanguages, subtitles: subtitles ?? base.subtitles,
                forced: forced ?? base.forced, avoidCommentary: avoidCommentary ?? base.avoidCommentary,
                resolution: resolution ?? base.resolution, codec: codec ?? base.codec, hdr: hdr ?? base.hdr)
        }

        public func resolve() -> Resolved {
            func langs(_ l: [String]?) -> [String] {
                var out: [String] = []
                for key in (l ?? []).map(languageKey) where key != "" && !out.contains(key) { out.append(key) }
                return out
            }
            let codec = self.codec.map(codecKey)
            return Resolved(
                audio: langs(audioLanguages), subtitleLangs: langs(subtitleLanguages), subtitles: subtitles ?? .keep,
                forced: forced ?? .auto, avoidCommentary: avoidCommentary ?? false, resolution: resolution ?? .any,
                codec: codec.flatMap { $0.isEmpty ? nil : $0 }, hdr: hdr ?? .any)
        }
    }

    /// `Rules` with defaults filled in and languages normalised, ready to match.
    public nonisolated struct Resolved: Sendable, Equatable {
        public var audio: [String]
        public var subtitleLangs: [String]
        public var subtitles: SubtitleMode
        public var forced: ForcedMode
        public var avoidCommentary: Bool
        public var resolution: Resolution
        public var codec: String?
        public var hdr: HdrMode

        public var hasTrackRules: Bool {
            !audio.isEmpty || avoidCommentary || subtitles != .keep
        }

        public var hasVersionRules: Bool {
            resolution != .any || codec != nil || hdr != .any
        }

        /// Languages subtitles are wanted in.
        public var subLangs: [String] {
            subtitleLangs.isEmpty ? audio : subtitleLangs
        }

        /// Languages that need no subtitles: the audio ones, else the subtitle ones.
        public var native: [String] {
            audio.isEmpty ? subtitleLangs : audio
        }
    }

    // MARK: Languages

    /// Names a language goes by; the first is the canonical ISO 639-2 code.
    nonisolated static let LANGUAGES: [[String]] = [
        ["eng", "en", "english"],
        ["jpn", "ja", "japanese"],
        ["fra", "fre", "fr", "french"],
        ["deu", "ger", "de", "german"],
        ["spa", "es", "spanish"],
        ["ita", "it", "italian"],
        ["por", "pt", "portuguese"],
        ["rus", "ru", "russian"],
        ["zho", "chi", "zh", "chinese", "mandarin", "cantonese"],
        ["kor", "ko", "korean"],
        ["nld", "dut", "nl", "dutch"],
        ["swe", "sv", "swedish"],
        ["nor", "nob", "nb", "no", "norwegian"],
        ["dan", "da", "danish"],
        ["fin", "fi", "finnish"],
        ["pol", "pl", "polish"],
        ["tur", "tr", "turkish"],
        ["ara", "ar", "arabic"],
        ["heb", "he", "hebrew"],
        ["hin", "hi", "hindi"],
        ["tha", "th", "thai"],
        ["ces", "cze", "cs", "czech"],
        ["hun", "hu", "hungarian"],
        ["ell", "gre", "el", "greek"],
        ["ukr", "uk", "ukrainian"],
        ["ind", "id", "indonesian"],
        ["vie", "vi", "vietnamese"],
        ["ron", "rum", "ro", "romanian"],
        ["bul", "bg", "bulgarian"],
        ["hrv", "hr", "croatian"],
        ["cat", "ca", "catalan"],
        ["fas", "per", "fa", "persian"],
        ["msa", "may", "ms", "malay"],
        ["tam", "ta", "tamil"],
        ["lat", "la", "latin"],
    ]

    /// Rust `to_ascii_lowercase`: only A-Z change.
    nonisolated static func asciiLower(_ s: String) -> String {
        String(s.map { $0.isASCII && $0.isUppercase ? Character($0.lowercased()) : $0 })
    }

    /// A language as a comparable key: "en", "eng", "English" and "en-US" are all "eng".
    /// Unknown languages are compared by their lowercase name.
    public nonisolated static func languageKey(_ name: String) -> String {
        let lower = asciiLower(name.trimmingCharacters(in: .whitespacesAndNewlines))
        let base = lower.split(whereSeparator: { " -_(".contains($0) }).first.map(String.init) ?? ""
        if ["und", "zxx", "mis", "mul", "unknown"].contains(base) { return "" }
        return LANGUAGES.first { $0.contains(base) }?[0] ?? base
    }

    public nonisolated static func streamLang(_ s: MediaStream) -> String? {
        for l in [s.languageCode, s.languageTag, s.language] {
            guard let l else { continue }
            let k = languageKey(l)
            if k != "" { return k }
        }
        return nil
    }

    nonisolated static func titles(_ s: MediaStream) -> String {
        [s.title, s.displayTitle, s.extendedDisplayTitle].compactMap { $0?.lowercased() }.joined(separator: " | ")
    }

    /// Commentary and audio-description tracks, spotted by their titles.
    public nonisolated static func isCommentary(_ s: MediaStream) -> Bool {
        let t = titles(s)
        return [
            "commentary", "commentaire", "comentario", "kommentar", "audio description", "audio-description",
            "described video", "descriptive", "isolated score",
        ].contains { t.contains($0) }
    }

    /// Forced-subtitle tracks, by flag or by title.
    public nonisolated static func isForced(_ s: MediaStream) -> Bool {
        s.forced || titles(s).contains("forced")
    }

    // MARK: Choosing tracks

    /// The subtitle outcome of a rule.
    public nonisolated enum SubPick: Sendable, Equatable {
        case off
        case track(Int)
    }

    /// The streams rules ask for; `nil` leaves Plex's selection alone.
    public nonisolated struct Choice: Sendable, Equatable {
        public var audio: Int?
        public var subtitle: SubPick?

        public init(audio: Int? = nil, subtitle: SubPick? = nil) {
            self.audio = audio
            self.subtitle = subtitle
        }
    }

    /// `Iterator::min_by_key`: the first element with the smallest (lexicographic) key.
    nonisolated static func minByKey<T>(_ items: [T], _ key: (T) -> [Int]) -> T? {
        var best: (T, [Int])?
        for item in items {
            let k = key(item)
            if best == nil || k.lexicographicallyPrecedes(best!.1) { best = (item, k) }
        }
        return best?.0
    }

    nonisolated static func flag(_ b: Bool) -> Int { b ? 1 : 0 }

    /// The stream to keep: the selected one, else the default one, else the first.
    nonisolated static func best(_ list: [MediaStream]) -> MediaStream? {
        minByKey(list) { [flag(!$0.selected), flag(!$0.default), $0.index ?? $0.id ?? Int.max] }
    }

    nonisolated static func pickAudio(_ r: Resolved, _ part: Part) -> MediaStream? {
        let all = Part.audioStreams(part)
        let plain = all.filter { !(r.avoidCommentary && isCommentary($0)) }
        let pool = plain.isEmpty ? all : plain
        for lang in r.audio {
            if let s = best(pool.filter { streamLang($0) == lang }) { return s }
        }
        // No preferred language is there; at least step off a commentary track.
        if r.avoidCommentary, let current = all.first(where: \.selected), isCommentary(current), !plain.isEmpty {
            return best(plain)
        }
        return nil
    }

    /// Best subtitle track of `lang` of the wanted kind, preferring the selected one, then
    /// non-SDH tracks.
    nonisolated static func pickKind(_ subs: [MediaStream], _ lang: String, _ forced: Bool) -> MediaStream? {
        minByKey(subs.filter { streamLang($0) == lang && isForced($0) == forced }) {
            [flag(!$0.selected), flag($0.hearingImpaired), flag(!$0.default), $0.index ?? $0.id ?? Int.max]
        }
    }

    nonisolated static func pickFullSubtitle(_ r: Resolved, _ subs: [MediaStream]) -> MediaStream? {
        let plain = subs.filter { !(r.avoidCommentary && isCommentary($0)) }
        let pool = plain.isEmpty ? subs : plain
        let order: [Bool] =
            switch r.forced {
            case .auto: [false, true]
            case .prefer: [true, false]
            case .avoid: [false]
            case .only: [true]
            }
        for lang in r.subLangs {
            for forced in order {
                if let s = pickKind(pool, lang, forced) { return s }
            }
        }
        return nil
    }

    nonisolated static func track(_ s: MediaStream) -> SubPick {
        .track(s.id ?? 0)
    }

    nonisolated static func pickSubtitle(_ r: Resolved, _ part: Part, _ audio: MediaStream?) -> SubPick? {
        let subs = Part.subtitleStreams(part)
        switch r.subtitles {
        case .keep:
            return nil
        case .off:
            return .off
        case .always:
            return pickFullSubtitle(r, subs).map(track)
        case .foreign:
            let audioLang = audio.flatMap(streamLang)
            if let audioLang, r.native.contains(audioLang) {
                // Only the foreign dialogue: a forced track in a subtitle language, else nothing.
                var forced: MediaStream?
                if r.forced != .avoid {
                    for lang in r.subLangs {
                        forced = pickKind(subs, lang, true)
                        if forced != nil { break }
                    }
                }
                return forced.map(track) ?? .off
            }
            return pickFullSubtitle(r, subs).map(track)
        }
    }

    /// The audio and subtitle streams `rules` ask for in `part`. The choice may name streams
    /// that are already selected.
    public nonisolated static func chooseStreams(_ r: Resolved, _ part: Part) -> Choice {
        guard r.hasTrackRules else { return Choice() }
        let audio = pickAudio(r, part)
        // Subtitles follow the audio that will actually play.
        let playing = audio ?? Part.selectedAudio(part)
        return Choice(audio: audio?.id, subtitle: pickSubtitle(r, part, playing))
    }

    /// Select `choice` in `part`. Returns the stream ids that changed, as Plex takes them (`0`
    /// for subtitles off).
    public nonisolated static func applyChoice(_ part: inout Part, _ choice: Choice) -> (Int?, Int?) {
        var changed: (Int?, Int?) = (nil, nil)
        if let id = choice.audio, Part.audioStreams(part).contains(where: { $0.id == id && !$0.selected }) {
            for i in part.stream.indices where part.stream[i].streamType == StreamType.audio {
                part.stream[i].selected = part.stream[i].id == id
            }
            changed.0 = id
        }
        switch choice.subtitle {
        case nil:
            break
        case .off:
            if Part.selectedSubtitle(part) != nil {
                for i in part.stream.indices where part.stream[i].streamType == StreamType.subtitle {
                    part.stream[i].selected = false
                }
                changed.1 = 0
            }
        case .track(let id):
            if Part.subtitleStreams(part).contains(where: { $0.id == id && !$0.selected }) {
                for i in part.stream.indices where part.stream[i].streamType == StreamType.subtitle {
                    part.stream[i].selected = part.stream[i].id == id
                }
                changed.1 = id
            }
        }
        return changed
    }

    // MARK: Choosing versions

    nonisolated static func codecKey(_ codec: String) -> String {
        let c = asciiLower(codec.trimmingCharacters(in: .whitespacesAndNewlines))
        switch c {
        case "hevc", "h265", "h.265", "x265": return "hevc"
        case "h264", "h.264", "avc", "x264", "avc1": return "h264"
        default: return c
        }
    }

    nonisolated static func mediaResolution(_ m: Media) -> Int? {
        if let r = m.videoResolution {
            switch asciiLower(r) {
            case "4k", "2160": return 4
            case "1080": return 3
            case "720": return 2
            case "576", "480", "sd": return 1
            default: break
            }
        }
        guard let h = m.height ?? m.part.first.flatMap(Part.videoStream)?.height else { return nil }
        if h >= 2000 { return 4 }
        if h >= 1000 { return 3 }
        if h >= 700 { return 2 }
        return 1
    }

    public nonisolated static func mediaIsHdr(_ m: Media) -> Bool {
        if let video = m.part.first.flatMap(Part.videoStream), MediaStream.hdrLabel(video) != nil { return true }
        let name = [m.title, m.editionTitle].compactMap { $0?.lowercased() }.joined(separator: " ")
        return ["hdr", "dolby vision", "hlg"].contains { name.contains($0) }
            || name.split(whereSeparator: { !($0.isLetter || $0.isNumber) }).contains("dv")
    }

    /// The index of the version of a title that best fits the rules; 0 (Plex's first) when the
    /// rules have no version preference or nothing fits better.
    public nonisolated static func pickVersion(_ r: Resolved, _ versions: [Media]) -> Int {
        guard versions.count >= 2, r.hasVersionRules else { return 0 }
        let want = r.resolution.rank
        func score(_ m: Media) -> Int {
            var s = 0
            if let want {
                // Closest wins; at equal distance the higher resolution does.
                if let have = mediaResolution(m) {
                    s += abs(have - want) * 100 + flag(have < want)
                } else {
                    s += 150
                }
            }
            if let codec = r.codec, m.videoCodec.map(codecKey) != codec { s += 20 }
            let hdr = mediaIsHdr(m)
            if (r.hdr == .prefer && !hdr) || (r.hdr == .avoid && hdr) { s += 30 }
            return s
        }
        return minByKey(Array(versions.indices)) { [score(versions[$0]), $0] } ?? 0
    }

    // MARK: Storage

    /// The connected account and server.
    public static func currentScope(_ app: AppState) -> Scope? {
        app.session().map(Scope.of)
    }

    /// The scope whose rules apply to `item`: its own server's when it was found on another one.
    public static func scopeFor(_ app: AppState, _ item: Metadata) -> Scope? {
        guard var scope = currentScope(app) else { return nil }
        if let id = Metadata.serverId(item) { scope.server = id }
        return scope
    }

    /// One saved set of rules: a profile's (`show` is `nil`) or a show's.
    public nonisolated struct Entry: Codable, Sendable, Equatable {
        public var account: String
        public var server: String
        /// Rating key of the show; `nil` for the profile's rules.
        public var show: String?
        public var title: String?
        public var rules: Rules

        public init(account: String, server: String, show: String?, title: String?, rules: Rules) {
            self.account = account
            self.server = server
            self.show = show
            self.title = title
            self.rules = rules
        }

        enum CodingKeys: String, CodingKey {
            case account, server, show, title, rules
        }

        public init(from decoder: any Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            func text(_ k: CodingKeys) -> String? {
                switch try? c.decodeIfPresent(JSONValue.self, forKey: k) {
                case .string(let s): s
                case .number(let n): n == n.rounded() && abs(n) < 1e15 ? String(Int(n)) : String(n)
                default: nil
                }
            }
            account = text(.account) ?? ""
            server = text(.server) ?? ""
            show = (try? c.decodeIfPresent(String.self, forKey: .show)) ?? nil
            title = (try? c.decodeIfPresent(String.self, forKey: .title)) ?? nil
            rules = ((try? c.decodeIfPresent(Rules.self, forKey: .rules)) ?? nil) ?? Rules()
        }

        public func encode(to encoder: any Encoder) throws {
            var c = encoder.container(keyedBy: CodingKeys.self)
            try c.encode(account, forKey: .account)
            try c.encode(server, forKey: .server)
            try c.encode(show, forKey: .show)
            try c.encode(title, forKey: .title)
            try c.encode(rules, forKey: .rules)
        }
    }

    static func path(_ paths: Paths) -> URL {
        paths.config.appendingPathComponent("playback-prefs.json")
    }

    static func load(_ paths: Paths) -> [Entry] {
        do {
            return try Storage.readJson(path(paths)) { [Entry]() }
        } catch {
            log.error("playback preferences: \(errorText(error))")
            return []
        }
    }

    public nonisolated static func inScope(_ entries: [Entry], _ scope: Scope) -> [Entry] {
        entries.filter { $0.account == scope.account && $0.server == scope.server }
    }

    nonisolated static func find(_ entries: [Entry], _ scope: Scope, _ show: String?) -> Entry? {
        entries.first { $0.account == scope.account && $0.server == scope.server && $0.show == show }
    }

    /// What applies to `show` (or to the profile alone): the show's rules over the profile's.
    public nonisolated static func effective(_ entries: [Entry], _ scope: Scope, _ show: String?) -> Rules {
        let profile = find(entries, scope, nil)?.rules ?? Rules()
        guard let show, let e = find(entries, scope, show) else { return profile }
        return e.rules.over(profile)
    }

    public nonisolated static func store(
        _ entries: inout [Entry], _ scope: Scope, _ show: String?, _ title: String?, _ rules: Rules
    ) {
        entries.removeAll { $0.account == scope.account && $0.server == scope.server && $0.show == show }
        if !rules.isEmpty {
            entries.append(Entry(account: scope.account, server: scope.server, show: show, title: title, rules: rules))
        }
    }

    /// The show an item belongs to, for per-show rules.
    nonisolated static func showOf(_ item: Metadata) -> String? {
        switch Metadata.kind(item) {
        case "episode": item.grandparentRatingKey
        case "season": item.parentRatingKey
        case "show": item.ratingKey
        default: nil
        }
    }

    /// Everything a front end shows about one profile or show.
    public nonisolated struct View: Sendable, Equatable {
        /// The profile's own rules.
        public var profile: Rules
        /// The show's own rules, when a show was asked for.
        public var show: Rules?
        /// What applies: the show's rules over the profile's.
        public var effective: Rules
    }

    /// A show with rules of its own.
    public nonisolated struct ShowRules: Sendable, Equatable {
        public var ratingKey: String
        public var title: String
        public var rules: Rules
    }

    static func scope(_ app: AppState) throws -> Scope {
        guard let s = currentScope(app) else { throw CoreError("Not connected to a server.") }
        return s
    }

    public static func view(_ app: AppState, _ show: String? = nil) throws -> View {
        let sc = try scope(app)
        let entries = load(app.paths)
        return View(
            profile: find(entries, sc, nil)?.rules ?? Rules(),
            show: show.map { find(entries, sc, $0)?.rules ?? Rules() }, effective: effective(entries, sc, show))
    }

    /// The shows of this profile that have rules of their own.
    public static func shows(_ app: AppState) throws -> [ShowRules] {
        let sc = try scope(app)
        var out: [ShowRules] = []
        for e in inScope(load(app.paths), sc) {
            guard let show = e.show else { continue }
            out.append(ShowRules(ratingKey: show, title: e.title ?? "Show", rules: e.rules))
        }
        // Stable sort by lowercase title, like `sort_by_key`.
        return out.enumerated().sorted { a, b in
            let (x, y) = (a.element.title.lowercased(), b.element.title.lowercased())
            return x == y ? a.offset < b.offset : x < y
        }.map(\.element)
    }

    /// Replace the rules of the profile (`show` is `nil`) or of a show. Empty rules remove the
    /// entry, so the show follows the profile again.
    public static func save(_ app: AppState, _ show: String?, _ title: String?, _ rules: Rules) throws {
        let sc = try scope(app)
        try Storage.updateJson(path(app.paths), { [Entry]() }) { store(&$0, sc, show, title, rules) }
    }

    public static func clear(_ app: AppState, _ show: String? = nil) throws {
        try save(app, show, nil, Rules())
    }

    /// What a person chose in the whole-show track dialog, as rules: the audio language, and
    /// subtitles off, or in a language (forced or full as picked).
    public nonisolated static func rulesFromTracks(_ audio: MediaStream?, _ subtitle: Tracks.SubTemplate) -> Rules {
        var rules = Rules()
        if let audio, let lang = streamLang(audio) {
            rules.audioLanguages = [lang]
            rules.avoidCommentary = !isCommentary(audio)
        }
        switch subtitle {
        case .keep:
            break
        case .off:
            rules.subtitles = .off
        case .stream(let s):
            if let l = streamLang(s) { rules.subtitleLanguages = [l] }
            rules.subtitles = .always
            rules.forced = isForced(s) ? .only : .avoid
        }
        return rules
    }

    /// Remember the choices of the track dialog as this show's rules.
    public static func rememberTracks(_ app: AppState, _ q: Tracks.Query) throws {
        let (audio, subtitle, title) = try Tracks.chosen(app, q)
        let rules = rulesFromTracks(audio, subtitle)
        if rules.isEmpty { throw CoreError("Choose an audio or subtitle track to remember.") }
        // Keep the show's other rules (version preferences) while replacing its tracks.
        let current = try view(app, q.show).show ?? Rules()
        let merged = Rules(
            audioLanguages: rules.audioLanguages ?? current.audioLanguages,
            subtitleLanguages: rules.subtitles != nil ? rules.subtitleLanguages : current.subtitleLanguages,
            subtitles: rules.subtitles ?? current.subtitles,
            forced: rules.subtitles != nil ? rules.forced : current.forced,
            avoidCommentary: rules.avoidCommentary ?? current.avoidCommentary, resolution: current.resolution,
            codec: current.codec, hdr: current.hdr)
        try save(app, q.show, title, merged)
        app.toast("Remembered these tracks for \(title)")
    }

    // MARK: Hooks used by playback

    /// The version of `item` to play: an explicit choice (`requested`, including the first
    /// version) always stands; otherwise the pick of the rules of `scope` (the account and the
    /// server the item plays from).
    public static func mediaIndex(_ paths: Paths, _ scope: Scope?, _ item: Metadata, _ requested: Int? = nil) -> Int {
        if let requested { return requested }
        guard item.media.count >= 2, ["movie", "episode"].contains(Metadata.kind(item)), let scope else { return 0 }
        return pickVersion(effective(load(paths), scope, showOf(item)).resolve(), item.media)
    }

    /// Which version of `item` the rules prefer (for pre-selecting it in a picker).
    public static func preferredVersion(_ app: AppState, _ item: Metadata) -> Int {
        mediaIndex(app.paths, scopeFor(app, item), item, nil)
    }

    /// The track rules that apply to `item` when it starts: `nil` for anything but a movie or
    /// episode, a resumed item (it keeps the tracks it had), or no track rules.
    static func trackRules(_ paths: Paths, _ scope: Scope?, _ item: Metadata) -> Resolved? {
        guard ["movie", "episode"].contains(Metadata.kind(item)), (item.viewOffset ?? 0) <= 0, let scope else {
            return nil
        }
        let rules = effective(load(paths), scope, showOf(item)).resolve()
        return rules.hasTrackRules ? rules : nil
    }

    /// Select the rules' tracks in `item`; returns the part and the streams that changed.
    nonisolated static func chooseIn(_ rules: Resolved, _ item: inout Metadata, _ mediaIdx: Int) -> (Int, Int?, Int?)? {
        let index = mediaIdx < item.media.count ? mediaIdx : 0
        guard item.media.indices.contains(index), !item.media[index].part.isEmpty,
            let partId = item.media[index].part[0].id
        else { return nil }
        let choice = chooseStreams(rules, item.media[index].part[0])
        let (audio, subtitle) = applyChoice(&item.media[index].part[0], choice)
        return audio != nil || subtitle != nil ? (partId, audio, subtitle) : nil
    }

    /// Apply the rules to the streams of an item that is about to play: select the tracks in
    /// `item` (so the engine is given them) and save them to Plex in the background so other
    /// apps agree. A resumed item keeps the tracks it had. `scope` is the account and the
    /// server the item plays from.
    public static func applyAuto(
        _ app: AppState, _ scope: Scope?, _ server: PlexServer, _ item: inout Metadata, _ mediaIdx: Int
    ) {
        guard let rules = trackRules(app.paths, scope, item), let (partId, audio, subtitle) = chooseIn(rules, &item, mediaIdx)
        else { return }
        app.spawn {
            do {
                try await server.selectStreams(partId, audio: audio, subtitle: subtitle, allParts: false)
            } catch {
                log.debug("could not save automatic track choice: \(errorText(error))")
            }
        }
    }

    /// The same choice for an item that will be transcoded: the transcoder plays (and burns in)
    /// the server's selection, so it is saved before the transcode starts. Queue entries can
    /// lack streams; the full item is fetched then.
    public static func saveAuto(
        _ paths: Paths, _ scope: Scope?, _ server: PlexServer, _ item: Metadata, _ mediaIdx: Int
    ) async throws {
        guard let rules = trackRules(paths, scope, item) else { return }
        let media = item.media.indices.contains(mediaIdx) ? item.media[mediaIdx] : item.media.first
        let hasStreams = !(media?.part.first?.stream.isEmpty ?? true)
        var full: Metadata
        if hasStreams {
            full = item
        } else {
            guard let fetched = try await server.metadata(Metadata.ratingKey(item)) else {
                throw CoreError("\"\(Metadata.title(item))\" was not found")
            }
            full = fetched
        }
        if let (partId, audio, subtitle) = chooseIn(rules, &full, mediaIdx) {
            try await server.selectStreams(partId, audio: audio, subtitle: subtitle, allParts: false)
        }
    }
}
