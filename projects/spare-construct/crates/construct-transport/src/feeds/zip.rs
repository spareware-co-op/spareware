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
    Some(u16::from_le_bytes([*d.get(i)?, *d.get(i.checked_add(1)?)?]))
}

fn u32_at(d: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes([*d.get(i)?, *d.get(i.checked_add(1)?)?, *d.get(i.checked_add(2)?)?, *d.get(i.checked_add(3)?)?]))
}

/// The members of a zip file by name (the part after the last `/`; a later member of the same name wins), as the prototype lists them.
///
/// # Errors
///
/// Returns the message to show when `buf` has no end of central directory record, or uses the ZIP64 format.
pub fn read_zip(buf: &[u8]) -> Result<HashMap<String, ZipEntry>, String> {
    let len = buf.len();
    let lowest = len.saturating_sub(65557);
    // The record is searched for from len - 22 down to lowest; a buffer shorter than 22 bytes has none
    let eocd = len.checked_sub(22).and_then(|start| (lowest..=start).rev().find(|&i| u32_at(buf, i) == Some(0x0605_4b50)));
    let Some(eocd) = eocd else { return Err("The download is not a zip file. The feed may have moved; try again later.".to_string()) };
    // Bound: eocd is at most len - 22, so these offsets never overflow
    let n_ent = u16_at(buf, eocd.saturating_add(10)).ok_or("The download is not a zip file. The feed may have moved; try again later.")?;
    let mut p = u32_at(buf, eocd.saturating_add(16)).ok_or("The download is not a zip file. The feed may have moved; try again later.")? as usize;
    if n_ent == 0xFFFF || p == 0xFFFF_FFFF {
        return Err("This feed uses the ZIP64 format, which Spare Construct cannot read yet.".to_string());
    }
    let mut files = HashMap::new();
    let mut k: u16 = 0;
    // Bound: p + 46 <= len inside the loop and each length is below 65536, so the sums stay far below usize::MAX
    while k < n_ent && p.saturating_add(46) <= len && u32_at(buf, p) == Some(0x0201_4b50) {
        let nl = u16_at(buf, p.saturating_add(28)).unwrap_or(0) as usize;
        let xl = u16_at(buf, p.saturating_add(30)).unwrap_or(0) as usize;
        let cl = u16_at(buf, p.saturating_add(32)).unwrap_or(0) as usize;
        let start = p.saturating_add(46);
        let name_bytes = buf.get(start..start.saturating_add(nl).min(len)).unwrap_or(&[]);
        let name = String::from_utf8_lossy(name_bytes).to_string();
        let base = name.rsplit('/').next().unwrap_or("").to_string();
        files.insert(
            base,
            ZipEntry { method: u16_at(buf, p.saturating_add(10)).unwrap_or(0), csize: u32_at(buf, p.saturating_add(20)).unwrap_or(0), lho: u32_at(buf, p.saturating_add(42)).unwrap_or(0) },
        );
        p = start.saturating_add(nl).saturating_add(xl).saturating_add(cl);
        // k < n_ent, so this never saturates
        k = k.saturating_add(1);
    }
    Ok(files)
}

/// The data of one member: stored as it is, or inflated. `name` is only used in the error texts.
///
/// # Errors
///
/// Returns the message to show when the member uses ZIP64 or a method other than stored or deflate, when the
/// file is cut short, or when the deflated data is corrupted.
pub fn member_bytes(buf: &[u8], f: &ZipEntry, name: &str) -> Result<Vec<u8>, String> {
    if f.csize == 0xFFFF_FFFF {
        return Err("This feed uses the ZIP64 format, which Spare Construct cannot read yet.".to_string());
    }
    let h = f.lho as usize;
    let nl = u16_at(buf, h.saturating_add(26)).ok_or("The zip file is cut short.")? as usize;
    let xl = u16_at(buf, h.saturating_add(28)).ok_or("The zip file is cut short.")? as usize;
    // Bound: h is a u32 and nl, xl below 65536, so the sum stays far below usize::MAX
    let at = h.saturating_add(30).saturating_add(nl).saturating_add(xl);
    let start = at.min(buf.len());
    let raw = buf.get(start..at.saturating_add(f.csize as usize).min(buf.len())).unwrap_or(&[]);
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
        // Bound: n is at most 13, so cnt stays below 21 and pos below data.len()
        while self.cnt < n {
            let b = *self.data.get(self.pos).ok_or("The compressed data ends too soon.")?;
            self.pos = self.pos.saturating_add(1);
            self.buf |= u32::from(b) << self.cnt;
            self.cnt = self.cnt.saturating_add(8);
        }
        let v = self.buf & (1u32 << n).saturating_sub(1);
        self.buf >>= n;
        self.cnt = self.cnt.saturating_sub(n);
        Ok(v)
    }
}

struct Huffman {
    count: [u16; 16],
    symbol: Vec<u16>,
}

// Bound: every code length passed in is below 16, and there are at most 320 of them, so the counts and
// offsets stay far below u16::MAX and every offset is an index into symbol
fn construct(lengths: &[u8]) -> Huffman {
    let mut count = [0u16; 16];
    for &l in lengths {
        if let Some(c) = count.get_mut(usize::from(l)) {
            *c = c.saturating_add(1);
        }
    }
    // offs[len] is the sum of count[1..len], for len from 2 to 15
    let mut offs = [0u16; 16];
    let mut acc = 0u16;
    for (slot, &c) in offs.iter_mut().skip(2).zip(count.iter().skip(1)) {
        acc = acc.saturating_add(c);
        *slot = acc;
    }
    let mut symbol = vec![0u16; lengths.len()];
    for (sym, &l) in lengths.iter().enumerate() {
        if l != 0
            && let Some(off) = offs.get_mut(usize::from(l))
        {
            if let Some(slot) = symbol.get_mut(usize::from(*off)) {
                *slot = u16::try_from(sym).unwrap_or(u16::MAX);
            }
            *off = off.saturating_add(1);
        }
    }
    Huffman { count, symbol }
}

fn decode(b: &mut Bits, h: &Huffman) -> Result<u16, String> {
    let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
    // Bound: code, first and index stay below 2^16, far inside i32
    for &n in h.count.iter().skip(1) {
        code |= b.need(1)?.cast_signed();
        let count = i32::from(n);
        if code.saturating_sub(count) < first {
            // The index is below the number of codes, so it is always in symbol
            let at = usize::try_from(index.saturating_add(code.saturating_sub(first))).ok();
            return at.and_then(|i| h.symbol.get(i)).copied().ok_or_else(|| "The compressed data is corrupted.".to_string());
        }
        index = index.saturating_add(count);
        first = first.saturating_add(count);
        first <<= 1;
        code <<= 1;
    }
    Err("The compressed data is corrupted.".to_string())
}

const LBASE: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEXT: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DBASE: [u16; 30] = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const DEXT: [u8; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];
const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

fn codes(b: &mut Bits, out: &mut Vec<u8>, lit: &Huffman, dist: &Huffman) -> Result<(), String> {
    loop {
        let sym = decode(b, lit)?;
        if let Ok(byte) = u8::try_from(sym) {
            out.push(byte);
        } else if sym == 256 {
            return Ok(());
        } else {
            // sym is above 256 here
            let s = usize::from(sym).saturating_sub(257);
            let (Some(&lbase), Some(&lext)) = (LBASE.get(s), LEXT.get(s)) else {
                return Err("The compressed data is corrupted.".to_string());
            };
            // Bound: lengths and distances are below 2^16, so the sums never overflow
            let len = usize::from(lbase).saturating_add(b.need(u32::from(lext))? as usize);
            let ds = usize::from(decode(b, dist)?);
            let (Some(&dbase), Some(&dext)) = (DBASE.get(ds), DEXT.get(ds)) else {
                return Err("The compressed data is corrupted.".to_string());
            };
            let d = usize::from(dbase).saturating_add(b.need(u32::from(dext))? as usize);
            if d > out.len() {
                return Err("The compressed data is corrupted.".to_string());
            }
            for _ in 0..len {
                // d is at least 1 and at most out.len(), so the index is always in out
                let Some(&x) = out.len().checked_sub(d).and_then(|i| out.get(i)) else {
                    return Err("The compressed data is corrupted.".to_string());
                };
                out.push(x);
            }
        }
    }
}

/// Inflates raw deflate data (what a zip member holds, and what `DecompressionStream('deflate-raw')` reads).
///
/// # Errors
///
/// Returns the message to show when the data ends too soon or is corrupted.
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
                let nlen = u16_at(data, b.pos.saturating_add(2)).ok_or("The compressed data ends too soon.")? as usize;
                if len != (!nlen & 0xFFFF) {
                    return Err("The compressed data is corrupted.".to_string());
                }
                // Bound: pos is at most data.len() and len below 65536
                b.pos = b.pos.saturating_add(4);
                let end = b.pos.saturating_add(len);
                let part = data.get(b.pos..end).ok_or("The compressed data ends too soon.")?;
                out.extend_from_slice(part);
                b.pos = end;
            }
            1 => {
                let mut l = [0u8; 288];
                for (i, x) in l.iter_mut().enumerate() {
                    *x = if i < 144 { 8 } else if i < 256 { 9 } else if i < 280 { 7 } else { 8 };
                }
                let lit = construct(&l);
                let dist = construct(&[5u8; 30]);
                codes(&mut b, &mut out, &lit, &dist)?;
            }
            2 => dynamic_block(&mut b, &mut out)?,
            _ => return Err("The compressed data is corrupted.".to_string()),
        }
        if last == 1 {
            break;
        }
    }
    Ok(out)
}

// A block with dynamic Huffman codes (BTYPE 2)
fn dynamic_block(b: &mut Bits, out: &mut Vec<u8>) -> Result<(), String> {
    // Bound: five- and four-bit values, so the sums are small
    let nlen = (b.need(5)? as usize).saturating_add(257);
    let ndist = (b.need(5)? as usize).saturating_add(1);
    let ncode = (b.need(4)? as usize).saturating_add(4);
    if nlen > 286 || ndist > 30 {
        return Err("The compressed data is corrupted.".to_string());
    }
    let mut lengths = [0u8; 320];
    for &o in ORDER.iter().take(ncode) {
        let v = b.need(3)?;
        // o is below 19 and v below 8
        if let Some(slot) = lengths.get_mut(o) {
            *slot = u8::try_from(v).unwrap_or(0);
        }
    }
    let lencode = construct(&lengths[..19]);
    let mut index: usize = 0;
    let mut ll = [0u8; 320];
    // Bound: total is at most 316, below ll's 320, so every index below is in ll
    let total = nlen.saturating_add(ndist);
    while index < total {
        let sym = decode(b, &lencode)?;
        if sym < 16 {
            if let Some(slot) = ll.get_mut(index) {
                *slot = u8::try_from(sym).unwrap_or(0);
            }
            index = index.saturating_add(1);
        } else {
            let (prev, rep) = match sym {
                16 => {
                    let Some(&prev) = index.checked_sub(1).and_then(|i| ll.get(i)) else {
                        return Err("The compressed data is corrupted.".to_string());
                    };
                    (prev, (b.need(2)? as usize).saturating_add(3))
                }
                17 => (0, (b.need(3)? as usize).saturating_add(3)),
                _ => (0, (b.need(7)? as usize).saturating_add(11)),
            };
            if index.saturating_add(rep) > total {
                return Err("The compressed data is corrupted.".to_string());
            }
            for slot in ll.iter_mut().skip(index).take(rep) {
                *slot = prev;
            }
            index = index.saturating_add(rep);
        }
    }
    let lit = construct(ll.get(..nlen).unwrap_or(&[]));
    let dist = construct(ll.get(nlen..total).unwrap_or(&[]));
    codes(b, out, &lit, &dist)
}
