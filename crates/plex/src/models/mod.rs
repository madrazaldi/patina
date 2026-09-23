//! Typed models for Plex responses.
//!
//! Numeric and boolean fields go through lenient deserializers because Plex
//! is not consistent about JSON types. Unknown attributes are preserved in
//! `extra` maps on the main containers so nothing the server says is lost.

mod container;
mod media;
mod metadata;
mod server;
mod tv;

pub use container::*;
pub use media::*;
pub use metadata::*;
pub use server::*;
pub use tv::*;
