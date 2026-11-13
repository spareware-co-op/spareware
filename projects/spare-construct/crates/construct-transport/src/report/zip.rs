//! A STORE-method ZIP writer for the data pack (no compression): local headers, central directory and end record. File names
//! are UTF-8 (flag bit 11). Port of the prototype's `crc32` and `zip`.

use super::pack::PackFile;
use crate::jsnum::f64_to_i64;

fn crc_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    for (n, slot) in t.iter_mut().enumerate() {
        // n is below 256
        let mut c = u32::try_from(n).unwrap_or(0);
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *slot = c;
    }
    t
}

/// `crc32(bytes)`: the CRC-32 of the bytes, as the ZIP format wants it.
#[must_use]
pub fn crc32(b: &[u8]) -> u32 {
    let t = crc_table();
    let mut c: u32 = 0xFFFF_FFFF;
    for &x in b {
        // The index is masked to 0..=255, so it is always in the table
        c = t.get(((c ^ u32::from(x)) & 0xFF) as usize).copied().unwrap_or(0) ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

// Days since 1970-01-01 to (year, month 1-12, day) in the proleptic Gregorian calendar.
// Bound: z is a JavaScript date's day count, at most 10^8 in size, so no step comes near i64's range.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z.saturating_add(719_468);
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = doe.saturating_sub(doe / 1460).saturating_add(doe / 36_524).saturating_sub(doe / 146_096) / 365;
    let y = yoe.saturating_add(era.saturating_mul(400));
    let doy = doe.saturating_sub(yoe.saturating_mul(365).saturating_add(yoe / 4).saturating_sub(yoe / 100));
    let mp = doy.saturating_mul(5).saturating_add(2) / 153;
    let d = doy.saturating_sub(mp.saturating_mul(153).saturating_add(2) / 5).saturating_add(1);
    let m = if mp < 10 { mp.saturating_add(3) } else { mp.saturating_sub(9) };
    (if m <= 2 { y.saturating_add(1) } else { y }, m, d)
}

// A field masked to 16 bits: the mask leaves 0..=0xFFFF, so the conversion never falls back
fn low16(v: i64) -> u16 {
    u16::try_from(v & 0xFFFF).unwrap_or(0)
}

/// `zip(files, when)`: the files as a STORE-method ZIP archive, time-stamped with `when_ms` (milliseconds since 1970, read as UTC).
#[must_use]
pub fn zip(files: &[PackFile], when_ms: f64) -> Vec<u8> {
    let t = when_ms.trunc();
    let days = f64_to_i64((t / 86_400_000.0).floor());
    let in_day = f64_to_i64(t.rem_euclid(86_400_000.0));
    let (year, month, mday) = civil_from_days(days);
    let (hours, minutes, seconds) = (in_day / 3_600_000, in_day / 60_000 % 60, in_day / 1000 % 60);
    let dos_t = low16((hours << 11) | (minutes << 5) | (seconds >> 1));
    let dos_d = low16((year.saturating_sub(1980).max(0) << 9) | (month << 5) | mday);
    let mut out: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();
    let mut off: u32 = 0;
    // Bound: the pack's files are text made in memory, far below 4 GiB in all, and their names far below 64 KiB
    for f in files {
        let name = f.name.as_bytes();
        let data = f.data.as_bytes();
        let crc = crc32(data);
        let (len, nlen) = (u32::try_from(data.len()).unwrap_or(u32::MAX), u16::try_from(name.len()).unwrap_or(u16::MAX));
        // local file header
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0x0800u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&dos_t.to_le_bytes());
        out.extend_from_slice(&dos_d.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&nlen.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(data);
        // central directory entry
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0x0800u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&dos_t.to_le_bytes());
        central.extend_from_slice(&dos_d.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&len.to_le_bytes());
        central.extend_from_slice(&len.to_le_bytes());
        central.extend_from_slice(&nlen.to_le_bytes());
        central.extend_from_slice(&[0u8; 12]);
        central.extend_from_slice(&off.to_le_bytes());
        central.extend_from_slice(name);
        off = off.saturating_add(30).saturating_add(u32::from(nlen)).saturating_add(len);
    }
    let cd = u32::try_from(central.len()).unwrap_or(u32::MAX);
    let n = u16::try_from(files.len()).unwrap_or(u16::MAX);
    out.extend_from_slice(&central);
    // end of central directory record
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&cd.to_le_bytes());
    out.extend_from_slice(&off.to_le_bytes());
    out.extend_from_slice(&[0u8; 2]);
    out
}
