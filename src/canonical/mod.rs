//! Canonical serialization (ACP §8.2): every byte sequence ACP hashes or signs is the RFC 8785
//! (JSON Canonicalization Scheme) serialization of the normalized value, and nothing else. A
//! digest's preimage is exactly those bytes, with no trailing newline.
//!
//! Normalization (§8.1, ordering changes, reviews, commits and evidence) happens before this, in
//! [`crate::normalize`]; this module only turns a normalized value into bytes. [`legacy`] keeps
//! the ACP 0.2 serialization so that 0.2 documents still validate.

use anyhow::Result;
use sha2::{Digest, Sha256};

/// The RFC 8785 serialization of `value`: the preimage of every ACP digest and the exact bytes
/// `acc` writes for its JSON outputs.
pub fn jcs_bytes<T: serde::Serialize>(value: &T) -> Result<String> {
    Ok(serde_json_canonicalizer::to_string(value)?)
}

/// `sha256:<hex>` over [`jcs_bytes`].
pub fn digest<T: serde::Serialize>(value: &T) -> Result<String> {
    Ok(sha256(jcs_bytes(value)?.as_bytes()))
}

/// `sha256:<hex>` over raw bytes.
pub fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

/// Which serialization a document's digests were computed with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Canonicalization {
    /// RFC 8785, ACP 0.3 and later.
    Jcs,
    /// The ACP 0.2 project canonicalization, read for validation only.
    Legacy,
}

impl Canonicalization {
    pub fn bytes<T: serde::Serialize>(self, value: &T) -> Result<String> {
        match self {
            Self::Jcs => jcs_bytes(value),
            Self::Legacy => legacy::bytes(value),
        }
    }

    pub fn digest<T: serde::Serialize>(self, value: &T) -> Result<String> {
        Ok(sha256(self.bytes(value)?.as_bytes()))
    }
}

/// The ACP 0.2 serialization: sorted keys (by UTF-8 bytes), compact separators and one trailing
/// LF, which was part of every preimage. It agrees with RFC 8785 on everything ACP 0.2 documents
/// contain except that LF. Only `acc validate` uses it, to check documents written by acc 0.4.
pub mod legacy {
    use anyhow::Result;

    pub fn bytes<T: serde::Serialize>(value: &T) -> Result<String> {
        Ok(serde_json::to_string(&serde_json::to_value(value)?)? + "\n")
    }

    pub fn digest<T: serde::Serialize>(value: &T) -> Result<String> {
        Ok(super::sha256(bytes(value)?.as_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn jcs_sorts_keys_compactly_without_a_trailing_newline() {
        let v = json!({"b": 1, "a": {"z": true, "y": null}});
        assert_eq!(jcs_bytes(&v).unwrap(), r#"{"a":{"y":null,"z":true},"b":1}"#);
        assert_eq!(
            legacy::bytes(&v).unwrap(),
            "{\"a\":{\"y\":null,\"z\":true},\"b\":1}\n"
        );
    }

    #[test]
    fn digests_are_over_the_exact_jcs_bytes() {
        let v = json!(["acme/api", "x"]);
        let d = digest(&v).unwrap();
        assert_eq!(d, sha256(br#"["acme/api","x"]"#));
        assert_eq!(d.len(), "sha256:".len() + 64);
        assert_eq!(Canonicalization::Jcs.digest(&v).unwrap(), d);
        assert_eq!(
            Canonicalization::Legacy.digest(&v).unwrap(),
            sha256(b"[\"acme/api\",\"x\"]\n")
        );
    }

    #[test]
    fn keys_sort_by_utf16_code_units_not_utf8_bytes() {
        // U+FB33 sorts after U+1F602 in UTF-16 (0xFB33 > 0xD83D) and before it in UTF-8.
        let v = json!({"\u{fb33}": 1, "\u{1f602}": 2});
        assert_eq!(jcs_bytes(&v).unwrap(), "{\"\u{1f602}\":2,\"\u{fb33}\":1}");
    }

    /// RFC 8785 string cases from cyberphone/json-canonicalization `testdata`: `weird.json` and
    /// `unicode.json`, which exercise escaping, non-ASCII output and UTF-16 key ordering.
    #[test]
    fn rfc8785_reference_vectors() {
        let weird = r#"{
  "\u20ac": "Euro Sign",
  "\r": "Carriage Return",
  "\u000a": "Newline",
  "1": "One",
  "\u0080": "Control\u007f",
  "\ud83d\ude02": "Smiley",
  "\u00f6": "Latin Small Letter O With Diaeresis",
  "\ufb33": "Hebrew Letter Dalet With Dagesh",
  "</script>": "Browser Challenge"
}"#;
        let expected = "{\"\\n\":\"Newline\",\"\\r\":\"Carriage Return\",\"1\":\"One\",\
\"</script>\":\"Browser Challenge\",\"\u{80}\":\"Control\u{7f}\",\
\"\u{f6}\":\"Latin Small Letter O With Diaeresis\",\"\u{20ac}\":\"Euro Sign\",\
\"\u{1f602}\":\"Smiley\",\"\u{fb33}\":\"Hebrew Letter Dalet With Dagesh\"}";
        let v: serde_json::Value = serde_json::from_str(weird).unwrap();
        assert_eq!(jcs_bytes(&v).unwrap(), expected);
        let unicode: serde_json::Value =
            serde_json::from_str(r#"{"Unnormalized Unicode":"A\u030a"}"#).unwrap();
        assert_eq!(
            jcs_bytes(&unicode).unwrap(),
            "{\"Unnormalized Unicode\":\"A\u{30a}\"}"
        );
        let controls = json!({"s": "\u{0}\u{8}\u{9}\u{c}\u{1f}\"\\/"});
        assert_eq!(
            jcs_bytes(&controls).unwrap(),
            r#"{"s":"\u0000\b\t\f\u001f\"\\/"}"#
        );
        let ints = json!([0, -1, 9007199254740991_i64, -9007199254740991_i64]);
        assert_eq!(
            jcs_bytes(&ints).unwrap(),
            "[0,-1,9007199254740991,-9007199254740991]"
        );
    }
}
