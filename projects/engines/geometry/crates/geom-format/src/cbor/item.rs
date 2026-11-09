//! One CBOR data item as the decoder yields it: a head and, for strings, the bytes it holds.
//!
//! What it does: [`Item`] names what each head of SKG's CBOR subset says (24.3). A container's
//! head carries its count, and its contents follow as later items, so a decoder hands out one
//! `Item` per head in the order the bytes hold them, and never builds a tree.
//!
//! Guarantees:
//!
//! - **One variant per kind of head the subset allows.** Indefinite lengths, `undefined` and every
//!   other simple value have no variant, because the subset refuses them (24.3).
//!
//! Method: none; it defines a type and nothing that computes.
//!
//! Cost: an `Item` is a few words; strings borrow the input and copy nothing.

/// One data item of SKG's CBOR subset (RFC 8949, section 3): the head, and the content of a string.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Item<'a> {
    /// Major type 0: an unsigned integer.
    Uint(u64),
    /// Major type 1: the negative integer −1 − n, held as n.
    Nint(u64),
    /// Major type 2: a byte string.
    Bytes(&'a [u8]),
    /// Major type 3: a text string, valid UTF-8.
    Text(&'a str),
    /// Major type 4: an array of this many items, which follow.
    Array(u64),
    /// Major type 5: a map of this many pairs; each key is followed by its value.
    Map(u64),
    /// Major type 6: a tag; the one item it tags follows.
    Tag(u64),
    /// Major type 7: a finite float, at the width the bytes gave it.
    Float(f64),
    /// Major type 7: `false` or `true`.
    Bool(bool),
    /// Major type 7: `null`.
    Null,
}
