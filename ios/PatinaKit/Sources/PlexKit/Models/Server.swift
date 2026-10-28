// Port of desktop/plex/models/server.ts.
// Server-side models: sessions, settings, activities, accounts, statistics, ... (port of crates/plex/src/models/server.rs).

import Foundation

public struct SessionUser: PlexModel {
    public var id: String? = nil
    public var title: String? = nil
    public var thumb: String? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        id: String? = nil,
        title: String? = nil,
        thumb: String? = nil
    ) {
        self.id = id
        self.title = title
        self.thumb = thumb
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "SessionUser")
        id = De.optString(o["id"])
        title = De.optString(o["title"])
        thumb = De.optString(o["thumb"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["id"] = JSONValue(id)
        o["title"] = JSONValue(title)
        o["thumb"] = JSONValue(thumb)
        return .object(o)
    }
}

/// The client playing a session.
public struct Player: PlexModel {
    public var address: String? = nil
    public var device: String? = nil
    public var machineIdentifier: String? = nil
    public var model: String? = nil
    public var platform: String? = nil
    public var platformVersion: String? = nil
    public var product: String? = nil
    public var profile: String? = nil
    public var remotePublicAddress: String? = nil
    public var state: String? = nil
    public var title: String? = nil
    public var vendor: String? = nil
    public var version: String? = nil
    public var local: Bool = false
    public var relayed: Bool = false
    public var secure: Bool = false

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        address: String? = nil,
        device: String? = nil,
        machineIdentifier: String? = nil,
        model: String? = nil,
        platform: String? = nil,
        platformVersion: String? = nil,
        product: String? = nil,
        profile: String? = nil,
        remotePublicAddress: String? = nil,
        state: String? = nil,
        title: String? = nil,
        vendor: String? = nil,
        version: String? = nil,
        local: Bool = false,
        relayed: Bool = false,
        secure: Bool = false
    ) {
        self.address = address
        self.device = device
        self.machineIdentifier = machineIdentifier
        self.model = model
        self.platform = platform
        self.platformVersion = platformVersion
        self.product = product
        self.profile = profile
        self.remotePublicAddress = remotePublicAddress
        self.state = state
        self.title = title
        self.vendor = vendor
        self.version = version
        self.local = local
        self.relayed = relayed
        self.secure = secure
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Player")
        address = De.optString(o["address"])
        device = De.optString(o["device"])
        machineIdentifier = De.optString(o["machineIdentifier"])
        model = De.optString(o["model"])
        platform = De.optString(o["platform"])
        platformVersion = De.optString(o["platformVersion"])
        product = De.optString(o["product"])
        profile = De.optString(o["profile"])
        remotePublicAddress = De.optString(o["remotePublicAddress"])
        state = De.optString(o["state"])
        title = De.optString(o["title"])
        vendor = De.optString(o["vendor"])
        version = De.optString(o["version"])
        local = De.boolean(o["local"])
        relayed = De.boolean(o["relayed"])
        secure = De.boolean(o["secure"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["address"] = JSONValue(address)
        o["device"] = JSONValue(device)
        o["machineIdentifier"] = JSONValue(machineIdentifier)
        o["model"] = JSONValue(model)
        o["platform"] = JSONValue(platform)
        o["platformVersion"] = JSONValue(platformVersion)
        o["product"] = JSONValue(product)
        o["profile"] = JSONValue(profile)
        o["remotePublicAddress"] = JSONValue(remotePublicAddress)
        o["state"] = JSONValue(state)
        o["title"] = JSONValue(title)
        o["vendor"] = JSONValue(vendor)
        o["version"] = JSONValue(version)
        o["local"] = JSONValue(local)
        o["relayed"] = JSONValue(relayed)
        o["secure"] = JSONValue(secure)
        return .object(o)
    }
}

public struct SessionInfo: PlexModel {
    public var id: String? = nil
    public var bandwidth: Int? = nil
    public var location: String? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        id: String? = nil,
        bandwidth: Int? = nil,
        location: String? = nil
    ) {
        self.id = id
        self.bandwidth = bandwidth
        self.location = location
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "SessionInfo")
        id = De.optString(o["id"])
        bandwidth = De.optI64(o["bandwidth"])
        location = De.optString(o["location"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["id"] = JSONValue(id)
        o["bandwidth"] = JSONValue(bandwidth)
        o["location"] = JSONValue(location)
        return .object(o)
    }
}

public struct TranscodeSession: PlexModel {
    public var key: String? = nil
    public var throttled: Bool = false
    public var complete: Bool = false
    public var progress: Double? = nil
    public var speed: Double? = nil
    public var duration: Int? = nil
    public var remaining: Int? = nil
    public var context: String? = nil
    public var sourceVideoCodec: String? = nil
    public var sourceAudioCodec: String? = nil
    public var videoDecision: String? = nil
    public var audioDecision: String? = nil
    public var subtitleDecision: String? = nil
    public var `protocol`: String? = nil
    public var container: String? = nil
    public var videoCodec: String? = nil
    public var audioCodec: String? = nil
    public var audioChannels: Int? = nil
    public var width: Int? = nil
    public var height: Int? = nil
    public var transcodeHwRequested: Bool = false
    public var transcodeHwFullPipeline: Bool = false
    public var maxOffsetAvailable: Double? = nil
    public var minOffsetAvailable: Double? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        key: String? = nil,
        throttled: Bool = false,
        complete: Bool = false,
        progress: Double? = nil,
        speed: Double? = nil,
        duration: Int? = nil,
        remaining: Int? = nil,
        context: String? = nil,
        sourceVideoCodec: String? = nil,
        sourceAudioCodec: String? = nil,
        videoDecision: String? = nil,
        audioDecision: String? = nil,
        subtitleDecision: String? = nil,
        protocol: String? = nil,
        container: String? = nil,
        videoCodec: String? = nil,
        audioCodec: String? = nil,
        audioChannels: Int? = nil,
        width: Int? = nil,
        height: Int? = nil,
        transcodeHwRequested: Bool = false,
        transcodeHwFullPipeline: Bool = false,
        maxOffsetAvailable: Double? = nil,
        minOffsetAvailable: Double? = nil
    ) {
        self.key = key
        self.throttled = throttled
        self.complete = complete
        self.progress = progress
        self.speed = speed
        self.duration = duration
        self.remaining = remaining
        self.context = context
        self.sourceVideoCodec = sourceVideoCodec
        self.sourceAudioCodec = sourceAudioCodec
        self.videoDecision = videoDecision
        self.audioDecision = audioDecision
        self.subtitleDecision = subtitleDecision
        self.`protocol` = `protocol`
        self.container = container
        self.videoCodec = videoCodec
        self.audioCodec = audioCodec
        self.audioChannels = audioChannels
        self.width = width
        self.height = height
        self.transcodeHwRequested = transcodeHwRequested
        self.transcodeHwFullPipeline = transcodeHwFullPipeline
        self.maxOffsetAvailable = maxOffsetAvailable
        self.minOffsetAvailable = minOffsetAvailable
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "TranscodeSession")
        key = De.optString(o["key"])
        throttled = De.boolean(o["throttled"])
        complete = De.boolean(o["complete"])
        progress = De.optF64(o["progress"])
        speed = De.optF64(o["speed"])
        duration = De.optI64(o["duration"])
        remaining = De.optI64(o["remaining"])
        context = De.optString(o["context"])
        sourceVideoCodec = De.optString(o["sourceVideoCodec"])
        sourceAudioCodec = De.optString(o["sourceAudioCodec"])
        videoDecision = De.optString(o["videoDecision"])
        audioDecision = De.optString(o["audioDecision"])
        subtitleDecision = De.optString(o["subtitleDecision"])
        `protocol` = De.optString(o["protocol"])
        container = De.optString(o["container"])
        videoCodec = De.optString(o["videoCodec"])
        audioCodec = De.optString(o["audioCodec"])
        audioChannels = De.optI64(o["audioChannels"])
        width = De.optI64(o["width"])
        height = De.optI64(o["height"])
        transcodeHwRequested = De.boolean(o["transcodeHwRequested"])
        transcodeHwFullPipeline = De.boolean(o["transcodeHwFullPipeline"])
        maxOffsetAvailable = De.optF64(o["maxOffsetAvailable"])
        minOffsetAvailable = De.optF64(o["minOffsetAvailable"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["key"] = JSONValue(key)
        o["throttled"] = JSONValue(throttled)
        o["complete"] = JSONValue(complete)
        o["progress"] = JSONValue(progress)
        o["speed"] = JSONValue(speed)
        o["duration"] = JSONValue(duration)
        o["remaining"] = JSONValue(remaining)
        o["context"] = JSONValue(context)
        o["sourceVideoCodec"] = JSONValue(sourceVideoCodec)
        o["sourceAudioCodec"] = JSONValue(sourceAudioCodec)
        o["videoDecision"] = JSONValue(videoDecision)
        o["audioDecision"] = JSONValue(audioDecision)
        o["subtitleDecision"] = JSONValue(subtitleDecision)
        o["protocol"] = JSONValue(`protocol`)
        o["container"] = JSONValue(container)
        o["videoCodec"] = JSONValue(videoCodec)
        o["audioCodec"] = JSONValue(audioCodec)
        o["audioChannels"] = JSONValue(audioChannels)
        o["width"] = JSONValue(width)
        o["height"] = JSONValue(height)
        o["transcodeHwRequested"] = JSONValue(transcodeHwRequested)
        o["transcodeHwFullPipeline"] = JSONValue(transcodeHwFullPipeline)
        o["maxOffsetAvailable"] = JSONValue(maxOffsetAvailable)
        o["minOffsetAvailable"] = JSONValue(minOffsetAvailable)
        return .object(o)
    }
}

/// A server or account preference.
public struct Setting: PlexModel {
    public var id: String? = nil
    public var label: String? = nil
    public var summary: String? = nil
    public var type: String? = nil
    public var `default`: JSONValue = .null
    public var value: JSONValue = .null
    public var hidden: Bool = false
    public var advanced: Bool = false
    public var group: String? = nil
    /// `value:Label|value:Label` pairs for enum settings.
    public var enumValues: String? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        id: String? = nil,
        label: String? = nil,
        summary: String? = nil,
        type: String? = nil,
        default: JSONValue = .null,
        value: JSONValue = .null,
        hidden: Bool = false,
        advanced: Bool = false,
        group: String? = nil,
        enumValues: String? = nil
    ) {
        self.id = id
        self.label = label
        self.summary = summary
        self.type = type
        self.`default` = `default`
        self.value = value
        self.hidden = hidden
        self.advanced = advanced
        self.group = group
        self.enumValues = enumValues
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Setting")
        id = De.optString(o["id"])
        label = De.optString(o["label"])
        summary = De.optString(o["summary"])
        type = De.optString(o["type"])
        `default` = De.value(o["default"])
        value = De.value(o["value"])
        hidden = De.boolean(o["hidden"])
        advanced = De.boolean(o["advanced"])
        group = De.optString(o["group"])
        enumValues = De.optString(o["enumValues"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["id"] = JSONValue(id)
        o["label"] = JSONValue(label)
        o["summary"] = JSONValue(summary)
        o["type"] = JSONValue(type)
        o["default"] = `default`
        o["value"] = value
        o["hidden"] = JSONValue(hidden)
        o["advanced"] = JSONValue(advanced)
        o["group"] = JSONValue(group)
        o["enumValues"] = JSONValue(enumValues)
        return .object(o)
    }
}

/// A background server activity (scan, refresh, optimise, ...).
public struct Activity: PlexModel {
    public var uuid: String? = nil
    public var type: String? = nil
    public var cancellable: Bool = false
    public var userID: Int? = nil
    public var title: String? = nil
    public var subtitle: String? = nil
    public var progress: Int? = nil
    public var context: JSONValue = .null

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        uuid: String? = nil,
        type: String? = nil,
        cancellable: Bool = false,
        userID: Int? = nil,
        title: String? = nil,
        subtitle: String? = nil,
        progress: Int? = nil,
        context: JSONValue = .null
    ) {
        self.uuid = uuid
        self.type = type
        self.cancellable = cancellable
        self.userID = userID
        self.title = title
        self.subtitle = subtitle
        self.progress = progress
        self.context = context
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Activity")
        uuid = De.optString(o["uuid"])
        type = De.optString(o["type"])
        cancellable = De.boolean(o["cancellable"])
        userID = De.optI64(o["userID"])
        title = De.optString(o["title"])
        subtitle = De.optString(o["subtitle"])
        progress = De.optI64(o["progress"])
        context = De.value(o["Context"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["uuid"] = JSONValue(uuid)
        o["type"] = JSONValue(type)
        o["cancellable"] = JSONValue(cancellable)
        o["userID"] = JSONValue(userID)
        o["title"] = JSONValue(title)
        o["subtitle"] = JSONValue(subtitle)
        o["progress"] = JSONValue(progress)
        o["Context"] = context
        return .object(o)
    }
}

public struct Account: PlexModel {
    public var id: Int? = nil
    public var key: String? = nil
    public var name: String? = nil
    public var defaultAudioLanguage: String? = nil
    public var autoSelectAudio: Bool = false
    public var defaultSubtitleLanguage: String? = nil
    public var subtitleMode: Int? = nil
    public var thumb: String? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        id: Int? = nil,
        key: String? = nil,
        name: String? = nil,
        defaultAudioLanguage: String? = nil,
        autoSelectAudio: Bool = false,
        defaultSubtitleLanguage: String? = nil,
        subtitleMode: Int? = nil,
        thumb: String? = nil
    ) {
        self.id = id
        self.key = key
        self.name = name
        self.defaultAudioLanguage = defaultAudioLanguage
        self.autoSelectAudio = autoSelectAudio
        self.defaultSubtitleLanguage = defaultSubtitleLanguage
        self.subtitleMode = subtitleMode
        self.thumb = thumb
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Account")
        id = De.optI64(o["id"])
        key = De.optString(o["key"])
        name = De.optString(o["name"])
        defaultAudioLanguage = De.optString(o["defaultAudioLanguage"])
        autoSelectAudio = De.boolean(o["autoSelectAudio"])
        defaultSubtitleLanguage = De.optString(o["defaultSubtitleLanguage"])
        subtitleMode = De.optI64(o["subtitleMode"])
        thumb = De.optString(o["thumb"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["id"] = JSONValue(id)
        o["key"] = JSONValue(key)
        o["name"] = JSONValue(name)
        o["defaultAudioLanguage"] = JSONValue(defaultAudioLanguage)
        o["autoSelectAudio"] = JSONValue(autoSelectAudio)
        o["defaultSubtitleLanguage"] = JSONValue(defaultSubtitleLanguage)
        o["subtitleMode"] = JSONValue(subtitleMode)
        o["thumb"] = JSONValue(thumb)
        return .object(o)
    }
}

public struct Device: PlexModel {
    public var id: Int? = nil
    public var name: String? = nil
    public var platform: String? = nil
    public var clientIdentifier: String? = nil
    public var createdAt: Int? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        id: Int? = nil,
        name: String? = nil,
        platform: String? = nil,
        clientIdentifier: String? = nil,
        createdAt: Int? = nil
    ) {
        self.id = id
        self.name = name
        self.platform = platform
        self.clientIdentifier = clientIdentifier
        self.createdAt = createdAt
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Device")
        id = De.optI64(o["id"])
        name = De.optString(o["name"])
        platform = De.optString(o["platform"])
        clientIdentifier = De.optString(o["clientIdentifier"])
        createdAt = De.optI64(o["createdAt"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["id"] = JSONValue(id)
        o["name"] = JSONValue(name)
        o["platform"] = JSONValue(platform)
        o["clientIdentifier"] = JSONValue(clientIdentifier)
        o["createdAt"] = JSONValue(createdAt)
        return .object(o)
    }
}

/// Entries from `/clients` and `/servers`.
public struct ServerDevice: PlexModel {
    public var name: String? = nil
    public var host: String? = nil
    public var address: String? = nil
    public var port: Int? = nil
    public var machineIdentifier: String? = nil
    public var version: String? = nil
    public var `protocol`: String? = nil
    public var product: String? = nil
    public var deviceClass: String? = nil
    public var protocolVersion: String? = nil
    public var protocolCapabilities: String? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        name: String? = nil,
        host: String? = nil,
        address: String? = nil,
        port: Int? = nil,
        machineIdentifier: String? = nil,
        version: String? = nil,
        protocol: String? = nil,
        product: String? = nil,
        deviceClass: String? = nil,
        protocolVersion: String? = nil,
        protocolCapabilities: String? = nil
    ) {
        self.name = name
        self.host = host
        self.address = address
        self.port = port
        self.machineIdentifier = machineIdentifier
        self.version = version
        self.`protocol` = `protocol`
        self.product = product
        self.deviceClass = deviceClass
        self.protocolVersion = protocolVersion
        self.protocolCapabilities = protocolCapabilities
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "ServerDevice")
        name = De.optString(o["name"])
        host = De.optString(o["host"])
        address = De.optString(o["address"])
        port = De.optI64(o["port"])
        machineIdentifier = De.optString(o["machineIdentifier"])
        version = De.optString(o["version"])
        `protocol` = De.optString(o["protocol"])
        product = De.optString(o["product"])
        deviceClass = De.optString(o["deviceClass"])
        protocolVersion = De.optString(o["protocolVersion"])
        protocolCapabilities = De.optString(o["protocolCapabilities"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["name"] = JSONValue(name)
        o["host"] = JSONValue(host)
        o["address"] = JSONValue(address)
        o["port"] = JSONValue(port)
        o["machineIdentifier"] = JSONValue(machineIdentifier)
        o["version"] = JSONValue(version)
        o["protocol"] = JSONValue(`protocol`)
        o["product"] = JSONValue(product)
        o["deviceClass"] = JSONValue(deviceClass)
        o["protocolVersion"] = JSONValue(protocolVersion)
        o["protocolCapabilities"] = JSONValue(protocolCapabilities)
        return .object(o)
    }
}

public struct StatisticsMedia: PlexModel {
    public var accountID: Int? = nil
    public var deviceID: Int? = nil
    public var timespan: Int? = nil
    public var at: Int? = nil
    public var metadataType: Int? = nil
    public var count: Int? = nil
    public var duration: Int? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        accountID: Int? = nil,
        deviceID: Int? = nil,
        timespan: Int? = nil,
        at: Int? = nil,
        metadataType: Int? = nil,
        count: Int? = nil,
        duration: Int? = nil
    ) {
        self.accountID = accountID
        self.deviceID = deviceID
        self.timespan = timespan
        self.at = at
        self.metadataType = metadataType
        self.count = count
        self.duration = duration
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "StatisticsMedia")
        accountID = De.optI64(o["accountID"])
        deviceID = De.optI64(o["deviceID"])
        timespan = De.optI64(o["timespan"])
        at = De.optI64(o["at"])
        metadataType = De.optI64(o["metadataType"])
        count = De.optI64(o["count"])
        duration = De.optI64(o["duration"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["accountID"] = JSONValue(accountID)
        o["deviceID"] = JSONValue(deviceID)
        o["timespan"] = JSONValue(timespan)
        o["at"] = JSONValue(at)
        o["metadataType"] = JSONValue(metadataType)
        o["count"] = JSONValue(count)
        o["duration"] = JSONValue(duration)
        return .object(o)
    }
}

public struct StatisticsResources: PlexModel {
    public var timespan: Int? = nil
    public var at: Int? = nil
    public var hostCpuUtilization: Double? = nil
    public var processCpuUtilization: Double? = nil
    public var hostMemoryUtilization: Double? = nil
    public var processMemoryUtilization: Double? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        timespan: Int? = nil,
        at: Int? = nil,
        hostCpuUtilization: Double? = nil,
        processCpuUtilization: Double? = nil,
        hostMemoryUtilization: Double? = nil,
        processMemoryUtilization: Double? = nil
    ) {
        self.timespan = timespan
        self.at = at
        self.hostCpuUtilization = hostCpuUtilization
        self.processCpuUtilization = processCpuUtilization
        self.hostMemoryUtilization = hostMemoryUtilization
        self.processMemoryUtilization = processMemoryUtilization
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "StatisticsResources")
        timespan = De.optI64(o["timespan"])
        at = De.optI64(o["at"])
        hostCpuUtilization = De.optF64(o["hostCpuUtilization"])
        processCpuUtilization = De.optF64(o["processCpuUtilization"])
        hostMemoryUtilization = De.optF64(o["hostMemoryUtilization"])
        processMemoryUtilization = De.optF64(o["processMemoryUtilization"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["timespan"] = JSONValue(timespan)
        o["at"] = JSONValue(at)
        o["hostCpuUtilization"] = JSONValue(hostCpuUtilization)
        o["processCpuUtilization"] = JSONValue(processCpuUtilization)
        o["hostMemoryUtilization"] = JSONValue(hostMemoryUtilization)
        o["processMemoryUtilization"] = JSONValue(processMemoryUtilization)
        return .object(o)
    }
}

public struct StatisticsBandwidth: PlexModel {
    public var accountID: Int? = nil
    public var deviceID: Int? = nil
    public var timespan: Int? = nil
    public var at: Int? = nil
    public var lan: Bool = false
    public var bytes: Int? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        accountID: Int? = nil,
        deviceID: Int? = nil,
        timespan: Int? = nil,
        at: Int? = nil,
        lan: Bool = false,
        bytes: Int? = nil
    ) {
        self.accountID = accountID
        self.deviceID = deviceID
        self.timespan = timespan
        self.at = at
        self.lan = lan
        self.bytes = bytes
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "StatisticsBandwidth")
        accountID = De.optI64(o["accountID"])
        deviceID = De.optI64(o["deviceID"])
        timespan = De.optI64(o["timespan"])
        at = De.optI64(o["at"])
        lan = De.boolean(o["lan"])
        bytes = De.optI64(o["bytes"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["accountID"] = JSONValue(accountID)
        o["deviceID"] = JSONValue(deviceID)
        o["timespan"] = JSONValue(timespan)
        o["at"] = JSONValue(at)
        o["lan"] = JSONValue(lan)
        o["bytes"] = JSONValue(bytes)
        return .object(o)
    }
}

public struct Release: PlexModel {
    public var key: String? = nil
    public var version: String? = nil
    public var added: String? = nil
    public var fixed: String? = nil
    public var downloadURL: String? = nil
    public var state: String? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        key: String? = nil,
        version: String? = nil,
        added: String? = nil,
        fixed: String? = nil,
        downloadURL: String? = nil,
        state: String? = nil
    ) {
        self.key = key
        self.version = version
        self.added = added
        self.fixed = fixed
        self.downloadURL = downloadURL
        self.state = state
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Release")
        key = De.optString(o["key"])
        version = De.optString(o["version"])
        added = De.optString(o["added"])
        fixed = De.optString(o["fixed"])
        downloadURL = De.optString(o["downloadURL"])
        state = De.optString(o["state"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["key"] = JSONValue(key)
        o["version"] = JSONValue(version)
        o["added"] = JSONValue(added)
        o["fixed"] = JSONValue(fixed)
        o["downloadURL"] = JSONValue(downloadURL)
        o["state"] = JSONValue(state)
        return .object(o)
    }
}

public struct ButlerTask: PlexModel {
    public var name: String? = nil
    public var title: String? = nil
    public var description: String? = nil
    public var interval: Int? = nil
    public var enabled: Bool = false
    public var scheduleRandomized: Bool = false

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        name: String? = nil,
        title: String? = nil,
        description: String? = nil,
        interval: Int? = nil,
        enabled: Bool = false,
        scheduleRandomized: Bool = false
    ) {
        self.name = name
        self.title = title
        self.description = description
        self.interval = interval
        self.enabled = enabled
        self.scheduleRandomized = scheduleRandomized
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "ButlerTask")
        name = De.optString(o["name"])
        title = De.optString(o["title"])
        description = De.optString(o["description"])
        interval = De.optI64(o["interval"])
        enabled = De.boolean(o["enabled"])
        scheduleRandomized = De.boolean(o["scheduleRandomized"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["name"] = JSONValue(name)
        o["title"] = JSONValue(title)
        o["description"] = JSONValue(description)
        o["interval"] = JSONValue(interval)
        o["enabled"] = JSONValue(enabled)
        o["scheduleRandomized"] = JSONValue(scheduleRandomized)
        return .object(o)
    }
}

/// `/identity` response.
public struct Identity: PlexModel {
    public var machineIdentifier: String? = nil
    public var version: String? = nil
    public var claimed: Bool = false

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        machineIdentifier: String? = nil,
        version: String? = nil,
        claimed: Bool = false
    ) {
        self.machineIdentifier = machineIdentifier
        self.version = version
        self.claimed = claimed
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Identity")
        machineIdentifier = De.optString(o["machineIdentifier"])
        version = De.optString(o["version"])
        claimed = De.boolean(o["claimed"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["machineIdentifier"] = JSONValue(machineIdentifier)
        o["version"] = JSONValue(version)
        o["claimed"] = JSONValue(claimed)
        return .object(o)
    }
}

extension Setting {
    public static func valueString(_ s: Setting) -> String {
        switch s.value {
        case .string(let v): v
        case .null: ""
        default: s.value.serialized()
        }
    }

    /// `value:Label|value:Label` pairs as `(value, label)`.
    public static func enumOptions(_ s: Setting) -> [(String, String)] {
        (s.enumValues ?? "").split(separator: "|", omittingEmptySubsequences: true).map { pair in
            if let i = pair.firstIndex(of: ":") {
                return (String(pair[..<i]), String(pair[pair.index(after: i)...]))
            }
            return (String(pair), String(pair))
        }
    }
}
