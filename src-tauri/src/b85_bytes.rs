//! `#[serde(with = "crate::b85_bytes")]` adapter: serialize a `Vec<u8>` as a base85 (RFC1924)
//! string instead of serde's default array of decimal integers.
//!
//! Why: `serde_json` writes `Vec<u8>` as `[0,12,255,…]` — ~3.7 characters per byte. The cached
//! analysis reports (`tracks.report_json`) and the tag-edit undo snapshots (`actions.meta`) are
//! dominated by that inflation. RFC1924 base85 costs 1.25 characters per byte, and its alphabet
//! (`0-9A-Za-z!#$%&()*+-;<=>?@^_`{|}~`) contains neither `"` nor `\`, so nothing is ever escaped
//! inside JSON.
//!
//! The Rust type stays `Vec<u8>`: only the wire/at-rest ENCODING changes, so every literal
//! construction and every `is_empty()` check in the codebase keeps working unchanged.
//!
//! Deserialization is deliberately TOLERANT — it accepts both the base85 string and the historic
//! integer array. That is mandatory for `actions.meta`: those rows hold cover bytes that are NOT
//! recomputable, so a failed decode is not a cache miss, it is the permanent loss of an undo
//! (see `tagging::TagsSnapshot`). It is free for `report_json`, which is recomputable.

use serde::de::{self, Deserializer, SeqAccess, Visitor};
use serde::Serializer;
use std::fmt;

/// Encodes the bytes as an RFC1924 base85 string. `&[]` encodes to `""`.
pub fn serialize<S>(v: &[u8], s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(&base85::encode(v))
}

struct B85BytesVisitor;

impl<'de> Visitor<'de> for B85BytesVisitor {
    type Value = Vec<u8>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("base85 string or byte array")
    }

    /// Current format.
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        base85::decode(v).map_err(|e| E::custom(format!("base85 decode: {e}")))
    }

    /// Historic format: a JSON array of decimal integers. Kept forever for `actions.meta`.
    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut out = Vec::with_capacity(seq.size_hint().unwrap_or(0));
        while let Some(b) = seq.next_element::<u8>()? {
            out.push(b);
        }
        Ok(out)
    }

    /// Non-JSON formats that carry bytes natively (robustness; unused today).
    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(v.to_vec())
    }

    fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(v)
    }
}

/// Accepts a base85 string (current) OR an array of integers (historic). `""` yields `vec![]`.
pub fn deserialize<'de, D>(d: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    d.deserialize_any(B85BytesVisitor)
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Holder {
        #[serde(with = "super")]
        b: Vec<u8>,
    }

    /// Every length 0..=17 covers the empty case and all four `n % 4` remainders. The crate
    /// encodes a remainder of `m` bytes on `m + 1` characters and, when decoding, fills the
    /// missing digits with the value 126 — this test freezes that the round-trip is exact for
    /// every remainder anyway.
    #[test]
    fn round_trips_every_length_and_remainder() {
        for n in 0..=17usize {
            let src: Vec<u8> = (0..n).map(|i| (i * 37 + 11) as u8).collect();
            let j = serde_json::to_string(&Holder { b: src.clone() }).unwrap();
            let back: Holder = serde_json::from_str(&j).unwrap();
            assert_eq!(back.b, src, "round-trip failed at length {n}");
        }
    }

    /// The RFC1924 alphabet contains neither `"` nor `\`, so a base85 payload is never escaped
    /// inside JSON. Freezing it here: an alphabet change would silently re-inflate every report.
    #[test]
    fn full_byte_range_round_trips_and_never_needs_json_escaping() {
        let src: Vec<u8> = (0..=255u8).chain(0..=255u8).collect();
        let j = serde_json::to_string(&Holder { b: src.clone() }).unwrap();
        let payload = j
            .split_once("\"b\":\"")
            .and_then(|(_, rest)| rest.split_once('"'))
            .map(|(p, _)| p.to_string())
            .expect("field must serialize as a JSON string");
        assert!(!payload.contains('"'), "base85 payload contains a quote");
        assert!(
            !payload.contains('\\'),
            "base85 payload contains a backslash"
        );
        let back: Holder = serde_json::from_str(&j).unwrap();
        assert_eq!(back.b, src);
    }

    /// Frozen cross-implementation vector: any independent decoder (e.g. the frontend one) must
    /// map this exact string back to the bytes 0x00..=0x0F. Do not regenerate it from the code
    /// it is meant to check.
    #[test]
    fn frozen_vector_matches_the_reference_encoding() {
        let src: Vec<u8> = (0u8..16).collect();
        let j = serde_json::to_string(&Holder { b: src.clone() }).unwrap();
        assert_eq!(j, r#"{"b":"009C61O)~M2nh-c3=Iws"}"#);
        let back: Holder = serde_json::from_str(&j).unwrap();
        assert_eq!(back.b, src);
    }

    /// Backward compatibility: rows written before this change hold an integer array.
    #[test]
    fn accepts_the_historic_integer_array_format() {
        let back: Holder = serde_json::from_str(r#"{"b":[1,2,3]}"#).unwrap();
        assert_eq!(back.b, vec![1u8, 2, 3]);
        let empty: Holder = serde_json::from_str(r#"{"b":[]}"#).unwrap();
        assert!(empty.b.is_empty());
    }

    /// An empty vector must survive as an empty vector: `ipc.rs` uses `mag_db.is_empty()` as the
    /// "spectrogram not computed" sentinel, so a lossy empty round-trip would break cache reads.
    #[test]
    fn empty_stays_empty_through_the_string_form() {
        let j = serde_json::to_string(&Holder { b: vec![] }).unwrap();
        assert_eq!(j, r#"{"b":""}"#);
        let back: Holder = serde_json::from_str(&j).unwrap();
        assert!(back.b.is_empty());
    }

    /// A malformed payload must fail loudly, never decode to garbage bytes.
    #[test]
    fn rejects_a_character_outside_the_alphabet() {
        let e = serde_json::from_str::<Holder>(r#"{"b":"00 9C"}"#).unwrap_err();
        assert!(
            e.to_string().contains("base85 decode"),
            "unexpected error: {e}"
        );
    }
}
