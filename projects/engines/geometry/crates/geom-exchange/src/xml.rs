//! The hardened XML reader every XML format goes through (3MF, IPC-2581, `LandXML`, later `ifcXML`):
//! a document read through a [`ByteSource`] within a size budget, then pulled event by event
//! within budgets on depth, attributes, names, values and text, with no document type
//! declaration and no entities.
//!
//! What it does: [`XmlText::read`] reads a whole document from a [`ByteSource`], refusing one past
//! [`XmlBudget::bytes`] before it allocates beyond it, one that is not UTF-8 (a UTF-16 byte-order
//! mark is named), and one whose declaration names another encoding. [`XmlText::events`] then
//! gives an [`Events`] reader whose [`Events::next`] yields one [`Event`] at a time, charging the
//! work meter for the bytes each consumed. The reader checks what the tokenizer ([`scan`]) leaves
//! to it: that every end tag closes the element open at that depth, that no element repeats an
//! attribute name, that every reference in text or a value is a character reference or one of
//! the five predefined entities, that there is exactly one root element, and every budget.
//! [`XmlValue::decode_into`] writes text or a value with its references replaced.
//!
//! **Why there is nothing for an XML attack to act on.** Entity expansion ("billion laughs") and
//! external entities (XXE) both need a document type declaration that defines entities; any
//! `<!DOCTYPE` is refused before it is read, and a reference to any entity but the five predefined
//! ones is refused where it stands. Deep nesting is held to [`XmlBudget::depth`] by an explicit
//! stack, never by recursion; a start tag with endless attributes to [`XmlBudget::attributes`];
//! huge names, values and text runs to their budgets; and the whole document to
//! [`XmlBudget::bytes`] (chapter 24.7).
//!
//! Guarantees:
//!
//! - **Refused with a typed error and a byte offset, never a panic.** Every failure is an
//!   [`XmlError`] naming its [`XmlErrorKind`] and the byte offset where it was found.
//! - **No document type, no entities, no network, no files.** The reader reads only the bytes it
//!   is given.
//! - **Every budget holds.** The document is at most `bytes` long, elements nest at most `depth`
//!   deep, a start tag has at most `attributes` attributes, and a name (an instruction's target
//!   included), a value, or a text run, comment or instruction's content is at most `name`,
//!   `value` or `text` bytes; the first breach is refused with its own kind.
//! - **Well formed or refused.** One root element; every end tag matches; attribute names unique
//!   within an element (prefix and local name together); text only inside the root.
//! - **The same bytes give the same events**, on every target: the reader is a pure function of
//!   the bytes and the budget.
//!
//! Method: chapter 24.7 (`projects/engines/geometry/architecture/24-file-format-exchange.md`); the
//! well-formedness constraints of XML 1.0, fifth edition (<https://www.w3.org/TR/xml/>, "Element
//! Type Match", "Unique Att Spec", "Entity Declared"), with entity declarations refused rather
//! than processed, as OWASP's XML External Entity Prevention Cheat Sheet advises
//! (<https://cheatsheetseries.owasp.org/cheatsheets/XML_External_Entity_Prevention_Cheat_Sheet.html>);
//! the tokenizer is [`scan`], ported from xmlparser.
//!
//! Cost: reading, one pass over the source with one allocation grown in steps within the size
//! budget; each event, linear in the bytes it covers, plus, for an attribute, a comparison with
//! each earlier attribute of its element (at most `attributes` of them); memory, the text and a
//! stack of at most `depth` names. No recursion.

pub mod scan;

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use geom_format::source::{ByteSource, SourceError};
use geom_run::WorkMeter;

use scan::{
    ElementEnd, ScanError, ScanErrorKind, Token, Tokens, check_references, decode_references,
};

/// The budgets a document is read within.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XmlBudget {
    /// The most bytes a document may have.
    pub bytes: usize,
    /// The deepest elements may nest.
    pub depth: usize,
    /// The most attributes one start tag may carry.
    pub attributes: usize,
    /// The longest name (prefix and local name together), in bytes.
    pub name: usize,
    /// The longest attribute value, in bytes as written.
    pub value: usize,
    /// The longest run of text or CDATA, comment, or processing instruction's content, in bytes
    /// as written.
    pub text: usize,
}

impl XmlBudget {
    /// Budgets that every exchange format's real files sit well inside: 256 MiB documents
    /// (a 3MF model part of a few million triangles), 256 levels of nesting (XML formats in use
    /// nest a few dozen deep), 1,024 attributes on one element, 1 KiB names, and 64 MiB values and
    /// text runs (an IPC-2581 or 3MF payload in one attribute or one text node). A caller lowers or
    /// raises them for its format.
    pub const DEFAULT: XmlBudget = XmlBudget {
        bytes: 256 << 20,
        depth: 256,
        attributes: 1_024,
        name: 1_024,
        value: 64 << 20,
        text: 64 << 20,
    };
}

/// Why a document was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XmlErrorKind {
    /// The byte source failed.
    Source(SourceError),
    /// The document is longer than its byte budget.
    TooLarge,
    /// The bytes are not UTF-8.
    NotUtf8,
    /// The document is UTF-16 (a byte-order mark says so) or declares an encoding other than
    /// UTF-8.
    Encoding,
    /// A token is not well formed, or a document type declaration or an entity reference was
    /// refused: what the tokenizer found.
    Scan(ScanErrorKind),
    /// An end tag does not close the element open at its depth.
    Mismatch,
    /// An element repeats an attribute name.
    DuplicateAttribute,
    /// The document has no root element.
    NoRoot,
    /// Elements nest deeper than the depth budget.
    TooDeep,
    /// A start tag carries more attributes than its budget.
    TooManyAttributes,
    /// A name is longer than its budget.
    NameTooLong,
    /// An attribute value is longer than its budget.
    ValueTooLong,
    /// A run of text or CDATA is longer than its budget.
    TextTooLong,
    /// The work meter stopped the read.
    Stopped,
    /// The allocator refused.
    OutOfMemory,
}

/// A refused document: what, and the byte offset where it was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XmlError {
    /// What was refused.
    pub kind: XmlErrorKind,
    /// The byte offset in the document.
    pub offset: usize,
}

impl fmt::Display for XmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "XML refused at byte {}: {:?}", self.offset, self.kind)
    }
}

impl core::error::Error for XmlError {}

impl From<ScanError> for XmlError {
    fn from(e: ScanError) -> XmlError {
        XmlError {
            kind: XmlErrorKind::Scan(e.kind),
            offset: e.offset,
        }
    }
}

const fn refused(kind: XmlErrorKind, offset: usize) -> XmlError {
    XmlError { kind, offset }
}

/// Text or an attribute value as written, with the byte offset it starts at; its references are
/// already checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XmlValue<'a> {
    raw: &'a str,
    offset: usize,
    cdata: bool,
}

impl<'a> XmlValue<'a> {
    /// The value as written, references unresolved (for CDATA, the content itself).
    #[must_use]
    pub const fn raw(&self) -> &'a str {
        self.raw
    }

    /// The byte offset it starts at.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Appends the value to `out` with every reference replaced by its character (CDATA as it
    /// is). The result is never longer than [`XmlValue::raw`].
    ///
    /// # Errors
    ///
    /// [`XmlErrorKind::OutOfMemory`] if `out` cannot grow; a reference error cannot happen, since
    /// the reader checked them all.
    pub fn decode_into(&self, out: &mut String) -> Result<(), XmlError> {
        out.try_reserve(self.raw.len())
            .map_err(|_| refused(XmlErrorKind::OutOfMemory, self.offset))?;
        if self.cdata {
            out.push_str(self.raw);
            return Ok(());
        }
        // The reservation above covers the whole decoded value, which is no longer than raw.
        decode_references(self.raw, self.offset, out)?;
        Ok(())
    }
}

/// A qualified name: its namespace prefix (empty for none) and local name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XmlName<'a> {
    /// The namespace prefix, or empty.
    pub prefix: &'a str,
    /// The local name.
    pub local: &'a str,
}

/// One event of a document, in document order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event<'a> {
    /// A processing instruction other than the XML declaration.
    Instruction {
        /// Its target.
        target: &'a str,
        /// Its content, if any.
        content: Option<&'a str>,
    },
    /// A comment's text.
    Comment(&'a str),
    /// A start tag's name.
    Start(XmlName<'a>),
    /// One attribute of the start tag just begun.
    Attribute {
        /// Its name.
        name: XmlName<'a>,
        /// Its value.
        value: XmlValue<'a>,
    },
    /// The element just started has content (`>`).
    Open,
    /// The element ends: its end tag, or an empty element's `/>`.
    End(XmlName<'a>),
    /// Character data or a CDATA section.
    Text(XmlValue<'a>),
}

/// A whole document read from a byte source, checked to be UTF-8.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XmlText {
    text: String,
}

/// UTF-8's byte-order mark, which a UTF-8 document may start with.
const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

impl XmlText {
    /// Reads a whole document from `source`, within `budget.bytes`.
    ///
    /// # Errors
    ///
    /// [`XmlErrorKind::Source`] if the source fails; [`XmlErrorKind::TooLarge`] past the byte
    /// budget (before allocating past it); [`XmlErrorKind::Encoding`] for a UTF-16 byte-order mark;
    /// [`XmlErrorKind::NotUtf8`] at the first byte that is not UTF-8;
    /// [`XmlErrorKind::OutOfMemory`].
    pub fn read<S: ByteSource + ?Sized>(
        source: &mut S,
        budget: &XmlBudget,
    ) -> Result<XmlText, XmlError> {
        let mut bytes: Vec<u8> = Vec::new();
        let mut chunk = [0_u8; 4096];
        loop {
            let n = source
                .read(&mut chunk)
                .map_err(|e| refused(XmlErrorKind::Source(e), bytes.len()))?;
            if n == 0 {
                break;
            }
            let piece = chunk.get(..n).ok_or(refused(
                XmlErrorKind::Source(SourceError::Overrun),
                bytes.len(),
            ))?;
            if bytes.len().saturating_add(n) > budget.bytes {
                return Err(refused(XmlErrorKind::TooLarge, budget.bytes));
            }
            bytes
                .try_reserve(n)
                .map_err(|_| refused(XmlErrorKind::OutOfMemory, bytes.len()))?;
            bytes.extend_from_slice(piece);
        }
        if bytes.starts_with(&[0xFE, 0xFF]) || bytes.starts_with(&[0xFF, 0xFE]) {
            return Err(refused(XmlErrorKind::Encoding, 0));
        }
        let skip = if bytes.starts_with(UTF8_BOM) {
            UTF8_BOM.len()
        } else {
            0
        };
        bytes.drain(..skip);
        let text = String::from_utf8(bytes).map_err(|e| {
            refused(
                XmlErrorKind::NotUtf8,
                e.utf8_error().valid_up_to().saturating_add(skip),
            )
        })?;
        Ok(XmlText { text })
    }

    /// The document's text (after any byte-order mark).
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// A reader of the document's events within `budget`.
    #[must_use]
    pub fn events(&self, budget: XmlBudget) -> Events<'_> {
        Events {
            tokens: Tokens::new(&self.text),
            budget,
            open: Vec::new(),
            attributes: Vec::new(),
            root_seen: false,
            pending_start: None,
            done: false,
        }
    }
}

/// The events of one document, pulled one at a time.
#[derive(Clone, Debug)]
pub struct Events<'a> {
    tokens: Tokens<'a>,
    budget: XmlBudget,
    open: Vec<XmlName<'a>>,
    attributes: Vec<XmlName<'a>>,
    root_seen: bool,
    pending_start: Option<XmlName<'a>>,
    done: bool,
}

impl<'a> Events<'a> {
    fn name(&self, prefix: &'a str, local: &'a str, at: usize) -> Result<XmlName<'a>, XmlError> {
        if prefix.len().saturating_add(local.len()) > self.budget.name {
            return Err(refused(XmlErrorKind::NameTooLong, at));
        }
        Ok(XmlName { prefix, local })
    }

    /// The next event, or `None` once the document is read to its end and found well formed.
    ///
    /// # Errors
    ///
    /// The first [`XmlError`]; after one, every later call returns `None`.
    pub fn next(&mut self, meter: &mut WorkMeter<'_>) -> Result<Option<Event<'a>>, XmlError> {
        if self.done {
            return Ok(None);
        }
        let got = self.step(meter);
        if !matches!(got, Ok(Some(_))) {
            self.done = true;
        }
        got
    }

    fn step(&mut self, meter: &mut WorkMeter<'_>) -> Result<Option<Event<'a>>, XmlError> {
        loop {
            let before = self.tokens.offset();
            let Some((at, token)) = self.tokens.next_token()? else {
                // An element left open, or a start tag never ended (`<a` at the end), is cut off.
                if !self.open.is_empty() || self.pending_start.is_some() {
                    return Err(refused(
                        XmlErrorKind::Scan(ScanErrorKind::UnexpectedEnd),
                        self.tokens.offset(),
                    ));
                }
                if !self.root_seen {
                    return Err(refused(XmlErrorKind::NoRoot, self.tokens.offset()));
                }
                return Ok(None);
            };
            let used = self.tokens.offset().saturating_sub(before);
            meter
                .charge(u64::try_from(used).unwrap_or(u64::MAX).saturating_add(1))
                .map_err(|_| refused(XmlErrorKind::Stopped, at))?;
            if let Some(event) = self.event(at, token)? {
                return Ok(Some(event));
            }
        }
    }

    /// The event of one token, or `None` for one that yields no event (the XML declaration).
    /// One attribute of the start tag being read, checked against its element's others and the
    /// budgets.
    fn attribute(
        &mut self,
        at: usize,
        name: XmlName<'a>,
        value: &'a str,
        offset: usize,
    ) -> Result<Event<'a>, XmlError> {
        if self.attributes.len() >= self.budget.attributes {
            return Err(refused(XmlErrorKind::TooManyAttributes, at));
        }
        if self.attributes.contains(&name) {
            return Err(refused(XmlErrorKind::DuplicateAttribute, at));
        }
        if value.len() > self.budget.value {
            return Err(refused(XmlErrorKind::ValueTooLong, at));
        }
        check_references(value, offset)?;
        self.attributes
            .try_reserve(1)
            .map_err(|_| refused(XmlErrorKind::OutOfMemory, at))?;
        self.attributes.push(name);
        Ok(Event::Attribute {
            name,
            value: XmlValue {
                raw: value,
                offset,
                cdata: false,
            },
        })
    }

    fn event(&mut self, at: usize, token: Token<'a>) -> Result<Option<Event<'a>>, XmlError> {
        let event = match token {
            Token::Declaration { encoding, .. } => {
                if encoding.is_some_and(|e| !e.eq_ignore_ascii_case("UTF-8")) {
                    return Err(refused(XmlErrorKind::Encoding, at));
                }
                return Ok(None);
            }
            Token::ProcessingInstruction { target, content } => {
                if target.len() > self.budget.name {
                    return Err(refused(XmlErrorKind::NameTooLong, at));
                }
                if content.is_some_and(|c| c.len() > self.budget.text) {
                    return Err(refused(XmlErrorKind::TextTooLong, at));
                }
                Event::Instruction { target, content }
            }
            Token::Comment(text) => {
                if text.len() > self.budget.text {
                    return Err(refused(XmlErrorKind::TextTooLong, at));
                }
                Event::Comment(text)
            }
            Token::ElementStart { prefix, local } => {
                // The tokenizer refuses a second root, so only nesting is left to check.
                let name = self.name(prefix, local, at)?;
                if self.open.len() >= self.budget.depth {
                    return Err(refused(XmlErrorKind::TooDeep, at));
                }
                self.root_seen = true;
                self.attributes.clear();
                self.pending_start = Some(name);
                Event::Start(name)
            }
            Token::Attribute {
                prefix,
                local,
                value,
                value_at,
            } => self.attribute(at, self.name(prefix, local, at)?, value, value_at)?,
            Token::ElementEnd(ElementEnd::Open) => {
                let name = self
                    .pending_start
                    .take()
                    .ok_or(refused(XmlErrorKind::Mismatch, at))?;
                self.open
                    .try_reserve(1)
                    .map_err(|_| refused(XmlErrorKind::OutOfMemory, at))?;
                self.open.push(name);
                Event::Open
            }
            Token::ElementEnd(ElementEnd::Empty) => {
                let name = self
                    .pending_start
                    .take()
                    .ok_or(refused(XmlErrorKind::Mismatch, at))?;
                Event::End(name)
            }
            Token::ElementEnd(ElementEnd::Close { prefix, local }) => {
                let name = XmlName { prefix, local };
                if self.open.pop() != Some(name) {
                    return Err(refused(XmlErrorKind::Mismatch, at));
                }
                Event::End(name)
            }
            Token::Text(text) => {
                if text.len() > self.budget.text {
                    return Err(refused(XmlErrorKind::TextTooLong, at));
                }
                check_references(text, at)?;
                Event::Text(XmlValue {
                    raw: text,
                    offset: at,
                    cdata: false,
                })
            }
            Token::Cdata(text, offset) => {
                if text.len() > self.budget.text {
                    return Err(refused(XmlErrorKind::TextTooLong, at));
                }
                Event::Text(XmlValue {
                    raw: text,
                    offset,
                    cdata: true,
                })
            }
        };
        Ok(Some(event))
    }
}
