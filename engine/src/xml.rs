// Minimal vendored XML parser (there is no tpt-xml base crate yet — see
// todo.md Phase 0 note). Scope: what config-file validation needs —
// well-formedness checking with precise line numbers, elements, attributes,
// text, comments, CDATA, processing instructions, DOCTYPE (skipped), and
// the five predefined entities. No namespaces beyond recording the raw
// qualified name, no DTD validation, no XSD.
//
// Tree convention (common XML→data shape):
//   - element → Value::Obj: `"@tag"` = element name, `"@attr"` = attribute,
//     `"#text"` = text content when present, child elements keyed by tag
//     (repeated tags become Value::Arr)
//   - an element with no attributes and text-only content collapses to
//     Value::Str so `project.artifactId` reads naturally

use crate::value::Value;
use std::collections::BTreeMap;

pub fn parse(text: &str) -> Result<Value, String> {
    Parser::new(text).parse_document()
}

struct Parser<'a> {
    src: &'a [u8],
    text: &'a str,
    pos: usize,
    line: usize,
}

#[derive(Debug, Clone)]
struct Element {
    name: String,
    attrs: BTreeMap<String, String>,
    children: BTreeMap<String, Value>,
    text: String,
}

impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Self {
        Parser {
            src: text.as_bytes(),
            text,
            pos: 0,
            line: 1,
        }
    }

    fn err(&self, msg: &str) -> String {
        format!("line {}: {msg}", self.line)
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let c = self.peek()?;
        self.pos += 1;
        if c == b'\n' {
            self.line += 1;
        }
        Some(c)
    }

    /// Bump one full UTF-8 char (for text/attribute content, where multibyte
    /// characters are legitimate).
    fn bump_char(&mut self) -> Option<char> {
        let ch = self.text[self.pos..].chars().next()?;
        self.pos += ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
        }
        Some(ch)
    }

    fn eat(&mut self, s: &str) -> bool {
        if self.text[self.pos..].starts_with(s) {
            for _ in s.chars() {
                self.bump();
            }
            true
        } else {
            false
        }
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ') | Some(b'\t') | Some(b'\r') | Some(b'\n')) {
            self.bump();
        }
    }

    fn parse_document(&mut self) -> Result<Value, String> {
        // Prolog: XML declaration, comments, PIs, DOCTYPE.
        loop {
            self.skip_ws();
            if self.eat("<?") {
                self.skip_until("?>")?;
            } else if self.eat("<!--") {
                self.skip_until("-->")?;
            } else if self.eat("<!DOCTYPE") {
                self.skip_doctype()?;
            } else {
                break;
            }
        }

        let root = self.parse_element()?.1;

        // Trailing junk after the root element (only whitespace/comments/PIs allowed).
        loop {
            self.skip_ws();
            if self.peek().is_none() {
                break;
            }
            if self.eat("<!--") {
                self.skip_until("-->")?;
            } else if self.eat("<?") {
                self.skip_until("?>")?;
            } else {
                return Err(self.err("unexpected content after the root element"));
            }
        }
        Ok(root)
    }

    fn skip_until(&mut self, terminator: &str) -> Result<(), String> {
        loop {
            if self.peek().is_none() {
                return Err(self.err(&format!("unterminated construct, expected `{terminator}`")));
            }
            if self.eat(terminator) {
                return Ok(());
            }
            self.bump();
        }
    }

    /// DOCTYPE may contain a bracketed internal subset; skip it nested.
    fn skip_doctype(&mut self) -> Result<(), String> {
        let mut depth = 0i64;
        loop {
            match self.peek() {
                None => return Err(self.err("unterminated DOCTYPE")),
                Some(b'[') => {
                    depth += 1;
                    self.bump();
                }
                Some(b']') => {
                    depth -= 1;
                    self.bump();
                }
                Some(b'>') => {
                    self.bump();
                    if depth <= 0 {
                        return Ok(());
                    }
                }
                _ => {
                    self.bump();
                }
            }
        }
    }

    fn parse_element(&mut self) -> Result<(String, Value), String> {
        if !self.eat("<") || matches!(self.peek(), Some(b'?') | Some(b'!')) {
            return Err(self.err("expected an element"));
        }
        let name = self.parse_name()?;
        let mut attrs = BTreeMap::new();
        let mut self_closing = false;

        loop {
            self.skip_ws();
            match self.peek() {
                None => return Err(self.err(&format!("<{name}> is never closed")),
                ),
                Some(b'>') => {
                    self.bump();
                    break;
                }
                Some(b'/') => {
                    self.bump();
                    if !self.eat(">") {
                        return Err(self.err("malformed self-closing tag"));
                    }
                    self_closing = true;
                    break;
                }
                Some(_) => {
                    let attr_name = self.parse_name()?;
                    self.skip_ws();
                    if !self.eat("=") {
                        return Err(self.err(&format!("attribute `{attr_name}` needs a value (`=`)")));
                    }
                    self.skip_ws();
                    let value = self.parse_attr_value()?;
                    if attrs.insert(attr_name.clone(), value).is_some() {
                        return Err(self.err(&format!("duplicate attribute `{attr_name}`")));
                    }
                }
            }
        }

        let mut element = Element { name: name.clone(), attrs, children: BTreeMap::new(), text: String::new() };
        if self_closing {
            return Ok((name.clone(), element.into_value()));
        }

        // Content loop.
        loop {
            match self.peek() {
                None => return Err(self.err(&format!("<{name}> is never closed")),
                ),
                Some(b'<') => {
                    if self.eat("<!--") {
                        self.skip_until("-->")?;
                    } else if self.eat("<![CDATA[") {
                        let start = self.pos;
                        let end = self.text[self.pos..]
                            .find("]]>")
                            .ok_or_else(|| self.err("unterminated CDATA section"))?;
                        for _ in self.text[start..start + end].chars() {
                            self.bump();
                        }
                        self.eat("]]>");
                        element.text.push_str(&self.text[start..start + end]);
                    } else if self.eat("<?") {
                        self.skip_until("?>")?;
                    } else if self.eat("</") {
                        let close_name = self.parse_name()?;
                        self.skip_ws();
                        if !self.eat(">") {
                            return Err(self.err("malformed closing tag"));
                        }
                        if close_name != name {
                            return Err(self.err(&format!(
                                "closing `</{close_name}>` does not match opened `<{name}>`"
                            )));
                        }
                        return Ok((name, element.into_value()));
                    } else {
                        let (child_tag, child_value) = self.parse_element()?;
                        element.push_child(child_tag, child_value);
                    }
                }
                Some(_) => {
                    let ch = self.bump_char().unwrap();
                    if ch.is_whitespace() {
                        element.text.push(' ');
                    } else if ch == '&' {
                        let next = self
                            .bump_char()
                            .ok_or_else(|| self.err("unterminated entity"))?;
                        let decoded = self.decode_entity(next)?;
                        element.text.push_str(&decoded);
                    } else {
                        element.text.push(ch);
                    }
                }
            }
        }
    }

    fn parse_name(&mut self) -> Result<String, String> {
        let start = self.pos;
        while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.' | b':')) {
            self.bump();
        }
        if self.pos == start {
            return Err(self.err("expected a name"));
        }
        Ok(self.text[start..self.pos].to_string())
    }

    fn parse_attr_value(&mut self) -> Result<String, String> {
        let quote = match self.peek() {
            Some(b'"') | Some(b'\'') => self.bump().unwrap(),
            _ => return Err(self.err("attribute value must be quoted")),
        };
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return Err(self.err("unterminated attribute value")),
                Some(b'"') | Some(b'\'') if Some(quote) == self.peek() => {
                    self.bump();
                    return Ok(out);
                }
                Some(b'<') => return Err(self.err("`<` inside an attribute value")),
                Some(b'&') => {
                    self.bump();
                    let ch = self.bump_char().ok_or_else(|| self.err("unterminated entity"))?;
                    out.push_str(&self.decode_entity(ch)?);
                }
                Some(_) => {
                    out.push(self.bump_char().unwrap());
                }
            }
        }
    }

    /// `ch` is the first char after an `&`.
    fn decode_entity(&mut self, ch: char) -> Result<String, String> {
        match ch {
            'l' if self.eat("t;") => Ok("<".into()),
            'g' if self.eat("t;") => Ok(">".into()),
            'a' if self.eat("mp;") => Ok("&".into()),
            'a' if self.eat("pos;") => Ok("'".into()),
            'q' if self.eat("uot;") => Ok("\"".into()),
            '#' => {
                let (value, rest) = if self.eat("x") {
                    let digits: String = self.take_while(|c| c.is_ascii_hexdigit());
                    let _ = self.eat(";");
                    (u32::from_str_radix(&digits, 16).ok(), digits)
                } else {
                    let digits: String = self.take_while(|c| c.is_ascii_digit());
                    let _ = self.eat(";");
                    (digits.parse::<u32>().ok(), digits)
                };
                value
                    .and_then(char::from_u32)
                    .map(String::from)
                    .ok_or_else(|| self.err(&format!("bad character reference `&#{rest};`")))
            }
            _ => Err(self.err("unknown entity — only &lt; &gt; &amp; &apos; &quot; and &#…; are supported")),
        }
    }

    fn take_while(&mut self, pred: impl Fn(u8) -> bool) -> String {
        let start = self.pos;
        while matches!(self.peek(), Some(c) if pred(c)) {
            self.bump();
        }
        self.text[start..self.pos].to_string()
    }
}

impl Element {
    fn push_child(&mut self, tag: String, child: Value) {
        match self.children.get_mut(&tag) {
            None => {
                self.children.insert(tag, child);
            }
            Some(Value::Arr(items)) => items.push(child),
            Some(existing) => {
                let moved = existing.clone();
                *existing = Value::Arr(vec![moved, child]);
            }
        }
    }

    fn into_value(self) -> Value {
        let text = self.text.trim().to_string();
        if self.attrs.is_empty() && self.children.is_empty() && !text.is_empty() {
            // Text-only leaf: a plain string value.
            return Value::Str(text);
        }
        let mut map = BTreeMap::new();
        map.insert("@tag".to_string(), Value::Str(self.name));
        for (k, v) in self.attrs {
            map.insert(format!("@{k}"), Value::Str(v));
        }
        if !text.is_empty() {
            map.insert("#text".to_string(), Value::Str(text));
        }
        for (k, v) in self.children {
            map.insert(k, v);
        }
        Value::Obj(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_document() {
        let v = parse("<a><b id='x'>hi</b><b>yo</b></a>").unwrap();
        let Value::Obj(root) = v else { panic!() };
        assert_eq!(root.get("@tag").and_then(Value::as_str), Some("a"));
        assert!(matches!(root.get("b"), Some(Value::Arr(_))));
    }

    #[test]
    fn text_only_leaf_collapses_to_string() {
        // parse() returns the root element itself, so <p> is `v` here.
        let v = parse("<p><version>1.0.0</version></p>").unwrap();
        assert_eq!(v.get("version").and_then(Value::as_str), Some("1.0.0"));
    }

    #[test]
    fn attributes_become_at_keys() {
        let v = parse("<a href=\"https://x\" target=\"_blank\">t</a>").unwrap();
        let Value::Obj(root) = v else { panic!() };
        assert_eq!(root.get("@href").and_then(Value::as_str), Some("https://x"));
        assert_eq!(root.get("#text").and_then(Value::as_str), Some("t"));
    }

    #[test]
    fn unclosed_tag_reports_line() {
        let e = parse("<a>\n  <b>\n").unwrap_err();
        assert!(e.contains("<b> is never closed"), "{e}");
    }

    #[test]
    fn mismatched_close_tag() {
        let e = parse("<a>\n  <b>\n</a>").unwrap_err();
        assert!(e.contains("does not match"), "{e}");
    }

    #[test]
    fn duplicate_attribute_rejected() {
        let e = parse("<a x='1' x='2'/>").unwrap_err();
        assert!(e.contains("duplicate attribute"), "{e}");
    }

    #[test]
    fn doctype_comments_cdata_and_entities() {
        let src = "<?xml version=\"1.0\"?><!-- c --><!DOCTYPE a [<!ENTITY x \"y\">]><a><![CDATA[<raw>]]>&amp;&#65;</a>";
        // Text-only root collapses to its string content.
        assert_eq!(parse(src).unwrap(), Value::Str("<raw>&A".into()));
    }

    #[test]
    fn multiple_roots_rejected() {
        assert!(parse("<a/><b/>").is_err());
    }

    #[test]
    fn self_closing_element_is_empty_object() {
        let v = parse("<a><br/></a>").unwrap();
        let br = v.get("br").unwrap();
        assert_eq!(br.get("@tag").and_then(Value::as_str), Some("br"));
    }
}
