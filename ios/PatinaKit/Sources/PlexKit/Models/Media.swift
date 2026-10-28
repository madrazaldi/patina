// Port of desktop/plex/models/media.ts.
// Media, parts and streams (port of crates/plex/src/models/media.rs).

import Foundation

/// One version of an item (a file or set of files).
public struct Media: PlexModel {
    public var id: Int? = nil
    public var duration: Int? = nil
    public var bitrate: Int? = nil
    public var width: Int? = nil
    public var height: Int? = nil
    public var aspectRatio: Double? = nil
    public var audioChannels: Int? = nil
    public var audioCodec: String? = nil
    public var videoCodec: String? = nil
    public var videoResolution: String? = nil
    public var container: String? = nil
    public var videoFrameRate: String? = nil
    public var videoProfile: String? = nil
    public var audioProfile: String? = nil
    public var title: String? = nil
    public var editionTitle: String? = nil
    public var optimizedForStreaming: Bool = false
    public var has64bitOffsets: Bool = false
    public var selected: Bool = false
    public var part: [Part] = []

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        id: Int? = nil,
        duration: Int? = nil,
        bitrate: Int? = nil,
        width: Int? = nil,
        height: Int? = nil,
        aspectRatio: Double? = nil,
        audioChannels: Int? = nil,
        audioCodec: String? = nil,
        videoCodec: String? = nil,
        videoResolution: String? = nil,
        container: String? = nil,
        videoFrameRate: String? = nil,
        videoProfile: String? = nil,
        audioProfile: String? = nil,
        title: String? = nil,
        editionTitle: String? = nil,
        optimizedForStreaming: Bool = false,
        has64bitOffsets: Bool = false,
        selected: Bool = false,
        part: [Part] = []
    ) {
        self.id = id
        self.duration = duration
        self.bitrate = bitrate
        self.width = width
        self.height = height
        self.aspectRatio = aspectRatio
        self.audioChannels = audioChannels
        self.audioCodec = audioCodec
        self.videoCodec = videoCodec
        self.videoResolution = videoResolution
        self.container = container
        self.videoFrameRate = videoFrameRate
        self.videoProfile = videoProfile
        self.audioProfile = audioProfile
        self.title = title
        self.editionTitle = editionTitle
        self.optimizedForStreaming = optimizedForStreaming
        self.has64bitOffsets = has64bitOffsets
        self.selected = selected
        self.part = part
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Media")
        id = De.optI64(o["id"])
        duration = De.optI64(o["duration"])
        bitrate = De.optI64(o["bitrate"])
        width = De.optI64(o["width"])
        height = De.optI64(o["height"])
        aspectRatio = De.optF64(o["aspectRatio"])
        audioChannels = De.optI64(o["audioChannels"])
        audioCodec = De.optString(o["audioCodec"])
        videoCodec = De.optString(o["videoCodec"])
        videoResolution = De.optString(o["videoResolution"])
        container = De.optString(o["container"])
        videoFrameRate = De.optString(o["videoFrameRate"])
        videoProfile = De.optString(o["videoProfile"])
        audioProfile = De.optString(o["audioProfile"])
        title = De.optString(o["title"])
        editionTitle = De.optString(o["editionTitle"])
        optimizedForStreaming = De.boolean(o["optimizedForStreaming"])
        has64bitOffsets = De.boolean(o["has64bitOffsets"])
        selected = De.boolean(o["selected"])
        part = try De.list(o["Part"], Part.init(json:), key: "Part")
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["id"] = JSONValue(id)
        o["duration"] = JSONValue(duration)
        o["bitrate"] = JSONValue(bitrate)
        o["width"] = JSONValue(width)
        o["height"] = JSONValue(height)
        o["aspectRatio"] = JSONValue(aspectRatio)
        o["audioChannels"] = JSONValue(audioChannels)
        o["audioCodec"] = JSONValue(audioCodec)
        o["videoCodec"] = JSONValue(videoCodec)
        o["videoResolution"] = JSONValue(videoResolution)
        o["container"] = JSONValue(container)
        o["videoFrameRate"] = JSONValue(videoFrameRate)
        o["videoProfile"] = JSONValue(videoProfile)
        o["audioProfile"] = JSONValue(audioProfile)
        o["title"] = JSONValue(title)
        o["editionTitle"] = JSONValue(editionTitle)
        o["optimizedForStreaming"] = JSONValue(optimizedForStreaming)
        o["has64bitOffsets"] = JSONValue(has64bitOffsets)
        o["selected"] = JSONValue(selected)
        o["Part"] = .array(part.map(\.json))
        return .object(o)
    }
}

/// A single file of a [`Media`].
public struct Part: PlexModel {
    public var id: Int? = nil
    public var key: String? = nil
    public var duration: Int? = nil
    public var file: String? = nil
    public var size: Int? = nil
    public var container: String? = nil
    public var videoProfile: String? = nil
    public var audioProfile: String? = nil
    public var indexes: String? = nil
    public var decision: String? = nil
    public var hasThumbnail: Bool = false
    public var selected: Bool = false
    public var stream: [MediaStream] = []

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        id: Int? = nil,
        key: String? = nil,
        duration: Int? = nil,
        file: String? = nil,
        size: Int? = nil,
        container: String? = nil,
        videoProfile: String? = nil,
        audioProfile: String? = nil,
        indexes: String? = nil,
        decision: String? = nil,
        hasThumbnail: Bool = false,
        selected: Bool = false,
        stream: [MediaStream] = []
    ) {
        self.id = id
        self.key = key
        self.duration = duration
        self.file = file
        self.size = size
        self.container = container
        self.videoProfile = videoProfile
        self.audioProfile = audioProfile
        self.indexes = indexes
        self.decision = decision
        self.hasThumbnail = hasThumbnail
        self.selected = selected
        self.stream = stream
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Part")
        id = De.optI64(o["id"])
        key = De.optString(o["key"])
        duration = De.optI64(o["duration"])
        file = De.optString(o["file"])
        size = De.optI64(o["size"])
        container = De.optString(o["container"])
        videoProfile = De.optString(o["videoProfile"])
        audioProfile = De.optString(o["audioProfile"])
        indexes = De.optString(o["indexes"])
        decision = De.optString(o["decision"])
        hasThumbnail = De.boolean(o["hasThumbnail"])
        selected = De.boolean(o["selected"])
        stream = try De.list(o["Stream"], MediaStream.init(json:), key: "Stream")
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["id"] = JSONValue(id)
        o["key"] = JSONValue(key)
        o["duration"] = JSONValue(duration)
        o["file"] = JSONValue(file)
        o["size"] = JSONValue(size)
        o["container"] = JSONValue(container)
        o["videoProfile"] = JSONValue(videoProfile)
        o["audioProfile"] = JSONValue(audioProfile)
        o["indexes"] = JSONValue(indexes)
        o["decision"] = JSONValue(decision)
        o["hasThumbnail"] = JSONValue(hasThumbnail)
        o["selected"] = JSONValue(selected)
        o["Stream"] = .array(stream.map(\.json))
        return .object(o)
    }
}

/// A video, audio, subtitle or lyrics stream inside a part.
public struct MediaStream: PlexModel {
    public var id: Int? = nil
    public var streamType: Int? = nil
    public var `default`: Bool = false
    public var selected: Bool = false
    public var forced: Bool = false
    public var hearingImpaired: Bool = false
    public var codec: String? = nil
    /// Container stream index (matches mpv's `ff-index`). Absent for sidecar files.
    public var index: Int? = nil
    public var bitrate: Int? = nil
    public var language: String? = nil
    public var languageCode: String? = nil
    public var languageTag: String? = nil
    public var title: String? = nil
    public var displayTitle: String? = nil
    public var extendedDisplayTitle: String? = nil
    /// Download path for sidecar subtitles.
    public var key: String? = nil
    public var format: String? = nil
    public var channels: Int? = nil
    public var samplingRate: Int? = nil
    public var audioChannelLayout: String? = nil
    public var profile: String? = nil
    public var width: Int? = nil
    public var height: Int? = nil
    public var frameRate: Double? = nil
    public var bitDepth: Int? = nil
    public var colorPrimaries: String? = nil
    public var colorTrc: String? = nil
    public var colorSpace: String? = nil
    public var doviPresent: Bool = false
    public var doviProfile: Int? = nil
    public var decision: String? = nil
    public var location: String? = nil
    public var providerTitle: String? = nil
    public var score: Int? = nil
    public var perfectMatch: Bool = false
    public var downloaded: Bool = false

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        id: Int? = nil,
        streamType: Int? = nil,
        default: Bool = false,
        selected: Bool = false,
        forced: Bool = false,
        hearingImpaired: Bool = false,
        codec: String? = nil,
        index: Int? = nil,
        bitrate: Int? = nil,
        language: String? = nil,
        languageCode: String? = nil,
        languageTag: String? = nil,
        title: String? = nil,
        displayTitle: String? = nil,
        extendedDisplayTitle: String? = nil,
        key: String? = nil,
        format: String? = nil,
        channels: Int? = nil,
        samplingRate: Int? = nil,
        audioChannelLayout: String? = nil,
        profile: String? = nil,
        width: Int? = nil,
        height: Int? = nil,
        frameRate: Double? = nil,
        bitDepth: Int? = nil,
        colorPrimaries: String? = nil,
        colorTrc: String? = nil,
        colorSpace: String? = nil,
        doviPresent: Bool = false,
        doviProfile: Int? = nil,
        decision: String? = nil,
        location: String? = nil,
        providerTitle: String? = nil,
        score: Int? = nil,
        perfectMatch: Bool = false,
        downloaded: Bool = false
    ) {
        self.id = id
        self.streamType = streamType
        self.`default` = `default`
        self.selected = selected
        self.forced = forced
        self.hearingImpaired = hearingImpaired
        self.codec = codec
        self.index = index
        self.bitrate = bitrate
        self.language = language
        self.languageCode = languageCode
        self.languageTag = languageTag
        self.title = title
        self.displayTitle = displayTitle
        self.extendedDisplayTitle = extendedDisplayTitle
        self.key = key
        self.format = format
        self.channels = channels
        self.samplingRate = samplingRate
        self.audioChannelLayout = audioChannelLayout
        self.profile = profile
        self.width = width
        self.height = height
        self.frameRate = frameRate
        self.bitDepth = bitDepth
        self.colorPrimaries = colorPrimaries
        self.colorTrc = colorTrc
        self.colorSpace = colorSpace
        self.doviPresent = doviPresent
        self.doviProfile = doviProfile
        self.decision = decision
        self.location = location
        self.providerTitle = providerTitle
        self.score = score
        self.perfectMatch = perfectMatch
        self.downloaded = downloaded
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Stream")
        id = De.optI64(o["id"])
        streamType = De.optI64(o["streamType"])
        `default` = De.boolean(o["default"])
        selected = De.boolean(o["selected"])
        forced = De.boolean(o["forced"])
        hearingImpaired = De.boolean(o["hearingImpaired"])
        codec = De.optString(o["codec"])
        index = De.optI64(o["index"])
        bitrate = De.optI64(o["bitrate"])
        language = De.optString(o["language"])
        languageCode = De.optString(o["languageCode"])
        languageTag = De.optString(o["languageTag"])
        title = De.optString(o["title"])
        displayTitle = De.optString(o["displayTitle"])
        extendedDisplayTitle = De.optString(o["extendedDisplayTitle"])
        key = De.optString(o["key"])
        format = De.optString(o["format"])
        channels = De.optI64(o["channels"])
        samplingRate = De.optI64(o["samplingRate"])
        audioChannelLayout = De.optString(o["audioChannelLayout"])
        profile = De.optString(o["profile"])
        width = De.optI64(o["width"])
        height = De.optI64(o["height"])
        frameRate = De.optF64(o["frameRate"])
        bitDepth = De.optI64(o["bitDepth"])
        colorPrimaries = De.optString(o["colorPrimaries"])
        colorTrc = De.optString(o["colorTrc"])
        colorSpace = De.optString(o["colorSpace"])
        doviPresent = De.boolean(o["DOVIPresent"])
        doviProfile = De.optI64(o["DOVIProfile"])
        decision = De.optString(o["decision"])
        location = De.optString(o["location"])
        providerTitle = De.optString(o["providerTitle"])
        score = De.optI64(o["score"])
        perfectMatch = De.boolean(o["perfectMatch"])
        downloaded = De.boolean(o["downloaded"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["id"] = JSONValue(id)
        o["streamType"] = JSONValue(streamType)
        o["default"] = JSONValue(`default`)
        o["selected"] = JSONValue(selected)
        o["forced"] = JSONValue(forced)
        o["hearingImpaired"] = JSONValue(hearingImpaired)
        o["codec"] = JSONValue(codec)
        o["index"] = JSONValue(index)
        o["bitrate"] = JSONValue(bitrate)
        o["language"] = JSONValue(language)
        o["languageCode"] = JSONValue(languageCode)
        o["languageTag"] = JSONValue(languageTag)
        o["title"] = JSONValue(title)
        o["displayTitle"] = JSONValue(displayTitle)
        o["extendedDisplayTitle"] = JSONValue(extendedDisplayTitle)
        o["key"] = JSONValue(key)
        o["format"] = JSONValue(format)
        o["channels"] = JSONValue(channels)
        o["samplingRate"] = JSONValue(samplingRate)
        o["audioChannelLayout"] = JSONValue(audioChannelLayout)
        o["profile"] = JSONValue(profile)
        o["width"] = JSONValue(width)
        o["height"] = JSONValue(height)
        o["frameRate"] = JSONValue(frameRate)
        o["bitDepth"] = JSONValue(bitDepth)
        o["colorPrimaries"] = JSONValue(colorPrimaries)
        o["colorTrc"] = JSONValue(colorTrc)
        o["colorSpace"] = JSONValue(colorSpace)
        o["DOVIPresent"] = JSONValue(doviPresent)
        o["DOVIProfile"] = JSONValue(doviProfile)
        o["decision"] = JSONValue(decision)
        o["location"] = JSONValue(location)
        o["providerTitle"] = JSONValue(providerTitle)
        o["score"] = JSONValue(score)
        o["perfectMatch"] = JSONValue(perfectMatch)
        o["downloaded"] = JSONValue(downloaded)
        return .object(o)
    }
}

public func channelLabel(_ channels: Int) -> String {
    switch channels {
    case 1: "Mono"
    case 2: "Stereo"
    case 3: "2.1"
    case 6: "5.1"
    case 7: "6.1"
    case 8: "7.1"
    default: "\(channels)ch"
    }
}

extension Media {
    /// Human label such as "4K HEVC · DV · TrueHD 7.1".
    public static func label(_ m: Media) -> String {
        var bits: [String] = []
        if let t = m.editionTitle ?? m.title { bits.append(t) }
        if let r = m.videoResolution {
            if r == "4k" || r == "4K" {
                bits.append("4K")
            } else if r == "sd" || r == "SD" {
                bits.append("SD")
            } else if r.allSatisfy({ ("0"..."9").contains($0) }) {
                bits.append("\(r)p")
            } else {
                bits.append(r.uppercased())
            }
        }
        if let codec = m.videoCodec { bits.append(codec.uppercased()) }
        let video = m.part.first.flatMap(Part.videoStream)
        if let hdr = video.flatMap(MediaStream.hdrLabel) { bits.append(hdr) }
        if let audio = m.audioCodec {
            let ch = m.audioChannels.map(channelLabel) ?? ""
            bits.append("\(audio.uppercased()) \(ch)".trimmingCharacters(in: .whitespaces))
        }
        return bits.joined(separator: " · ")
    }
}

extension Part {
    public static func streamsOf(_ p: Part, _ kind: Int) -> [MediaStream] {
        p.stream.filter { $0.streamType == kind }
    }

    public static func videoStream(_ p: Part) -> MediaStream? {
        streamsOf(p, StreamType.video).first
    }

    public static func audioStreams(_ p: Part) -> [MediaStream] {
        streamsOf(p, StreamType.audio)
    }

    public static func subtitleStreams(_ p: Part) -> [MediaStream] {
        streamsOf(p, StreamType.subtitle)
    }

    public static func selectedAudio(_ p: Part) -> MediaStream? {
        let audio = audioStreams(p)
        return audio.first(where: \.selected) ?? audio.first
    }

    public static func selectedSubtitle(_ p: Part) -> MediaStream? {
        subtitleStreams(p).first(where: \.selected)
    }

    /// The stream in this part that best corresponds to `template`, which
    /// usually comes from another episode. Language must match (and for
    /// subtitles, forced-ness too). Ties go to the stream that comes first.
    public static func matchingStream(_ p: Part, _ template: MediaStream) -> MediaStream? {
        guard let kind = template.streamType else { return nil }
        var best: (score: Int, stream: MediaStream)?
        for s in streamsOf(p, kind) {
            guard let score = MediaStream.similarity(template, s) else { continue }
            if best == nil || score > best!.score { best = (score, s) }
        }
        return best?.stream
    }
}

extension MediaStream {
    public static func label(_ s: MediaStream) -> String {
        s.extendedDisplayTitle ?? s.displayTitle ?? s.title ?? s.language ?? "Track \(s.index ?? 0)"
    }

    /// Short label: "English (AC3 5.1)" for audio, "English (SRT)" for subtitles.
    public static func shortLabel(_ s: MediaStream) -> String {
        s.displayTitle ?? label(s)
    }

    public static func isExternal(_ s: MediaStream) -> Bool {
        s.key != nil && s.index == nil
    }

    /// Language identity used for matching: the code, else the tag, else the name.
    private static func languageKey(_ s: MediaStream) -> String? {
        (s.languageCode ?? s.languageTag ?? s.language).map { v in
            String(v.map { $0.isASCII && $0.isUppercase ? Character($0.lowercased()) : $0 })
        }
    }

    /// How closely `other` resembles this stream, or `nil` if it can't stand in
    /// for it at all (a different type or language, or forced vs. full subtitles).
    public static func similarity(_ s: MediaStream, _ other: MediaStream) -> Int? {
        if s.streamType != other.streamType || languageKey(s) != languageKey(other) { return nil }
        let subtitle = s.streamType == StreamType.subtitle
        if subtitle && s.forced != other.forced { return nil }
        func same(_ a: String?, _ b: String?) -> Bool {
            guard let a, let b else { return false }
            return a.lowercased() == b.lowercased()
        }
        var score = 1
        if same(s.title, other.title) { score += 8 }
        if same(s.extendedDisplayTitle, other.extendedDisplayTitle) {
            score += 4
        } else if same(s.displayTitle, other.displayTitle) {
            score += 2
        }
        if same(s.codec, other.codec) { score += 3 }
        if s.channels != nil && s.channels == other.channels { score += 3 }
        if s.hearingImpaired == other.hearingImpaired { score += 2 }
        if isExternal(s) == isExternal(other) { score += 1 }
        return score
    }

    public static func hdrLabel(_ s: MediaStream) -> String? {
        if s.doviPresent { return "Dolby Vision" }
        switch s.colorTrc {
        case "smpte2084": return "HDR10"
        case "arib-std-b67": return "HLG"
        default: return nil
        }
    }
}
