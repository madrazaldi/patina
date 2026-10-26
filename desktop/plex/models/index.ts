// Typed models for Plex responses (port of crates/plex/src/models/mod.rs).
//
// Models are plain objects holding exactly the fields the Rust structs serialize (same
// names, `null` for absent `Option`s, `[]` and `false` as defaults). They are built by
// `normalize*` functions that whitelist fields; attributes that are not modelled are kept
// next to them on the containers that have a flattened `extra` map (read with `.extra()`).
// The `impl` methods of the Rust types live in namespaces of the same name, taking the
// value as first argument: `m.title()` becomes `Metadata.title(m)`.

export * from "./container";
export * from "./media";
export * from "./metadata";
export * from "./server";
export * from "./tv";
