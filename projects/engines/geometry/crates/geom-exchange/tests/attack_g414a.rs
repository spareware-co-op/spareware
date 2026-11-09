//! Piece G414a's attack: the hardened XML reader against the known XML attack families (entity
//! expansion, external entities, deep nesting, attribute floods, oversized names, values and text,
//! encoding tricks) and against malformed documents, each refused with its typed error at its
//! byte offset; and well-formed documents read to the events they hold, the same however the
//! source splits the bytes.
//!
//! The attack documents are the standard constructions of each family, written here (no corpus
//! file is copied): billion laughs (CVE-2003-1564's nested entities), external entities (OWASP's
//! XXE cheat sheet: a `SYSTEM` entity naming a file, and a parameter entity), quadratic blowup (one
//! large entity referenced many times), and an entity reference with no declaration.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failure should stop loudly"
)]

use std::fmt::Write as _;

use geom_exchange::xml::scan::ScanErrorKind;
use geom_exchange::xml::{Event, XmlBudget, XmlError, XmlErrorKind, XmlName, XmlText};
use geom_format::source::{ByteSource, SliceSource, SourceError};
use geom_run::{Budget, WorkMeter};

/// Reads `doc` within `budget` to its end: the events, flattened to text, or the first error.
fn read_with(doc: &[u8], budget: XmlBudget) -> Result<Vec<String>, XmlError> {
    let text = XmlText::read(&mut SliceSource::new(doc), &budget)?;
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let mut events = text.events(budget);
    let mut out = Vec::new();
    while let Some(e) = events.next(&mut meter)? {
        out.push(show(e));
    }
    Ok(out)
}

fn read(doc: &str) -> Result<Vec<String>, XmlError> {
    read_with(doc.as_bytes(), XmlBudget::DEFAULT)
}

fn name(n: XmlName<'_>) -> String {
    if n.prefix.is_empty() {
        n.local.to_owned()
    } else {
        format!("{}:{}", n.prefix, n.local)
    }
}

fn show(e: Event<'_>) -> String {
    match e {
        Event::Instruction { target, content } => format!("pi {target} {content:?}"),
        Event::Comment(t) => format!("comment {t}"),
        Event::Start(n) => format!("start {}", name(n)),
        Event::Attribute { name: n, value } => {
            let mut v = String::new();
            value.decode_into(&mut v).unwrap();
            format!("attr {}={v}", name(n))
        }
        Event::Open => "open".to_owned(),
        Event::End(n) => format!("end {}", name(n)),
        Event::Text(value) => {
            let mut v = String::new();
            value.decode_into(&mut v).unwrap();
            format!("text {v}")
        }
    }
}

fn refused(doc: &str) -> XmlError {
    read(doc).expect_err(doc)
}

#[test]
fn a_document_reads_to_its_events_with_references_resolved() {
    let doc = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- a model -->\n\
               <m:model xmlns:m=\"urn:x\" unit=\"mm\">\
               <part id='1' name=\"A &amp; B &#x41;&#66;\">bolt&lt;M8&gt;</part>\
               <empty/><![CDATA[<raw & text>]]><?hint fast?></m:model>";
    let got = read(doc).unwrap();
    assert_eq!(
        got,
        [
            "comment  a model ",
            "start m:model",
            "attr xmlns:m=urn:x",
            "attr unit=mm",
            "open",
            "start part",
            "attr id=1",
            "attr name=A & B AB",
            "open",
            "text bolt<M8>",
            "end part",
            "start empty",
            "end empty",
            "text <raw & text>",
            "pi hint Some(\"fast\")",
            "end m:model",
        ]
    );
}

#[test]
fn the_billion_laughs_is_refused_at_its_document_type() {
    let doc = "<?xml version=\"1.0\"?>\n<!DOCTYPE lolz [\n <!ENTITY lol \"lol\">\n \
               <!ENTITY lol1 \"&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;\">\n \
               <!ENTITY lol2 \"&lol1;&lol1;&lol1;&lol1;&lol1;&lol1;&lol1;&lol1;&lol1;&lol1;\">\n]>\n\
               <lolz>&lol2;</lolz>";
    let e = refused(doc);
    assert_eq!(e.kind, XmlErrorKind::Scan(ScanErrorKind::DocumentType));
    assert_eq!(e.offset, doc.find("<!DOCTYPE").unwrap());
}

#[test]
fn external_entities_are_refused_at_their_document_type() {
    for doc in [
        "<?xml version=\"1.0\"?><!DOCTYPE foo [<!ENTITY xxe SYSTEM \"file:///etc/passwd\">]><foo>&xxe;</foo>",
        "<!DOCTYPE foo SYSTEM \"http://example.invalid/evil.dtd\"><foo/>",
        "<!DOCTYPE foo [<!ENTITY % pe SYSTEM \"http://example.invalid/x\"> %pe;]><foo/>",
        // Quadratic blowup: one large entity, referenced many times.
        "<!DOCTYPE q [<!ENTITY a \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\">]><q>&a;&a;&a;&a;&a;&a;&a;</q>",
    ] {
        let e = refused(doc);
        assert_eq!(
            e.kind,
            XmlErrorKind::Scan(ScanErrorKind::DocumentType),
            "{doc}"
        );
        assert_eq!(e.offset, doc.find("<!DOCTYPE").unwrap(), "{doc}");
    }
    // A document type inside the root is refused too.
    let doc = "<a><!DOCTYPE b></a>";
    assert_eq!(
        refused(doc).kind,
        XmlErrorKind::Scan(ScanErrorKind::DocumentType)
    );
}

#[test]
fn an_entity_never_declared_is_refused_where_it_stands() {
    let doc = "<a>fine &amp; then &xxe; here</a>";
    let e = refused(doc);
    assert_eq!(e.kind, XmlErrorKind::Scan(ScanErrorKind::Entity));
    assert_eq!(e.offset, doc.find("&xxe;").unwrap());
    let doc = "<a b=\"&secret;\"/>";
    let e = refused(doc);
    assert_eq!(e.kind, XmlErrorKind::Scan(ScanErrorKind::Entity));
    assert_eq!(e.offset, doc.find("&secret;").unwrap());
}

#[test]
fn bad_character_references_are_refused() {
    for doc in [
        "<a>&#0;</a>",
        "<a>&#x110000;</a>",
        "<a>&#xFFFE;</a>",
        "<a>&#99999999999999999999;</a>",
        "<a>&#;</a>",
        "<a>&#x;</a>",
        "<a>& b</a>",
        "<a>&amp</a>",
    ] {
        assert_eq!(
            refused(doc).kind,
            XmlErrorKind::Scan(ScanErrorKind::Reference),
            "{doc}"
        );
    }
}

#[test]
fn deep_nesting_stops_at_the_depth_budget_without_recursion() {
    let depth = 100_000;
    let doc = format!("{}{}", "<a>".repeat(depth), "</a>".repeat(depth));
    let e = refused(&doc);
    assert_eq!(e.kind, XmlErrorKind::TooDeep);
    assert_eq!(e.offset, 3 * XmlBudget::DEFAULT.depth);
    // Exactly the budget is allowed.
    let d = XmlBudget::DEFAULT.depth;
    let doc = format!("{}{}", "<a>".repeat(d), "</a>".repeat(d));
    assert!(read(&doc).is_ok());
}

#[test]
fn every_budget_is_held() {
    let small = XmlBudget {
        bytes: 1_000,
        depth: 4,
        attributes: 3,
        name: 8,
        value: 10,
        text: 12,
    };
    let kind = |doc: &str| read_with(doc.as_bytes(), small).unwrap_err().kind;
    assert_eq!(
        kind("<a b='1' c='2' d='3' e='4'/>"),
        XmlErrorKind::TooManyAttributes
    );
    assert!(read_with(b"<a b='1' c='2' d='3'/>", small).is_ok());
    assert_eq!(kind("<abcdefghi/>"), XmlErrorKind::NameTooLong);
    assert_eq!(kind("<a abcdefghi='1'/>"), XmlErrorKind::NameTooLong);
    assert!(read_with(b"<abcdefgh/>", small).is_ok());
    assert_eq!(kind("<a b='12345678901'/>"), XmlErrorKind::ValueTooLong);
    assert_eq!(kind("<a>1234567890123</a>"), XmlErrorKind::TextTooLong);
    assert_eq!(
        kind("<a><![CDATA[1234567890123]]></a>"),
        XmlErrorKind::TextTooLong
    );
    assert_eq!(
        kind("<a><!--1234567890123--></a>"),
        XmlErrorKind::TextTooLong
    );
    assert_eq!(
        kind("<a><?p 1234567890123?></a>"),
        XmlErrorKind::TextTooLong
    );
    assert_eq!(kind("<a><?abcdefghi?></a>"), XmlErrorKind::NameTooLong);
    let big = format!("<a>{}</a>", "x".repeat(2_000));
    let e = read_with(big.as_bytes(), small).unwrap_err();
    assert_eq!(e.kind, XmlErrorKind::TooLarge);
    // An attribute flood under the default budget.
    let mut flood = String::new();
    for i in 0..2_000 {
        write!(flood, " a{i}='1'").unwrap();
    }
    assert_eq!(
        refused(&format!("<a{flood}/>")).kind,
        XmlErrorKind::TooManyAttributes
    );
}

#[test]
fn encodings_other_than_utf8_are_refused() {
    // UTF-16 with either byte-order mark.
    for bom in [[0xFE_u8, 0xFF], [0xFF, 0xFE]] {
        let mut doc = bom.to_vec();
        doc.extend_from_slice(b"<\0a\0/\0>\0");
        assert_eq!(
            read_with(&doc, XmlBudget::DEFAULT).unwrap_err().kind,
            XmlErrorKind::Encoding
        );
    }
    // A declared encoding other than UTF-8, in any case.
    assert_eq!(
        refused("<?xml version='1.0' encoding='ISO-8859-1'?><a/>").kind,
        XmlErrorKind::Encoding
    );
    assert!(read("<?xml version='1.0' encoding='utf-8'?><a/>").is_ok());
    // Bytes that are not UTF-8, at the first bad byte.
    let e = read_with(b"<a>caf\xE9</a>", XmlBudget::DEFAULT).unwrap_err();
    assert_eq!((e.kind, e.offset), (XmlErrorKind::NotUtf8, 6));
    // A UTF-8 byte-order mark is skipped.
    let mut doc = vec![0xEF, 0xBB, 0xBF];
    doc.extend_from_slice(b"<a/>");
    assert_eq!(
        read_with(&doc, XmlBudget::DEFAULT).unwrap(),
        ["start a", "end a"]
    );
}

#[test]
fn malformed_documents_are_refused_with_their_kind() {
    let cases: [(&str, XmlErrorKind); 16] = [
        ("<a", XmlErrorKind::Scan(ScanErrorKind::UnexpectedEnd)),
        ("<a b='1'", XmlErrorKind::Scan(ScanErrorKind::UnexpectedEnd)),
        ("", XmlErrorKind::NoRoot),
        ("<!-- only a comment -->", XmlErrorKind::NoRoot),
        ("<a></b>", XmlErrorKind::Mismatch),
        ("<a><b></a></b>", XmlErrorKind::Mismatch),
        ("<a x='1' x='2'/>", XmlErrorKind::DuplicateAttribute),
        ("<a p:x='1' p:x='2'/>", XmlErrorKind::DuplicateAttribute),
        ("<a>", XmlErrorKind::Scan(ScanErrorKind::UnexpectedEnd)),
        ("<a/><b/>", XmlErrorKind::Scan(ScanErrorKind::Misplaced)),
        ("text<a/>", XmlErrorKind::Scan(ScanErrorKind::Misplaced)),
        ("<a/>tail", XmlErrorKind::Scan(ScanErrorKind::Misplaced)),
        (
            "<a><!-- x -- y --></a>",
            XmlErrorKind::Scan(ScanErrorKind::Comment),
        ),
        (
            "<a>x ]]> y</a>",
            XmlErrorKind::Scan(ScanErrorKind::CharacterData),
        ),
        (
            "<a b=\"<\"/>",
            XmlErrorKind::Scan(ScanErrorKind::Unexpected),
        ),
        ("<1a/>", XmlErrorKind::Scan(ScanErrorKind::Name)),
    ];
    for (doc, kind) in cases {
        let e = refused(doc);
        assert_eq!(e.kind, kind, "{doc}");
        assert!(e.offset <= doc.len(), "{doc}");
    }
    assert_eq!(
        refused("<a>\u{FFFE}</a>").kind,
        XmlErrorKind::Scan(ScanErrorKind::NotXmlChar)
    );
    assert_eq!(
        refused("<?xml version='2.0'?><a/>").kind,
        XmlErrorKind::Scan(ScanErrorKind::Declaration)
    );
    // An XML declaration anywhere but the first bytes, or with a tab after `xml`, is no
    // instruction.
    for doc in [
        "<a/><?xml version='1.0'?>",
        "<?xml\tversion='1.0'?><a/>",
        "<a><?XmL x?></a>",
    ] {
        assert_eq!(
            refused(doc).kind,
            XmlErrorKind::Scan(ScanErrorKind::Misplaced),
            "{doc}"
        );
    }
}

/// A source that hands out one byte per read, then fails if asked to.
struct Trickle<'a> {
    rest: &'a [u8],
    fail: bool,
}

impl ByteSource for Trickle<'_> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SourceError> {
        match (self.rest.split_first(), buf.first_mut()) {
            (Some((b, rest)), Some(slot)) => {
                *slot = *b;
                self.rest = rest;
                Ok(1)
            }
            _ if self.fail => Err(SourceError::Failed(7)),
            _ => Ok(0),
        }
    }
}

#[test]
fn the_split_of_the_source_never_matters_and_its_failure_is_reported() {
    let doc = "<r a='&lt;1'><s>text &#x263A;</s><![CDATA[x]]></r>";
    let whole = read(doc).unwrap();
    let text = XmlText::read(
        &mut Trickle {
            rest: doc.as_bytes(),
            fail: false,
        },
        &XmlBudget::DEFAULT,
    )
    .unwrap();
    assert_eq!(text.as_str(), doc);
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let mut events = text.events(XmlBudget::DEFAULT);
    let mut split = Vec::new();
    while let Some(e) = events.next(&mut meter).unwrap() {
        split.push(show(e));
    }
    assert_eq!(split, whole);
    let e = XmlText::read(
        &mut Trickle {
            rest: b"<a/>",
            fail: true,
        },
        &XmlBudget::DEFAULT,
    )
    .unwrap_err();
    assert_eq!(
        (e.kind, e.offset),
        (XmlErrorKind::Source(SourceError::Failed(7)), 4)
    );
}

#[test]
fn the_meter_stops_a_read_and_an_error_ends_the_events() {
    let doc = format!("<a>{}</a>", "<b/>".repeat(100));
    let text = XmlText::read(&mut SliceSource::new(doc.as_bytes()), &XmlBudget::DEFAULT).unwrap();
    let mut meter = WorkMeter::new(Budget::of_work(50));
    let mut events = text.events(XmlBudget::DEFAULT);
    let mut stopped = None;
    for _ in 0..1_000 {
        match events.next(&mut meter) {
            Ok(Some(_)) => {}
            Ok(None) => break,
            Err(e) => {
                stopped = Some(e.kind);
                break;
            }
        }
    }
    assert_eq!(stopped, Some(XmlErrorKind::Stopped));
    // After an error, nothing more.
    assert_eq!(
        events.next(&mut WorkMeter::new(Budget::UNLIMITED)),
        Ok(None)
    );
}
