//! A STORE-method ZIP writer for the data pack (no compression): local headers, central directory and end record. File names
//! are UTF-8 (flag bit 11). Port of the prototype's `crc32` and `zip`.

use super::pack::PackFile;

fn crc_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    for (n, slot) in t.iter_mut().enumerate() {
        let mut c = n as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *slot = c;
    }
    t
}

/// `crc32(bytes)`: the CRC-32 of the bytes, as the ZIP format wants it.
pub fn crc32(b: &[u8]) -> u32 {
    let t = crc_table();
    let mut c: u32 = 0xFFFF_FFFF;
    for &x in b {
        c = t[((c ^ x as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

// Days since 1970-01-01 to (year, month 1-12, day) in the proleptic Gregorian calendar
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `zip(files, when)`: the files as a STORE-method ZIP archive, time-stamped with `when_ms` (milliseconds since 1970, read as UTC).
pub fn zip(files: &[PackFile], when_ms: f64) -> Vec<u8> {
    let t = when_ms.trunc();
    let days = (t / 86_400_000.0).floor() as i64;
    let in_day = t.rem_euclid(86_400_000.0) as i64;
    let (year, month, date) = civil_from_days(days);
    let (hours, minutes, seconds) = (in_day / 3_600_000, in_day / 60_000 % 60, in_day / 1000 % 60);
    let dos_t = (((hours << 11) | (minutes << 5) | (seconds >> 1)) & 0xFFFF) as u16;
    let dos_d = ((((year - 1980).max(0) << 9) | (month << 5) | date) & 0xFFFF) as u16;
    let mut out: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();
    let mut off: u32 = 0;
    for f in files {
        let name = f.name.as_bytes();
        let data = f.data.as_bytes();
        let crc = crc32(data);
        let (len, nlen) = (data.len() as u32, name.len() as u16);
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
        off += 30 + nlen as u32 + len;
    }
    let cd = central.len() as u32;
    let n = files.len() as u16;
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
