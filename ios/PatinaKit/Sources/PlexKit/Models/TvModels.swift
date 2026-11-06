// Port of desktop/plex/models/tv.ts.
// plex.tv models: PIN, account, resources, Home users, friends (port of crates/plex/src/models/tv.rs).

import Foundation

/// A PIN used for the plex.tv/link sign-in flow.
public struct Pin: PlexModel {
    public var id: Int? = nil
    public var code: String? = nil
    public var authToken: String? = nil
    public var expiresAt: String? = nil
    public var expiresIn: Int? = nil
    public var trusted: Bool = false

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        id: Int? = nil,
        code: String? = nil,
        authToken: String? = nil,
        expiresAt: String? = nil,
        expiresIn: Int? = nil,
        trusted: Bool = false
    ) {
        self.id = id
        self.code = code
        self.authToken = authToken
        self.expiresAt = expiresAt
        self.expiresIn = expiresIn
        self.trusted = trusted
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Pin")
        id = De.optI64(o["id"])
        code = De.optString(o["code"])
        authToken = De.optString(o["authToken"])
        expiresAt = De.optString(o["expiresAt"])
        expiresIn = De.optI64(o["expiresIn"])
        trusted = De.boolean(o["trusted"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["id"] = JSONValue(id)
        o["code"] = JSONValue(code)
        o["authToken"] = JSONValue(authToken)
        o["expiresAt"] = JSONValue(expiresAt)
        o["expiresIn"] = JSONValue(expiresIn)
        o["trusted"] = JSONValue(trusted)
        return .object(o)
    }
}

/// The signed-in plex.tv account.
public struct User: PlexModel {
    public var id: Int? = nil
    public var uuid: String? = nil
    public var username: String? = nil
    public var title: String? = nil
    public var friendlyName: String? = nil
    public var email: String? = nil
    public var thumb: String? = nil
    public var authToken: String? = nil
    public var homeAdmin: Bool = false
    public var restricted: Bool = false
    public var subscription: Subscription? = nil
    /// Attributes Plex sent that are not modelled (serde's flattened `extra` map).
    public var extra: [String: JSONValue] = [:]

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        id: Int? = nil,
        uuid: String? = nil,
        username: String? = nil,
        title: String? = nil,
        friendlyName: String? = nil,
        email: String? = nil,
        thumb: String? = nil,
        authToken: String? = nil,
        homeAdmin: Bool = false,
        restricted: Bool = false,
        subscription: Subscription? = nil,
        extra: [String: JSONValue] = [:]
    ) {
        self.id = id
        self.uuid = uuid
        self.username = username
        self.title = title
        self.friendlyName = friendlyName
        self.email = email
        self.thumb = thumb
        self.authToken = authToken
        self.homeAdmin = homeAdmin
        self.restricted = restricted
        self.subscription = subscription
        self.extra = extra
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "User")
        id = De.optI64(o["id"])
        uuid = De.optString(o["uuid"])
        username = De.optString(o["username"])
        title = De.optString(o["title"])
        friendlyName = De.optString(o["friendlyName"])
        email = De.optString(o["email"])
        thumb = De.optString(o["thumb"])
        authToken = De.optString(o["authToken"])
        homeAdmin = De.boolean(o["homeAdmin"])
        restricted = De.boolean(o["restricted"])
        subscription = try De.optional(o["subscription"], Subscription.init(json:), key: "subscription")
        extra = De.extraOf(o, Self.keys)
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = extra
        o["id"] = JSONValue(id)
        o["uuid"] = JSONValue(uuid)
        o["username"] = JSONValue(username)
        o["title"] = JSONValue(title)
        o["friendlyName"] = JSONValue(friendlyName)
        o["email"] = JSONValue(email)
        o["thumb"] = JSONValue(thumb)
        o["authToken"] = JSONValue(authToken)
        o["homeAdmin"] = JSONValue(homeAdmin)
        o["restricted"] = JSONValue(restricted)
        o["subscription"] = subscription?.json ?? .null
        return .object(o)
    }

    /// The modelled keys; everything else goes to `extra`.
    static let keys: Set<String> = ["id", "uuid", "username", "title", "friendlyName", "email", "thumb", "authToken", "homeAdmin", "restricted", "subscription"]
}

public struct Subscription: PlexModel {
    public var active: Bool = false
    public var status: String? = nil
    public var plan: String? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        active: Bool = false,
        status: String? = nil,
        plan: String? = nil
    ) {
        self.active = active
        self.status = status
        self.plan = plan
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Subscription")
        active = De.boolean(o["active"])
        status = De.optString(o["status"])
        plan = De.optString(o["plan"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["active"] = JSONValue(active)
        o["status"] = JSONValue(status)
        o["plan"] = JSONValue(plan)
        return .object(o)
    }
}

/// A device linked to the account (servers, players) from `/api/v2/resources`.
public struct Resource: PlexModel {
    public var name: String? = nil
    public var product: String? = nil
    public var productVersion: String? = nil
    public var platform: String? = nil
    public var platformVersion: String? = nil
    public var device: String? = nil
    public var clientIdentifier: String? = nil
    public var createdAt: String? = nil
    public var lastSeenAt: String? = nil
    public var provides: String? = nil
    public var ownerId: Int? = nil
    public var sourceTitle: String? = nil
    public var publicAddress: String? = nil
    public var accessToken: String? = nil
    public var owned: Bool = false
    public var home: Bool = false
    public var synced: Bool = false
    public var relay: Bool = false
    public var presence: Bool = false
    public var httpsRequired: Bool = false
    public var publicAddressMatches: Bool = false
    public var connections: [Connection] = []

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        name: String? = nil,
        product: String? = nil,
        productVersion: String? = nil,
        platform: String? = nil,
        platformVersion: String? = nil,
        device: String? = nil,
        clientIdentifier: String? = nil,
        createdAt: String? = nil,
        lastSeenAt: String? = nil,
        provides: String? = nil,
        ownerId: Int? = nil,
        sourceTitle: String? = nil,
        publicAddress: String? = nil,
        accessToken: String? = nil,
        owned: Bool = false,
        home: Bool = false,
        synced: Bool = false,
        relay: Bool = false,
        presence: Bool = false,
        httpsRequired: Bool = false,
        publicAddressMatches: Bool = false,
        connections: [Connection] = []
    ) {
        self.name = name
        self.product = product
        self.productVersion = productVersion
        self.platform = platform
        self.platformVersion = platformVersion
        self.device = device
        self.clientIdentifier = clientIdentifier
        self.createdAt = createdAt
        self.lastSeenAt = lastSeenAt
        self.provides = provides
        self.ownerId = ownerId
        self.sourceTitle = sourceTitle
        self.publicAddress = publicAddress
        self.accessToken = accessToken
        self.owned = owned
        self.home = home
        self.synced = synced
        self.relay = relay
        self.presence = presence
        self.httpsRequired = httpsRequired
        self.publicAddressMatches = publicAddressMatches
        self.connections = connections
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Resource")
        name = De.optString(o["name"])
        product = De.optString(o["product"])
        productVersion = De.optString(o["productVersion"])
        platform = De.optString(o["platform"])
        platformVersion = De.optString(o["platformVersion"])
        device = De.optString(o["device"])
        clientIdentifier = De.optString(o["clientIdentifier"])
        createdAt = De.optString(o["createdAt"])
        lastSeenAt = De.optString(o["lastSeenAt"])
        provides = De.optString(o["provides"])
        ownerId = De.optI64(o["ownerId"])
        sourceTitle = De.optString(o["sourceTitle"])
        publicAddress = De.optString(o["publicAddress"])
        accessToken = De.optString(o["accessToken"])
        owned = De.boolean(o["owned"])
        home = De.boolean(o["home"])
        synced = De.boolean(o["synced"])
        relay = De.boolean(o["relay"])
        presence = De.boolean(o["presence"])
        httpsRequired = De.boolean(o["httpsRequired"])
        publicAddressMatches = De.boolean(o["publicAddressMatches"])
        connections = try De.list(o["connections"], Connection.init(json:), key: "connections")
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["name"] = JSONValue(name)
        o["product"] = JSONValue(product)
        o["productVersion"] = JSONValue(productVersion)
        o["platform"] = JSONValue(platform)
        o["platformVersion"] = JSONValue(platformVersion)
        o["device"] = JSONValue(device)
        o["clientIdentifier"] = JSONValue(clientIdentifier)
        o["createdAt"] = JSONValue(createdAt)
        o["lastSeenAt"] = JSONValue(lastSeenAt)
        o["provides"] = JSONValue(provides)
        o["ownerId"] = JSONValue(ownerId)
        o["sourceTitle"] = JSONValue(sourceTitle)
        o["publicAddress"] = JSONValue(publicAddress)
        o["accessToken"] = JSONValue(accessToken)
        o["owned"] = JSONValue(owned)
        o["home"] = JSONValue(home)
        o["synced"] = JSONValue(synced)
        o["relay"] = JSONValue(relay)
        o["presence"] = JSONValue(presence)
        o["httpsRequired"] = JSONValue(httpsRequired)
        o["publicAddressMatches"] = JSONValue(publicAddressMatches)
        o["connections"] = .array(connections.map(\.json))
        return .object(o)
    }
}

public struct Connection: PlexModel {
    public var `protocol`: String? = nil
    public var address: String? = nil
    public var port: Int? = nil
    public var uri: String? = nil
    public var local: Bool = false
    public var relay: Bool = false
    public var ipv6: Bool = false

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        protocol: String? = nil,
        address: String? = nil,
        port: Int? = nil,
        uri: String? = nil,
        local: Bool = false,
        relay: Bool = false,
        ipv6: Bool = false
    ) {
        self.`protocol` = `protocol`
        self.address = address
        self.port = port
        self.uri = uri
        self.local = local
        self.relay = relay
        self.ipv6 = ipv6
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Connection")
        `protocol` = De.optString(o["protocol"])
        address = De.optString(o["address"])
        port = De.optI64(o["port"])
        uri = De.optString(o["uri"])
        local = De.boolean(o["local"])
        relay = De.boolean(o["relay"])
        ipv6 = De.boolean(o["IPv6"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["protocol"] = JSONValue(`protocol`)
        o["address"] = JSONValue(address)
        o["port"] = JSONValue(port)
        o["uri"] = JSONValue(uri)
        o["local"] = JSONValue(local)
        o["relay"] = JSONValue(relay)
        o["IPv6"] = JSONValue(ipv6)
        return .object(o)
    }
}

/// A member of the Plex Home.
public struct HomeUser: PlexModel {
    public var id: Int? = nil
    public var uuid: String? = nil
    public var title: String? = nil
    public var username: String? = nil
    public var email: String? = nil
    public var thumb: String? = nil
    public var restricted: Bool = false
    /// Whether switching to this user requires a PIN.
    public var `protected`: Bool = false
    public var admin: Bool = false
    public var guest: Bool = false

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        id: Int? = nil,
        uuid: String? = nil,
        title: String? = nil,
        username: String? = nil,
        email: String? = nil,
        thumb: String? = nil,
        restricted: Bool = false,
        protected: Bool = false,
        admin: Bool = false,
        guest: Bool = false
    ) {
        self.id = id
        self.uuid = uuid
        self.title = title
        self.username = username
        self.email = email
        self.thumb = thumb
        self.restricted = restricted
        self.`protected` = `protected`
        self.admin = admin
        self.guest = guest
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "HomeUser")
        id = De.optI64(o["id"])
        uuid = De.optString(o["uuid"])
        title = De.optString(o["title"])
        username = De.optString(o["username"])
        email = De.optString(o["email"])
        thumb = De.optString(o["thumb"])
        restricted = De.boolean(o["restricted"])
        `protected` = De.boolean(o["protected"])
        admin = De.boolean(o["admin"])
        guest = De.boolean(o["guest"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["id"] = JSONValue(id)
        o["uuid"] = JSONValue(uuid)
        o["title"] = JSONValue(title)
        o["username"] = JSONValue(username)
        o["email"] = JSONValue(email)
        o["thumb"] = JSONValue(thumb)
        o["restricted"] = JSONValue(restricted)
        o["protected"] = JSONValue(`protected`)
        o["admin"] = JSONValue(admin)
        o["guest"] = JSONValue(guest)
        return .object(o)
    }
}

/// A friend / shared user.
public struct Friend: PlexModel {
    public var id: Int? = nil
    public var uuid: String? = nil
    public var title: String? = nil
    public var username: String? = nil
    public var email: String? = nil
    public var thumb: String? = nil
    public var restricted: Bool = false
    public var home: Bool = false
    public var status: String? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        id: Int? = nil,
        uuid: String? = nil,
        title: String? = nil,
        username: String? = nil,
        email: String? = nil,
        thumb: String? = nil,
        restricted: Bool = false,
        home: Bool = false,
        status: String? = nil
    ) {
        self.id = id
        self.uuid = uuid
        self.title = title
        self.username = username
        self.email = email
        self.thumb = thumb
        self.restricted = restricted
        self.home = home
        self.status = status
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Friend")
        id = De.optI64(o["id"])
        uuid = De.optString(o["uuid"])
        title = De.optString(o["title"])
        username = De.optString(o["username"])
        email = De.optString(o["email"])
        thumb = De.optString(o["thumb"])
        restricted = De.boolean(o["restricted"])
        home = De.boolean(o["home"])
        status = De.optString(o["status"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["id"] = JSONValue(id)
        o["uuid"] = JSONValue(uuid)
        o["title"] = JSONValue(title)
        o["username"] = JSONValue(username)
        o["email"] = JSONValue(email)
        o["thumb"] = JSONValue(thumb)
        o["restricted"] = JSONValue(restricted)
        o["home"] = JSONValue(home)
        o["status"] = JSONValue(status)
        return .object(o)
    }
}

/// Per-user state for a discover item (watchlist membership, views).
public struct UserState: PlexModel {
    public var watchlistedAt: Int? = nil
    public var viewCount: Int? = nil
    public var viewOffset: Int? = nil
    public var lastViewedAt: Int? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        watchlistedAt: Int? = nil,
        viewCount: Int? = nil,
        viewOffset: Int? = nil,
        lastViewedAt: Int? = nil
    ) {
        self.watchlistedAt = watchlistedAt
        self.viewCount = viewCount
        self.viewOffset = viewOffset
        self.lastViewedAt = lastViewedAt
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "UserState")
        watchlistedAt = De.optI64(o["watchlistedAt"])
        viewCount = De.optI64(o["viewCount"])
        viewOffset = De.optI64(o["viewOffset"])
        lastViewedAt = De.optI64(o["lastViewedAt"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["watchlistedAt"] = JSONValue(watchlistedAt)
        o["viewCount"] = JSONValue(viewCount)
        o["viewOffset"] = JSONValue(viewOffset)
        o["lastViewedAt"] = JSONValue(lastViewedAt)
        return .object(o)
    }
}

extension User {
    public static func displayName(_ u: User) -> String {
        let friendly = u.friendlyName.flatMap { $0.isEmpty ? nil : $0 }
        return friendly ?? u.title ?? u.username ?? "Plex user"
    }
}

extension Resource {
    public static func name(_ r: Resource) -> String {
        r.name ?? "Plex Media Server"
    }

    public static func isServer(_ r: Resource) -> Bool {
        (r.provides ?? "").split(separator: ",", omittingEmptySubsequences: false).contains("server")
    }

    public static func id(_ r: Resource) -> String {
        r.clientIdentifier ?? ""
    }
}

extension HomeUser {
    public static func name(_ h: HomeUser) -> String {
        h.title ?? h.username ?? "User"
    }
}
