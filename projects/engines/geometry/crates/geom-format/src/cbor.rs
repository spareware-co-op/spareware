//! SKG's CBOR subset: the strict decoder that refuses anything but deterministic CBOR, its columns,
//! and the lossless text form.
//!
//! What it does: SKG's bytes are deterministic CBOR (RFC 8949, section 4.2.1) with the rules of
//! 24.3. [`decode`] reads them head by head and refuses any other encoding; [`column`] reads the
//! `i64` and `f64` columns coordinates and reals are stored in; [`text`] writes the diagnostic text
//! form of 24.6, and [`parse`] reads it back into the same bytes. The canonical encoder that writes
//! SKG's bytes from values is `geom-op`'s (piece G86), so that `NameId` hashing below this crate uses
//! it too (30.5, "Placed to keep the crate graph acyclic").
//!
//! The subset, item by item:
//!
//! | Item | Allowed | Refused |
//! | --- | --- | --- |
//! | Integers (major types 0 and 1) | −2^64 to 2^64 − 1, shortest head | longer heads |
//! | Bignums (tags 2 and 3) | values beyond 64 bits, no leading zero byte | values that fit, leading zeros |
//! | Byte and text strings | definite length; text in UTF-8 | indefinite lengths, invalid UTF-8 |
//! | Arrays and maps | definite length; map keys in bytewise order of their encodings | indefinite lengths, unsorted or repeated keys |
//! | Floats | finite, in the shortest of half, single and double precision that holds them | NaN, infinities, wider encodings |
//! | Columns (tags 79 and 86) | a byte string of little-endian `i64` or finite `f64` | other lengths, NaN, infinities |
//! | Simple values | `false`, `true`, `null` | `undefined` and every other |
//! | Other tags | none | all |
//!
//! Guarantees:
//!
//! - **One encoding per value.** The decoder accepts exactly the bytes the canonical form gives a
//!   value, so equal accepted values are equal bytes, and a content hash over them is a hash of the
//!   value (24.2, 24.3).
//! - **Text and bytes convert both ways, bit for bit.** For every input the decoder accepts, the
//!   text form reads back to the same bytes.
//!
//! Method: none; it declares its modules and states the subset they share.
//!
//! Cost: see each module.

pub mod column;
pub mod decode;
pub mod error;
mod float;
pub mod item;
pub mod parse;
pub mod text;

pub use column::{float_column, int_column};
pub use decode::{Decoder, SCAN_BYTES_PER_CHARGE, check, item_len};
pub use error::{CborError, CborErrorKind, TextError, TextErrorKind};
pub use item::Item;
pub use parse::from_text;
pub use text::to_text;
