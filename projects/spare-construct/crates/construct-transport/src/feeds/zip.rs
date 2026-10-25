//! Reading a zip file and inflating its members, with no library (GTFS timetables come as zip files).
//!
//! The prototype reads the central directory itself and streams each member through the browser's `DecompressionStream`
//! (raw deflate). The same is done here: [`read_zip`] lists the members, [`member_bytes`] gives one member's data
//! (stored or deflated), and [`inflate`] is a decoder for raw deflate data (RFC 1951).

use std::collections::HashMap;

/// A member of a zip file, from the central directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZipEntry {
    /// Compression method: 0 stored, 8 deflate.
    pub method: u16,
    /// Compressed size.
    pub csize: u32,
    /// Where the member's local header starts.
    pub lho: u32,
}

fn u16_at(d: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*d.get(i)?, *d.get(i + 1)?]))
}

fn u32_at(d: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes([*d.get(i)?, *d.get(i + 1)?, *d.get(i + 2)?, *d.get(i + 3)?]))
}

/// The members of a zip file by name (the part after the last `/`; a later member of the same name wins), as the prototype lists them.
pub fn read_zip(buf: &[u8]) -> Result<HashMap<String, ZipEntry>, String> {
    let len = buf.len();
    let mut eocd: Option<usize> = None;
    let lowest = len.saturating_sub(65557);
    let mut i = len as i64 - 22;
    while i >= lowest as i64 {
        if u32_at(buf, i as usize) == Some(0x06054b50) {
            eocd = Some(i as usize);
            break;
        }
        i -= 1;
    }
    let Some(eocd) = eocd else { return Err("The download is not a zip file. The feed may have moved; try again later.".to_string()) };
    let n_ent = u16_at(buf, eocd + 10).ok_or("The download is not a zip file. The feed may have moved; try again later.")?;
    let mut p = u32_at(buf, eocd + 16).ok_or("The download is not a zip file. The feed may have moved; try again later.")? as usize;
    if n_ent == 0xFFFF || p == 0xFFFF_FFFF {
        return Err("This feed uses the ZIP64 format, which Spare Construct cannot read yet.".to_string());
    }
    let mut files = HashMap::new();
    let mut k = 0;
    while k < n_ent && p + 46 <= len && u32_at(buf, p) == Some(0x02014b50) {
        let nl = u16_at(buf, p + 28).unwrap_or(0) as usize;
        let xl = u16_at(buf, p + 30).unwrap_or(0) as usize;
        let cl = u16_at(buf, p + 32).unwrap_or(0) as usize;
        let name_bytes = &buf[(p + 46).min(len)..(p + 46 + nl).min(len)];
        let name = String::from_utf8_lossy(name_bytes).to_string();
        let base = name.rsplit('/').next().unwrap_or("").to_string();
        files.insert(
            base,
            ZipEntry { method: u16_at(buf, p + 10).unwrap_or(0), csize: u32_at(buf, p + 20).unwrap_or(0), lho: u32_at(buf, p + 42).unwrap_or(0) },
        );
        p += 46 + nl + xl + cl;
        k += 1;
    }
    Ok(files)
}

/// The data of one member: stored as it is, or inflated. `name` is only used in the error texts.
pub fn member_bytes(buf: &[u8], f: &ZipEntry, name: &str) -> Result<Vec<u8>, String> {
    if f.csize == 0xFFFF_FFFF {
        return Err("This feed uses the ZIP64 format, which Spare Construct cannot read yet.".to_string());
    }
    let h = f.lho as usize;
    let nl = u16_at(buf, h + 26).ok_or("The zip file is cut short.")? as usize;
    let xl = u16_at(buf, h + 28).ok_or("The zip file is cut short.")? as usize;
    let at = h + 30 + nl + xl;
    let raw = &buf[at.min(buf.len())..(at + f.csize as usize).min(buf.len())];
    match f.method {
        0 => Ok(raw.to_vec()),
        8 => inflate(raw),
        m => Err(format!("{name} uses zip method {m}, which Spare Construct cannot read.")),
    }
}

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    buf: u32,
    cnt: u32,
}

impl Bits<'_> {
    fn need(&mut self, n: u32) -> Result<u32, String> {
        while self.cnt < n {
            let b = *self.data.get(self.pos).ok_or("The compressed data ends too soon.")?;
            self.pos += 1;
            self.buf |= (b as u32) << self.cnt;
            self.cnt += 8;
        }
        let v = self.buf & ((1u32 << n) - 1);
        self.buf >>= n;
        self.cnt -= n;
        Ok(v)
    }
}

struct Huffman {
    count: [u16; 16],
    symbol: Vec<u16>,
}

fn construct(lengths: &[u8]) -> Result<Huffman, String> {
    let mut count = [0u16; 16];
    for &l in lengths {
        count[l as usize] += 1;
    }
    let mut offs = [0u16; 16];
    for len in 1..15 {
        offs[len + 1] = offs[len] + count[len];
    }
    let mut symbol = vec![0u16; lengths.len()];
    for (sym, &l) in lengths.iter().enumerate() {
        if l != 0 {
            symbol[offs[l as usize] as usize] = sym as u16;
            offs[l as usize] += 1;
        }
    }
    Ok(Huffman { count, symbol })
}

fn decode(b: &mut Bits, h: &Huffman) -> Result<u16, String> {
    let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
    for len in 1..16 {
        code |= b.need(1)? as i32;
        let count = h.count[len] as i32;
        if code - count < first {
            return Ok(h.symbol[(index + (code - first)) as usize]);
        }
        index += count;
        first += count;
        first <<= 1;
        code <<= 1;
    }
    Err("The compressed data is corrupted.".to_string())
}

const LBASE: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEXT: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DBASE: [u16; 30] = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const DEXT: [u8; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

fn codes(b: &mut Bits, out: &mut Vec<u8>, lit: &Huffman, dist: &Huffman) -> Result<(), String> {
    loop {
        let sym = decode(b, lit)? as usize;
        if sym < 256 {
            out.push(sym as u8);
        } else if sym == 256 {
            return Ok(());
        } else {
            let s = sym - 257;
            if s >= 29 {
                return Err("The compressed data is corrupted.".to_string());
            }
            let len = LBASE[s] as usize + b.need(LEXT[s] as u32)? as usize;
            let ds = decode(b, dist)? as usize;
            if ds >= 30 {
                return Err("The compressed data is corrupted.".to_string());
            }
            let d = DBASE[ds] as usize + b.need(DEXT[ds] as u32)? as usize;
            if d > out.len() {
                return Err("The compressed data is corrupted.".to_string());
            }
            for _ in 0..len {
                let x = out[out.len() - d];
                out.push(x);
            }
        }
    }
}

/// Inflates raw deflate data (what a zip member holds, and what `DecompressionStream('deflate-raw')` reads).
pub fn inflate(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut b = Bits { data, pos: 0, buf: 0, cnt: 0 };
    let mut out: Vec<u8> = Vec::new();
    loop {
        let last = b.need(1)?;
        match b.need(2)? {
            0 => {
                b.buf = 0;
                b.cnt = 0;
                let len = u16_at(data, b.pos).ok_or("The compressed data ends too soon.")? as usize;
                let nlen = u16_at(data, b.pos + 2).ok_or("The compressed data ends too soon.")? as usize;
                if len != (!nlen & 0xFFFF) {
                    return Err("The compressed data is corrupted.".to_string());
                }
                b.pos += 4;
                let end = b.pos + len;
                let part = data.get(b.pos..end).ok_or("The compressed data ends too soon.")?;
                out.extend_from_slice(part);
                b.pos = end;
            }
            1 => {
                let mut l = [0u8; 288];
                for (i, x) in l.iter_mut().enumerate() {
                    *x = if i < 144 { 8 } else if i < 256 { 9 } else if i < 280 { 7 } else { 8 };
                }
                let lit = construct(&l)?;
                let dist = construct(&[5u8; 30])?;
                codes(&mut b, &mut out, &lit, &dist)?;
            }
            2 => {
                let nlen = b.need(5)? as usize + 257;
                let ndist = b.need(5)? as usize + 1;
                let ncode = b.need(4)? as usize + 4;
                if nlen > 286 || ndist > 30 {
                    return Err("The compressed data is corrupted.".to_string());
                }
                const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
                let mut lengths = [0u8; 320];
                for &o in ORDER.iter().take(ncode) {
                    lengths[o] = b.need(3)? as u8;
                }
                let lencode = construct(&lengths[..19])?;
                let mut index = 0;
                let mut ll = [0u8; 320];
                while index < nlen + ndist {
                    let sym = decode(&mut b, &lencode)?;
                    if sym < 16 {
                        ll[index] = sym as u8;
                        index += 1;
                    } else {
                        let (prev, rep) = match sym {
                            16 => {
                                if index == 0 {
                                    return Err("The compressed data is corrupted.".to_string());
                                }
                                (ll[index - 1], 3 + b.need(2)? as usize)
                            }
                            17 => (0, 3 + b.need(3)? as usize),
                            _ => (0, 11 + b.need(7)? as usize),
                        };
                        if index + rep > nlen + ndist {
                            return Err("The compressed data is corrupted.".to_string());
                        }
                        for _ in 0..rep {
                            ll[index] = prev;
                            index += 1;
                        }
                    }
                }
                let lit = construct(&ll[..nlen])?;
                let dist = construct(&ll[nlen..nlen + ndist])?;
                codes(&mut b, &mut out, &lit, &dist)?;
            }
            _ => return Err("The compressed data is corrupted.".to_string()),
        }
        if last == 1 {
            break;
        }
    }
    Ok(out)
}
