//! The streaming Part 21 reader: one header record or entity instance at a time from a byte
//! source, in memory that does not grow with the file.
//!
//! What it does: a [`Reader`] pulls bytes from a [`ByteSource`] through one fixed buffer and
//! hands back [`Event`]s in file order: a section opening, a header record, an entity instance,
//! a section's end, a skipped section. The record or instance just read is in the reader's
//! [`Tokens`], a flat list ([`Token`]) the next call overwrites, so reading costs no allocation
//! once the buffers have grown to the largest record. [`Tokens::records`] gives the same thing as
//! a tree ([`crate::value`]).
//!
//! The grammar read, which is ISO 10303-21's exchange structure with the leniencies named:
//!
//! - `ISO-10303-21;`, then `HEADER;` with its records and `ENDSEC;`, then any number of `DATA`
//!   sections (each may carry a parameter list), then `END-ISO-10303-21;`. A UTF-8 byte order
//!   mark before the first is skipped. Bytes after the last are not read.
//! - `ANCHOR`, `REFERENCE` and `SIGNATURE` sections (edition 3) are skipped to their `ENDSEC;`,
//!   reading strings and comments as such, so an `ENDSEC;` inside one of those does not end the
//!   skip.
//! - An instance is `#id=NAME(...);` or, complex, `#id=(A(...)B(...));`. A name is an optional
//!   `!`, then a letter or `_`, then letters, digits and `_`; lower case is read as upper case.
//! - A parameter is `$`, `*`, an integer, a real, a string, `.ENUM.`, `"binary"`, `#id`, a list
//!   or a typed value `NAME(...)`. A real may be written without its decimal point when it has an
//!   exponent (`1E5`), with a lower-case `e`, or with no digit before the point (`.5`); an integer
//!   has neither point nor exponent.
//! - White space and `/* ... */` comments may stand between any two tokens. A line break inside
//!   a string is skipped.
//!
//! Guarantees:
//!
//! - **Every malformed input is a typed error**, an [`Error`] with the line and byte offset where
//!   reading stopped, never a panic, and the reader gives the same error again if asked again.
//! - **The split never matters.** The events, tokens and errors are the same however the source
//!   cuts its reads and whatever [`Options::buffer_bytes`] is.
//! - **Memory is bounded by the options, not the file.** The reader holds its buffer and one
//!   record or instance: at most [`Options::items`] items and [`Options::text_bytes`] of text,
//!   plus one buffer more ([`Reader::held_bytes`] reports it). Offsets and lines are 64-bit, so
//!   files beyond 4 GiB read on a 32-bit target.
//! - **No recursion.** Lists nest to [`MAX_DEPTH`] and are walked by a counter.
//! - **The work is metered.** Every byte taken from the source is one unit charged to the
//!   caller's [`WorkMeter`], which also carries the cancellation, read at each refill.
//!
//! Method: a hand-written scanner and state machine over ISO 10303-21's exchange structure
//! (<https://www.iso.org/standard/63141.html>), under the reader rules of chapter 24.7 of
//! `projects/engines/geometry/architecture/24-file-format-exchange.md`.
//!
//! Cost: linear in the bytes read; one pass, one call of the source per buffer. Allocation only
//! when a record is larger than every one before it. Recursion: none.
//!
//! Source: PORT from solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/a3a8244566081e0777284dc182f77a91bb1a456d/crates/solidcore-record/src/step/p21.rs>
//!   Copyright (c) 2026 The solidcore contributors
//!   Changed for Spare Kit: the `Lexer` and `parse_limited`, turned inside out into a pull
//!   reader over a byte source in `no_std` Rust: it yields one record at a time as flat tokens
//!   where the origin parsed a whole `&str` into a map. Errors are kinds with a line and an
//!   offset counted as bytes are read (the origin formatted messages and recounted lines).
//!   Parameter lists are walked by a counter, not by recursion. The caps on the whole file moved
//!   to `exchange`, and caps on one record took their place here. Stricter than the origin: a
//!   number follows the grammar above (the origin took any run of digits, signs, points and `E`
//!   that Rust then parsed), a real beyond `f64` is refused (it read as infinity), an
//!   enumeration and a binary are checked (any bytes between the marks were taken), an instance
//!   needs a record (`#1=();` was read), `HEADER` must come first and once, and a malformed
//!   header record is an error (the origin skipped to the next line, in silence). Skipped
//!   sections are scanned by token (the origin's stated limit: the first raw `ENDSEC;`). The cap
//!   of 64 parts on a complex instance is gone: the item cap bounds it. The table of AP203 short
//!   entity names is not taken: it is schema knowledge, for the reader of the schema.
//! SPDX-License-Identifier: Apache-2.0

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::num::IntErrorKind;

use geom_format::source::{ByteSource, SourceError};
use geom_run::WorkMeter;

use crate::error::{Error, ErrorKind};
use crate::text;
use crate::value::{List, MAX_DEPTH, Param, Record};

/// What a [`Reader`] may hold and how it reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    /// The most items (parameters, list ends and record names) in one header record or instance.
    pub items: usize,
    /// The most bytes of names and string text in one header record or instance.
    pub text_bytes: usize,
    /// Whether text that is not Unicode is replaced by U+FFFD and counted
    /// ([`Reader::replaced`]) rather than refused ([`ErrorKind::BadText`]).
    pub lossy_text: bool,
    /// The size of the buffer the source is read through; 0 is read as 1.
    pub buffer_bytes: usize,
}

impl Options {
    /// The defaults: 4,000,000 items (64 MB of tokens) and 64 MiB of text in one record, strict
    /// text, and a 64 KiB buffer. The largest instances of real files, point lists and meshes
    /// written as one instance, hold a few hundred thousand items.
    pub const DEFAULT: Self = Self {
        items: 4_000_000,
        text_bytes: 64 << 20,
        lossy_text: false,
        buffer_bytes: 64 << 10,
    };
}

impl Default for Options {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// A section of the exchange structure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    /// `HEADER`.
    Header,
    /// `DATA`.
    Data,
    /// `ANCHOR` (edition 3).
    Anchor,
    /// `REFERENCE` (edition 3).
    Reference,
    /// `SIGNATURE` (edition 3).
    Signature,
}

/// What the reader read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Event {
    /// A `HEADER` or `DATA` section opened. For `DATA(...)` the tokens hold its parameters as one
    /// record named `DATA`; otherwise they are empty.
    Section(Section),
    /// A header record; it is in the tokens.
    HeaderRecord,
    /// The entity instance with this id; its records are in the tokens.
    Instance(u64),
    /// `ENDSEC;` of the open section.
    EndSection,
    /// A whole section that was skipped.
    Skipped(Section),
}

/// One stored token: text is a span of [`Tokens::text`].
#[derive(Clone, Copy, Debug, PartialEq)]
enum Item {
    Record(u32, u32),
    Typed(u32, u32),
    Open,
    Close,
    Int(i64),
    Real(f64),
    Ref(u64),
    Str(u32, u32),
    Enum(u32, u32),
    Binary(u32, u32),
    Null,
    Derived,
}

/// One token of a record or instance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Token<'a> {
    /// A record opens: its entity name, in upper case. Its parameters follow, to the matching
    /// [`Token::Close`].
    Record(&'a str),
    /// A typed value opens: its name, in upper case. Its parameters follow, to the matching
    /// [`Token::Close`].
    Typed(&'a str),
    /// A list opens.
    Open,
    /// The innermost open list, typed value or record closes.
    Close,
    /// An integer.
    Int(i64),
    /// A real, finite.
    Real(f64),
    /// An entity instance name, `#n`.
    Ref(u64),
    /// A string, its escapes decoded.
    Str(&'a str),
    /// An enumeration, without its dots, in upper case.
    Enum(&'a str),
    /// A binary's digits.
    Binary(&'a str),
    /// `$`
    Null,
    /// `*`
    Derived,
}

/// The header record, `DATA` parameters or instance the reader read last, as flat tokens.
#[derive(Clone, Debug, Default)]
pub struct Tokens {
    items: Vec<Item>,
    text: String,
    params: usize,
}

impl Tokens {
    fn clear(&mut self) {
        self.items.clear();
        self.text.clear();
        self.params = 0;
    }

    fn str(&self, at: u32, len: u32) -> &str {
        let at = usize::try_from(at).unwrap_or(usize::MAX);
        let len = usize::try_from(len).unwrap_or(usize::MAX);
        self.text.get(at..at.saturating_add(len)).unwrap_or("")
    }

    fn token(&self, item: Item) -> Token<'_> {
        match item {
            Item::Record(a, n) => Token::Record(self.str(a, n)),
            Item::Typed(a, n) => Token::Typed(self.str(a, n)),
            Item::Open => Token::Open,
            Item::Close => Token::Close,
            Item::Int(i) => Token::Int(i),
            Item::Real(x) => Token::Real(x),
            Item::Ref(r) => Token::Ref(r),
            Item::Str(a, n) => Token::Str(self.str(a, n)),
            Item::Enum(a, n) => Token::Enum(self.str(a, n)),
            Item::Binary(a, n) => Token::Binary(self.str(a, n)),
            Item::Null => Token::Null,
            Item::Derived => Token::Derived,
        }
    }

    /// The tokens, in file order.
    pub fn iter(&self) -> impl Iterator<Item = Token<'_>> {
        self.items.iter().map(|item| self.token(*item))
    }

    /// Whether there is no token.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// How many parameters the tokens hold, nested ones and each list counted.
    #[must_use]
    pub const fn params(&self) -> usize {
        self.params
    }

    /// How many records the tokens hold.
    #[must_use]
    pub fn record_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| matches!(item, Item::Record(..)))
            .count()
    }

    /// The tokens as records: one for a header record or a simple instance, several for a
    /// complex instance.
    #[must_use]
    pub fn records(&self) -> Vec<Record> {
        enum Frame {
            Record(String),
            Typed(String),
            List,
        }
        let mut records = Vec::new();
        let mut stack: Vec<(Frame, Vec<Param>)> = Vec::new();
        for token in self.iter() {
            let param = match token {
                Token::Record(name) => {
                    stack.push((Frame::Record(name.into()), Vec::new()));
                    continue;
                }
                Token::Typed(name) => {
                    stack.push((Frame::Typed(name.into()), Vec::new()));
                    continue;
                }
                Token::Open => {
                    stack.push((Frame::List, Vec::new()));
                    continue;
                }
                Token::Close => match stack.pop() {
                    Some((Frame::Record(name), params)) => {
                        records.push(Record { name, params });
                        continue;
                    }
                    Some((Frame::Typed(name), items)) => {
                        Param::Typed(name, List::within_bound(items))
                    }
                    Some((Frame::List, items)) => Param::List(List::within_bound(items)),
                    None => continue,
                },
                Token::Int(i) => Param::Int(i),
                Token::Real(x) => Param::Real(x),
                Token::Ref(r) => Param::Ref(r),
                Token::Str(s) => Param::Str(s.into()),
                Token::Enum(s) => Param::Enum(s.into()),
                Token::Binary(s) => Param::Binary(s.into()),
                Token::Null => Param::Null,
                Token::Derived => Param::Derived,
            };
            if let Some((_, items)) = stack.last_mut() {
                items.push(param);
            }
        }
        records
    }
}

/// The buffered byte source, with the place reading has reached.
struct Lexer<'m, 'c, S> {
    source: S,
    meter: &'m mut WorkMeter<'c>,
    buf: Vec<u8>,
    /// The next byte of `buf` to read.
    pos: usize,
    /// The bytes of `buf` the source filled.
    len: usize,
    ended: bool,
    offset: u64,
    line: u64,
}

impl<S: ByteSource> Lexer<'_, '_, S> {
    const fn error(&self, kind: ErrorKind) -> Error {
        Error {
            kind,
            line: self.line,
            offset: self.offset,
        }
    }

    /// The next byte, not taken; `None` at the end of the source.
    #[inline]
    fn peek(&mut self) -> Result<Option<u8>, Error> {
        if self.pos < self.len {
            return Ok(self.buf.get(self.pos).copied());
        }
        self.refill()
    }

    /// Fills the buffer from the source and gives its first byte; `None` at the source's end.
    #[cold]
    #[inline(never)]
    fn refill(&mut self) -> Result<Option<u8>, Error> {
        if self.ended {
            return Ok(None);
        }
        let n = self
            .source
            .read(&mut self.buf)
            .map_err(|e| self.error(ErrorKind::Source(e)))?;
        if n > self.buf.len() {
            return Err(self.error(ErrorKind::Source(SourceError::Overrun)));
        }
        self.pos = 0;
        self.len = n;
        if n == 0 {
            self.ended = true;
            return Ok(None);
        }
        self.meter
            .charge(u64::try_from(n).unwrap_or(u64::MAX))
            .map_err(|stopped| self.error(ErrorKind::Stopped(stopped)))?;
        Ok(self.buf.first().copied())
    }

    /// Takes the byte `c` that [`peek`](Self::peek) just gave.
    #[inline]
    fn bump(&mut self, c: u8) {
        self.pos = self.pos.saturating_add(1);
        self.offset = self.offset.saturating_add(1);
        if c == b'\n' {
            self.line = self.line.saturating_add(1);
        }
    }

    /// The bytes buffered and not yet taken.
    fn chunk(&self) -> &[u8] {
        self.buf.get(self.pos..self.len).unwrap_or(&[])
    }

    /// Takes `n` buffered bytes, none of them a line feed.
    fn advance(&mut self, n: usize) {
        self.pos = self.pos.saturating_add(n);
        self.offset = self
            .offset
            .saturating_add(u64::try_from(n).unwrap_or(u64::MAX));
    }
}

/// Where in the exchange structure the reader is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Start,
    InHeader,
    Between,
    InData,
    Done,
    Failed(Error),
}

/// Where in a list the reader is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum At {
    /// After `(`: a value or `)`.
    Fresh,
    /// After `,`: a value.
    Value,
    /// After a value: `,` or `)`.
    After,
}

/// A number's text, read to its value.
fn number(text: &[u8]) -> Result<Item, ErrorKind> {
    let mut rest = text;
    let digits = |rest: &mut &[u8]| {
        let n = rest.iter().take_while(|c| c.is_ascii_digit()).count();
        *rest = rest.get(n..).unwrap_or(&[]);
        n
    };
    if let [b'+' | b'-', tail @ ..] = rest {
        rest = tail;
    }
    let whole = digits(&mut rest);
    let mut real = false;
    let mut fraction = 0;
    if let [b'.', tail @ ..] = rest {
        rest = tail;
        real = true;
        fraction = digits(&mut rest);
    }
    if whole == 0 && fraction == 0 {
        return Err(ErrorKind::BadNumber);
    }
    if let [b'E' | b'e', tail @ ..] = rest {
        rest = tail;
        real = true;
        if let [b'+' | b'-', tail @ ..] = rest {
            rest = tail;
        }
        if digits(&mut rest) == 0 {
            return Err(ErrorKind::BadNumber);
        }
    }
    if !rest.is_empty() {
        return Err(ErrorKind::BadNumber);
    }
    let text = core::str::from_utf8(text).map_err(|_| ErrorKind::BadNumber)?;
    if real {
        // Rust reads `1.`, `.5` and `1.E-3` as they are, rounding correctly.
        match text.parse::<f64>() {
            Ok(x) if x.is_finite() => Ok(Item::Real(x)),
            Ok(_) => Err(ErrorKind::RealOutOfRange),
            Err(_) => Err(ErrorKind::BadNumber),
        }
    } else {
        // An integer is a count or an index: one beyond i64 is refused, not rounded into a real.
        text.parse::<i64>().map(Item::Int).map_err(|e| {
            if matches!(
                e.kind(),
                IntErrorKind::PosOverflow | IntErrorKind::NegOverflow
            ) {
                ErrorKind::IntegerOutOfRange
            } else {
                ErrorKind::BadNumber
            }
        })
    }
}

const fn is_name_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// A streaming Part 21 reader over the byte source `S`.
pub struct Reader<'m, 'c, S> {
    lx: Lexer<'m, 'c, S>,
    options: Options,
    state: State,
    tokens: Tokens,
    /// The bytes of the token being read.
    scratch: Vec<u8>,
    replaced: u64,
}

impl<'m, 'c, S: ByteSource> Reader<'m, 'c, S> {
    /// A reader of `source` from its first byte, charging `meter` one unit for each byte read.
    pub fn new(source: S, options: Options, meter: &'m mut WorkMeter<'c>) -> Self {
        Self {
            lx: Lexer {
                source,
                meter,
                buf: vec![0; options.buffer_bytes.max(1)],
                pos: 0,
                len: 0,
                ended: false,
                offset: 0,
                line: 1,
            },
            options,
            state: State::Start,
            tokens: Tokens::default(),
            scratch: Vec::new(),
            replaced: 0,
        }
    }

    /// The header record, `DATA` parameters or instance the last event read.
    #[must_use]
    pub const fn tokens(&self) -> &Tokens {
        &self.tokens
    }

    /// The line reading has reached, counted from 1.
    #[must_use]
    pub const fn line(&self) -> u64 {
        self.lx.line
    }

    /// The bytes read so far.
    #[must_use]
    pub const fn offset(&self) -> u64 {
        self.lx.offset
    }

    /// How many replacement characters lossy text decoding has put in so far.
    #[must_use]
    pub const fn replaced(&self) -> u64 {
        self.replaced
    }

    /// An error of `kind` at the place reading has reached.
    #[must_use]
    pub const fn error(&self, kind: ErrorKind) -> Error {
        self.lx.error(kind)
    }

    /// The bytes the reader's buffers hold now: the source buffer, the tokens and their text.
    #[must_use]
    pub fn held_bytes(&self) -> usize {
        self.lx
            .buf
            .capacity()
            .saturating_add(self.scratch.capacity())
            .saturating_add(self.tokens.text.capacity())
            .saturating_add(
                self.tokens
                    .items
                    .capacity()
                    .saturating_mul(size_of::<Item>()),
            )
    }

    /// Reads on to the next event; `None` once `END-ISO-10303-21;` has been read.
    ///
    /// # Errors
    ///
    /// An [`Error`] for a malformed text, a record beyond the options, a failed source or a
    /// stopped meter. The reader then gives the same error at every later call.
    pub fn next_event(&mut self) -> Result<Option<Event>, Error> {
        match self.state {
            State::Failed(e) => return Err(e),
            State::Done => return Ok(None),
            _ => {}
        }
        self.tokens.clear();
        let step = self.step();
        if let Err(e) = step {
            self.tokens.clear();
            self.state = State::Failed(e);
        }
        step
    }

    fn fail<T>(&self, kind: ErrorKind) -> Result<T, Error> {
        Err(self.lx.error(kind))
    }

    fn step(&mut self) -> Result<Option<Event>, Error> {
        match self.state {
            State::Start => {
                self.magic()?;
                self.ws()?;
                self.word()?;
                if self.scratch != b"HEADER" {
                    return self.fail(ErrorKind::MissingHeader);
                }
                self.expect(b';')?;
                self.state = State::InHeader;
                Ok(Some(Event::Section(Section::Header)))
            }
            State::InHeader => {
                self.ws()?;
                if self.lx.peek()?.is_none() {
                    return self.fail(ErrorKind::UnexpectedEnd);
                }
                let (at, len) = self.keyword()?;
                if self.tokens.text == "ENDSEC" {
                    self.tokens.clear();
                    self.expect(b';')?;
                    self.state = State::Between;
                    return Ok(Some(Event::EndSection));
                }
                self.expect(b'(')?;
                self.push(Item::Record(at, len))?;
                self.list()?;
                self.expect(b';')?;
                Ok(Some(Event::HeaderRecord))
            }
            State::Between => self.section(),
            State::InData => self.instance(),
            State::Done | State::Failed(_) => Ok(None),
        }
    }

    /// `ISO-10303-21;`, after an optional byte order mark.
    fn magic(&mut self) -> Result<(), Error> {
        if self.lx.peek()? == Some(0xEF) {
            for mark in [0xEF, 0xBB, 0xBF] {
                match self.lx.peek()? {
                    Some(c) if c == mark => self.lx.bump(c),
                    _ => return self.fail(ErrorKind::NotP21),
                }
            }
        }
        self.ws()?;
        self.word()?;
        if self.scratch != b"ISO-10303-21" {
            return self.fail(ErrorKind::NotP21);
        }
        self.ws()?;
        match self.lx.peek()? {
            Some(c @ b';') => {
                self.lx.bump(c);
                Ok(())
            }
            _ => self.fail(ErrorKind::NotP21),
        }
    }

    /// What follows a closed section: another section or the end.
    fn section(&mut self) -> Result<Option<Event>, Error> {
        self.ws()?;
        if self.lx.peek()?.is_none() {
            return self.fail(ErrorKind::UnexpectedEnd);
        }
        self.word()?;
        let skipped = match self.scratch.as_slice() {
            b"DATA" => {
                self.ws()?;
                if let Some(c @ b'(') = self.lx.peek()? {
                    self.lx.bump(c);
                    let (at, len) = self.span()?;
                    self.push(Item::Record(at, len))?;
                    self.list()?;
                }
                self.expect(b';')?;
                self.state = State::InData;
                return Ok(Some(Event::Section(Section::Data)));
            }
            b"END-ISO-10303-21" => {
                self.expect(b';')?;
                self.state = State::Done;
                return Ok(None);
            }
            b"ANCHOR" => Section::Anchor,
            b"REFERENCE" => Section::Reference,
            b"SIGNATURE" => Section::Signature,
            b"" => return self.fail(ErrorKind::ExpectedKeyword),
            _ => return self.fail(ErrorKind::UnexpectedSection),
        };
        self.skip_section()?;
        Ok(Some(Event::Skipped(skipped)))
    }

    /// Reads past a section's content to its `ENDSEC;`, taking strings and comments as such.
    fn skip_section(&mut self) -> Result<(), Error> {
        const ENDSEC: &[u8] = b"ENDSEC";
        loop {
            let Some(c) = self.lx.peek()? else {
                return self.fail(ErrorKind::UnexpectedEnd);
            };
            self.lx.bump(c);
            match c {
                b'\'' => loop {
                    let Some(c) = self.lx.peek()? else {
                        return self.fail(ErrorKind::UnterminatedString);
                    };
                    self.lx.bump(c);
                    if c == b'\'' {
                        // A doubled quote reads as the end of one string and the start of the
                        // next, which skips the same bytes.
                        break;
                    }
                },
                b'/' => {
                    if let Some(c @ b'*') = self.lx.peek()? {
                        self.lx.bump(c);
                        self.comment()?;
                    }
                }
                c if c.is_ascii_alphabetic() || c == b'_' => {
                    // A word: `ENDSEC` only when it is the whole word.
                    let mut matched = usize::from(c.eq_ignore_ascii_case(&b'E'));
                    let mut exact = matched == 1;
                    while let Some(c) = self.lx.peek()?.filter(|c| is_name_byte(*c) || *c == b'-') {
                        self.lx.bump(c);
                        if exact
                            && ENDSEC
                                .get(matched)
                                .is_some_and(|e| c.eq_ignore_ascii_case(e))
                        {
                            matched = matched.saturating_add(1);
                        } else {
                            exact = false;
                        }
                    }
                    if exact && matched == ENDSEC.len() {
                        self.ws()?;
                        if let Some(c @ b';') = self.lx.peek()? {
                            self.lx.bump(c);
                            return Ok(());
                        }
                    }
                }
                c if c.is_ascii_digit() => {
                    // Digits open no word: `1ENDSEC` is not the keyword.
                    while let Some(c) = self.lx.peek()?.filter(|c| is_name_byte(*c) || *c == b'-') {
                        self.lx.bump(c);
                    }
                }
                _ => {}
            }
        }
    }

    /// One instance of a `DATA` section, or the section's `ENDSEC;`.
    fn instance(&mut self) -> Result<Option<Event>, Error> {
        self.ws()?;
        match self.lx.peek()? {
            None => return self.fail(ErrorKind::UnexpectedEnd),
            Some(c @ b'#') => self.lx.bump(c),
            Some(_) => {
                self.word()?;
                if self.scratch != b"ENDSEC" {
                    return self.fail(ErrorKind::Expected(b'#'));
                }
                self.expect(b';')?;
                self.state = State::Between;
                return Ok(Some(Event::EndSection));
            }
        }
        let id = self.id()?;
        self.expect(b'=')?;
        self.ws()?;
        if let Some(c @ b'(') = self.lx.peek()? {
            self.lx.bump(c);
            loop {
                self.ws()?;
                if let Some(c @ b')') = self.lx.peek()? {
                    self.lx.bump(c);
                    break;
                }
                self.record()?;
            }
            if self.tokens.is_empty() {
                return self.fail(ErrorKind::EmptyInstance);
            }
        } else {
            self.record()?;
        }
        self.expect(b';')?;
        Ok(Some(Event::Instance(id)))
    }

    /// `NAME(...)`.
    fn record(&mut self) -> Result<(), Error> {
        if self.lx.peek()?.is_none() {
            return self.fail(ErrorKind::UnexpectedEnd);
        }
        let (at, len) = self.keyword()?;
        self.expect(b'(')?;
        self.push(Item::Record(at, len))?;
        self.list()
    }

    /// Skips white space and comments.
    fn ws(&mut self) -> Result<(), Error> {
        loop {
            match self.lx.peek()? {
                Some(c) if c.is_ascii_whitespace() => self.lx.bump(c),
                Some(c @ b'/') => {
                    self.lx.bump(c);
                    match self.lx.peek()? {
                        Some(c @ b'*') => {
                            self.lx.bump(c);
                            self.comment()?;
                        }
                        _ => return self.fail(ErrorKind::UnexpectedByte(b'/')),
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    /// The rest of a comment whose `/*` was read.
    fn comment(&mut self) -> Result<(), Error> {
        let mut star = false;
        loop {
            let Some(c) = self.lx.peek()? else {
                return self.fail(ErrorKind::UnterminatedComment);
            };
            self.lx.bump(c);
            if star && c == b'/' {
                return Ok(());
            }
            star = c == b'*';
        }
    }

    /// The byte `c`, after white space.
    fn expect(&mut self, c: u8) -> Result<(), Error> {
        self.ws()?;
        match self.lx.peek()? {
            Some(found) if found == c => {
                self.lx.bump(found);
                Ok(())
            }
            Some(_) => self.fail(ErrorKind::Expected(c)),
            None => self.fail(ErrorKind::UnexpectedEnd),
        }
    }

    fn check_text(&self) -> Result<(), Error> {
        if self.tokens.text.len().saturating_add(self.scratch.len()) > self.options.text_bytes {
            return self.fail(ErrorKind::TooMuchText);
        }
        Ok(())
    }

    /// Takes bytes onto the scratch while `keep` holds; `keep` never holds for a line feed.
    fn run(&mut self, keep: impl Fn(u8) -> bool) -> Result<(), Error> {
        while self.lx.peek()?.is_some() {
            let chunk = self.lx.chunk();
            let n = chunk.iter().position(|c| !keep(*c)).unwrap_or(chunk.len());
            let whole = n == chunk.len();
            self.scratch
                .extend_from_slice(chunk.get(..n).unwrap_or(&[]));
            self.lx.advance(n);
            self.check_text()?;
            if !whole {
                break;
            }
        }
        Ok(())
    }

    /// Reads a section word (letters, digits, `_` and `-`) onto the scratch, in upper case; the
    /// scratch is empty when no such byte is next.
    fn word(&mut self) -> Result<(), Error> {
        self.scratch.clear();
        self.run(|c| is_name_byte(c) || c == b'-')?;
        self.scratch.make_ascii_uppercase();
        Ok(())
    }

    /// Moves the scratch, which is ASCII, onto the tokens' text and gives its span.
    fn span(&mut self) -> Result<(u32, u32), Error> {
        let at = self.tokens.text.len();
        self.tokens
            .text
            .push_str(core::str::from_utf8(&self.scratch).unwrap_or(""));
        self.scratch.clear();
        self.text_span(at)
    }

    /// The span of the tokens' text from `at` to its end.
    fn text_span(&self, at: usize) -> Result<(u32, u32), Error> {
        let len = self.tokens.text.len().saturating_sub(at);
        match (u32::try_from(at), u32::try_from(len)) {
            (Ok(at), Ok(len)) => Ok((at, len)),
            _ => self.fail(ErrorKind::TooMuchText),
        }
    }

    /// A name: an optional `!`, a letter or `_`, then letters, digits and `_`; read as upper case.
    fn keyword(&mut self) -> Result<(u32, u32), Error> {
        self.scratch.clear();
        if let Some(c @ b'!') = self.lx.peek()? {
            self.lx.bump(c);
            self.scratch.push(c);
        }
        let start = self.scratch.len();
        self.run(is_name_byte)?;
        match self.scratch.get(start) {
            Some(c) if !c.is_ascii_digit() => {}
            _ => return self.fail(ErrorKind::ExpectedKeyword),
        }
        self.scratch.make_ascii_uppercase();
        self.span()
    }

    /// The digits of an instance name whose `#` was read.
    fn id(&mut self) -> Result<u64, Error> {
        self.scratch.clear();
        self.run(|c| c.is_ascii_digit())?;
        let id = core::str::from_utf8(&self.scratch)
            .ok()
            .and_then(|t| t.parse::<u64>().ok());
        match id {
            Some(id) => Ok(id),
            None => self.fail(ErrorKind::BadInstanceName),
        }
    }

    fn push(&mut self, item: Item) -> Result<(), Error> {
        if self.tokens.items.len() >= self.options.items {
            return self.fail(ErrorKind::TooManyItems);
        }
        if !matches!(item, Item::Record(..) | Item::Close) {
            self.tokens.params = self.tokens.params.saturating_add(1);
        }
        self.tokens.items.push(item);
        Ok(())
    }

    /// The parameters of a list whose `(` was read, to its `)`; nested lists are walked by a
    /// counter.
    fn list(&mut self) -> Result<(), Error> {
        // The lists open inside this one.
        let mut depth = 0_usize;
        let mut at = At::Fresh;
        loop {
            self.ws()?;
            let Some(c) = self.lx.peek()? else {
                return self.fail(ErrorKind::UnexpectedEnd);
            };
            match (at, c) {
                (At::Fresh | At::After, b')') => {
                    self.lx.bump(c);
                    self.push(Item::Close)?;
                    let Some(outer) = depth.checked_sub(1) else {
                        return Ok(());
                    };
                    depth = outer;
                    at = At::After;
                }
                (At::After, b',') => {
                    self.lx.bump(c);
                    at = At::Value;
                }
                (At::After, _) => return self.fail(ErrorKind::Expected(b')')),
                (At::Fresh | At::Value, _) => {
                    if self.value(c)? {
                        if depth >= MAX_DEPTH {
                            return self.fail(ErrorKind::TooDeep);
                        }
                        depth = depth.saturating_add(1);
                        at = At::Fresh;
                    } else {
                        at = At::After;
                    }
                }
            }
        }
    }

    /// One parameter starting with the byte `c`, not yet taken; true when it opened a list.
    fn value(&mut self, c: u8) -> Result<bool, Error> {
        match c {
            b'$' => {
                self.lx.bump(c);
                self.push(Item::Null)?;
            }
            b'*' => {
                self.lx.bump(c);
                self.push(Item::Derived)?;
            }
            b'\'' => {
                self.lx.bump(c);
                self.string()?;
            }
            b'"' => {
                self.lx.bump(c);
                self.binary()?;
            }
            b'#' => {
                self.lx.bump(c);
                let id = self.id()?;
                self.push(Item::Ref(id))?;
            }
            b'.' => {
                self.lx.bump(c);
                self.scratch.clear();
                if self.lx.peek()?.is_some_and(|c| c.is_ascii_digit()) {
                    self.scratch.push(b'.');
                    self.number()?;
                } else {
                    self.enumeration()?;
                }
            }
            b'(' => {
                self.lx.bump(c);
                self.push(Item::Open)?;
                return Ok(true);
            }
            b'0'..=b'9' | b'+' | b'-' => {
                self.scratch.clear();
                self.number()?;
            }
            b'!' | b'_' | b'A'..=b'Z' | b'a'..=b'z' => {
                let (at, len) = self.keyword()?;
                self.expect(b'(')?;
                self.push(Item::Typed(at, len))?;
                return Ok(true);
            }
            _ => return self.fail(ErrorKind::UnexpectedByte(c)),
        }
        Ok(false)
    }

    /// A number, the scratch holding what of it was read already.
    fn number(&mut self) -> Result<(), Error> {
        self.run(|c| c.is_ascii_digit() || matches!(c, b'+' | b'-' | b'.' | b'E' | b'e'))?;
        match number(&self.scratch) {
            Ok(item) => self.push(item),
            Err(kind) => self.fail(kind),
        }
    }

    /// An enumeration whose opening `.` was read.
    fn enumeration(&mut self) -> Result<(), Error> {
        self.run(is_name_byte)?;
        let named = self.scratch.first().is_some_and(|c| !c.is_ascii_digit());
        match self.lx.peek()? {
            Some(c @ b'.') if named => self.lx.bump(c),
            _ => return self.fail(ErrorKind::BadEnumeration),
        }
        self.scratch.make_ascii_uppercase();
        let (at, len) = self.span()?;
        self.push(Item::Enum(at, len))
    }

    /// A binary whose opening `"` was read.
    fn binary(&mut self) -> Result<(), Error> {
        self.scratch.clear();
        self.run(|c| c.is_ascii_digit() || (b'A'..=b'F').contains(&c))?;
        let led = self
            .scratch
            .first()
            .is_some_and(|c| (b'0'..=b'3').contains(c));
        match self.lx.peek()? {
            Some(c @ b'"') if led => self.lx.bump(c),
            None => return self.fail(ErrorKind::UnexpectedEnd),
            _ => return self.fail(ErrorKind::BadBinary),
        }
        let (at, len) = self.span()?;
        self.push(Item::Binary(at, len))
    }

    /// A string whose opening `'` was read.
    fn string(&mut self) -> Result<(), Error> {
        self.scratch.clear();
        loop {
            self.run(|c| !matches!(c, b'\'' | b'\n' | b'\r'))?;
            let Some(c) = self.lx.peek()? else {
                return self.fail(ErrorKind::UnterminatedString);
            };
            self.lx.bump(c);
            if c == b'\'' {
                match self.lx.peek()? {
                    Some(c @ b'\'') => {
                        self.lx.bump(c);
                        self.scratch.push(c);
                    }
                    _ => break,
                }
            }
        }
        let at = self.tokens.text.len();
        match text::decode(
            &self.scratch,
            &mut self.tokens.text,
            self.options.lossy_text,
        ) {
            Ok(replaced) => self.replaced = self.replaced.saturating_add(replaced),
            Err(text::BadText) => return self.fail(ErrorKind::BadText),
        }
        self.scratch.clear();
        self.check_text()?;
        let (at, len) = self.text_span(at)?;
        self.push(Item::Str(at, len))
    }
}
