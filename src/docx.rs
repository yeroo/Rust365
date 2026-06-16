//! WordprocessingML -> HTML conversion. Port of Fast365's docx.cpp. Single
//! streaming pass over word/document.xml with lookup tables from styles.xml,
//! numbering.xml and the relationship parts; tables buffered into a grid model
//! so vMerge becomes rowspan.

use std::collections::HashMap;

use crate::htmlutil::{append_escaped_attr, append_escaped_html, base64_encode};
use crate::xml::{Event, XmlParser};
use crate::zip::ZipArchive;

pub struct ConvertOptions {
    pub fragment: bool,
    pub embed_images: bool,
    pub title: String,
}
impl Default for ConvertOptions {
    fn default() -> Self {
        ConvertOptions { fragment: false, embed_images: true, title: String::new() }
    }
}

const MAX_DEPTH: i32 = 128;

// ---- helpers ----

fn sv_to_int(s: &str) -> i32 {
    let b = s.as_bytes();
    let mut v = 0i32;
    let mut neg = false;
    let mut i = 0;
    if !b.is_empty() && (b[0] == b'-' || b[0] == b'+') {
        neg = b[0] == b'-';
        i = 1;
    }
    while i < b.len() {
        if !b[i].is_ascii_digit() {
            break;
        }
        v = v.wrapping_mul(10).wrapping_add((b[i] - b'0') as i32);
        i += 1;
    }
    if neg {
        -v
    } else {
        v
    }
}

fn sv_to_hex(s: &str) -> u32 {
    let mut v = 0u32;
    for c in s.bytes() {
        v <<= 4;
        match c {
            b'0'..=b'9' => v |= (c - b'0') as u32,
            b'a'..=b'f' => v |= (c - b'a' + 10) as u32,
            b'A'..=b'F' => v |= (c - b'A' + 10) as u32,
            _ => return 0,
        }
    }
    v
}

fn to_lower(s: &str) -> String {
    s.to_ascii_lowercase()
}

fn decode_attr(raw: &str) -> String {
    let mut s = String::new();
    XmlParser::append_decoded(raw, &mut s);
    s
}

fn toggle_on(val: &str) -> bool {
    !(val == "0" || val == "false" || val == "none" || val == "off")
}

fn map_align(jc: &str) -> &'static str {
    match jc {
        "center" => "center",
        "right" | "end" => "right",
        "both" | "distribute" => "justify",
        "left" | "start" => "left",
        _ => "",
    }
}

fn highlight_css(v: &str) -> String {
    if v == "darkYellow" {
        "#808000".to_string()
    } else {
        to_lower(v)
    }
}

fn append_pt(css: &mut String, prop: &str, twips: i32) {
    if twips == 0 {
        return;
    }
    css.push_str(&format!("{}:{}pt;", prop, twips as f64 / 20.0));
}

fn mime_for_path(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().map(to_lower).unwrap_or_default();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "tif" | "tiff" => "image/tiff",
        "emf" => "image/emf",
        "wmf" => "image/wmf",
        _ => "application/octet-stream",
    }
}

fn resolve_zip_path(base_dir: &str, target: &str) -> String {
    let full = if target.starts_with('/') {
        target[1..].to_string()
    } else {
        format!("{base_dir}{target}")
    };
    let mut parts: Vec<&str> = Vec::new();
    for part in full.split('/') {
        if part == ".." {
            parts.pop();
        } else if !part.is_empty() && part != "." {
            parts.push(part);
        }
    }
    parts.join("/")
}

fn hyperlink_from_instr(instr: &str) -> String {
    let b = instr.as_bytes();
    let n = b.len();
    let mut i = 0usize;
    let skip_ws = |i: &mut usize| {
        while *i < n && (b[*i] == b' ' || b[*i] == b'\t' || b[*i] == b'\r' || b[*i] == b'\n') {
            *i += 1;
        }
    };
    skip_ws(&mut i);
    let ks = i;
    while i < n && b[i] != b' ' && b[i] != b'\t' {
        i += 1;
    }
    if to_lower(&instr[ks..i]) != "hyperlink" {
        return String::new();
    }
    let mut url = String::new();
    let mut anchor = String::new();
    let mut pending: u8 = 0;
    while i < n {
        skip_ws(&mut i);
        if i >= n {
            break;
        }
        if b[i] == b'\\' {
            let f = if i + 1 < n { b[i + 1] } else { 0 };
            i += 2;
            pending = if f == b'l' || f == b'o' || f == b't' { f } else { 0 };
            continue;
        }
        let tok;
        if b[i] == b'"' {
            let e = instr[i + 1..].find('"').map(|x| i + 1 + x).unwrap_or(n);
            tok = instr[i + 1..e].to_string();
            i = if e == n { n } else { e + 1 };
        } else {
            let s = i;
            while i < n && b[i] != b' ' && b[i] != b'\t' {
                i += 1;
            }
            tok = instr[s..i].to_string();
        }
        if pending == b'l' {
            anchor = tok;
        } else if pending == b'o' || pending == b't' {
            // tooltip/frame: drop
        } else if url.is_empty() {
            url = tok;
        }
        pending = 0;
    }
    if url.is_empty() && anchor.is_empty() {
        String::new()
    } else if url.is_empty() {
        format!("#{anchor}")
    } else if !anchor.is_empty() {
        format!("{url}#{anchor}")
    } else {
        url
    }
}

struct Relationship {
    target: String,
    external: bool,
}

#[derive(Default, Clone)]
struct RunProps {
    b: bool,
    i: bool,
    u: bool,
    strike: bool,
    caps: bool,
    small_caps: bool,
    vanish: bool,
    vert: i32,
    color: String,
    highlight: String,
}

fn apply_run_prop(rp: &mut RunProps, n: &str, val: &str) {
    let on = toggle_on(val);
    match n {
        "w:b" => rp.b = on,
        "w:i" => rp.i = on,
        "w:u" => rp.u = on,
        "w:strike" | "w:dstrike" => rp.strike = on,
        "w:caps" => rp.caps = on,
        "w:smallCaps" => rp.small_caps = on,
        "w:vanish" | "w:webHidden" => rp.vanish = on,
        "w:vertAlign" => {
            rp.vert = if val == "superscript" {
                1
            } else if val == "subscript" {
                -1
            } else {
                0
            }
        }
        "w:color" => {
            if !val.is_empty() && val != "auto" {
                rp.color = val.to_string();
            }
        }
        "w:highlight" => {
            if !val.is_empty() && val != "none" {
                rp.highlight = val.to_string();
            }
        }
        _ => {}
    }
}

#[derive(Default)]
struct ParaProps {
    heading: i32,
    align: &'static str,
    num_id: i32,
    ilvl: i32,
    rtl: bool,
    style_id: String,
    css: String,
}
impl ParaProps {
    fn new() -> Self {
        ParaProps { num_id: -1, ..Default::default() }
    }
}

#[derive(Default)]
struct CellData {
    html: String,
    css: String,
    colspan: i32,
    vmerge: i32,
}

#[derive(Default)]
struct RowData {
    cells: Vec<CellData>,
    header: bool,
}

struct Field {
    instr: String,
    collecting: bool,
    open: bool,
}

struct OpenList {
    tag: &'static str,
    li_open: bool,
}

#[derive(Clone, Copy)]
struct StyleNum {
    num_id: i32,
    ilvl: i32,
}

fn list_key(num_id: i32, ilvl: i32) -> u64 {
    ((num_id as u32 as u64) << 8) | (ilvl as u32 & 0xFF) as u64
}

struct Converter<'a> {
    opts: &'a ConvertOptions,
    zip: ZipArchive<'a>,
    base_dir: String,
    rels: HashMap<String, Relationship>,
    heading_by_style: HashMap<String, i32>,
    style_num: HashMap<String, StyleNum>,
    char_style: HashMap<String, RunProps>,
    num_to_abstract: HashMap<i32, i32>,
    abstract_fmt: HashMap<i32, HashMap<i32, String>>,
    abstract_start: HashMap<i32, HashMap<i32, i32>>,
    num_fmt_override: HashMap<i32, HashMap<i32, String>>,
    num_start_override: HashMap<i32, HashMap<i32, i32>>,
    list_emitted: HashMap<u64, i32>,
    title: String,
    list_stack: Vec<OpenList>,
    fields: Vec<Field>,
    fn_order: Vec<String>,
    en_order: Vec<String>,
    out: String,
}

pub fn convert_docx_to_html(data: &[u8], opts: &ConvertOptions) -> Result<String, String> {
    let zip = match ZipArchive::open(data) {
        Some(z) => z,
        None => {
            const OLE2: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
            if data.len() >= 8 && data[..8] == OLE2 {
                return Err("OLE compound file: either a legacy binary .doc or a \
                            password-protected document (neither is supported)"
                    .into());
            }
            return Err("not a valid .docx file (ZIP archive could not be read)".into());
        }
    };
    let mut c = Converter {
        opts,
        zip,
        base_dir: String::new(),
        rels: HashMap::new(),
        heading_by_style: HashMap::new(),
        style_num: HashMap::new(),
        char_style: HashMap::new(),
        num_to_abstract: HashMap::new(),
        abstract_fmt: HashMap::new(),
        abstract_start: HashMap::new(),
        num_fmt_override: HashMap::new(),
        num_start_override: HashMap::new(),
        list_emitted: HashMap::new(),
        title: String::new(),
        list_stack: Vec::new(),
        fields: Vec::new(),
        fn_order: Vec::new(),
        en_order: Vec::new(),
        out: String::new(),
    };
    c.run()
}

include!("docx_run.rs");
