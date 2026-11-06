// Port of desktop/plex/de.ts.
// Lenient decoding helpers (port of crates/plex/src/de.rs). Plex is inconsistent about JSON
// types: ids come as strings or numbers, booleans as `true`, `1` or `"1"`, and so on.
// Models are built by explicit normalizers that copy only whitelisted fields; the helpers
// here do the scalar coercions and the single-object-where-a-list-is-expected handling.

import Foundation

/// A decoding problem, with the serde_path_to_error style path of the offending value.
public struct DecodeFailure: Error, LocalizedError, Sendable, Hashable {
    /// Dotted path such as `MediaContainer.Metadata[0].Media`; empty for the root.
    public let path: String
    /// The message without the path.
    public let detail: String

    public init(_ path: String, _ detail: String) {
        self.path = path
        self.detail = detail
    }

    public var message: String { path.isEmpty ? detail : "\(path): \(detail)" }
    public var errorDescription: String? { message }

    /// The same failure seen from inside `segment` (a field name or `name[index]`).
    public func under(_ segment: String) -> DecodeFailure {
        let p = path.isEmpty ? segment : path.hasPrefix("[") ? segment + path : "\(segment).\(path)"
        return DecodeFailure(p, detail)
    }
}

public enum De {
    /// serde's wording for "what we found" in `invalid type` errors.
    static func describe(_ v: JSONValue?) -> String {
        switch v {
        case nil, .null?: return "null"
        case .bool(let b)?: return "boolean `\(b)`"
        case .number(let n)?:
            return n.rounded(.towardZero) == n && n.isFinite
                ? "integer `\(jsNumberString(n))`" : "floating point `\(jsNumberString(n))`"
        case .string(let s)?: return "string \(JSONValue.string(s).serialized())"
        case .array?: return "sequence"
        case .object?: return "map"
        }
    }

    /// Asserts `v` is a JSON object (what a serde struct needs) and returns it.
    public static func obj(_ v: JSONValue?, _ expected: String) throws -> [String: JSONValue] {
        guard case .object(let o)? = v else {
            throw DecodeFailure("", "invalid type: \(describe(v)), expected struct \(expected)")
        }
        return o
    }

    /// Rust's `f64 as i64`: truncates, saturates and maps NaN to 0.
    static func toI64(_ f: Double) -> Int {
        if f.isNaN { return 0 }
        if f >= 9223372036854775808.0 { return Int.max }
        if f <= -9223372036854775808.0 { return -Int.max }
        return Int(f.rounded(.towardZero))
    }

    /// Rust's `str::parse::<f64>()` (no surrounding whitespace, accepts inf/nan).
    static func parseF64(_ s: String) -> Double? {
        if s.wholeMatch(of: #/[+-]?(\d+\.?\d*([eE][+-]?\d+)?|\.\d+([eE][+-]?\d+)?)/#) != nil { return Double(s) }
        let lower = s.lowercased()
        if lower.wholeMatch(of: #/[+-]?(inf|infinity)/#) != nil { return lower.hasPrefix("-") ? -.infinity : .infinity }
        if lower.wholeMatch(of: #/[+-]?nan/#) != nil { return .nan }
        return nil
    }

    private static func trim(_ s: String) -> String {
        s.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    public static func optString(_ v: JSONValue?) -> String? {
        switch v {
        case .string(let s)?: return s
        case .number(let n)?: return jsNumberString(n)
        case .bool(let b)?: return String(b)
        default: return nil
        }
    }

    public static func optI64(_ v: JSONValue?) -> Int? {
        switch v {
        case .number(let n)?: return toI64(n)
        case .string(let s)?: return parseF64(trim(s)).map(toI64)
        case .bool(let b)?: return b ? 1 : 0
        default: return nil
        }
    }

    public static func optF64(_ v: JSONValue?) -> Double? {
        switch v {
        case .number(let n)?: return n
        case .string(let s)?: return parseF64(trim(s))
        case .bool(let b)?: return b ? 1 : 0
        default: return nil
        }
    }

    public static func boolean(_ v: JSONValue?) -> Bool {
        switch v {
        case .bool(let b)?: return b
        case .number(let n)?: return n != 0
        case .string(let s)?: return s == "1" || s == "true" || s == "True" || s == "yes"
        default: return false
        }
    }

    /// A list that Plex sometimes sends as a single object (e.g. `OnDeck.Metadata`).
    public static func oneOrMany<T>(_ v: JSONValue?, _ f: (JSONValue) throws -> T, key: String = "") throws -> [T] {
        switch v {
        case nil, .null?: return []
        case .array(let items)?: return try mapItems(items, f, key)
        case let item?: return [try underKey(key) { try f(item) }]
        }
    }

    /// A plain `Vec<T>` field: absent means empty.
    public static func list<T>(_ v: JSONValue?, _ f: (JSONValue) throws -> T, key: String = "") throws -> [T] {
        switch v {
        case nil, .null?: return []
        case .array(let items)?: return try mapItems(items, f, key)
        default: throw DecodeFailure(key, "invalid type: \(describe(v)), expected a sequence")
        }
    }

    /// An `Option<T>` struct field: absent or null means `nil`.
    public static func optional<T>(_ v: JSONValue?, _ f: (JSONValue) throws -> T, key: String = "") throws -> T? {
        switch v {
        case nil, .null?: return nil
        case let item?: return try underKey(key) { try f(item) }
        }
    }

    /// A `serde_json::Value` field: absent means null.
    public static func value(_ v: JSONValue?) -> JSONValue {
        v ?? .null
    }

    /// The attributes of `o` that are not in `known` (serde's `#[serde(flatten)]` catch-all map).
    public static func extraOf(_ o: [String: JSONValue], _ known: Set<String>) -> [String: JSONValue] {
        o.filter { !known.contains($0.key) }
    }

    private static func mapItems<T>(_ items: [JSONValue], _ f: (JSONValue) throws -> T, _ key: String) throws -> [T] {
        var out: [T] = []
        out.reserveCapacity(items.count)
        for (i, item) in items.enumerated() {
            out.append(try underKey("\(key)[\(i)]") { try f(item) })
        }
        return out
    }

    private static func underKey<T>(_ segment: String, _ f: () throws -> T) throws -> T {
        do {
            return try f()
        } catch let e as DecodeFailure where !segment.isEmpty {
            throw e.under(segment)
        }
    }

    /// Navigate a JSON value like `Value::pointer` (`/A/B`). `nil` when the path does not exist.
    public static func pointer(_ v: JSONValue?, _ path: String) -> JSONValue? {
        var cur = v
        for seg in path.split(separator: "/", omittingEmptySubsequences: false).dropFirst() {
            let key = seg.replacingOccurrences(of: "~1", with: "/").replacingOccurrences(of: "~0", with: "~")
            switch cur {
            case .array(let a)?:
                guard key.wholeMatch(of: #/0|[1-9]\d*/#) != nil, let i = Int(key), a.indices.contains(i) else { return nil }
                cur = a[i]
            case .object(let o)?:
                guard let next = o[key] else { return nil }
                cur = next
            default:
                return nil
            }
        }
        return cur
    }
}
