//! `u64` on the wire: a decimal string, never a JSON number, so JavaScript
//! cannot round it through a 53-bit `Number`.

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// A `u64` serialized as its canonical decimal string: no sign, no leading
/// zeros (except `"0"`), at most 20 digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct U64String(pub u64);

impl U64String {
    /// Parses the canonical form; anything else is rejected.
    pub fn parse(s: &str) -> Option<Self> {
        let canonical = !s.is_empty()
            && s.len() <= 20
            && s.bytes().all(|b| b.is_ascii_digit())
            && (s == "0" || !s.starts_with('0'));
        if !canonical {
            return None;
        }
        s.parse().ok().map(Self)
    }
}

impl From<u64> for U64String {
    fn from(v: u64) -> Self {
        Self(v)
    }
}

impl Serialize for U64String {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for U64String {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = Cow::<'de, str>::deserialize(d)?;
        Self::parse(&s)
            .ok_or_else(|| de::Error::custom("expected a u64 as a canonical decimal string"))
    }
}

impl JsonSchema for U64String {
    fn schema_name() -> Cow<'static, str> {
        "U64String".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "A u64 as a canonical decimal string (no sign or leading zeros). Never convert it to a JavaScript Number unless it is known to be below 2^53.",
            "type": "string",
            "pattern": "^(0|[1-9][0-9]{0,19})$"
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_the_full_range() {
        for v in [0, 1, 9_007_199_254_740_993, u64::MAX] {
            let json = serde_json::to_string(&U64String(v)).unwrap();
            assert_eq!(json, format!("\"{v}\""));
            assert_eq!(
                serde_json::from_str::<U64String>(&json).unwrap(),
                U64String(v)
            );
        }
    }

    #[test]
    fn rejects_non_canonical_forms() {
        for bad in [
            "\"\"",
            "\"01\"",
            "\"-1\"",
            "\"+1\"",
            "\" 1\"",
            "\"1.0\"",
            "\"18446744073709551616\"",
            "\"123456789012345678901\"",
            "1",
            "null",
        ] {
            assert!(
                serde_json::from_str::<U64String>(bad).is_err(),
                "{bad} accepted"
            );
        }
    }
}
