//! HTML escaping and base64. Port of html_util.cpp.

pub fn append_escaped_html(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
}

pub fn append_escaped_attr(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
}

pub fn base64_encode(data: &[u8]) -> String {
    const TBL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity((data.len() + 2) / 3 * 4);
    let len = data.len();
    let mut i = 0;
    while i + 3 <= len {
        let v = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8) | data[i + 2] as u32;
        s.push(TBL[((v >> 18) & 63) as usize] as char);
        s.push(TBL[((v >> 12) & 63) as usize] as char);
        s.push(TBL[((v >> 6) & 63) as usize] as char);
        s.push(TBL[(v & 63) as usize] as char);
        i += 3;
    }
    if i + 1 == len {
        let v = (data[i] as u32) << 16;
        s.push(TBL[((v >> 18) & 63) as usize] as char);
        s.push(TBL[((v >> 12) & 63) as usize] as char);
        s.push_str("==");
    } else if i + 2 == len {
        let v = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8);
        s.push(TBL[((v >> 18) & 63) as usize] as char);
        s.push(TBL[((v >> 12) & 63) as usize] as char);
        s.push(TBL[((v >> 6) & 63) as usize] as char);
        s.push('=');
    }
    s
}
