//! Spare Kit geometry kernel: the ISO 10303-21 encoding.
//!
//! One job: The ISO 10303-21 clear-text encoding: tokenizer, streaming parser, canonical writer.
//!
//! Layer: 12, Format and exchange (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Schema-free: it knows the encoding, not the entities (24.8).
//! - Reads through the `no_std` byte-source traits, checks every length, walks nesting without
//!   recursion, and returns typed errors with the line and the byte offset, never a panic (24.7).
//! - Streams: the reader holds one record at a time, so memory does not grow with the file.
//! - The writer is canonical: canonical instance order and each `f64` as its shortest round-trip
//!   decimal, so write, read, write gives identical bytes (24.10, 24.12).
//! - A port of solidcore's Part 21 reader and writer, converted to the kernel's rules; each
//!   module's `Source:` record says what changed (24.11, 29.20).
//!
//! Sections: 24.7, 24.8, 24.10 and 24.11; open question OQ-24.6; 30.5, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Its dependency on `geom-format`, for the byte sources it parses, is section 30.5's amendment.
//!
//! Its direct dependencies are its row of chapter 27's crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Status: built by piece G415.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod error;
pub mod exchange;
pub mod read;
mod text;
pub mod value;
pub mod write;

pub use error::{Error, ErrorKind, WriteError};
pub use exchange::{Exchange, Header, Totals};
pub use read::{Event, Options, Reader, Section, Token, Tokens};
pub use value::{Entity, List, MAX_DEPTH, Param, Record, TooDeepError};
pub use write::{ByteSink, Writer};
