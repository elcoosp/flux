//! Minimal JSON parser + canonicalizer for trace frames.
//!
//! The reconcile trace format emits simple JSON objects (string / number / bool /
//! null leaves, shallow object and array nesting). Rather than pull in a JSON
//! dependency, we ship a tiny recursive-descent parser sufficient for trace
//! frames and a canonicalizer that sorts object keys and compacts whitespace.

/// A parsed JSON value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Json {
    /// `null`.
    Null,
    /// Boolean.
    Bool(bool),
    /// Number; preserved as its raw decimal text so `1` and `1.0` stay distinct
    /// (the trace format is exact on this point).
    Num(String),
    /// UTF-8 string value (un-escaped).
    Str(String),
    /// Ordered array of values.
    Arr(Vec<Json>),
    /// Object: ordered `(key, value)` pairs.
    Obj(Vec<(String, Json)>),
}

impl Json {
    /// Parses a complete JSON document from `input`, requiring the whole input
    /// to be consumed.
    ///
    /// # Errors
    /// Returns a descriptive message on any syntax error.
    pub(super) fn parse(input: &str) -> Result<Json, String> {
        let bytes = input.as_bytes();
        let mut p = Parser { bytes, pos: 0 };
        p.skip_ws();
        let value = p.parse_value()?;
        p.skip_ws();
        if p.pos != p.bytes.len() {
            return Err(format!("trailing characters at byte {}", p.pos));
        }
        Ok(value)
    }

    /// Renders the canonical (key-sorted, compact) form.
    #[must_use]
    pub(super) fn canonical(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Num(n) => out.push_str(n),
            Json::Str(s) => {
                out.push('"');
                for ch in s.chars() {
                    match ch {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        '\r' => out.push_str("\\r"),
                        '\t' => out.push_str("\\t"),
                        c => out.push(c),
                    }
                }
                out.push('"');
            }
            Json::Arr(items) => {
                out.push('[');
                for (i, v) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    v.write(out);
                }
                out.push(']');
            }
            Json::Obj(pairs) => {
                let mut sorted: Vec<&(String, Json)> = pairs.iter().collect();
                sorted.sort_by(|a, b| a.0.cmp(&b.0));
                out.push('{');
                let mut first = true;
                for (k, v) in &sorted {
                    // `span` is a host-specific line:col reference and is not part
                    // of the canonical frame (reconcile-trace-format v1).
                    if k == "span" {
                        continue;
                    }
                    if !first {
                        out.push(',');
                    }
                    first = false;
                    out.push('"');
                    for ch in k.chars() {
                        if ch == '"' {
                            out.push_str("\\\"");
                        } else {
                            out.push(ch);
                        }
                    }
                    out.push_str("\":");
                    v.write(out);
                }
                out.push('}');
            }
        }
    }
}

/// A cursor over the input bytes.
struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len() {
            match self.bytes[self.pos] {
                b' ' | b'\t' | b'\n' | b'\r' => self.pos += 1,
                _ => break,
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn parse_value(&mut self) -> Result<Json, String> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => Ok(Json::Str(self.parse_string()?)),
            Some(b't') | Some(b'f') => self.parse_bool(),
            Some(b'n') => self.parse_null(),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.parse_number(),
            Some(c) => Err(format!("unexpected byte '{}' at {}", c as char, self.pos)),
            None => Err("unexpected end of input".to_owned()),
        }
    }

    fn parse_object(&mut self) -> Result<Json, String> {
        self.expect(b'{')?;
        let mut pairs = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Json::Obj(pairs));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(format!("expected object key at {}", self.pos));
            }
            let key = self.parse_string()?;
            self.skip_ws();
            self.expect(b':')?;
            let value = self.parse_value()?;
            pairs.push((key, value));
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b'}') => {
                    self.pos += 1;
                    break;
                }
                _ => return Err(format!("expected ',' or '}}' at {}", self.pos)),
            }
        }
        Ok(Json::Obj(pairs))
    }

    fn parse_array(&mut self) -> Result<Json, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            let value = self.parse_value()?;
            items.push(value);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b']') => {
                    self.pos += 1;
                    break;
                }
                _ => return Err(format!("expected ',' or ']' at {}", self.pos)),
            }
        }
        Ok(Json::Arr(items))
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        // `self.pos` is now at the FIRST CONTENT byte of the string (the
        // opening `"` was consumed by `expect`). The previous version treated
        // `start` as the offset of the opening quote and then sliced
        // `&self.bytes[start + 1..end]`, which dropped one extra byte — the
        // first character of every parsed string.
        let start = self.pos;
        // Fast path: find the extent of the raw byte slice up to the closing
        // quote (respecting `\\` escapes), then decode as UTF-8 with
        // `str::from_utf8`. The previous version reinterpreted each *byte*
        // as a `char` (`self.bytes[pos] as char`), which mangles every
        // multi-byte UTF-8 character into 2-4 garbage Latin-1 chars. For a
        // trace containing non-ASCII (a user string, an emoji in a comment),
        // the entire canonical frame was silently corrupted.
        let mut escaped = false;
        let mut end = None;
        let mut i = self.pos;
        while i < self.bytes.len() {
            let b = self.bytes[i];
            if escaped {
                escaped = false;
                i += 1;
                continue;
            }
            match b {
                b'\\' => {
                    escaped = true;
                    i += 1;
                }
                b'"' => {
                    end = Some(i);
                    break;
                }
                _ => {
                    i += 1;
                }
            }
        }
        let end = end.ok_or_else(|| format!("unterminated string at {start}"))?;

        // Process escape sequences in a second pass, decoding `\uXXXX` (incl.
        // surrogate pairs) explicitly and letting every other byte flow into
        // a UTF-8 decode buffer.
        let raw = &self.bytes[start..end];
        let mut out = String::with_capacity(raw.len());
        let mut buf = [0u8; 4];
        let mut j = 0usize;
        while j < raw.len() {
            let b = raw[j];
            if b == b'\\' {
                j += 1;
                if j >= raw.len() {
                    return Err(format!("trailing backslash at {}", start + j));
                }
                match raw[j] {
                    b'"' => {
                        out.push('"');
                        j += 1;
                    }
                    b'\\' => {
                        out.push('\\');
                        j += 1;
                    }
                    b'/' => {
                        out.push('/');
                        j += 1;
                    }
                    b'n' => {
                        out.push('\n');
                        j += 1;
                    }
                    b't' => {
                        out.push('\t');
                        j += 1;
                    }
                    b'r' => {
                        out.push('\r');
                        j += 1;
                    }
                    b'u' => {
                        // \uXXXX — parse 4 hex digits, handle surrogate pairs.
                        let hex = raw.get(j + 1..j + 5).ok_or_else(|| {
                            format!("truncated \\u escape at {}", start + j)
                        })?;
                        let hi = u16::from_str_radix(
                            std::str::from_utf8(hex).map_err(|_| "bad hex digits")?,
                            16,
                        )
                        .map_err(|_| "bad hex digits")?;
                        j += 5;
                        let code = if (0xD800..=0xDBFF).contains(&hi) {
                            // High surrogate: must be followed by a low `\uDC00..\uDFFF`.
                            if raw.get(j) != Some(&b'\\') || raw.get(j + 1) != Some(&b'u') {
                                return Err(format!("unpaired surrogate at {}", start + j));
                            }
                            let hex2 = raw.get(j + 2..j + 6).ok_or_else(|| {
                                format!("truncated \\u low surrogate at {}", start + j)
                            })?;
                            let lo = u16::from_str_radix(
                                std::str::from_utf8(hex2).map_err(|_| "bad hex digits")?,
                                16,
                            )
                            .map_err(|_| "bad hex digits")?;
                            j += 6;
                            let c = char::decode_utf16([hi, lo])
                                .collect::<Result<String, _>>()
                                .map_err(|_| "invalid surrogate pair")?;
                            out.push_str(&c);
                            continue;
                        } else {
                            char::from_u32(u32::from(hi)).ok_or("invalid scalar value")?
                        };
                        out.push(code);
                    }
                    other => {
                        return Err(format!(
                            "unsupported escape \\{} at {}",
                            other as char, start + j
                        ));
                    }
                }
            } else {
                // Copy the next UTF-8 sequence. UTF-8 continuation bytes are
                // safe to pass through; we read byte-by-byte into `buf` and
                // then decode the whole multibyte sequence as a char.
                let width = Self::utf8_width(b);
                if j + width > raw.len() {
                    return Err(format!("truncated UTF-8 at {}", start + j));
                }
                buf[..width].copy_from_slice(&raw[j..j + width]);
                let s = std::str::from_utf8(&buf[..width])
                    .map_err(|_| format!("invalid UTF-8 at {}", start + j))?;
                out.push_str(s);
                j += width;
            }
        }
        self.pos = end + 1;
        Ok(out)
    }

    /// Expected byte width of a UTF-8 sequence starting with `b`.
    fn utf8_width(b: u8) -> usize {
        if b < 0x80 {
            1
        } else if b < 0xE0 {
            2
        } else if b < 0xF0 {
            3
        } else {
            4
        }
    }

    fn parse_bool(&mut self) -> Result<Json, String> {
        if self.bytes[self.pos..].starts_with(b"true") {
            self.pos += 4;
            Ok(Json::Bool(true))
        } else if self.bytes[self.pos..].starts_with(b"false") {
            self.pos += 5;
            Ok(Json::Bool(false))
        } else {
            Err(format!("invalid literal at {}", self.pos))
        }
    }

    fn parse_null(&mut self) -> Result<Json, String> {
        if self.bytes[self.pos..].starts_with(b"null") {
            self.pos += 4;
            Ok(Json::Null)
        } else {
            Err(format!("invalid literal at {}", self.pos))
        }
    }

    fn parse_number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || c == b'.' || c == b'e' || c == b'E' || c == b'+' || c == b'-' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| "invalid number bytes".to_owned())?
            .to_owned();
        if text.is_empty() || text == "-" {
            return Err(format!("invalid number at {}", start));
        }
        Ok(Json::Num(text))
    }

    fn expect(&mut self, b: u8) -> Result<(), String> {
        if self.peek() == Some(b) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!("expected '{}' at {}", b as char, self.pos))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scalars() {
        assert_eq!(Json::parse("null").unwrap(), Json::Null);
        assert_eq!(Json::parse("true").unwrap(), Json::Bool(true));
        assert_eq!(Json::parse("42").unwrap(), Json::Num("42".into()));
        assert_eq!(Json::parse("\"hi\"").unwrap(), Json::Str("hi".into()));
    }

    #[test]
    fn sorts_object_keys() {
        let v = Json::parse(r#"{"b":1,"a":2}"#).unwrap();
        assert_eq!(v.canonical(), r#"{"a":2,"b":1}"#);
    }

    #[test]
    fn drops_span_key() {
        let v = Json::parse(r#"{"event":"x","span":"flux://m#L1","n":"n1"}"#).unwrap();
        assert_eq!(v.canonical(), r#"{"event":"x","n":"n1"}"#);
    }

    #[test]
    fn rejects_garbage() {
        assert!(Json::parse("{not json}").is_err());
        assert!(Json::parse("{\"a\":}").is_err());
    }
}
