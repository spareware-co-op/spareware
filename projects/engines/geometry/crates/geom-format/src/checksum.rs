//! CRC-32C, the cheap check every record carries so damage is found before its bytes are trusted
//! (24.2); and CRC-32, the check every zip entry carries (24.7).
//!
//! What it does: [`crc32c`] is the CRC-32C (Castagnoli) of a byte string, computed by the `crc`
//! crate with its catalogue entry `CRC_32_ISCSI`: polynomial 0x1EDC6F41, reflected input and output,
//! initial value and final XOR 0xFFFFFFFF.
//!
//! Guarantees:
//!
//! - **The checksum RFC 7143 and RFC 3720 define.** The tests check the catalogue's check value
//!   (the CRC of `123456789` is 0xE3069283) and RFC 3720's appendix B.4 examples.
//! - **The same value on every target.** The crate is table-driven integer code with no processor
//!   paths, so the bits depend only on the input.
//! - **No panic, no `unsafe`.** The crate forbids `unsafe` code, and its table reads are bounded
//!   by its loop guards and an 8-bit mask (its audit, cited in `deny-no-std.toml`).
//!
//! Method: CRC-32C as RFC 3720 appendix B.4 and RFC 7143 section 13.1 specify it,
//! <https://www.rfc-editor.org/rfc/rfc7143>, computed by the `crc` crate (MIT OR Apache-2.0, a
//! dependency, not a port); chapter 24.2 of the kernel architecture,
//! `projects/engines/geometry/architecture/24-file-format-exchange.md`.
//!
//! Cost: linear in the input, one table read per byte; the table is a constant built at compile
//! time, and nothing is allocated.

use crc::{CRC_32_ISCSI, CRC_32_ISO_HDLC, Crc};

/// The CRC-32C engine, its table built at compile time.
const CASTAGNOLI: Crc<u32> = Crc::<u32>::new(&CRC_32_ISCSI);

/// The CRC-32 engine of zip (ISO-HDLC: polynomial 0x04C11DB7, reflected, initial value and final
/// XOR 0xFFFFFFFF), as PKWARE's APPNOTE 4.4.7 defines it; its table built at compile time.
const ISO_HDLC: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

/// The CRC-32 of `bytes`, as a zip entry declares it.
#[must_use]
pub fn crc32(bytes: &[u8]) -> u32 {
    ISO_HDLC.checksum(bytes)
}

/// The CRC-32C of `bytes`.
#[must_use]
pub fn crc32c(bytes: &[u8]) -> u32 {
    CASTAGNOLI.checksum(bytes)
}
