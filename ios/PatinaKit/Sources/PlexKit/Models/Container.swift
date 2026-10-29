// Port of desktop/plex/models/container.ts.
// MediaContainer, hubs and directories (port of crates/plex/src/models/container.rs).

import Foundation

/// The generic container almost every server endpoint returns. Each endpoint
/// populates a different subset of the collections.
public struct MediaContainer: PlexModel {
    public var size: Int? = nil
    public var totalSize: Int? = nil
    public var offset: Int? = nil
    public var title1: String? = nil
    public var title2: String? = nil
    public var identifier: String? = nil
    public var key: String? = nil
    public var art: String? = nil
    public var thumb: String? = nil
    public var viewGroup: String? = nil
    public var librarySectionID: Int? = nil
    public var librarySectionTitle: String? = nil
    public var librarySectionUUID: String? = nil
    public var machineIdentifier: String? = nil
    public var playQueueID: Int? = nil
    public var playQueueSelectedItemID: Int? = nil
    public var playQueueSelectedItemOffset: Int? = nil
    public var playQueueSelectedMetadataItemID: String? = nil
    public var playQueueShuffled: Bool = false
    public var playQueueSourceURI: String? = nil
    public var playQueueTotalCount: Int? = nil
    public var playQueueVersion: Int? = nil
    public var canInstall: Bool = false
    public var checkedAt: Int? = nil
    public var downloadURL: String? = nil
    public var status: String? = nil
    public var metadata: [Metadata] = []
    public var directory: [Directory] = []
    public var hub: [Hub] = []
    public var setting: [Setting] = []
    public var activity: [Activity] = []
    public var account: [Account] = []
    public var device: [Device] = []
    public var server: [ServerDevice] = []
    public var transcodeSession: [TranscodeSession] = []
    public var statisticsMedia: [StatisticsMedia] = []
    public var statisticsResources: [StatisticsResources] = []
    public var statisticsBandwidth: [StatisticsBandwidth] = []
    public var release: [Release] = []
    public var searchResult: [MatchResult] = []
    public var meta: LibraryMeta? = nil
    /// Attributes Plex sent that are not modelled (serde's flattened `extra` map).
    public var extra: [String: JSONValue] = [:]

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        size: Int? = nil,
        totalSize: Int? = nil,
        offset: Int? = nil,
        title1: String? = nil,
        title2: String? = nil,
        identifier: String? = nil,
        key: String? = nil,
        art: String? = nil,
        thumb: String? = nil,
        viewGroup: String? = nil,
        librarySectionID: Int? = nil,
        librarySectionTitle: String? = nil,
        librarySectionUUID: String? = nil,
        machineIdentifier: String? = nil,
        playQueueID: Int? = nil,
        playQueueSelectedItemID: Int? = nil,
        playQueueSelectedItemOffset: Int? = nil,
        playQueueSelectedMetadataItemID: String? = nil,
        playQueueShuffled: Bool = false,
        playQueueSourceURI: String? = nil,
        playQueueTotalCount: Int? = nil,
        playQueueVersion: Int? = nil,
        canInstall: Bool = false,
        checkedAt: Int? = nil,
        downloadURL: String? = nil,
        status: String? = nil,
        metadata: [Metadata] = [],
        directory: [Directory] = [],
        hub: [Hub] = [],
        setting: [Setting] = [],
        activity: [Activity] = [],
        account: [Account] = [],
        device: [Device] = [],
        server: [ServerDevice] = [],
        transcodeSession: [TranscodeSession] = [],
        statisticsMedia: [StatisticsMedia] = [],
        statisticsResources: [StatisticsResources] = [],
        statisticsBandwidth: [StatisticsBandwidth] = [],
        release: [Release] = [],
        searchResult: [MatchResult] = [],
        meta: LibraryMeta? = nil,
        extra: [String: JSONValue] = [:]
    ) {
        self.size = size
        self.totalSize = totalSize
        self.offset = offset
        self.title1 = title1
        self.title2 = title2
        self.identifier = identifier
        self.key = key
        self.art = art
        self.thumb = thumb
        self.viewGroup = viewGroup
        self.librarySectionID = librarySectionID
        self.librarySectionTitle = librarySectionTitle
        self.librarySectionUUID = librarySectionUUID
        self.machineIdentifier = machineIdentifier
        self.playQueueID = playQueueID
        self.playQueueSelectedItemID = playQueueSelectedItemID
        self.playQueueSelectedItemOffset = playQueueSelectedItemOffset
        self.playQueueSelectedMetadataItemID = playQueueSelectedMetadataItemID
        self.playQueueShuffled = playQueueShuffled
        self.playQueueSourceURI = playQueueSourceURI
        self.playQueueTotalCount = playQueueTotalCount
        self.playQueueVersion = playQueueVersion
        self.canInstall = canInstall
        self.checkedAt = checkedAt
        self.downloadURL = downloadURL
        self.status = status
        self.metadata = metadata
        self.directory = directory
        self.hub = hub
        self.setting = setting
        self.activity = activity
        self.account = account
        self.device = device
        self.server = server
        self.transcodeSession = transcodeSession
        self.statisticsMedia = statisticsMedia
        self.statisticsResources = statisticsResources
        self.statisticsBandwidth = statisticsBandwidth
        self.release = release
        self.searchResult = searchResult
        self.meta = meta
        self.extra = extra
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "MediaContainer")
        size = De.optI64(o["size"])
        totalSize = De.optI64(o["totalSize"])
        offset = De.optI64(o["offset"])
        title1 = De.optString(o["title1"])
        title2 = De.optString(o["title2"])
        identifier = De.optString(o["identifier"])
        key = De.optString(o["key"])
        art = De.optString(o["art"])
        thumb = De.optString(o["thumb"])
        viewGroup = De.optString(o["viewGroup"])
        librarySectionID = De.optI64(o["librarySectionID"])
        librarySectionTitle = De.optString(o["librarySectionTitle"])
        librarySectionUUID = De.optString(o["librarySectionUUID"])
        machineIdentifier = De.optString(o["machineIdentifier"])
        playQueueID = De.optI64(o["playQueueID"])
        playQueueSelectedItemID = De.optI64(o["playQueueSelectedItemID"])
        playQueueSelectedItemOffset = De.optI64(o["playQueueSelectedItemOffset"])
        playQueueSelectedMetadataItemID = De.optString(o["playQueueSelectedMetadataItemID"])
        playQueueShuffled = De.boolean(o["playQueueShuffled"])
        playQueueSourceURI = De.optString(o["playQueueSourceURI"])
        playQueueTotalCount = De.optI64(o["playQueueTotalCount"])
        playQueueVersion = De.optI64(o["playQueueVersion"])
        canInstall = De.boolean(o["canInstall"])
        checkedAt = De.optI64(o["checkedAt"])
        downloadURL = De.optString(o["downloadURL"])
        status = De.optString(o["status"])
        metadata = try De.list(o["Metadata"], Metadata.init(json:), key: "Metadata")
        directory = try De.list(o["Directory"], Directory.init(json:), key: "Directory")
        hub = try De.list(o["Hub"], Hub.init(json:), key: "Hub")
        setting = try De.list(o["Setting"], Setting.init(json:), key: "Setting")
        activity = try De.list(o["Activity"], Activity.init(json:), key: "Activity")
        account = try De.list(o["Account"], Account.init(json:), key: "Account")
        device = try De.list(o["Device"], Device.init(json:), key: "Device")
        server = try De.list(o["Server"], ServerDevice.init(json:), key: "Server")
        transcodeSession = try De.list(o["TranscodeSession"], TranscodeSession.init(json:), key: "TranscodeSession")
        statisticsMedia = try De.list(o["StatisticsMedia"], StatisticsMedia.init(json:), key: "StatisticsMedia")
        statisticsResources = try De.list(o["StatisticsResources"], StatisticsResources.init(json:), key: "StatisticsResources")
        statisticsBandwidth = try De.list(o["StatisticsBandwidth"], StatisticsBandwidth.init(json:), key: "StatisticsBandwidth")
        release = try De.list(o["Release"], Release.init(json:), key: "Release")
        searchResult = try De.list(o["SearchResult"], MatchResult.init(json:), key: "SearchResult")
        meta = try De.optional(o["Meta"], LibraryMeta.init(json:), key: "Meta")
        extra = De.extraOf(o, Self.keys)
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = extra
        o["size"] = JSONValue(size)
        o["totalSize"] = JSONValue(totalSize)
        o["offset"] = JSONValue(offset)
        o["title1"] = JSONValue(title1)
        o["title2"] = JSONValue(title2)
        o["identifier"] = JSONValue(identifier)
        o["key"] = JSONValue(key)
        o["art"] = JSONValue(art)
        o["thumb"] = JSONValue(thumb)
        o["viewGroup"] = JSONValue(viewGroup)
        o["librarySectionID"] = JSONValue(librarySectionID)
        o["librarySectionTitle"] = JSONValue(librarySectionTitle)
        o["librarySectionUUID"] = JSONValue(librarySectionUUID)
        o["machineIdentifier"] = JSONValue(machineIdentifier)
        o["playQueueID"] = JSONValue(playQueueID)
        o["playQueueSelectedItemID"] = JSONValue(playQueueSelectedItemID)
        o["playQueueSelectedItemOffset"] = JSONValue(playQueueSelectedItemOffset)
        o["playQueueSelectedMetadataItemID"] = JSONValue(playQueueSelectedMetadataItemID)
        o["playQueueShuffled"] = JSONValue(playQueueShuffled)
        o["playQueueSourceURI"] = JSONValue(playQueueSourceURI)
        o["playQueueTotalCount"] = JSONValue(playQueueTotalCount)
        o["playQueueVersion"] = JSONValue(playQueueVersion)
        o["canInstall"] = JSONValue(canInstall)
        o["checkedAt"] = JSONValue(checkedAt)
        o["downloadURL"] = JSONValue(downloadURL)
        o["status"] = JSONValue(status)
        o["Metadata"] = .array(metadata.map(\.json))
        o["Directory"] = .array(directory.map(\.json))
        o["Hub"] = .array(hub.map(\.json))
        o["Setting"] = .array(setting.map(\.json))
        o["Activity"] = .array(activity.map(\.json))
        o["Account"] = .array(account.map(\.json))
        o["Device"] = .array(device.map(\.json))
        o["Server"] = .array(server.map(\.json))
        o["TranscodeSession"] = .array(transcodeSession.map(\.json))
        o["StatisticsMedia"] = .array(statisticsMedia.map(\.json))
        o["StatisticsResources"] = .array(statisticsResources.map(\.json))
        o["StatisticsBandwidth"] = .array(statisticsBandwidth.map(\.json))
        o["Release"] = .array(release.map(\.json))
        o["SearchResult"] = .array(searchResult.map(\.json))
        o["Meta"] = meta?.json ?? .null
        return .object(o)
    }

    /// The modelled keys; everything else goes to `extra`.
    static let keys: Set<String> = ["size", "totalSize", "offset", "title1", "title2", "identifier", "key", "art", "thumb", "viewGroup", "librarySectionID", "librarySectionTitle", "librarySectionUUID", "machineIdentifier", "playQueueID", "playQueueSelectedItemID", "playQueueSelectedItemOffset", "playQueueSelectedMetadataItemID", "playQueueShuffled", "playQueueSourceURI", "playQueueTotalCount", "playQueueVersion", "canInstall", "checkedAt", "downloadURL", "status", "Metadata", "Directory", "Hub", "Setting", "Activity", "Account", "Device", "Server", "TranscodeSession", "StatisticsMedia", "StatisticsResources", "StatisticsBandwidth", "Release", "SearchResult", "Meta"]
}

/// A hub (row) of related content: "Continue Watching", "Recently Added", search groups, ...
public struct Hub: PlexModel {
    public var hubKey: String? = nil
    public var key: String? = nil
    public var title: String? = nil
    public var type: String? = nil
    public var hubIdentifier: String? = nil
    public var context: String? = nil
    public var size: Int? = nil
    public var more: Bool = false
    public var style: String? = nil
    public var promoted: Bool = false
    public var metadata: [Metadata] = []
    public var directory: [Directory] = []

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        hubKey: String? = nil,
        key: String? = nil,
        title: String? = nil,
        type: String? = nil,
        hubIdentifier: String? = nil,
        context: String? = nil,
        size: Int? = nil,
        more: Bool = false,
        style: String? = nil,
        promoted: Bool = false,
        metadata: [Metadata] = [],
        directory: [Directory] = []
    ) {
        self.hubKey = hubKey
        self.key = key
        self.title = title
        self.type = type
        self.hubIdentifier = hubIdentifier
        self.context = context
        self.size = size
        self.more = more
        self.style = style
        self.promoted = promoted
        self.metadata = metadata
        self.directory = directory
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Hub")
        hubKey = De.optString(o["hubKey"])
        key = De.optString(o["key"])
        title = De.optString(o["title"])
        type = De.optString(o["type"])
        hubIdentifier = De.optString(o["hubIdentifier"])
        context = De.optString(o["context"])
        size = De.optI64(o["size"])
        more = De.boolean(o["more"])
        style = De.optString(o["style"])
        promoted = De.boolean(o["promoted"])
        metadata = try De.list(o["Metadata"], Metadata.init(json:), key: "Metadata")
        directory = try De.list(o["Directory"], Directory.init(json:), key: "Directory")
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["hubKey"] = JSONValue(hubKey)
        o["key"] = JSONValue(key)
        o["title"] = JSONValue(title)
        o["type"] = JSONValue(type)
        o["hubIdentifier"] = JSONValue(hubIdentifier)
        o["context"] = JSONValue(context)
        o["size"] = JSONValue(size)
        o["more"] = JSONValue(more)
        o["style"] = JSONValue(style)
        o["promoted"] = JSONValue(promoted)
        o["Metadata"] = .array(metadata.map(\.json))
        o["Directory"] = .array(directory.map(\.json))
        return .object(o)
    }
}

/// A "directory" entry: library sections, filter values, sorts, tag lists, capabilities.
public struct Directory: PlexModel {
    public var key: String? = nil
    public var title: String? = nil
    public var type: String? = nil
    public var fastKey: String? = nil
    public var thumb: String? = nil
    public var art: String? = nil
    public var composite: String? = nil
    public var count: Int? = nil
    /// Item count of a `firstCharacter` entry.
    public var size: Int? = nil
    public var tag: String? = nil
    public var filter: String? = nil
    public var filterType: String? = nil
    public var `default`: String? = nil
    public var defaultDirection: String? = nil
    public var descKey: String? = nil
    public var firstCharacterKey: String? = nil
    public var secondary: Bool = false
    public var search: Bool = false
    public var prompt: String? = nil
    public var agent: String? = nil
    public var scanner: String? = nil
    public var language: String? = nil
    public var uuid: String? = nil
    public var updatedAt: Int? = nil
    public var createdAt: Int? = nil
    public var scannedAt: Int? = nil
    public var contentChangedAt: Int? = nil
    public var refreshing: Bool = false
    public var hidden: Bool = false
    public var allowSync: Bool = false
    public var location: [Location] = []

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        key: String? = nil,
        title: String? = nil,
        type: String? = nil,
        fastKey: String? = nil,
        thumb: String? = nil,
        art: String? = nil,
        composite: String? = nil,
        count: Int? = nil,
        size: Int? = nil,
        tag: String? = nil,
        filter: String? = nil,
        filterType: String? = nil,
        default: String? = nil,
        defaultDirection: String? = nil,
        descKey: String? = nil,
        firstCharacterKey: String? = nil,
        secondary: Bool = false,
        search: Bool = false,
        prompt: String? = nil,
        agent: String? = nil,
        scanner: String? = nil,
        language: String? = nil,
        uuid: String? = nil,
        updatedAt: Int? = nil,
        createdAt: Int? = nil,
        scannedAt: Int? = nil,
        contentChangedAt: Int? = nil,
        refreshing: Bool = false,
        hidden: Bool = false,
        allowSync: Bool = false,
        location: [Location] = []
    ) {
        self.key = key
        self.title = title
        self.type = type
        self.fastKey = fastKey
        self.thumb = thumb
        self.art = art
        self.composite = composite
        self.count = count
        self.size = size
        self.tag = tag
        self.filter = filter
        self.filterType = filterType
        self.`default` = `default`
        self.defaultDirection = defaultDirection
        self.descKey = descKey
        self.firstCharacterKey = firstCharacterKey
        self.secondary = secondary
        self.search = search
        self.prompt = prompt
        self.agent = agent
        self.scanner = scanner
        self.language = language
        self.uuid = uuid
        self.updatedAt = updatedAt
        self.createdAt = createdAt
        self.scannedAt = scannedAt
        self.contentChangedAt = contentChangedAt
        self.refreshing = refreshing
        self.hidden = hidden
        self.allowSync = allowSync
        self.location = location
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Directory")
        key = De.optString(o["key"])
        title = De.optString(o["title"])
        type = De.optString(o["type"])
        fastKey = De.optString(o["fastKey"])
        thumb = De.optString(o["thumb"])
        art = De.optString(o["art"])
        composite = De.optString(o["composite"])
        count = De.optI64(o["count"])
        size = De.optI64(o["size"])
        tag = De.optString(o["tag"])
        filter = De.optString(o["filter"])
        filterType = De.optString(o["filterType"])
        `default` = De.optString(o["default"])
        defaultDirection = De.optString(o["defaultDirection"])
        descKey = De.optString(o["descKey"])
        firstCharacterKey = De.optString(o["firstCharacterKey"])
        secondary = De.boolean(o["secondary"])
        search = De.boolean(o["search"])
        prompt = De.optString(o["prompt"])
        agent = De.optString(o["agent"])
        scanner = De.optString(o["scanner"])
        language = De.optString(o["language"])
        uuid = De.optString(o["uuid"])
        updatedAt = De.optI64(o["updatedAt"])
        createdAt = De.optI64(o["createdAt"])
        scannedAt = De.optI64(o["scannedAt"])
        contentChangedAt = De.optI64(o["contentChangedAt"])
        refreshing = De.boolean(o["refreshing"])
        hidden = De.boolean(o["hidden"])
        allowSync = De.boolean(o["allowSync"])
        location = try De.list(o["Location"], Location.init(json:), key: "Location")
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["key"] = JSONValue(key)
        o["title"] = JSONValue(title)
        o["type"] = JSONValue(type)
        o["fastKey"] = JSONValue(fastKey)
        o["thumb"] = JSONValue(thumb)
        o["art"] = JSONValue(art)
        o["composite"] = JSONValue(composite)
        o["count"] = JSONValue(count)
        o["size"] = JSONValue(size)
        o["tag"] = JSONValue(tag)
        o["filter"] = JSONValue(filter)
        o["filterType"] = JSONValue(filterType)
        o["default"] = JSONValue(`default`)
        o["defaultDirection"] = JSONValue(defaultDirection)
        o["descKey"] = JSONValue(descKey)
        o["firstCharacterKey"] = JSONValue(firstCharacterKey)
        o["secondary"] = JSONValue(secondary)
        o["search"] = JSONValue(search)
        o["prompt"] = JSONValue(prompt)
        o["agent"] = JSONValue(agent)
        o["scanner"] = JSONValue(scanner)
        o["language"] = JSONValue(language)
        o["uuid"] = JSONValue(uuid)
        o["updatedAt"] = JSONValue(updatedAt)
        o["createdAt"] = JSONValue(createdAt)
        o["scannedAt"] = JSONValue(scannedAt)
        o["contentChangedAt"] = JSONValue(contentChangedAt)
        o["refreshing"] = JSONValue(refreshing)
        o["hidden"] = JSONValue(hidden)
        o["allowSync"] = JSONValue(allowSync)
        o["Location"] = .array(location.map(\.json))
        return .object(o)
    }
}

public struct Location: PlexModel {
    public var id: Int? = nil
    public var path: String? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        id: Int? = nil,
        path: String? = nil
    ) {
        self.id = id
        self.path = path
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Location")
        id = De.optI64(o["id"])
        path = De.optString(o["path"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["id"] = JSONValue(id)
        o["path"] = JSONValue(path)
        return .object(o)
    }
}

/// `includeMeta=1` payload describing a library's types, filters, sorts and fields.
public struct LibraryMeta: PlexModel {
    public var type: [LibraryMetaType] = []
    public var fieldType: [FieldType] = []

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        type: [LibraryMetaType] = [],
        fieldType: [FieldType] = []
    ) {
        self.type = type
        self.fieldType = fieldType
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "LibraryMeta")
        type = try De.list(o["Type"], LibraryMetaType.init(json:), key: "Type")
        fieldType = try De.list(o["FieldType"], FieldType.init(json:), key: "FieldType")
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["Type"] = .array(type.map(\.json))
        o["FieldType"] = .array(fieldType.map(\.json))
        return .object(o)
    }
}

public struct LibraryMetaType: PlexModel {
    public var key: String? = nil
    public var type: String? = nil
    public var title: String? = nil
    public var active: Bool = false
    public var filter: [Directory] = []
    public var sort: [Directory] = []
    public var field: [Field] = []

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        key: String? = nil,
        type: String? = nil,
        title: String? = nil,
        active: Bool = false,
        filter: [Directory] = [],
        sort: [Directory] = [],
        field: [Field] = []
    ) {
        self.key = key
        self.type = type
        self.title = title
        self.active = active
        self.filter = filter
        self.sort = sort
        self.field = field
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "LibraryMetaType")
        key = De.optString(o["key"])
        type = De.optString(o["type"])
        title = De.optString(o["title"])
        active = De.boolean(o["active"])
        filter = try De.list(o["Filter"], Directory.init(json:), key: "Filter")
        sort = try De.list(o["Sort"], Directory.init(json:), key: "Sort")
        field = try De.list(o["Field"], Field.init(json:), key: "Field")
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["key"] = JSONValue(key)
        o["type"] = JSONValue(type)
        o["title"] = JSONValue(title)
        o["active"] = JSONValue(active)
        o["Filter"] = .array(filter.map(\.json))
        o["Sort"] = .array(sort.map(\.json))
        o["Field"] = .array(field.map(\.json))
        return .object(o)
    }
}

public struct Field: PlexModel {
    public var key: String? = nil
    public var title: String? = nil
    public var type: String? = nil
    public var subType: String? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        key: String? = nil,
        title: String? = nil,
        type: String? = nil,
        subType: String? = nil
    ) {
        self.key = key
        self.title = title
        self.type = type
        self.subType = subType
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Field")
        key = De.optString(o["key"])
        title = De.optString(o["title"])
        type = De.optString(o["type"])
        subType = De.optString(o["subType"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["key"] = JSONValue(key)
        o["title"] = JSONValue(title)
        o["type"] = JSONValue(type)
        o["subType"] = JSONValue(subType)
        return .object(o)
    }
}

public struct FieldType: PlexModel {
    public var type: String? = nil
    public var `operator`: [Operator] = []

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        type: String? = nil,
        operator: [Operator] = []
    ) {
        self.type = type
        self.`operator` = `operator`
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "FieldType")
        type = De.optString(o["type"])
        `operator` = try De.list(o["Operator"], Operator.init(json:), key: "Operator")
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["type"] = JSONValue(type)
        o["Operator"] = .array(`operator`.map(\.json))
        return .object(o)
    }
}

public struct Operator: PlexModel {
    public var key: String? = nil
    public var title: String? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        key: String? = nil,
        title: String? = nil
    ) {
        self.key = key
        self.title = title
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Operator")
        key = De.optString(o["key"])
        title = De.optString(o["title"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["key"] = JSONValue(key)
        o["title"] = JSONValue(title)
        return .object(o)
    }
}

/// Top-level response wrapper: `{"MediaContainer": {...}}`.
public struct Envelope: PlexModel {
    public var mediaContainer = MediaContainer()

    public init() {}

    public init(mediaContainer: MediaContainer) {
        self.mediaContainer = mediaContainer
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Envelope")
        do {
            mediaContainer = try o["MediaContainer"].map(MediaContainer.init(json:)) ?? MediaContainer()
        } catch let e as DecodeFailure {
            throw e.under("MediaContainer")
        }
    }

    public var json: JSONValue {
        .object(["MediaContainer": mediaContainer.json])
    }
}

extension MediaContainer {
    /// Look up an attribute from `extra` as a string.
    public static func attr(_ mc: MediaContainer, _ key: String) -> String? {
        if keys.contains(key) { return nil }
        switch mc.extra[key] {
        case nil, .null?: return nil
        case .string(let s)?: return s
        case let v?: return v.serialized()
        }
    }

    /// Whether there are more items after this page.
    public static func hasMore(_ mc: MediaContainer) -> Bool {
        if let offset = mc.offset, let size = mc.size, let total = mc.totalSize {
            return offset + size < total
        }
        return false
    }
}

extension Hub {
    public static func isEmpty(_ h: Hub) -> Bool {
        h.metadata.isEmpty && h.directory.isEmpty
    }
}

extension Directory {
    public static func title(_ d: Directory) -> String {
        d.title ?? d.tag ?? ""
    }
}
