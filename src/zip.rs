//! Read-only ZIP reader (stored + deflate). Port of zip.cpp.

use crate::inflate::inflate_raw;

pub struct ZipEntry {
    pub name: String,
    pub method: u16,
    pub comp_size: u32,
    pub uncomp_size: u32,
    pub local_offset: u32,
}

const EOCD_SIG: u32 = 0x06054b50;
const CENTRAL_SIG: u32 = 0x02014b50;
const LOCAL_SIG: u32 = 0x04034b50;

fn rd16(p: &[u8]) -> u16 {
    p[0] as u16 | ((p[1] as u16) << 8)
}
fn rd32(p: &[u8]) -> u32 {
    p[0] as u32 | ((p[1] as u32) << 8) | ((p[2] as u32) << 16) | ((p[3] as u32) << 24)
}

pub struct ZipArchive<'a> {
    data: &'a [u8],
    entries: Vec<ZipEntry>,
}

impl<'a> ZipArchive<'a> {
    pub fn open(data: &'a [u8]) -> Option<ZipArchive<'a>> {
        let size = data.len();
        let mut entries = Vec::new();
        if size < 22 {
            return None;
        }
        let max_back = size.min(22 + 65535);
        let stop = size - max_back;
        let mut off = size - 22;
        loop {
            if rd32(&data[off..]) == EOCD_SIG {
                break;
            }
            if off == stop {
                return None;
            }
            off -= 1;
        }
        let count = rd16(&data[off + 10..]) as usize;
        let cd_offset = rd32(&data[off + 16..]) as usize;
        let mut p = cd_offset;
        entries.reserve(count);
        for _ in 0..count {
            if p + 46 > size || rd32(&data[p..]) != CENTRAL_SIG {
                return None;
            }
            let method = rd16(&data[p + 10..]);
            let comp_size = rd32(&data[p + 20..]);
            let uncomp_size = rd32(&data[p + 24..]);
            let name_len = rd16(&data[p + 28..]) as usize;
            let extra_len = rd16(&data[p + 30..]) as usize;
            let comment_len = rd16(&data[p + 32..]) as usize;
            let local_offset = rd32(&data[p + 42..]);
            if p + 46 + name_len > size {
                return None;
            }
            let name = String::from_utf8_lossy(&data[p + 46..p + 46 + name_len]).into_owned();
            entries.push(ZipEntry { name, method, comp_size, uncomp_size, local_offset });
            p += 46 + name_len + extra_len + comment_len;
        }
        Some(ZipArchive { data, entries })
    }

    pub fn find(&self, name: &str) -> Option<&ZipEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    pub fn entries(&self) -> &[ZipEntry] {
        &self.entries
    }

    pub fn extract(&self, entry: &ZipEntry) -> Option<Vec<u8>> {
        let p = entry.local_offset as usize;
        if p + 30 > self.data.len() || rd32(&self.data[p..]) != LOCAL_SIG {
            return None;
        }
        let name_len = rd16(&self.data[p + 26..]) as usize;
        let extra_len = rd16(&self.data[p + 28..]) as usize;
        let data_offset = p + 30 + name_len + extra_len;
        if data_offset + entry.comp_size as usize > self.data.len() {
            return None;
        }
        let src = &self.data[data_offset..data_offset + entry.comp_size as usize];
        if entry.method == 0 {
            if entry.comp_size != entry.uncomp_size {
                return None;
            }
            return Some(src.to_vec());
        }
        if entry.method == 8 {
            let out = inflate_raw(src, entry.uncomp_size as usize)?;
            if out.len() == entry.uncomp_size as usize {
                return Some(out);
            }
            return None;
        }
        None
    }
}
