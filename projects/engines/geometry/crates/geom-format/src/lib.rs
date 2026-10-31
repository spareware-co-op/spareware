//! Spare Kit geometry kernel: the native file format.
//!
//! One job: The native file format, its versions and canonical bytes; content hashes, `CacheKey`
//! and the reference memo store.
//!
//! Layer: 12, Format and exchange (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Canonical bytes: deterministic CBOR (RFC 8949, section 4.2), a pure function of the stored
//!   value on every machine and thread count (24.1, 24.3).
//! - Never stores decimal text or rounds a number twice; NaN and infinities are refused both ways
//!   (24.3, 24.13).
//! - Reads untrusted bytes through the `no_std` byte-source traits, checks every length before
//!   allocating, walks nesting with an explicit stack and never panics (24.7).
//! - An unknown required record stops the reader with `UnsupportedRecord`; an unknown optional
//!   record is skipped and listed (24.2).
//! - Readers for every old version are kept, and a file opens without replaying anything (24.6,
//!   26.2).
//! - Never depends on an exchange crate or on a recipe's owner (24.4).
//!
//! Sections: 12.12, 16.1, 18.10, 21, 23.8, 23.11 and 24.1 to 24.7, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! "Everything stored" in chapter 27's row is the list section 24.4 gives: each crate whose values
//! the records hold.
//!
//! Its direct dependencies are its row of chapter 27's crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`), and `crc`, for records' CRC-32C (24.2).
//!
//! Licences of the published crates this crate depends on (DEPEND, chapter 29.1), as each one's
//! manifest at the version in `Cargo.lock` states it:
//!
//! - MIT OR Apache-2.0: `crc`.
//!
//! Status: the strict CBOR decoder, its columns and the text form built (G91); the record layer,
//! byte sources, files, salvage, chunks and range checks built (G92); the header record and the
//! registry built (G93); the rest by pieces G94, G95, G96, G105, G108, G112, G175, G318, G405, G410, G411, G459.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod attributes;
pub mod cbor;
pub mod checksum;
pub mod chunk;
pub mod content;
pub mod file;
pub mod header;
pub mod history;
pub mod names;
pub mod operation;
pub mod payload;
pub mod range;
pub mod recipe;
pub mod record;
pub mod registry;
pub mod source;
