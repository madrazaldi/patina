// Port of desktop/plex/models/index.ts.
// Typed models for Plex responses (port of crates/plex/src/models/mod.rs).
//
// Models are structs holding exactly the fields the Rust structs serialize. Properties are the
// JSON keys in Swift casing (`Media` is `media`, `DOVIPresent` is `doviPresent`; `Guid` and
// `Rating` on `Metadata` are `guids` and `ratings`, next to the scalar `guid` and `rating`).
// `init(json:)` is the TypeScript `normalize*` function: it whitelists fields through the
// lenient `De` helpers. `json` writes the fields back under Plex's names (`nil` as null), so a
// stored model reads back as it was received. Attributes that are not modelled are kept in
// `extra` on the types that have a flattened map (`MediaContainer`, `Metadata`, `User`).
// The `impl` methods of the Rust types are static functions taking the value, as in the
// TypeScript namespaces: `Metadata.title(m)`.

import Foundation

public protocol PlexModel: Codable, Sendable, Hashable {
    /// `Type::default()`.
    init()
    /// The `normalize*` function: decode leniently from JSON.
    init(json: JSONValue) throws
    /// The serialized shape, with Plex's field names.
    var json: JSONValue { get }
}

extension PlexModel {
    /// `normalizeX(v)`.
    public static func normalize(_ v: JSONValue) throws -> Self {
        try Self(json: v)
    }

    public init(from decoder: Decoder) throws {
        try self.init(json: JSONValue(from: decoder))
    }

    public func encode(to encoder: Encoder) throws {
        try json.encode(to: encoder)
    }
}
