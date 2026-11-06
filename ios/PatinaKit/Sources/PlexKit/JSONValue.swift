// A JSON value: what `unknown` holds in desktop/plex (no TypeScript twin). The lenient
// decoders in `De.swift` read it, models write it back, and raw endpoints return it.

import Foundation

public enum JSONValue: Sendable, Hashable {
    case null
    case bool(Bool)
    /// JavaScript numbers: integers and floats alike.
    case number(Double)
    case string(String)
    case array([JSONValue])
    case object([String: JSONValue])

    public init(_ v: String?) { self = v.map { .string($0) } ?? .null }
    public init(_ v: Int?) { self = v.map { .number(Double($0)) } ?? .null }
    public init(_ v: Double?) { self = v.map { .number($0) } ?? .null }
    public init(_ v: Bool) { self = .bool(v) }

    /// `JSON.parse`. Top-level scalars are accepted.
    public static func parse(_ data: Data) throws -> JSONValue {
        try JSONDecoder().decode(JSONValue.self, from: data)
    }

    public static func parse(_ text: String) throws -> JSONValue {
        try parse(Data(text.utf8))
    }

    /// `JSON.stringify` (keys sorted, so the output is stable).
    public func serialized() -> String {
        let enc = JSONEncoder()
        enc.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        guard let data = try? enc.encode(self) else { return "null" }
        return String(decoding: data, as: UTF8.self)
    }

    public var isNull: Bool { self == .null }

    public var stringValue: String? {
        if case .string(let s) = self { return s }
        return nil
    }

    public var doubleValue: Double? {
        if case .number(let n) = self { return n }
        return nil
    }

    /// The number as an integer, when it is one.
    public var intValue: Int? {
        if case .number(let n) = self, n.rounded(.towardZero) == n, abs(n) < 9.2e18 { return Int(n) }
        return nil
    }

    public var boolValue: Bool? {
        if case .bool(let b) = self { return b }
        return nil
    }

    public var arrayValue: [JSONValue]? {
        if case .array(let a) = self { return a }
        return nil
    }

    public var objectValue: [String: JSONValue]? {
        if case .object(let o) = self { return o }
        return nil
    }

    /// Member of an object (`nil` when absent or not an object).
    public subscript(key: String) -> JSONValue? {
        if case .object(let o) = self { return o[key] }
        return nil
    }

    /// Element of an array (`nil` when out of range or not an array).
    public subscript(index: Int) -> JSONValue? {
        if case .array(let a) = self, a.indices.contains(index) { return a[index] }
        return nil
    }
}

extension JSONValue: Codable {
    public init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil() {
            self = .null
        } else if let b = try? c.decode(Bool.self) {
            self = .bool(b)
        } else if let n = try? c.decode(Double.self) {
            self = .number(n)
        } else if let s = try? c.decode(String.self) {
            self = .string(s)
        } else if let a = try? c.decode([JSONValue].self) {
            self = .array(a)
        } else {
            self = .object(try c.decode([String: JSONValue].self))
        }
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        switch self {
        case .null:
            try c.encodeNil()
        case .bool(let b):
            try c.encode(b)
        case .number(let n):
            // Like JSON.stringify: integral values without a fraction, non-finite ones as null.
            if !n.isFinite {
                try c.encodeNil()
            } else if n.rounded(.towardZero) == n, abs(n) < 9.2e18 {
                try c.encode(Int64(n))
            } else {
                try c.encode(n)
            }
        case .string(let s):
            try c.encode(s)
        case .array(let a):
            try c.encode(a)
        case .object(let o):
            try c.encode(o)
        }
    }
}

extension JSONValue: ExpressibleByNilLiteral, ExpressibleByBooleanLiteral, ExpressibleByIntegerLiteral,
    ExpressibleByFloatLiteral, ExpressibleByStringLiteral, ExpressibleByArrayLiteral, ExpressibleByDictionaryLiteral
{
    public init(nilLiteral: ()) { self = .null }
    public init(booleanLiteral value: Bool) { self = .bool(value) }
    public init(integerLiteral value: Int) { self = .number(Double(value)) }
    public init(floatLiteral value: Double) { self = .number(value) }
    public init(stringLiteral value: String) { self = .string(value) }
    public init(arrayLiteral elements: JSONValue...) { self = .array(elements) }
    public init(dictionaryLiteral elements: (String, JSONValue)...) {
        self = .object(Dictionary(elements, uniquingKeysWith: { _, last in last }))
    }
}

/// JavaScript's `String(number)`.
func jsNumberString(_ n: Double) -> String {
    if n.isNaN { return "NaN" }
    if n.isInfinite { return n < 0 ? "-Infinity" : "Infinity" }
    if n.rounded(.towardZero) == n, abs(n) < 1e21 {
        if abs(n) < 9.2e18 { return String(Int64(n)) }
        return String(format: "%.0f", n)
    }
    return "\(n)"
}
