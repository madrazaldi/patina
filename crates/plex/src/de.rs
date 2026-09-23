//! Lenient deserializers. Plex is inconsistent about JSON types: ids come as
//! strings or numbers, booleans as `true`, `1` or `"1"`, and so on.

use serde::de::{self, Deserializer, Visitor};
use std::fmt;

struct AnyScalar;

enum Scalar {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Null,
}

impl<'de> Visitor<'de> for AnyScalar {
    type Value = Scalar;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a scalar value")
    }
    fn visit_bool<E>(self, v: bool) -> Result<Scalar, E> {
        Ok(Scalar::Bool(v))
    }
    fn visit_i64<E>(self, v: i64) -> Result<Scalar, E> {
        Ok(Scalar::Int(v))
    }
    fn visit_u64<E>(self, v: u64) -> Result<Scalar, E> {
        Ok(Scalar::Int(v as i64))
    }
    fn visit_f64<E>(self, v: f64) -> Result<Scalar, E> {
        Ok(Scalar::Float(v))
    }
    fn visit_str<E>(self, v: &str) -> Result<Scalar, E> {
        Ok(Scalar::Str(v.to_string()))
    }
    fn visit_string<E>(self, v: String) -> Result<Scalar, E> {
        Ok(Scalar::Str(v))
    }
    fn visit_none<E>(self) -> Result<Scalar, E> {
        Ok(Scalar::Null)
    }
    fn visit_unit<E>(self) -> Result<Scalar, E> {
        Ok(Scalar::Null)
    }
    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Scalar, D::Error> {
        d.deserialize_any(AnyScalar)
    }
    // Objects/arrays where a scalar is expected are treated as absent.
    fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Scalar, A::Error> {
        while map.next_entry::<de::IgnoredAny, de::IgnoredAny>()?.is_some() {}
        Ok(Scalar::Null)
    }
    fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Scalar, A::Error> {
        while seq.next_element::<de::IgnoredAny>()?.is_some() {}
        Ok(Scalar::Null)
    }
}

pub fn opt_string<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(match d.deserialize_any(AnyScalar)? {
        Scalar::Str(s) => Some(s),
        Scalar::Int(i) => Some(i.to_string()),
        Scalar::Float(f) => Some(f.to_string()),
        Scalar::Bool(b) => Some(b.to_string()),
        Scalar::Null => None,
    })
}

pub fn opt_i64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    Ok(match d.deserialize_any(AnyScalar)? {
        Scalar::Int(i) => Some(i),
        Scalar::Float(f) => Some(f as i64),
        Scalar::Str(s) => s.trim().parse::<f64>().ok().map(|f| f as i64),
        Scalar::Bool(b) => Some(b as i64),
        Scalar::Null => None,
    })
}

pub fn opt_f64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    Ok(match d.deserialize_any(AnyScalar)? {
        Scalar::Int(i) => Some(i as f64),
        Scalar::Float(f) => Some(f),
        Scalar::Str(s) => s.trim().parse().ok(),
        Scalar::Bool(b) => Some(b as i64 as f64),
        Scalar::Null => None,
    })
}

pub fn boolean<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(match d.deserialize_any(AnyScalar)? {
        Scalar::Bool(b) => b,
        Scalar::Int(i) => i != 0,
        Scalar::Float(f) => f != 0.0,
        Scalar::Str(s) => matches!(s.as_str(), "1" | "true" | "True" | "yes"),
        Scalar::Null => false,
    })
}
