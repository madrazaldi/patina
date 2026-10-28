// Port of desktop/core/tracks.ts.
// Set audio and subtitle tracks for a whole show or season at once (like PASTA): pick tracks
// in one episode and the closest match is selected in every other episode.
//
// The front end loads a show once (`load`), then asks for a fresh `View` whenever a choice
// changes, and finally `apply`s the planned changes.

import Foundation
import PlexKit

public enum Tracks {
    public nonisolated enum SubChoice: Sendable, Equatable {
        case keep
        case off
        case track(Int)
    }

    public nonisolated static func parseSubChoice(_ v: String) -> SubChoice {
        if v == "off" { return .off }
        // Rust `str::parse::<i64>`: an optional sign and digits only.
        if v.wholeMatch(of: /[+-]?\d+/) != nil, let id = Int(v) { return .track(id) }
        return .keep
    }

    public nonisolated struct PartChange: Sendable, Equatable {
        public var partId: Int
        public var audio: Int?
        public var subtitle: Int?

        public init(partId: Int, audio: Int?, subtitle: Int?) {
            self.partId = partId
            self.audio = audio
            self.subtitle = subtitle
        }
    }

    /// What applying a choice would do across the episodes in scope.
    public nonisolated struct Plan: Sendable, Equatable {
        public var changes: [PartChange] = []
        /// Episodes (with changes) that would be touched.
        public var episodes = 0
        public var noAudio: [String] = []
        public var noSubtitle: [String] = []
    }

    nonisolated static func episodeLabel(_ ep: Metadata) -> String {
        "\(Metadata.episodeCode(ep)) · \(Metadata.title(ep))"
    }

    /// The subtitle choice of a query: `keep` leaves subtitles alone, `off` turns them off, a
    /// stream is the template to match (`Stream | null | undefined` in TypeScript).
    public nonisolated enum SubTemplate: Sendable, Equatable {
        case keep
        case off
        case stream(MediaStream)
    }

    nonisolated static func planPart(_ part: Part, _ audio: MediaStream?, _ subtitle: SubTemplate) -> (
        Int?, Int?, Bool, Bool
    ) {
        var a: Int?
        var s: Int?
        var missA = false
        var missS = false
        if let audio {
            if let m = Part.matchingStream(part, audio) {
                if !m.selected { a = m.id }
            } else {
                missA = true
            }
        }
        switch subtitle {
        case .keep:
            break
        case .off:
            // Turn subtitles off.
            if Part.selectedSubtitle(part) != nil { s = 0 }
        case .stream(let tpl):
            if let m = Part.matchingStream(part, tpl) {
                if !m.selected { s = m.id }
            } else {
                missS = true
            }
        }
        return (a, s, missA, missS)
    }

    public nonisolated static func plan(_ episodes: [Metadata], _ audio: MediaStream?, _ subtitle: SubTemplate) -> Plan {
        var out = Plan()
        for ep in episodes {
            var touched = false
            var missA = false
            var missS = false
            for part in ep.media.flatMap(\.part) {
                guard let partId = part.id else { continue }
                let (a, s, ma, ms) = planPart(part, audio, subtitle)
                missA = missA || ma
                missS = missS || ms
                if a != nil || s != nil {
                    touched = true
                    out.changes.append(PartChange(partId: partId, audio: a, subtitle: s))
                }
            }
            if touched { out.episodes += 1 }
            if missA { out.noAudio.append(episodeLabel(ep)) }
            if missS { out.noSubtitle.append(episodeLabel(ep)) }
        }
        return out
    }

    /// Load every episode of a show (with streams) for the dialog.
    public static func load(_ app: AppState, _ show: String) async throws {
        let episodes = try await app.server().allLeavesWithStreams(show)
        app.tracks = (show, episodes)
    }

    /// The dialog's current choices.
    public nonisolated struct Query: Sendable, Equatable {
        public var show: String
        /// Season index; `nil` means every season.
        public var season: Int?
        /// Episode whose tracks are offered; defaults to the first in scope.
        public var reference: String?
        public var audio: Int?
        /// "keep", "off" or a stream id.
        public var subtitle: String

        public init(show: String, season: Int? = nil, reference: String? = nil, audio: Int? = nil, subtitle: String) {
            self.show = show
            self.season = season
            self.reference = reference
            self.audio = audio
            self.subtitle = subtitle
        }
    }

    public nonisolated struct Season: Sendable, Equatable {
        public var index: Int
        public var title: String
        public var count: Int
    }

    public nonisolated struct Episode: Sendable, Equatable {
        public var ratingKey: String
        public var label: String
    }

    public nonisolated struct Track: Sendable, Equatable {
        public var id: Int
        public var label: String
        public var external: Bool
    }

    nonisolated static func trackOf(_ s: MediaStream) -> Track {
        Track(id: s.id ?? 0, label: MediaStream.label(s), external: MediaStream.isExternal(s))
    }

    /// Everything the dialog renders for a query.
    public nonisolated struct View: Sendable, Equatable {
        public var total: Int
        public var seasons: [Season]
        public var scope: [Episode]
        public var reference: String?
        public var audio: [Track]
        public var subtitles: [Track]
        public var audioName: String?
        public var subtitleName: String?
        public var nothingChosen: Bool
        public var plan: Plan
    }

    /// The episodes in scope for a query, the reference episode, its tracks and the templates
    /// the choices name.
    nonisolated struct Chosen {
        var scope: [Metadata]
        var reference: Metadata?
        var audio: [MediaStream]
        var subs: [MediaStream]
        var audioTpl: MediaStream?
        var subTpl: SubTemplate
    }

    nonisolated static func choose(_ all: [Metadata], _ q: Query) -> Chosen {
        let scope = all.filter { q.season == nil || $0.parentIndex == q.season }
        let reference = q.reference.flatMap { r in scope.first { Metadata.ratingKey($0) == r } } ?? scope.first
        let refPart = reference?.media.first?.part.first
        let audio = refPart.map(Part.audioStreams) ?? []
        let subs = refPart.map(Part.subtitleStreams) ?? []
        let audioTpl = q.audio.flatMap { id in audio.first { $0.id == id } }
        let subTpl: SubTemplate =
            switch parseSubChoice(q.subtitle) {
            case .keep: .keep
            case .off: .off
            // A track id that is not offered leaves the choice unset, like `.find(..).map(..)` yielding None.
            case .track(let id): subs.first { $0.id == id }.map(SubTemplate.stream) ?? .keep
            }
        return Chosen(scope: scope, reference: reference, audio: audio, subs: subs, audioTpl: audioTpl, subTpl: subTpl)
    }

    static func loaded(_ app: AppState, _ show: String) throws -> [Metadata] {
        if let (loadedShow, episodes) = app.tracks, loadedShow == show { return episodes }
        throw CoreError("The episode list is not loaded.")
    }

    /// The streams a query names, and the show's title; what the dialog's "remember for this
    /// show" turns into playback rules.
    public static func chosen(_ app: AppState, _ q: Query) throws -> (MediaStream?, SubTemplate, String) {
        let all = try loaded(app, q.show)
        let c = choose(all, q)
        return (c.audioTpl, c.subTpl, all.first?.grandparentTitle ?? "")
    }

    public static func view(_ app: AppState, _ q: Query) throws -> View {
        let all = try loaded(app, q.show)

        // Seasons in library order, with episode counts.
        var seasons: [Season] = []
        for ep in all {
            let idx = ep.parentIndex ?? 0
            if let i = seasons.firstIndex(where: { $0.index == idx }) {
                seasons[i].count += 1
            } else {
                seasons.append(Season(index: idx, title: ep.parentTitle ?? "Season \(idx)", count: 1))
            }
        }
        let c = choose(all, q)
        let subtitleName: String? = if case .stream(let s) = c.subTpl { MediaStream.shortLabel(s) } else { nil }
        return View(
            total: all.count, seasons: seasons,
            scope: c.scope.map { Episode(ratingKey: Metadata.ratingKey($0), label: episodeLabel($0)) },
            reference: c.reference.map(Metadata.ratingKey), audio: c.audio.map(trackOf), subtitles: c.subs.map(trackOf),
            audioName: c.audioTpl.map(MediaStream.shortLabel), subtitleName: subtitleName,
            nothingChosen: c.audioTpl == nil && c.subTpl == .keep, plan: plan(c.scope, c.audioTpl, c.subTpl))
    }

    /// Save the changes to Plex a few at a time, reporting `(done, total)` as they land.
    public static func apply(_ app: AppState, _ changes: [PartChange], _ progress: (Int, Int) -> Void) async throws {
        let server = try app.server()
        let total = changes.count
        progress(0, total)
        var done = 0
        var failed = 0
        var firstError: String?
        try await Ops.Util.eachCompleted(
            changes, 6,
            { c -> String? in
                do {
                    try await server.selectStreams(c.partId, audio: c.audio, subtitle: c.subtitle, allParts: false)
                    return nil
                } catch {
                    return errorText(error)
                }
            },
            { r, _ in
                done += 1
                if let r {
                    failed += 1
                    if firstError == nil { firstError = r }
                }
                progress(done, total)
            })
        app.tracks = nil
        app.bump()
        guard let firstError else {
            app.toast("Updated tracks for \(total) file\(total == 1 ? "" : "s")")
            return
        }
        throw CoreError("\(failed) of \(total) updates failed: \(firstError)")
    }
}
