//! The XML tokenizer under [`crate::xml`]'s reader: a pull scanner over UTF-8 text that yields the
//! tokens of an XML 1.0 document with their byte offsets, and refuses a document type declaration
//! or an entity reference outright.
//!
//! What it does: [`Tokens`] walks a `&str` once, front to back, with a state machine (declaration,
//! prolog, elements, attributes, after the root), and yields [`Token`]s: the XML declaration,
//! processing instructions, comments, element starts, attributes, element ends, text and CDATA
//! sections, each a slice of the text. It does not check that end tags match, that attributes are
//! unique or that references resolve: the reader does, with its budgets.
//!
//! **What is left out of the source, on purpose.** xmlparser reads a document type declaration
//! and its entity declarations into tokens for its caller to expand. Here `<!DOCTYPE` anywhere is
//! [`ScanErrorKind::DocumentType`], and an entity reference other than the five XML predefines
//! (`&lt;` `&gt;` `&amp;` `&apos;` `&quot;`) and character references is [`ScanErrorKind::Entity`]:
//! with no document type there is nothing for entity-expansion (billion laughs) or
//! external-entity attacks to act on (chapter 24.7). Positions are byte offsets, not the source's
//! line and column, which it found by rescanning the text.
//!
//! Guarantees:
//!
//! - **No panics, no indexing, no unchecked arithmetic.** Every slice is taken with `get`, every
//!   position step is a checked addition, and a step past the end is an error.
//! - **Each byte is read once, in order, with no recursion.** The scanner holds one position and
//!   one state; nothing nests.
//! - **No document type, no entities.** As above: refused with the offset of the `<!DOCTYPE` or
//!   the `&`.
//! - **Well-formed tokens or an error.** Names follow XML 1.0's `Name` and Namespaces' `QName`,
//!   comments refuse `--`, text refuses `]]>`, character references must name an XML `Char`, and
//!   only `version="1.x"`, `encoding` and `standalone` may appear in the declaration.
//!
//! Method: the tokenizer of xmlparser 0.13.6 (below), which follows the productions of Extensible
//! Markup Language (XML) 1.0, fifth edition, W3C Recommendation, 26 November 2008
//! (<https://www.w3.org/TR/xml/>) and Namespaces in XML 1.0, third edition
//! (<https://www.w3.org/TR/xml-names/>).
//!
//! Source: PORT from xmlparser, MIT,
//!   <https://github.com/RazrFalcon/xmlparser/blob/9c8e34305723118c67d7feba95772705b2247776/src/lib.rs>
//!   <https://github.com/RazrFalcon/xmlparser/blob/9c8e34305723118c67d7feba95772705b2247776/src/stream.rs>
//!   <https://github.com/RazrFalcon/xmlparser/blob/9c8e34305723118c67d7feba95772705b2247776/src/xmlchar.rs>
//!   Copyright (c) 2018 Reizner Evgeniy
//!   Changed for Spare Kit: taken under the MIT choice of its MIT OR Apache-2.0; only the
//!   tokenizer's state machine, its stream and its character classes are kept, rewritten to the
//!   kernel's lints (`get` for every slice, checked position steps, no `unwrap`); the document
//!   type and entity declarations are refused instead of read; positions are byte offsets; the
//!   ASCII fast path of the name classes tests `< 128`, where the source's `<= 128` sent U+0080
//!   down the byte path; a character reference is refused when its digits overflow instead of
//!   being parsed by the standard library and replaced; a processing instruction whose target is
//!   `xml` in any case is refused, as XML 1.0's `PITarget` asks, where the source read it.
//! SPDX-License-Identifier: Apache-2.0 AND MIT
//!
//! Cost: linear in the text: each byte is visited a bounded number of times (names and
//! references are rescanned at most once); no allocation.

use alloc::string::String;

/// Why the text is not a well-formed XML token stream, and where.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScanError {
    /// What went wrong.
    pub kind: ScanErrorKind,
    /// The byte offset in the text where it was found.
    pub offset: usize,
}

/// What [`ScanError`] found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanErrorKind {
    /// The text ended inside a token.
    UnexpectedEnd,
    /// A document type declaration, `<!DOCTYPE`, which is never read.
    DocumentType,
    /// An entity reference other than the five predefined ones.
    Entity,
    /// A malformed reference: `&` not followed by a name or a character number and `;`, or a
    /// number that names no XML character.
    Reference,
    /// A name that is empty or starts with a character a name cannot start with.
    Name,
    /// A character XML 1.0 does not allow in a document.
    NotXmlChar,
    /// A byte other than the one the grammar expects here.
    Unexpected,
    /// A comment holding `--` or ending in `-`.
    Comment,
    /// Text holding `]]>`.
    CharacterData,
    /// A malformed XML declaration.
    Declaration,
    /// Markup that is not allowed here (`<!` other than a comment or CDATA, a second XML
    /// declaration, text outside the root element).
    Misplaced,
}

/// How an element's start tag ended, or an end tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementEnd<'a> {
    /// `>`: the element has content.
    Open,
    /// `/>`: the element is empty.
    Empty,
    /// `</prefix:local>`.
    Close {
        /// The namespace prefix, or empty.
        prefix: &'a str,
        /// The local name.
        local: &'a str,
    },
}

/// One token of the document, with the byte offset where it starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token<'a> {
    /// The XML declaration.
    Declaration {
        /// The version, `1.` and digits.
        version: &'a str,
        /// The declared encoding, if any.
        encoding: Option<&'a str>,
        /// The declared standalone flag, if any.
        standalone: Option<bool>,
    },
    /// A processing instruction.
    ProcessingInstruction {
        /// Its target.
        target: &'a str,
        /// Its content, if any.
        content: Option<&'a str>,
    },
    /// A comment's text.
    Comment(&'a str),
    /// `<prefix:local`.
    ElementStart {
        /// The namespace prefix, or empty.
        prefix: &'a str,
        /// The local name.
        local: &'a str,
    },
    /// `prefix:local="value"`, the value as written (references unresolved).
    Attribute {
        /// The namespace prefix, or empty.
        prefix: &'a str,
        /// The local name.
        local: &'a str,
        /// The value between the quotes.
        value: &'a str,
        /// The byte offset the value starts at.
        value_at: usize,
    },
    /// The end of a start tag, or an end tag.
    ElementEnd(ElementEnd<'a>),
    /// Character data as written (references unresolved).
    Text(&'a str),
    /// A CDATA section's content, and the byte offset it starts at.
    Cdata(&'a str, usize),
}

/// A reference in text or an attribute value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reference {
    /// A character, from a character reference or one of the five predefined entities.
    Char(char),
}

/// `[0-9]`.
const fn is_digit(b: u8) -> bool {
    b.is_ascii_digit()
}

/// `[0-9A-Fa-f]`.
const fn is_hex_digit(b: u8) -> bool {
    b.is_ascii_hexdigit()
}

/// XML white space: `[ \t\n\r]`.
const fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r')
}

/// The ASCII bytes of `NameChar`.
const fn is_name_byte(b: u8) -> bool {
    matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b':' | b'_' | b'-' | b'.')
}

/// XML 1.0's `NameStartChar`.
fn is_name_start(ch: char) -> bool {
    let c = u32::from(ch);
    if c < 128 {
        return matches!(ch, 'A'..='Z' | 'a'..='z' | ':' | '_');
    }
    matches!(
        c,
        0xC0..=0xD6
            | 0xD8..=0xF6
            | 0xF8..=0x2FF
            | 0x370..=0x37D
            | 0x37F..=0x1FFF
            | 0x200C..=0x200D
            | 0x2070..=0x218F
            | 0x2C00..=0x2FEF
            | 0x3001..=0xD7FF
            | 0xF900..=0xFDCF
            | 0xFDF0..=0xFFFD
            | 0x10000..=0xEFFFF
    )
}

/// XML 1.0's `NameChar`.
fn is_name_char(ch: char) -> bool {
    let c = u32::from(ch);
    if c < 128 {
        return u8::try_from(c).is_ok_and(is_name_byte);
    }
    matches!(
        c,
        0xB7 | 0xC0..=0xD6
            | 0xD8..=0xF6
            | 0xF8..=0x2FF
            | 0x300..=0x36F
            | 0x370..=0x37D
            | 0x37F..=0x1FFF
            | 0x200C..=0x200D
            | 0x203F..=0x2040
            | 0x2070..=0x218F
            | 0x2C00..=0x2FEF
            | 0x3001..=0xD7FF
            | 0xF900..=0xFDCF
            | 0xFDF0..=0xFFFD
            | 0x10000..=0xEFFFF
    )
}

/// XML 1.0's `Char`: tab, line feed, carriage return and everything from U+0020 but the
/// surrogates (which a `char` never is) and U+FFFE and U+FFFF.
fn is_xml_char(ch: char) -> bool {
    let c = u32::from(ch);
    if c < 0x20 {
        return u8::try_from(c).is_ok_and(is_space);
    }
    !matches!(c, 0xFFFE | 0xFFFF)
}

type Scan<T> = Result<T, ScanError>;

/// A position in the text.
#[derive(Clone, Copy, Debug)]
struct Cursor<'a> {
    text: &'a str,
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn fail<T>(&self, kind: ScanErrorKind) -> Scan<T> {
        Err(ScanError {
            kind,
            offset: self.pos,
        })
    }

    fn at_end(&self) -> bool {
        self.pos >= self.text.len()
    }

    fn rest(&self) -> &'a [u8] {
        self.text.as_bytes().get(self.pos..).unwrap_or(&[])
    }

    fn byte(&self) -> Option<u8> {
        self.rest().first().copied()
    }

    fn next_byte(&self) -> Option<u8> {
        self.rest().get(1).copied()
    }

    fn starts_with(&self, s: &[u8]) -> bool {
        self.rest().starts_with(s)
    }

    fn starts_with_space(&self) -> bool {
        self.byte().is_some_and(is_space)
    }

    /// Moves `n` bytes on, never past the end.
    fn advance(&mut self, n: usize) -> Scan<()> {
        match self.pos.checked_add(n) {
            Some(p) if p <= self.text.len() => {
                self.pos = p;
                Ok(())
            }
            _ => self.fail(ScanErrorKind::UnexpectedEnd),
        }
    }

    fn slice(&self, from: usize) -> Scan<&'a str> {
        match self.text.get(from..self.pos) {
            Some(s) => Ok(s),
            None => self.fail(ScanErrorKind::Unexpected),
        }
    }

    fn consume_byte(&mut self, c: u8) -> Scan<()> {
        match self.byte() {
            Some(b) if b == c => self.advance(1),
            Some(_) => self.fail(ScanErrorKind::Unexpected),
            None => self.fail(ScanErrorKind::UnexpectedEnd),
        }
    }

    fn try_consume_byte(&mut self, c: u8) -> bool {
        self.byte() == Some(c) && self.advance(1).is_ok()
    }

    fn skip_string(&mut self, s: &[u8]) -> Scan<()> {
        if self.starts_with(s) {
            self.advance(s.len())
        } else if self.at_end() {
            self.fail(ScanErrorKind::UnexpectedEnd)
        } else {
            self.fail(ScanErrorKind::Unexpected)
        }
    }

    fn skip_spaces(&mut self) {
        while self.starts_with_space() {
            // A space is one byte, and there is one.
            self.pos = self.pos.saturating_add(1);
        }
    }

    /// Skips bytes while `keep` holds; the bytes must be ASCII for the result to stay on a
    /// character boundary, which every caller's predicate ensures.
    fn skip_bytes(&mut self, keep: impl Fn(u8) -> bool) {
        while self.byte().is_some_and(&keep) {
            self.pos = self.pos.saturating_add(1);
        }
    }

    /// Skips characters while `keep` holds (given the cursor before each), refusing one XML does
    /// not allow.
    fn skip_chars(&mut self, keep: impl Fn(&Cursor<'a>, char) -> bool) -> Scan<()> {
        loop {
            let Some(c) = self.text.get(self.pos..).and_then(|t| t.chars().next()) else {
                return Ok(());
            };
            if !is_xml_char(c) {
                return self.fail(ScanErrorKind::NotXmlChar);
            }
            if !keep(self, c) {
                return Ok(());
            }
            self.advance(c.len_utf8())?;
        }
    }

    fn consume_chars(&mut self, keep: impl Fn(&Cursor<'a>, char) -> bool) -> Scan<&'a str> {
        let start = self.pos;
        self.skip_chars(keep)?;
        self.slice(start)
    }

    fn first_char(&self) -> Option<char> {
        self.text.get(self.pos..).and_then(|t| t.chars().next())
    }

    /// XML 1.0's `Name`.
    fn consume_name(&mut self) -> Scan<&'a str> {
        let start = self.pos;
        match self.first_char() {
            Some(c) if is_name_start(c) => self.advance(c.len_utf8())?,
            Some(_) => return self.fail(ScanErrorKind::Name),
            None => return self.fail(ScanErrorKind::UnexpectedEnd),
        }
        while let Some(c) = self.first_char().filter(|&c| is_name_char(c)) {
            self.advance(c.len_utf8())?;
        }
        self.slice(start)
    }

    /// Namespaces in XML's `QName`: (prefix, local), the prefix empty when there is none.
    fn consume_qname(&mut self) -> Scan<(&'a str, &'a str)> {
        let start = self.pos;
        let mut colon = None;
        while let Some(c) = self.first_char() {
            if c == ':' {
                if colon.is_some() {
                    return self.fail(ScanErrorKind::Name);
                }
                colon = Some(self.pos);
                self.advance(1)?;
            } else if is_name_char(c) {
                self.advance(c.len_utf8())?;
            } else {
                break;
            }
        }
        let name = self.slice(start)?;
        let (prefix, local) = match colon {
            Some(at) => {
                let split = at.checked_sub(start).ok_or(ScanError {
                    kind: ScanErrorKind::Name,
                    offset: at,
                })?;
                // The colon is one byte, so both sides are on character boundaries.
                let p = name.get(..split).unwrap_or("");
                let l = name.get(split.saturating_add(1)..).unwrap_or("");
                (p, l)
            }
            None => ("", name),
        };
        let starts = |s: &str| s.chars().next().is_some_and(is_name_start);
        if (!prefix.is_empty() && !starts(prefix)) || !starts(local) {
            return Err(ScanError {
                kind: ScanErrorKind::Name,
                offset: start,
            });
        }
        Ok((prefix, local))
    }

    fn consume_eq(&mut self) -> Scan<()> {
        self.skip_spaces();
        self.consume_byte(b'=')?;
        self.skip_spaces();
        Ok(())
    }

    fn consume_quote(&mut self) -> Scan<u8> {
        match self.byte() {
            Some(q @ (b'\'' | b'"')) => {
                self.advance(1)?;
                Ok(q)
            }
            Some(_) => self.fail(ScanErrorKind::Unexpected),
            None => self.fail(ScanErrorKind::UnexpectedEnd),
        }
    }

    /// A reference at the cursor, `&` included: a character reference or one of the five
    /// predefined entities. Any other entity is [`ScanErrorKind::Entity`].
    fn consume_reference(&mut self) -> Scan<Reference> {
        let start = self.pos;
        let refused = |kind| ScanError {
            kind,
            offset: start,
        };
        if !self.try_consume_byte(b'&') {
            return Err(refused(ScanErrorKind::Reference));
        }
        let c = if self.try_consume_byte(b'#') {
            let hex = self.try_consume_byte(b'x');
            let digits_start = self.pos;
            if hex {
                self.skip_bytes(is_hex_digit);
            } else {
                self.skip_bytes(is_digit);
            }
            let digits = self.slice(digits_start)?;
            let radix = if hex { 16 } else { 10 };
            let mut n: u32 = 0;
            for b in digits.bytes() {
                let d = char::from(b)
                    .to_digit(radix)
                    .ok_or(refused(ScanErrorKind::Reference))?;
                n = n
                    .checked_mul(radix)
                    .and_then(|v| v.checked_add(d))
                    .ok_or(refused(ScanErrorKind::Reference))?;
            }
            let c = char::from_u32(n)
                .filter(|&c| !digits.is_empty() && is_xml_char(c))
                .ok_or(refused(ScanErrorKind::Reference))?;
            Reference::Char(c)
        } else {
            let name = self
                .consume_name()
                .map_err(|_| refused(ScanErrorKind::Reference))?;
            match name {
                "quot" => Reference::Char('"'),
                "amp" => Reference::Char('&'),
                "apos" => Reference::Char('\''),
                "lt" => Reference::Char('<'),
                "gt" => Reference::Char('>'),
                _ => return Err(refused(ScanErrorKind::Entity)),
            }
        };
        self.consume_byte(b';')
            .map_err(|_| refused(ScanErrorKind::Reference))?;
        Ok(c)
    }
}

/// Checks every reference in `raw` (text or an attribute value starting at byte `offset`): each
/// must be a character reference or a predefined entity.
///
/// # Errors
///
/// [`ScanErrorKind::Entity`] or [`ScanErrorKind::Reference`] at the reference's offset.
pub fn check_references(raw: &str, offset: usize) -> Result<(), ScanError> {
    let mut at = 0_usize;
    while let Some(found) = next_ampersand(raw, at) {
        let mut c = Cursor {
            text: raw,
            pos: at.saturating_add(found),
        };
        c.consume_reference().map_err(|e| ScanError {
            kind: e.kind,
            offset: offset.saturating_add(e.offset),
        })?;
        at = c.pos;
    }
    Ok(())
}

/// The distance from `at` to the next `&` in `raw`, if any (`&` is one byte, never inside a
/// longer character).
fn next_ampersand(raw: &str, at: usize) -> Option<usize> {
    raw.as_bytes().get(at..)?.iter().position(|&b| b == b'&')
}

/// Appends `raw` to `out` with every reference replaced by its character; `raw` should have
/// passed [`check_references`]. The caller reserves the room: the result is never longer than
/// `raw`.
///
/// # Errors
///
/// What [`check_references`] refuses, if it was not called first.
pub fn decode_references(raw: &str, offset: usize, out: &mut String) -> Result<(), ScanError> {
    let mut at = 0_usize;
    while let Some(found) = next_ampersand(raw, at) {
        let amp = at.saturating_add(found);
        out.push_str(raw.get(at..amp).unwrap_or(""));
        let mut c = Cursor {
            text: raw,
            pos: amp,
        };
        let Reference::Char(ch) = c.consume_reference().map_err(|e| ScanError {
            kind: e.kind,
            offset: offset.saturating_add(e.offset),
        })?;
        out.push(ch);
        at = c.pos;
    }
    out.push_str(raw.get(at..).unwrap_or(""));
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Declaration,
    Prolog,
    Elements,
    Attributes,
    AfterRoot,
    End,
}

/// The tokens of a document, one at a time.
#[derive(Clone, Debug)]
pub struct Tokens<'a> {
    cursor: Cursor<'a>,
    state: State,
    depth: usize,
}

impl<'a> Tokens<'a> {
    /// The tokens of `text`.
    #[must_use]
    pub const fn new(text: &'a str) -> Tokens<'a> {
        Tokens {
            cursor: Cursor { text, pos: 0 },
            state: State::Declaration,
            depth: 0,
        }
    }

    /// The byte offset the next token starts at (or the end).
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.cursor.pos
    }

    /// The next token and its byte offset, `None` at the end of the document.
    ///
    /// # Errors
    ///
    /// [`ScanError`] for the first token that is not well formed; after one, the scanner is at
    /// its end.
    pub fn next_token(&mut self) -> Result<Option<(usize, Token<'a>)>, ScanError> {
        loop {
            if self.cursor.at_end() || self.state == State::End {
                return Ok(None);
            }
            let start = self.cursor.pos;
            match self.step() {
                Ok(Some(t)) => return Ok(Some((start, t))),
                Ok(None) => {}
                Err(e) => {
                    self.state = State::End;
                    return Err(e);
                }
            }
        }
    }

    /// One step of the state machine: a token, or `None` when it only moved on (spaces) or
    /// changed state.
    fn step(&mut self) -> Scan<Option<Token<'a>>> {
        let s = &mut self.cursor;
        match self.state {
            State::Declaration => {
                self.state = State::Prolog;
                if s.starts_with(b"<?xml ") {
                    return declaration(s).map(Some);
                }
                Ok(None)
            }
            State::Prolog | State::AfterRoot => {
                if s.starts_with(b"<!DOCTYPE") {
                    s.fail(ScanErrorKind::DocumentType)
                } else if s.starts_with(b"<!--") {
                    comment(s).map(Some)
                } else if s.starts_with(b"<?xml ") {
                    s.fail(ScanErrorKind::Misplaced)
                } else if s.starts_with(b"<?") {
                    instruction(s).map(Some)
                } else if s.starts_with_space() {
                    s.skip_spaces();
                    Ok(None)
                } else if self.state == State::Prolog
                    && s.starts_with(b"<")
                    && !s.starts_with(b"<!")
                {
                    self.state = State::Attributes;
                    element_start(s).map(Some)
                } else {
                    s.fail(ScanErrorKind::Misplaced)
                }
            }
            State::Elements => match (s.byte(), s.next_byte()) {
                (Some(b'<'), Some(b'!')) => {
                    if s.starts_with(b"<!--") {
                        comment(s).map(Some)
                    } else if s.starts_with(b"<![CDATA[") {
                        cdata(s).map(Some)
                    } else if s.starts_with(b"<!DOCTYPE") {
                        s.fail(ScanErrorKind::DocumentType)
                    } else {
                        s.fail(ScanErrorKind::Misplaced)
                    }
                }
                (Some(b'<'), Some(b'?')) => {
                    if s.starts_with(b"<?xml ") {
                        s.fail(ScanErrorKind::Misplaced)
                    } else {
                        instruction(s).map(Some)
                    }
                }
                (Some(b'<'), Some(b'/')) => {
                    self.depth = self.depth.saturating_sub(1);
                    self.state = if self.depth == 0 {
                        State::AfterRoot
                    } else {
                        State::Elements
                    };
                    close_element(s).map(Some)
                }
                (Some(b'<'), Some(_)) => {
                    self.state = State::Attributes;
                    element_start(s).map(Some)
                }
                (Some(b'<'), None) | (None, _) => s.fail(ScanErrorKind::UnexpectedEnd),
                (Some(_), _) => text(s).map(Some),
            },
            State::Attributes => {
                let t = attribute(s)?;
                if let Token::ElementEnd(end) = t {
                    if end == ElementEnd::Open {
                        self.depth = self.depth.saturating_add(1);
                    }
                    self.state = if self.depth == 0 {
                        State::AfterRoot
                    } else {
                        State::Elements
                    };
                }
                Ok(Some(t))
            }
            State::End => Ok(None),
        }
    }
}

/// `XMLDecl ::= '<?xml' VersionInfo EncodingDecl? SDDecl? S? '?>'`.
fn declaration<'a>(s: &mut Cursor<'a>) -> Scan<Token<'a>> {
    let start = s.pos;
    let bad = |e: ScanError| ScanError {
        kind: if e.kind == ScanErrorKind::UnexpectedEnd {
            e.kind
        } else {
            ScanErrorKind::Declaration
        },
        offset: start,
    };
    let between = |s: &mut Cursor<'a>| -> Scan<()> {
        if s.starts_with_space() {
            s.skip_spaces();
            Ok(())
        } else if s.starts_with(b"?>") || s.at_end() {
            Ok(())
        } else {
            s.fail(ScanErrorKind::Declaration)
        }
    };
    s.advance(6).map_err(bad)?;
    // VersionInfo ::= S 'version' Eq ("'" VersionNum "'" | '"' VersionNum '"'), VersionNum ::=
    // '1.' [0-9]+
    s.skip_spaces();
    s.skip_string(b"version").map_err(bad)?;
    s.consume_eq().map_err(bad)?;
    let quote = s.consume_quote().map_err(bad)?;
    let v = s.pos;
    s.skip_string(b"1.").map_err(bad)?;
    s.skip_bytes(is_digit);
    let version = s.slice(v).map_err(bad)?;
    s.consume_byte(quote).map_err(bad)?;
    between(s).map_err(bad)?;
    // EncodingDecl ::= S 'encoding' Eq ('"' EncName '"' | "'" EncName "'" )
    let encoding = if s.starts_with(b"encoding") {
        s.advance(8).map_err(bad)?;
        s.consume_eq().map_err(bad)?;
        let quote = s.consume_quote().map_err(bad)?;
        let e = s.pos;
        s.skip_bytes(|c| matches!(c, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-' | b'_'));
        let name = s.slice(e).map_err(bad)?;
        if !name.bytes().next().is_some_and(|c| c.is_ascii_alphabetic()) {
            return Err(bad(ScanError {
                kind: ScanErrorKind::Declaration,
                offset: e,
            }));
        }
        s.consume_byte(quote).map_err(bad)?;
        between(s).map_err(bad)?;
        Some(name)
    } else {
        None
    };
    // SDDecl ::= S 'standalone' Eq (("'" ('yes' | 'no') "'") | ('"' ('yes' | 'no') '"'))
    let standalone = if s.starts_with(b"standalone") {
        s.advance(10).map_err(bad)?;
        s.consume_eq().map_err(bad)?;
        let quote = s.consume_quote().map_err(bad)?;
        let flag = if s.starts_with(b"yes") {
            s.advance(3).map_err(bad)?;
            true
        } else if s.starts_with(b"no") {
            s.advance(2).map_err(bad)?;
            false
        } else {
            return s.fail(ScanErrorKind::Declaration);
        };
        s.consume_byte(quote).map_err(bad)?;
        Some(flag)
    } else {
        None
    };
    s.skip_spaces();
    s.skip_string(b"?>").map_err(bad)?;
    Ok(Token::Declaration {
        version,
        encoding,
        standalone,
    })
}

/// `Comment ::= '<!--' ((Char - '-') | ('-' (Char - '-')))* '-->'`.
fn comment<'a>(s: &mut Cursor<'a>) -> Scan<Token<'a>> {
    let start = s.pos;
    s.advance(4)?;
    let text = s.consume_chars(|s, c| !(c == '-' && s.starts_with(b"-->")))?;
    s.skip_string(b"-->")?;
    if text.contains("--") || text.ends_with('-') {
        return Err(ScanError {
            kind: ScanErrorKind::Comment,
            offset: start,
        });
    }
    Ok(Token::Comment(text))
}

/// `PI ::= '<?' PITarget (S (Char* - (Char* '?>' Char*)))? '?>'`.
fn instruction<'a>(s: &mut Cursor<'a>) -> Scan<Token<'a>> {
    let start = s.pos;
    s.advance(2)?;
    let target = s.consume_name()?;
    // `PITarget ::= Name - (('X' | 'x') ('M' | 'm') ('L' | 'l'))`: an XML declaration that is not
    // the document's first bytes, or is malformed, is never read as an instruction.
    if target.eq_ignore_ascii_case("xml") {
        return Err(ScanError {
            kind: ScanErrorKind::Misplaced,
            offset: start,
        });
    }
    s.skip_spaces();
    let content = s.consume_chars(|s, c| !(c == '?' && s.starts_with(b"?>")))?;
    s.skip_string(b"?>")?;
    Ok(Token::ProcessingInstruction {
        target,
        content: (!content.is_empty()).then_some(content),
    })
}

/// `CDSect ::= '<![CDATA[' (Char* - (Char* ']]>' Char*)) ']]>'`.
fn cdata<'a>(s: &mut Cursor<'a>) -> Scan<Token<'a>> {
    s.advance(9)?;
    let at = s.pos;
    let text = s.consume_chars(|s, c| !(c == ']' && s.starts_with(b"]]>")))?;
    s.skip_string(b"]]>")?;
    Ok(Token::Cdata(text, at))
}

/// `'<' QName`.
fn element_start<'a>(s: &mut Cursor<'a>) -> Scan<Token<'a>> {
    s.advance(1)?;
    let (prefix, local) = s.consume_qname()?;
    Ok(Token::ElementStart { prefix, local })
}

/// `'</' QName S? '>'`.
fn close_element<'a>(s: &mut Cursor<'a>) -> Scan<Token<'a>> {
    s.advance(2)?;
    let (prefix, local) = s.consume_qname()?;
    s.skip_spaces();
    s.consume_byte(b'>')?;
    Ok(Token::ElementEnd(ElementEnd::Close { prefix, local }))
}

/// `S QName Eq AttValue`, or the end of the start tag.
fn attribute<'a>(s: &mut Cursor<'a>) -> Scan<Token<'a>> {
    let spaced = s.starts_with_space();
    s.skip_spaces();
    match s.byte() {
        Some(b'/') => {
            s.advance(1)?;
            s.consume_byte(b'>')?;
            return Ok(Token::ElementEnd(ElementEnd::Empty));
        }
        Some(b'>') => {
            s.advance(1)?;
            return Ok(Token::ElementEnd(ElementEnd::Open));
        }
        Some(_) if !spaced => return s.fail(ScanErrorKind::Unexpected),
        Some(_) => {}
        None => return s.fail(ScanErrorKind::UnexpectedEnd),
    }
    let (prefix, local) = s.consume_qname()?;
    s.consume_eq()?;
    let quote = s.consume_quote()?;
    let q = char::from(quote);
    // An attribute value may not hold `<`.
    let value_at = s.pos;
    let value = s.consume_chars(|_, c| c != q && c != '<')?;
    s.consume_byte(quote)?;
    Ok(Token::Attribute {
        prefix,
        local,
        value,
        value_at,
    })
}

/// Character data up to the next `<`, which may not hold `]]>`.
fn text<'a>(s: &mut Cursor<'a>) -> Scan<Token<'a>> {
    let start = s.pos;
    let text = s.consume_chars(|_, c| c != '<')?;
    if text.contains("]]>") {
        return Err(ScanError {
            kind: ScanErrorKind::CharacterData,
            offset: start,
        });
    }
    Ok(Token::Text(text))
}
