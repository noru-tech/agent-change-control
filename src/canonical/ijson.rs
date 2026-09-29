//! I-JSON input constraints (RFC 7493, tightened by ACP §8.2).
//!
//! An ACP document is I-JSON with these limits: every number is an integer literal (no fraction,
//! no exponent) in ±(2^53 − 1); strings contain no unpaired surrogate escapes; member names are
//! unique within each object; and nesting is at most [`MAX_DEPTH`] deep, counting the outermost
//! container as depth 1. [`parse`] reads JSON text into a value while enforcing all of them and
//! stops at the first container past the depth bound without reading further. [`check_value`]
//! applies the constraints that survive into a parsed value (numbers and depth) to YAML input,
//! whose parser already rejects duplicate keys and cannot produce surrogates.
//!
//! Violations carry the codes ACV005 to ACV009. Messages give a byte offset, never input text.

use anyhow::{Result, anyhow};
use serde_json::{Map, Value};

/// The deepest nesting an ACP document may have; the outermost container is depth 1.
pub const MAX_DEPTH: usize = 128;
/// The largest integer magnitude an ACP document may carry, 2^53 − 1.
pub const MAX_INTEGER: u64 = (1 << 53) - 1;

pub const NON_INTEGER: &str = "ACV005";
pub const INTEGER_RANGE: &str = "ACV006";
pub const UNPAIRED_SURROGATE: &str = "ACV007";
pub const DUPLICATE_MEMBER: &str = "ACV008";
pub const TOO_DEEP: &str = "ACV009";

enum Stop {
    /// Not JSON at all; the caller may try another syntax.
    Syntax,
    /// JSON that violates an ACP constraint.
    Violation(anyhow::Error),
}

type Step<T> = std::result::Result<T, Stop>;

fn violation<T>(code: &str, what: &str, at: usize) -> Step<T> {
    Err(Stop::Violation(anyhow!("{code} {what} at byte {at}")))
}

/// Parse JSON text under the ACP constraints. `Ok(None)` means the text is not JSON (it may be
/// YAML); an error means it is JSON that violates a constraint.
pub fn parse(text: &str) -> Result<Option<Value>> {
    let mut s = Scanner {
        b: text.as_bytes(),
        i: 0,
    };
    s.ws();
    let result = s.value(0).and_then(|v| {
        s.ws();
        if s.i == s.b.len() {
            Ok(v)
        } else {
            Err(Stop::Syntax)
        }
    });
    match result {
        Ok(v) => Ok(Some(v)),
        Err(Stop::Syntax) => Ok(None),
        Err(Stop::Violation(e)) => Err(e),
    }
}

/// Parse JSON text that must be JSON (a JSON Lines line, for example).
pub fn parse_json(text: &str) -> Result<Value> {
    parse(text)?.ok_or_else(|| anyhow!("input is not valid JSON"))
}

/// Apply the constraints a parsed value can still violate: integer numbers in range and the
/// depth bound.
pub fn check_value(v: &Value) -> Result<()> {
    fn walk(v: &Value, depth: usize) -> Result<()> {
        match v {
            Value::Array(items) => {
                if depth + 1 > MAX_DEPTH {
                    return Err(anyhow!("{TOO_DEEP} nesting deeper than {MAX_DEPTH}"));
                }
                items.iter().try_for_each(|x| walk(x, depth + 1))
            }
            Value::Object(members) => {
                if depth + 1 > MAX_DEPTH {
                    return Err(anyhow!("{TOO_DEEP} nesting deeper than {MAX_DEPTH}"));
                }
                members.values().try_for_each(|x| walk(x, depth + 1))
            }
            Value::Number(n) => {
                let magnitude = match (n.as_i64(), n.as_u64()) {
                    (Some(i), _) => i.unsigned_abs(),
                    (None, Some(u)) => u,
                    (None, None) => return Err(anyhow!("{NON_INTEGER} non-integer number")),
                };
                if magnitude > MAX_INTEGER {
                    return Err(anyhow!("{INTEGER_RANGE} integer outside ±(2^53 − 1)"));
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    walk(v, 0)
}

struct Scanner<'a> {
    b: &'a [u8],
    i: usize,
}

impl Scanner<'_> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    fn expect(&mut self, c: u8) -> Step<()> {
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(Stop::Syntax)
        }
    }

    fn literal(&mut self, word: &[u8], v: Value) -> Step<Value> {
        if self.b[self.i..].starts_with(word) {
            self.i += word.len();
            Ok(v)
        } else {
            Err(Stop::Syntax)
        }
    }

    /// A value whose enclosing container is at `depth` (0 for the document itself).
    fn value(&mut self, depth: usize) -> Step<Value> {
        match self.peek() {
            Some(b'{') => self.object(depth + 1),
            Some(b'[') => self.array(depth + 1),
            Some(b'"') => self.string().map(Value::String),
            Some(b't') => self.literal(b"true", Value::Bool(true)),
            Some(b'f') => self.literal(b"false", Value::Bool(false)),
            Some(b'n') => self.literal(b"null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(Stop::Syntax),
        }
    }

    fn object(&mut self, depth: usize) -> Step<Value> {
        if depth > MAX_DEPTH {
            return violation(TOO_DEEP, "nesting deeper than 128", self.i);
        }
        self.i += 1;
        let mut members = Map::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Value::Object(members));
        }
        loop {
            self.ws();
            if self.peek() != Some(b'"') {
                return Err(Stop::Syntax);
            }
            let at = self.i;
            let key = self.string()?;
            if members.contains_key(&key) {
                return violation(DUPLICATE_MEMBER, "duplicate member name", at);
            }
            self.ws();
            self.expect(b':')?;
            self.ws();
            let v = self.value(depth)?;
            members.insert(key, v);
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Value::Object(members));
                }
                _ => return Err(Stop::Syntax),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Step<Value> {
        if depth > MAX_DEPTH {
            return violation(TOO_DEEP, "nesting deeper than 128", self.i);
        }
        self.i += 1;
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.ws();
            items.push(self.value(depth)?);
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(Stop::Syntax),
            }
        }
    }

    fn hex4(&mut self) -> Step<u32> {
        let digits = self.b.get(self.i..self.i + 4).ok_or(Stop::Syntax)?;
        let text = std::str::from_utf8(digits).map_err(|_| Stop::Syntax)?;
        let n = u32::from_str_radix(text, 16).map_err(|_| Stop::Syntax)?;
        if !digits.iter().all(u8::is_ascii_hexdigit) {
            return Err(Stop::Syntax);
        }
        self.i += 4;
        Ok(n)
    }

    fn string(&mut self) -> Step<String> {
        self.i += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = self.peek().ok_or(Stop::Syntax)?;
            match c {
                b'"' => {
                    self.i += 1;
                    // Raw bytes come from valid UTF-8 input between ASCII delimiters, and
                    // escapes are pushed as whole scalar values.
                    return String::from_utf8(out).map_err(|_| Stop::Syntax);
                }
                b'\\' => {
                    let at = self.i;
                    self.i += 1;
                    let e = self.peek().ok_or(Stop::Syntax)?;
                    self.i += 1;
                    let ch = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let unit = self.hex4()?;
                            match unit {
                                0xD800..=0xDBFF => {
                                    if !self.b[self.i..].starts_with(b"\\u") {
                                        return violation(
                                            UNPAIRED_SURROGATE,
                                            "unpaired surrogate",
                                            at,
                                        );
                                    }
                                    self.i += 2;
                                    let low = self.hex4()?;
                                    if !(0xDC00..=0xDFFF).contains(&low) {
                                        return violation(
                                            UNPAIRED_SURROGATE,
                                            "unpaired surrogate",
                                            at,
                                        );
                                    }
                                    let cp = 0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00);
                                    char::from_u32(cp).ok_or(Stop::Syntax)?
                                }
                                0xDC00..=0xDFFF => {
                                    return violation(UNPAIRED_SURROGATE, "unpaired surrogate", at);
                                }
                                _ => char::from_u32(unit).ok_or(Stop::Syntax)?,
                            }
                        }
                        _ => return Err(Stop::Syntax),
                    };
                    let mut buf = [0; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                0x00..=0x1F => return Err(Stop::Syntax),
                _ => {
                    out.push(c);
                    self.i += 1;
                }
            }
        }
    }

    fn number(&mut self) -> Step<Value> {
        let start = self.i;
        let negative = self.peek() == Some(b'-');
        if negative {
            self.i += 1;
        }
        let digits = self.i;
        match self.peek() {
            Some(b'0') => self.i += 1,
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.i += 1;
                }
            }
            _ => return Err(Stop::Syntax),
        }
        if matches!(self.peek(), Some(b'.' | b'e' | b'E')) {
            return violation(NON_INTEGER, "non-integer number", start);
        }
        if matches!(self.peek(), Some(b'0'..=b'9')) {
            // A leading zero followed by more digits.
            return Err(Stop::Syntax);
        }
        let text = std::str::from_utf8(&self.b[digits..self.i]).map_err(|_| Stop::Syntax)?;
        let magnitude = match text.parse::<u64>() {
            Ok(m) if m <= MAX_INTEGER => m,
            _ => return violation(INTEGER_RANGE, "integer outside ±(2^53 − 1)", start),
        };
        let n = i64::try_from(magnitude).map_err(|_| Stop::Syntax)?;
        Ok(Value::from(if negative { -n } else { n }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn code(text: &str) -> String {
        parse(text).unwrap_err().to_string()[..6].to_string()
    }

    fn nested(depth: usize) -> String {
        format!("{}{}", "[".repeat(depth), "]".repeat(depth))
    }

    #[test]
    fn valid_json_parses_to_the_same_value_serde_json_reads() {
        let text = r#" {"a": [1, -2, 0, true, false, null, "xé😂\n"], "b": {}, "c": []} "#;
        let expected: Value = serde_json::from_str(text).unwrap();
        assert_eq!(parse(text).unwrap(), Some(expected));
        assert_eq!(parse("9007199254740991").unwrap(), Some(json!(MAX_INTEGER)));
        assert_eq!(
            parse("-9007199254740991").unwrap(),
            Some(json!(-(MAX_INTEGER as i64)))
        );
    }

    #[test]
    fn non_json_is_not_an_error() {
        for text in [
            "version: '0.1'\n",
            "{a: 1}",
            "",
            "[1,]",
            "01",
            "\"unterminated",
            "{} {}",
            "\"\t\"",
        ] {
            assert_eq!(parse(text).unwrap(), None, "{text:?}");
        }
    }

    #[test]
    fn each_violation_has_its_code() {
        assert_eq!(code(r#"{"n": 1.5}"#), NON_INTEGER);
        assert_eq!(code(r#"{"n": 1e3}"#), NON_INTEGER);
        assert_eq!(code(r#"{"n": 1.0}"#), NON_INTEGER);
        assert_eq!(code(r#"{"n": 9007199254740992}"#), INTEGER_RANGE);
        assert_eq!(code(r#"{"n": -9007199254740992}"#), INTEGER_RANGE);
        assert_eq!(
            code(r#"{"n": 123456789012345678901234567890}"#),
            INTEGER_RANGE
        );
        assert_eq!(code(r#"{"s": "\ud83d"}"#), UNPAIRED_SURROGATE);
        assert_eq!(code(r#"{"s": "\ud83dA"}"#), UNPAIRED_SURROGATE);
        assert_eq!(code(r#"{"s": "\ude02"}"#), UNPAIRED_SURROGATE);
        assert_eq!(code(r#"{"\ude02": 1}"#), UNPAIRED_SURROGATE);
        assert_eq!(code(r#"{"a": 1, "a": 1}"#), DUPLICATE_MEMBER);
        // Escaped and literal spellings of one name are the same name.
        assert_eq!(code(r#"{"a": 1, "a": 2}"#), DUPLICATE_MEMBER);
        assert_eq!(code(&nested(MAX_DEPTH + 1)), TOO_DEEP);
    }

    #[test]
    fn depth_counts_the_outermost_container_as_one() {
        assert!(parse(&nested(MAX_DEPTH)).unwrap().is_some());
        let objects = format!("{}1{}", r#"{"a":"#.repeat(MAX_DEPTH), "}".repeat(MAX_DEPTH));
        assert!(parse(&objects).unwrap().is_some());
        let too_deep = format!(
            "{}1{}",
            r#"{"a":"#.repeat(MAX_DEPTH + 1),
            "}".repeat(MAX_DEPTH + 1)
        );
        assert_eq!(code(&too_deep), TOO_DEEP);
    }

    #[test]
    fn the_depth_bound_stops_before_reading_further() {
        // Past the bound, the rest of the input (here, not even JSON) is never looked at.
        let text = format!("{}garbage", "[".repeat(MAX_DEPTH + 1));
        assert_eq!(code(&text), TOO_DEEP);
        // A million unclosed brackets do not recurse a million times.
        assert_eq!(code(&"[".repeat(1_000_000)), TOO_DEEP);
    }

    #[test]
    fn parsed_values_are_checked_for_numbers_and_depth() {
        assert!(check_value(&json!({"a": [1, -2, "x"]})).is_ok());
        let e = |v: Value| check_value(&v).unwrap_err().to_string()[..6].to_string();
        assert_eq!(e(json!({"n": 1.5})), NON_INTEGER);
        assert_eq!(e(json!({"n": MAX_INTEGER + 1})), INTEGER_RANGE);
        assert_eq!(e(json!({"n": u64::MAX})), INTEGER_RANGE);
        assert_eq!(e(json!({"n": i64::MIN})), INTEGER_RANGE);
        let mut deep = json!(1);
        for _ in 0..MAX_DEPTH {
            deep = json!([deep]);
        }
        assert!(check_value(&deep).is_ok());
        assert_eq!(e(json!([deep])), TOO_DEEP);
    }
}
