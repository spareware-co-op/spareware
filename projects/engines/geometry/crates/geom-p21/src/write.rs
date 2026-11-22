//! The canonical Part 21 writer: one way to write each file, so writing, reading and writing again
//! gives the same bytes.
//!
//! What it does: a [`Writer`] writes an exchange structure to a [`ByteSink`] a record at a time,
//! in file order: [`Writer::begin`], header records, [`Writer::end_section`], then any number of
//! `DATA` sections of instances, then [`Writer::finish`]. The canonical form:
//!
//! - one line per section keyword, header record and instance, each ended by a line feed; no
//!   other white space and no comments;
//! - instances in rising order of id through the whole file;
//! - a complex instance's records in the order of their names, as Part 21 asks;
//! - an integer in decimal; a real as its shortest decimal that reads back to the same double,
//!   always with a decimal point, the exponent after `E`;
//! - a string in printable ASCII, a quote and a backslash doubled, every other character in an
//!   `\X2\` or `\X4\` run, neighbours sharing a run.
//!
//! Guarantees:
//!
//! - **What it writes, the reader reads**, to the same values: every name, enumeration, binary
//!   and real is checked first, and what Part 21 cannot hold is a [`WriteError`], with nothing of
//!   that record written.
//! - **Canonical.** The bytes depend only on the values written, never on the machine, the
//!   thread or the time: the header's time stamp and author are the caller's inputs.
//! - **Memory does not grow with the file.** The writer holds one line and hands the sink at most
//!   about 64 KiB at a time.
//! - **No recursion.** Nested lists are walked with an explicit stack.
//!
//! Method: the exchange structure of ISO 10303-21 (<https://www.iso.org/standard/63141.html>),
//! written deterministically as chapter 24.10 of
//! `projects/engines/geometry/architecture/24-file-format-exchange.md` asks.
//!
//! Cost: linear in what is written; one call of the sink per 64 KiB. Recursion: none.
//!
//! Source: PORT from solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/a3a8244566081e0777284dc182f77a91bb1a456d/crates/solidcore-step/src/write.rs>
//!   Copyright (c) 2026 The solidcore contributors
//!   Changed for Spare Kit: the Part 21 half of the origin's `Writer` (`param`, `params`,
//!   `simple`, `add_complex` and `file`), in `no_std` Rust, streaming to a sink where the origin
//!   kept every line and built one `String`. The caller gives each instance its id, in rising
//!   order; the origin numbered them itself and shared identical ones through a `HashMap`, which
//!   is the schema writer's business and a type the kernel bans. The header is the caller's
//!   records (the origin wrote a fixed one naming its own product). Lists are walked with a
//!   stack, not by recursion, and every value is checked before it is written (the origin wrote
//!   any name and wrote `0.` for a real that is not finite).
//! SPDX-License-Identifier: Apache-2.0

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

use crate::error::WriteError;
use crate::text;
use crate::value::{Param, Record};

/// Where a writer's bytes go, in order.
pub trait ByteSink {
    /// Takes `bytes`, all of them.
    ///
    /// # Errors
    ///
    /// The sink's own failure, as the program's number; the writer returns it as
    /// [`WriteError::Sink`].
    fn put(&mut self, bytes: &[u8]) -> Result<(), u32>;
}

impl ByteSink for Vec<u8> {
    fn put(&mut self, bytes: &[u8]) -> Result<(), u32> {
        self.extend_from_slice(bytes);
        Ok(())
    }
}

impl<K: ByteSink + ?Sized> ByteSink for &mut K {
    fn put(&mut self, bytes: &[u8]) -> Result<(), u32> {
        (**self).put(bytes)
    }
}

/// The bytes the writer gathers before it calls the sink.
const FLUSH_BYTES: usize = 64 << 10;

/// Where in the file the writer is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Start,
    Header,
    Between,
    Data,
}

/// Whether `name` is `[A-Z_][A-Z0-9_]*`.
fn is_plain_name(name: &[u8]) -> bool {
    match name {
        [first, rest @ ..] => {
            (first.is_ascii_uppercase() || *first == b'_')
                && rest
                    .iter()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == b'_')
        }
        [] => false,
    }
}

/// Whether `name` is an entity or type name: an optional `!`, then a plain name.
fn is_name(name: &str) -> bool {
    let name = name.as_bytes();
    is_plain_name(name.strip_prefix(b"!").unwrap_or(name))
}

/// Whether `digits` is a binary: a first digit `0` to `3`, then upper-case hexadecimal digits.
fn is_binary(digits: &str) -> bool {
    match digits.as_bytes() {
        [first, rest @ ..] => {
            (b'0'..=b'3').contains(first)
                && rest
                    .iter()
                    .all(|c| c.is_ascii_digit() || (b'A'..=b'F').contains(c))
        }
        [] => false,
    }
}

/// A canonical Part 21 writer to the sink `K`.
pub struct Writer<K> {
    sink: K,
    state: State,
    /// The id of the last instance written.
    last: Option<u64>,
    /// Bytes not yet given to the sink.
    out: Vec<u8>,
    /// Where the record being written starts in `out`, so a refused one is taken back.
    mark: usize,
    /// Working space for numbers.
    number: String,
}

impl<K: ByteSink> Writer<K> {
    /// A writer to `sink`, which has written nothing yet.
    pub const fn new(sink: K) -> Self {
        Self {
            sink,
            state: State::Start,
            last: None,
            out: Vec::new(),
            mark: 0,
            number: String::new(),
        }
    }

    fn flush(&mut self) -> Result<(), WriteError> {
        let put = self.sink.put(&self.out);
        self.out.clear();
        put.map_err(WriteError::Sink)
    }

    /// Ends a line, and hands the sink what has gathered once it is enough.
    fn line(&mut self) -> Result<(), WriteError> {
        self.out.push(b'\n');
        if self.out.len() >= FLUSH_BYTES {
            self.flush()?;
        }
        Ok(())
    }

    /// Runs `write`, and takes back what it wrote of its record when it refuses.
    fn record_or_nothing(
        &mut self,
        write: impl FnOnce(&mut Self) -> Result<(), WriteError>,
    ) -> Result<(), WriteError> {
        self.mark = self.out.len();
        let done = write(self);
        if done.is_err() {
            self.out.truncate(self.mark);
        }
        done
    }

    /// Writes `ISO-10303-21;` and opens the header.
    ///
    /// # Errors
    ///
    /// [`WriteError::OutOfOrder`] unless this is the first call.
    pub fn begin(&mut self) -> Result<(), WriteError> {
        if self.state != State::Start {
            return Err(WriteError::OutOfOrder);
        }
        self.out.extend_from_slice(b"ISO-10303-21;\nHEADER;");
        self.state = State::Header;
        self.line()
    }

    /// Writes one header record.
    ///
    /// # Errors
    ///
    /// [`WriteError::OutOfOrder`] outside the header; a value Part 21 cannot hold.
    pub fn header_record(&mut self, record: &Record) -> Result<(), WriteError> {
        if self.state != State::Header {
            return Err(WriteError::OutOfOrder);
        }
        self.record_or_nothing(|w| {
            w.record(record)?;
            w.out.push(b';');
            Ok(())
        })?;
        self.line()
    }

    /// Writes `ENDSEC;`, closing the header or the open `DATA` section.
    ///
    /// # Errors
    ///
    /// [`WriteError::OutOfOrder`] when no section is open.
    pub fn end_section(&mut self) -> Result<(), WriteError> {
        if !matches!(self.state, State::Header | State::Data) {
            return Err(WriteError::OutOfOrder);
        }
        self.out.extend_from_slice(b"ENDSEC;");
        self.state = State::Between;
        self.line()
    }

    /// Writes `DATA;`, opening a section of instances.
    ///
    /// # Errors
    ///
    /// [`WriteError::OutOfOrder`] before the header is closed or inside a section.
    pub fn data(&mut self) -> Result<(), WriteError> {
        if self.state != State::Between {
            return Err(WriteError::OutOfOrder);
        }
        self.out.extend_from_slice(b"DATA;");
        self.state = State::Data;
        self.line()
    }

    /// Writes the instance `#id` with its records: one for a simple instance, several for a
    /// complex one, which are written in the order of their names.
    ///
    /// # Errors
    ///
    /// [`WriteError::OutOfOrder`] outside a `DATA` section; [`WriteError::InstanceOrder`] when
    /// `id` is not above the last one written; [`WriteError::EmptyInstance`]; a value Part 21
    /// cannot hold.
    pub fn instance(&mut self, id: u64, records: &[Record]) -> Result<(), WriteError> {
        if self.state != State::Data {
            return Err(WriteError::OutOfOrder);
        }
        if let Some(last) = self.last.filter(|last| id <= *last) {
            return Err(WriteError::InstanceOrder { id, last });
        }
        self.record_or_nothing(|w| {
            w.out.push(b'#');
            w.integer(id);
            w.out.push(b'=');
            match records {
                [] => return Err(WriteError::EmptyInstance),
                [one] => w.record(one)?,
                many => {
                    let mut sorted: Vec<&Record> = many.iter().collect();
                    sorted.sort_by(|a, b| a.name.cmp(&b.name));
                    w.out.push(b'(');
                    for record in sorted {
                        w.record(record)?;
                    }
                    w.out.push(b')');
                }
            }
            w.out.push(b';');
            Ok(())
        })?;
        self.last = Some(id);
        self.line()
    }

    /// Writes `END-ISO-10303-21;` and gives the sink back, everything handed to it.
    ///
    /// # Errors
    ///
    /// [`WriteError::OutOfOrder`] while a section is open or before the header.
    pub fn finish(mut self) -> Result<K, WriteError> {
        if self.state != State::Between {
            return Err(WriteError::OutOfOrder);
        }
        self.out.extend_from_slice(b"END-ISO-10303-21;\n");
        self.flush()?;
        Ok(self.sink)
    }

    fn integer(&mut self, i: impl core::fmt::Display) {
        self.number.clear();
        // Writing into a `String` cannot fail.
        let _ = write!(self.number, "{i}");
        self.out.extend_from_slice(self.number.as_bytes());
    }

    fn name(&mut self, name: &str) -> Result<(), WriteError> {
        if !is_name(name) {
            return Err(WriteError::BadName);
        }
        self.out.extend_from_slice(name.as_bytes());
        Ok(())
    }

    /// `NAME(params)`, its nested lists walked with a stack of the lists left open.
    fn record(&mut self, record: &Record) -> Result<(), WriteError> {
        self.name(&record.name)?;
        self.out.push(b'(');
        let mut open: Vec<core::slice::Iter<'_, Param>> = Vec::new();
        let mut items = record.params.iter();
        let mut first = true;
        loop {
            let Some(param) = items.next() else {
                self.out.push(b')');
                match open.pop() {
                    Some(outer) => {
                        items = outer;
                        first = false;
                        continue;
                    }
                    None => return Ok(()),
                }
            };
            if !first {
                self.out.push(b',');
            }
            first = false;
            let inner = match param {
                Param::List(list) => list,
                Param::Typed(name, list) => {
                    self.name(name)?;
                    list
                }
                Param::Int(i) => {
                    self.integer(*i);
                    continue;
                }
                Param::Real(x) => {
                    if !text::real(*x, &mut self.number, &mut self.out) {
                        return Err(WriteError::NonFiniteReal);
                    }
                    continue;
                }
                Param::Str(s) => {
                    text::encode(s, &mut self.out);
                    continue;
                }
                Param::Enum(e) => {
                    if !is_plain_name(e.as_bytes()) {
                        return Err(WriteError::BadEnumeration);
                    }
                    self.out.push(b'.');
                    self.out.extend_from_slice(e.as_bytes());
                    self.out.push(b'.');
                    continue;
                }
                Param::Ref(r) => {
                    self.out.push(b'#');
                    self.integer(*r);
                    continue;
                }
                Param::Binary(b) => {
                    if !is_binary(b) {
                        return Err(WriteError::BadBinary);
                    }
                    self.out.push(b'"');
                    self.out.extend_from_slice(b.as_bytes());
                    self.out.push(b'"');
                    continue;
                }
                Param::Null => {
                    self.out.push(b'$');
                    continue;
                }
                Param::Derived => {
                    self.out.push(b'*');
                    continue;
                }
            };
            self.out.push(b'(');
            open.push(core::mem::replace(&mut items, inner.items().iter()));
            first = true;
        }
    }
}
