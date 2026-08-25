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
        let mut s = String::new();
        loop {
            let c = self.peek().ok_or("unterminated string")?;
            match c {
                b'"' => {
                    self.pos += 1;
                    break;
                }
                b'\\' => {
                    self.pos += 1;
                    let e = self.peek().ok_or("unterminated escape")?;
                    match e {
                        b'"' => s.push('"'),
                        b'\\' => s.push('\\'),
                        b'/' => s.push('/'),
                        b'n' => s.push('\n'),
                        b't' => s.push('\t'),
                        b'r' => s.push('\r'),
                        _ => {
                            return Err(format!(
                                "unsupported escape \\{} at {}",
                                e as char, self.pos
                            ));
                        }
                    }
                    self.pos += 1;
                }
                _ => {
                    let ch = self.bytes[self.pos] as char;
                    s.push(ch);
                    self.pos += 1;
                }
            }
        }
        Ok(s)
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
