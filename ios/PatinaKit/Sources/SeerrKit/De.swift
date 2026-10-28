// Lenient decoding of Seerr's JSON; no TypeScript twin. JavaScript reads a field
// of an unexpected type without complaint, so `desktop/seerr` takes the JSON as
// it comes. `Codable` would fail the whole response instead, so the models read
// each field through these helpers (named after `desktop/plex/de.ts`): a missing,
// null or odd field reads as nil.

import Foundation

extension KeyedDecodingContainer {
    /// A string, or a number as its text.
    func optString(_ key: Key) -> String? {
        if let s = try? decodeIfPresent(String.self, forKey: key) { return s }
        if let n = try? decodeIfPresent(Int.self, forKey: key) { return String(n) }
        if let d = try? decodeIfPresent(Double.self, forKey: key) { return String(d) }
        return nil
    }

    /// A whole number, or a string holding one.
    func optI64(_ key: Key) -> Int? {
        if let n = try? decodeIfPresent(Int.self, forKey: key) { return n }
        if let d = try? decodeIfPresent(Double.self, forKey: key) { return Int(exactly: d) }
        if let s = try? decodeIfPresent(String.self, forKey: key) { return Int(s.trimmingCharacters(in: .whitespaces)) }
        return nil
    }

    /// A number, or a string holding one.
    func optF64(_ key: Key) -> Double? {
        if let d = try? decodeIfPresent(Double.self, forKey: key) { return d }
        if let s = try? decodeIfPresent(String.self, forKey: key) { return Double(s.trimmingCharacters(in: .whitespaces)) }
        return nil
    }

    /// `true`/`false`, `1`/`0` or their strings.
    func boolean(_ key: Key) -> Bool? {
        if let b = try? decodeIfPresent(Bool.self, forKey: key) { return b }
        if let n = try? decodeIfPresent(Int.self, forKey: key) { return n != 0 }
        switch try? decodeIfPresent(String.self, forKey: key) {
        case "true", "1": return true
        case "false", "0": return false
        default: return nil
        }
    }

    /// A nested object (or enum), nil when it doesn't decode.
    func lenient<T: Decodable>(_ key: Key) -> T? {
        try? decodeIfPresent(T.self, forKey: key)
    }

    /// A list, or a single value as a list of one. Elements that don't decode are left out.
    func oneOrMany<T: Decodable>(_ key: Key) -> [T]? {
        if let list = try? decodeIfPresent([Lenient<T>].self, forKey: key) { return list.compactMap(\.value) }
        return lenient(key).map { [$0] }
    }
}

/// One element of a list, nil when it doesn't decode (decoding it still moves past it).
struct Lenient<T: Decodable>: Decodable {
    let value: T?

    init(from decoder: any Decoder) {
        value = try? T(from: decoder)
    }
}
