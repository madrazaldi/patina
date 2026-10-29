// What the app hands the core (no TypeScript twin): where files go, where secrets go, how to
// describe the device to Plex, and the HTTP session. On the desktop these come from the
// process (`homedir()`, `process.env`, the config file); on iOS the app passes them in, and
// tests pass temporary directories and an in-memory secret store.
//
// Named `AppEnvironment` because PatinaUI imports SwiftUI, whose `Environment` would clash.

import Foundation
import Security
import Synchronization

/// The directories the core writes to.
public nonisolated struct Paths: Sendable, Equatable {
    /// `config.json`, `downloads.json`, `offline-progress.json` (the desktop's config dir).
    public var config: URL
    /// Checkpoints, saved queues, artwork of downloads (the desktop's data dir).
    public var data: URL
    /// The artwork cache, which the system may purge.
    public var cache: URL
    /// Downloaded media; shown in the Files app.
    public var downloads: URL

    public init(config: URL, data: URL, cache: URL, downloads: URL) {
        self.config = config
        self.data = data
        self.cache = cache
        self.downloads = downloads
    }

    /// Application Support/Patina (config) and Application Support/Patina/data, Caches/Patina
    /// and Documents/Downloads.
    public static func standard() -> Paths {
        let fm = FileManager.default
        let support = fm.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("Patina")
        let caches = fm.urls(for: .cachesDirectory, in: .userDomainMask)[0].appendingPathComponent("Patina")
        let documents = fm.urls(for: .documentDirectory, in: .userDomainMask)[0]
        return Paths(
            config: support, data: support.appendingPathComponent("data"), cache: caches,
            downloads: documents.appendingPathComponent("Downloads"))
    }

    /// Everything under one directory (tests).
    public static func under(_ root: URL) -> Paths {
        Paths(
            config: root.appendingPathComponent("config"), data: root.appendingPathComponent("data"),
            cache: root.appendingPathComponent("cache"), downloads: root.appendingPathComponent("downloads"))
    }
}

/// Where tokens are kept: never in `config.json`.
public nonisolated protocol SecretStore: Sendable {
    func get(_ key: String) -> String?
    /// `nil` deletes the secret.
    func set(_ key: String, _ value: String?) throws
}

/// Secrets in the Keychain (generic passwords of service `io.github.madrazaldi.patina`,
/// available after the first unlock so background work can sign requests). Values are
/// cached: this process is the only writer.
public nonisolated final class KeychainSecretStore: SecretStore {
    public static let service = "io.github.madrazaldi.patina"
    private let service: String
    private let cache = Mutex<[String: String?]>([:])

    public init(service: String = KeychainSecretStore.service) {
        self.service = service
    }

    private func query(_ key: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service, kSecAttrAccount as String: key]
    }

    public func get(_ key: String) -> String? {
        if let hit = cache.withLock({ $0[key] }) { return hit }
        var q = query(key)
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var out: CFTypeRef?
        let status = SecItemCopyMatching(q as CFDictionary, &out)
        let value = status == errSecSuccess ? (out as? Data).map { String(decoding: $0, as: UTF8.self) } : nil
        // Don't remember a failure (the device may still be locked): only found or absent.
        if status == errSecSuccess || status == errSecItemNotFound { cache.withLock { $0[key] = .some(value) } }
        return value
    }

    public func set(_ key: String, _ value: String?) throws {
        let q = query(key)
        var status: OSStatus
        if let value {
            let attrs: [String: Any] = [
                kSecValueData as String: Data(value.utf8),
                kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
            ]
            status = SecItemUpdate(q as CFDictionary, attrs as CFDictionary)
            if status == errSecItemNotFound {
                status = SecItemAdd(q.merging(attrs) { $1 } as CFDictionary, nil)
            }
        } else {
            status = SecItemDelete(q as CFDictionary)
            if status == errSecItemNotFound { status = errSecSuccess }
        }
        guard status == errSecSuccess else {
            cache.withLock { _ = $0.removeValue(forKey: key) }
            let text = SecCopyErrorMessageString(status, nil).map { $0 as String } ?? "error \(status)"
            throw CoreError("Could not save to the Keychain: \(text)")
        }
        cache.withLock { $0[key] = .some(value) }
    }
}

/// Secrets in memory (tests, previews).
public nonisolated final class MemorySecretStore: SecretStore {
    private let values = Mutex<[String: String]>([:])

    public init(_ initial: [String: String] = [:]) {
        values.withLock { $0 = initial }
    }

    public func get(_ key: String) -> String? {
        values.withLock { $0[key] }
    }

    public func set(_ key: String, _ value: String?) throws {
        values.withLock { $0[key] = value }
    }

    /// Everything stored (tests).
    public var all: [String: String] { values.withLock { $0 } }
}

/// Everything `AppState` takes from its surroundings.
public nonisolated struct AppEnvironment: Sendable {
    public var paths: Paths
    public var secrets: any SecretStore
    /// `X-Plex-Platform`.
    public var platform: String
    /// `X-Plex-Platform-Version`: the OS version.
    public var platformVersion: String
    /// `X-Plex-Device`: the device model ("iPhone", "iPad").
    public var device: String
    /// `X-Plex-Device-Name`: the name the user gave the device.
    public var deviceName: String
    /// The session every Plex client the core creates uses; `nil` is PlexKit's default.
    /// Tests route it to a fake server (see `MockPMS`).
    public var urlSession: URLSession?
    /// How often the background poller runs; `nil` turns it off.
    public var tickInterval: Duration?

    public init(
        paths: Paths, secrets: any SecretStore, platform: String = "iOS", platformVersion: String = "",
        device: String, deviceName: String, urlSession: URLSession? = nil, tickInterval: Duration? = .seconds(2)
    ) {
        self.paths = paths
        self.secrets = secrets
        self.platform = platform
        self.platformVersion = platformVersion
        self.device = device
        self.deviceName = deviceName
        self.urlSession = urlSession
        self.tickInterval = tickInterval
    }

    /// The app's environment: the standard directories and the Keychain. The app passes the
    /// device model, name and OS version (`UIDevice`), which PatinaCore can't import.
    public static func live(device: String, deviceName: String, platformVersion: String) -> AppEnvironment {
        AppEnvironment(
            paths: .standard(), secrets: KeychainSecretStore(), platformVersion: platformVersion, device: device,
            deviceName: deviceName)
    }
}
