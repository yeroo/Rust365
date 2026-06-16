//! Minimal zero-copy XML pull parser, tuned for OOXML. Port of xml.cpp.

pub struct XmlAttr<'a> {
    pub name: &'a str,
    pub value: &'a str, // raw, entities NOT decoded
}

#[derive(PartialEq, Clone, Copy)]
pub enum Event {
    Start,
    End,
    Text,
    Eof,
}

pub struct XmlParser<'a> {
    xml: &'a [u8],
    pos: usize,
    m_name: &'a str,
    m_text: &'a str,
    m_attrs: Vec<XmlAttr<'a>>,
    pending_end: bool,
}

fn is_ws(c: u8) -> bool {
    c == b' ' || c == b'\t' || c == b'\r' || c == b'\n'
}
fn is_name_end(c: u8) -> bool {
    is_ws(c) || c == b'>' || c == b'/' || c == b'='
}

fn append_utf8(cp: u32, out: &mut String) {
    if let Some(ch) = char::from_u32(cp) {
        out.push(ch);
    }
}

impl<'a> XmlParser<'a> {
    pub fn new(xml: &'a str) -> Self {
        XmlParser {
            xml: xml.as_bytes(),
            pos: 0,
            m_name: "",
            m_text: "",
            m_attrs: Vec::new(),
            pending_end: false,
        }
    }

    fn slice(&self, a: usize, b: usize) -> &'a str {
        std::str::from_utf8(&self.xml[a..b]).unwrap_or("")
    }
    fn find_from(&self, from: usize, byte: u8) -> Option<usize> {
        self.xml[from..].iter().position(|&c| c == byte).map(|x| from + x)
    }
    fn starts_with(&self, at: usize, pat: &[u8]) -> bool {
        self.xml.len() >= at + pat.len() && &self.xml[at..at + pat.len()] == pat
    }

    pub fn name(&self) -> &'a str {
        self.m_name
    }
    pub fn text(&self) -> &'a str {
        self.m_text
    }
    pub fn attr(&self, name: &str) -> &'a str {
        self.m_attrs.iter().find(|a| a.name == name).map(|a| a.value).unwrap_or("")
    }
    pub fn attrs(&self) -> &[XmlAttr<'a>] {
        &self.m_attrs
    }

    pub fn next(&mut self) -> Event {
        if self.pending_end {
            self.pending_end = false;
            return Event::End;
        }
        let size = self.xml.len();
        loop {
            if self.pos >= size {
                return Event::Eof;
            }
            if self.xml[self.pos] != b'<' {
                let start = self.pos;
                let lt = self.find_from(self.pos, b'<').unwrap_or(size);
                self.pos = lt;
                self.m_text = self.slice(start, lt);
                return Event::Text;
            }
            self.pos += 1; // consume '<'
            if self.pos >= size {
                return Event::Eof;
            }
            let c = self.xml[self.pos];

            if c == b'/' {
                self.pos += 1;
                let start = self.pos;
                let gt = match self.find_from(self.pos, b'>') {
                    Some(x) => x,
                    None => return Event::Eof,
                };
                let mut end = start;
                while end < gt && !is_ws(self.xml[end]) {
                    end += 1;
                }
                self.m_name = self.slice(start, end);
                self.pos = gt + 1;
                return Event::End;
            }

            if c == b'?' {
                self.pos = match self.xml[self.pos..].windows(2).position(|w| w == b"?>") {
                    Some(x) => self.pos + x + 2,
                    None => size,
                };
                continue;
            }

            if c == b'!' {
                if self.starts_with(self.pos, b"!--") {
                    self.pos = match self.xml[self.pos + 3..].windows(3).position(|w| w == b"-->") {
                        Some(x) => self.pos + 3 + x + 3,
                        None => size,
                    };
                    continue;
                }
                if self.starts_with(self.pos, b"![CDATA[") {
                    let start = self.pos + 8;
                    let e = self.xml[start..].windows(3).position(|w| w == b"]]>").map(|x| start + x);
                    match e {
                        Some(e) => {
                            self.m_text = self.slice(start, e);
                            self.pos = e + 3;
                        }
                        None => {
                            self.m_text = self.slice(start, size);
                            self.pos = size;
                        }
                    }
                    return Event::Text;
                }
                self.pos = match self.find_from(self.pos, b'>') {
                    Some(e) => e + 1,
                    None => size,
                };
                continue;
            }

            // start tag
            let start = self.pos;
            while self.pos < size && !is_name_end(self.xml[self.pos]) {
                self.pos += 1;
            }
            self.m_name = self.slice(start, self.pos);
            self.m_attrs.clear();

            loop {
                while self.pos < size && is_ws(self.xml[self.pos]) {
                    self.pos += 1;
                }
                if self.pos >= size {
                    return Event::Eof;
                }
                let d = self.xml[self.pos];
                if d == b'>' {
                    self.pos += 1;
                    return Event::Start;
                }
                if d == b'/' {
                    self.pos += 1;
                    if self.pos < size && self.xml[self.pos] == b'>' {
                        self.pos += 1;
                    }
                    self.pending_end = true;
                    return Event::Start;
                }
                // attribute
                let as_ = self.pos;
                while self.pos < size && !is_name_end(self.xml[self.pos]) {
                    self.pos += 1;
                }
                let an = self.slice(as_, self.pos);
                while self.pos < size && is_ws(self.xml[self.pos]) {
                    self.pos += 1;
                }
                if self.pos < size && self.xml[self.pos] == b'=' {
                    self.pos += 1;
                    while self.pos < size && is_ws(self.xml[self.pos]) {
                        self.pos += 1;
                    }
                    if self.pos < size && (self.xml[self.pos] == b'"' || self.xml[self.pos] == b'\'') {
                        let q = self.xml[self.pos];
                        self.pos += 1;
                        let vs = self.pos;
                        let ve = match self.find_from(self.pos, q) {
                            Some(x) => x,
                            None => return Event::Eof,
                        };
                        self.m_attrs.push(XmlAttr { name: an, value: self.slice(vs, ve) });
                        self.pos = ve + 1;
                        continue;
                    }
                }
                self.m_attrs.push(XmlAttr { name: an, value: "" });
            }
        }
    }

    /// Call immediately after a Start event: consumes through the matching End.
    pub fn skip_element(&mut self) {
        let mut depth = 1;
        while depth > 0 {
            match self.next() {
                Event::Eof => return,
                Event::Start => depth += 1,
                Event::End => depth -= 1,
                _ => {}
            }
        }
    }

    /// Appends `raw` to `out`, decoding the five XML entities and numeric refs.
    pub fn append_decoded(raw: &str, out: &mut String) {
        let b = raw.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if b[i] != b'&' {
                // copy one UTF-8 char starting at i
                let ch_len = utf8_len(b[i]);
                let end = (i + ch_len).min(b.len());
                out.push_str(std::str::from_utf8(&b[i..end]).unwrap_or(""));
                i = end;
                continue;
            }
            let semi = raw[i + 1..].find(';').map(|x| i + 1 + x);
            match semi {
                Some(semi) if semi - i <= 12 => {
                    let ent = &raw[i + 1..semi];
                    match ent {
                        "amp" => out.push('&'),
                        "lt" => out.push('<'),
                        "gt" => out.push('>'),
                        "quot" => out.push('"'),
                        "apos" => out.push('\''),
                        _ if ent.starts_with('#') => {
                            let eb = ent.as_bytes();
                            let mut cp = 0u32;
                            let mut ok = ent.len() > 1;
                            if ent.len() > 2 && (eb[1] == b'x' || eb[1] == b'X') {
                                for &h in &eb[2..] {
                                    cp <<= 4;
                                    match h {
                                        b'0'..=b'9' => cp |= (h - b'0') as u32,
                                        b'a'..=b'f' => cp |= (h - b'a' + 10) as u32,
                                        b'A'..=b'F' => cp |= (h - b'A' + 10) as u32,
                                        _ => {
                                            ok = false;
                                            break;
                                        }
                                    }
                                }
                            } else {
                                for &d in &eb[1..] {
                                    if !d.is_ascii_digit() {
                                        ok = false;
                                        break;
                                    }
                                    cp = cp * 10 + (d - b'0') as u32;
                                }
                            }
                            if ok && cp != 0 && cp <= 0x10FFFF {
                                append_utf8(cp, out);
                            }
                        }
                        _ => out.push_str(&raw[i..semi + 1]),
                    }
                    i = semi + 1;
                }
                _ => {
                    out.push('&');
                    i += 1;
                }
            }
        }
    }
}

fn utf8_len(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b >= 0xF0 {
        4
    } else if b >= 0xE0 {
        3
    } else if b >= 0xC0 {
        2
    } else {
        1
    }
}
