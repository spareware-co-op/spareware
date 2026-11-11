//! The Part 21 codec's behaviour (piece G415): what the reader reads, the error each malformed
//! text gives and where, what the writer writes, and the whole file in memory.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::format_push_string,
    clippy::range_plus_one,
    clippy::naive_bytecount,
    clippy::assert_is_empty,
    clippy::float_cmp,
    reason = "tests: a failure stops the test"
)]

mod common;

use common::{data, events, exchange, instances, meter, refusal};
use geom_format::source::{ByteSource, SliceSource, SourceError};
use geom_p21::{
    ErrorKind, Event, Exchange, Header, List, Options, Param, Reader, Record, Section, Token,
    Totals, WriteError, Writer,
};

const SAMPLE: &str = "ISO-10303-21;\nHEADER;FILE_DESCRIPTION((''),'2;1');\nFILE_NAME('a','',(''),(''),'','sys','');\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN'));ENDSEC;\nDATA;\n#1=CARTESIAN_POINT('p',(0.,1.5E1,-2));\n/* c */ #2=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));\n#3=X('it''s','\\X2\\00E9\\X0\\',$,#1,LENGTH_MEASURE(1.E-3),\"0F\");\nENDSEC;\nEND-ISO-10303-21;\n";

fn list(items: Vec<Param>) -> Param {
    Param::List(List::new(items).unwrap())
}

#[test]
fn parses_simple_and_complex() {
    let ex = exchange(SAMPLE.as_bytes()).unwrap();
    assert_eq!(ex.header.schemas, vec!["AUTOMOTIVE_DESIGN"]);
    assert_eq!(ex.header.originating_system, "sys");
    assert_eq!(ex.header.file_name, "a");
    assert_eq!(ex.header.implementation_level, "2;1");
    let p = ex.get(1).unwrap();
    assert_eq!(p.name(), "CARTESIAN_POINT");
    assert!(!p.is_complex());
    let xyz: Vec<f64> = p.params()[1]
        .as_list()
        .unwrap()
        .iter()
        .filter_map(Param::as_f64)
        .collect();
    assert_eq!(xyz, vec![0.0, 15.0, -2.0]);
    assert_eq!(p.params()[1].as_list().unwrap()[2].as_i64(), Some(-2));
    let u = ex.get(2).unwrap();
    assert!(u.is_complex() && u.has("SI_UNIT") && !u.has("SI"));
    assert_eq!(u.record("SI_UNIT").unwrap()[0].as_enum(), Some("MILLI"));
    assert_eq!(u.record("NAMED_UNIT").unwrap(), [Param::Derived]);
    assert_eq!(u.record("NONE"), None);
    let x = ex.get(3).unwrap().params();
    assert_eq!(x[0].as_str(), Some("it's"));
    assert_eq!(x[1].as_str(), Some("\u{e9}"));
    assert_eq!(x[2], Param::Null);
    assert_eq!(x[3].as_ref_id(), Some(1));
    assert_eq!(x[4].as_f64(), Some(1e-3));
    assert_eq!(x[5], Param::Binary("0F".into()));
    assert!(ex.duplicates.is_empty());
    assert_eq!(ex.get(4), None);
}

#[test]
fn the_events_come_in_file_order_with_their_tokens() {
    let text = b"ISO-10303-21;HEADER;FILE_SCHEMA(('S'));ENDSEC;ANCHOR;<a>=#1;ENDSEC;DATA('d',('S'));#7=A(1,(2,B(3)),.t.);ENDSEC;DATA;ENDSEC;SIGNATURE x'ENDSEC;' /*ENDSEC;*/ ENDSECX; 1ENDSEC; ENDSEC ;END-ISO-10303-21;trailing";
    let mut meter = meter();
    let mut reader = Reader::new(SliceSource::new(text), Options::DEFAULT, &mut meter);
    let mut seen = Vec::new();
    while let Some(event) = reader.next_event().unwrap() {
        seen.push(event);
        if event == Event::Instance(7) {
            let tokens: Vec<Token<'_>> = reader.tokens().iter().collect();
            assert_eq!(
                tokens,
                [
                    Token::Record("A"),
                    Token::Int(1),
                    Token::Open,
                    Token::Int(2),
                    Token::Typed("B"),
                    Token::Int(3),
                    Token::Close,
                    Token::Close,
                    Token::Enum("T"),
                    Token::Close,
                ]
            );
            assert_eq!(reader.tokens().params(), 6);
            assert_eq!(reader.tokens().record_count(), 1);
        }
        if event == Event::Section(Section::Data) && seen.len() == 5 {
            let records = reader.tokens().records();
            assert_eq!(records[0].name, "DATA");
            assert_eq!(records[0].params[0].as_str(), Some("d"));
        }
    }
    assert_eq!(
        seen,
        [
            Event::Section(Section::Header),
            Event::HeaderRecord,
            Event::EndSection,
            Event::Skipped(Section::Anchor),
            Event::Section(Section::Data),
            Event::Instance(7),
            Event::EndSection,
            Event::Section(Section::Data),
            Event::EndSection,
            Event::Skipped(Section::Signature),
        ]
    );
    // The bytes after the end are not read, and the reader stays at its end.
    assert_eq!(reader.offset(), (text.len() - "trailing".len()) as u64);
    assert_eq!(reader.next_event(), Ok(None));
    assert_eq!(reader.replaced(), 0);
}

#[test]
fn numbers_read_exactly() {
    let read = |text: &str| instances(&format!("#1=A({text});"))[0].1[0].params[0].clone();
    for (text, value) in [
        ("0", 0),
        ("-0", 0),
        ("+7", 7),
        ("9223372036854775807", i64::MAX),
        ("-9223372036854775808", i64::MIN),
    ] {
        assert_eq!(read(text), Param::Int(value), "{text}");
    }
    for (text, value) in [
        ("0.", 0.0),
        ("1.", 1.0),
        ("-1.5", -1.5),
        ("+1.5E1", 15.0),
        ("1.E-3", 1e-3),
        ("1.e-3", 1e-3),
        ("1E5", 1e5),
        (".5", 0.5),
        ("1.7976931348623157E308", f64::MAX),
        ("4.9E-324", 5e-324),
        ("1.E-999", 0.0),
        ("0.1", 0.1),
    ] {
        assert_eq!(read(text), Param::Real(value), "{text}");
    }
    assert_eq!(
        read("-0.").as_f64().unwrap().to_bits(),
        (-0.0_f64).to_bits()
    );
    // An integer is a real only where a double holds it exactly.
    assert_eq!(Param::Int(1 << 53).as_f64(), Some(9_007_199_254_740_992.0));
    assert_eq!(
        Param::Int(-(1 << 53)).as_f64(),
        Some(-9_007_199_254_740_992.0)
    );
    assert_eq!(Param::Int((1 << 53) + 1).as_f64(), None);
    assert_eq!(Param::Int(i64::MIN).as_f64(), None);
    assert_eq!(Param::Real(f64::INFINITY).as_f64(), None);
    assert_eq!(Param::Real(1.0).as_i64(), None);
    assert_eq!(Param::Enum("U".into()).as_bool(), None);
    assert_eq!(Param::Enum("T".into()).as_bool(), Some(true));
    assert_eq!(Param::Enum("F".into()).as_bool(), Some(false));
    assert_eq!(Param::Null.as_bool(), None);
    // A typed value gives its one number, and no more than one level of it.
    let typed = |items| Param::Typed("T".into(), List::new(items).unwrap());
    assert_eq!(typed(vec![Param::Int(2)]).as_f64(), Some(2.0));
    assert_eq!(typed(vec![Param::Int(2), Param::Int(3)]).as_f64(), None);
    assert_eq!(typed(vec![typed(vec![Param::Int(2)])]).as_f64(), None);
    assert_eq!(typed(vec![]).as_f64(), None);
    assert_eq!(Param::Null.as_f64(), None);
    assert_eq!(Param::Null.as_str(), None);
    assert_eq!(Param::Null.as_enum(), None);
    assert_eq!(Param::Null.as_ref_id(), None);
    assert_eq!(Param::Null.as_list(), None);
    assert_eq!(List::empty().depth(), 1);
    assert_eq!(List::empty().into_items(), vec![]);
}

#[test]
fn names_fold_to_upper_case_and_white_space_is_free() {
    let got = instances("  #1 = a_b1 ( .f. , !user ( 1 ) , \"3F\" ) ;");
    assert_eq!(
        got,
        [(
            1,
            vec![Record {
                name: "A_B1".into(),
                params: vec![
                    Param::Enum("F".into()),
                    Param::Typed("!USER".into(), List::new(vec![Param::Int(1)]).unwrap()),
                    Param::Binary("3F".into()),
                ],
            }]
        )]
    );
    // A line break inside a string is skipped; a doubled quote is one.
    let got = instances("#2=A('a\r\nb''c','');");
    assert_eq!(
        got[0].1[0].params,
        [Param::Str("ab'c".into()), Param::Str(String::new())]
    );
    // A byte order mark and comments anywhere between tokens.
    let text = "\u{feff}/**/ISO-10303-21/*x*/;HEADER;/***/ENDSEC;DATA;#1/*/ */=A(/* ) */);ENDSEC;END-ISO-10303-21;";
    assert_eq!(events(text.as_bytes()).1, None);
    assert_eq!(
        events(b"iso-10303-21;header;endsec;end-iso-10303-21;").1,
        None
    );
}

#[test]
fn every_malformed_text_is_its_typed_error() {
    use ErrorKind as K;
    let cases: Vec<(Vec<u8>, K)> = vec![
        (b"".to_vec(), K::NotP21),
        (b"garbage".to_vec(), K::NotP21),
        (b"ISO-10303-21".to_vec(), K::NotP21),
        (b"ISO-10303-22;".to_vec(), K::NotP21),
        (b"\xEF\xBBISO-10303-21;".to_vec(), K::NotP21),
        (b"/*".to_vec(), K::UnterminatedComment),
        (b"/ISO-10303-21;".to_vec(), K::UnexpectedByte(b'/')),
        (b"ISO-10303-21;".to_vec(), K::MissingHeader),
        (b"ISO-10303-21;DATA;".to_vec(), K::MissingHeader),
        (b"ISO-10303-21;HEADER".to_vec(), K::UnexpectedEnd),
        (b"ISO-10303-21;HEADER:".to_vec(), K::Expected(b';')),
        (b"ISO-10303-21;HEADER;".to_vec(), K::UnexpectedEnd),
        (b"ISO-10303-21;HEADER;1A();".to_vec(), K::ExpectedKeyword),
        (b"ISO-10303-21;HEADER;!();".to_vec(), K::ExpectedKeyword),
        (b"ISO-10303-21;HEADER;A;".to_vec(), K::Expected(b'(')),
        (b"ISO-10303-21;HEADER;A()".to_vec(), K::UnexpectedEnd),
        (
            b"ISO-10303-21;HEADER;A()ENDSEC;".to_vec(),
            K::Expected(b';'),
        ),
        (b"ISO-10303-21;HEADER;ENDSEC;".to_vec(), K::UnexpectedEnd),
        (
            b"ISO-10303-21;HEADER;ENDSEC;HEADER;".to_vec(),
            K::UnexpectedSection,
        ),
        (
            b"ISO-10303-21;HEADER;ENDSEC;#1=A();".to_vec(),
            K::ExpectedKeyword,
        ),
        (
            b"ISO-10303-21;HEADER;ENDSEC;DATA".to_vec(),
            K::UnexpectedEnd,
        ),
        (
            b"ISO-10303-21;HEADER;ENDSEC;DATA(".to_vec(),
            K::UnexpectedEnd,
        ),
        (
            b"ISO-10303-21;HEADER;ENDSEC;DATA;".to_vec(),
            K::UnexpectedEnd,
        ),
        (
            b"ISO-10303-21;HEADER;ENDSEC;DATA;ENDSEC;".to_vec(),
            K::UnexpectedEnd,
        ),
        (
            b"ISO-10303-21;HEADER;ENDSEC;END-ISO-10303-21".to_vec(),
            K::UnexpectedEnd,
        ),
        (
            b"ISO-10303-21;HEADER;ENDSEC;ANCHOR;".to_vec(),
            K::UnexpectedEnd,
        ),
        (
            b"ISO-10303-21;HEADER;ENDSEC;ANCHOR;'ENDSEC;".to_vec(),
            K::UnterminatedString,
        ),
        (
            b"ISO-10303-21;HEADER;ENDSEC;ANCHOR;/*ENDSEC;".to_vec(),
            K::UnterminatedComment,
        ),
        (data("X;"), K::Expected(b'#')),
        (data("$"), K::Expected(b'#')),
        (data("#=A();"), K::BadInstanceName),
        (data("#99999999999999999999999=A();"), K::BadInstanceName),
        (data("#1 A();"), K::Expected(b'=')),
        (data("#1=;"), K::ExpectedKeyword),
        (data("#1=();"), K::EmptyInstance),
        (
            b"ISO-10303-21;HEADER;ENDSEC;DATA;#1=(A()".to_vec(),
            K::UnexpectedEnd,
        ),
        (data("#1=(A()"), K::Expected(b'(')),
        (data("#1=(A();"), K::ExpectedKeyword),
        (data("#1=A()"), K::Expected(b';')),
        (data("#1=A("), K::Expected(b'(')),
        (data("#1=A(,);"), K::UnexpectedByte(b',')),
        (data("#1=A(1,);"), K::UnexpectedByte(b')')),
        (data("#1=A(1 2);"), K::Expected(b')')),
        (data("#1=A(1;"), K::Expected(b')')),
        (data("#1=A(?);"), K::UnexpectedByte(b'?')),
        (data("#1=A(\u{ff});"), K::UnexpectedByte(0xC3)),
        (data("#1=A(B);"), K::Expected(b'(')),
        (data("#1=A(#);"), K::BadInstanceName),
        (data("#1=A(#-1);"), K::BadInstanceName),
        (data("#1=A(+);"), K::BadNumber),
        (data("#1=A(-);"), K::BadNumber),
        (data("#1=A(+.);"), K::BadNumber),
        (data("#1=A(-.E3);"), K::BadNumber),
        (data("#1=A(1E);"), K::BadNumber),
        (data("#1=A(1E+);"), K::BadNumber),
        (data("#1=A(1.5.2);"), K::BadNumber),
        (data("#1=A(1e999999.);"), K::BadNumber),
        (data("#1=A(1-2);"), K::BadNumber),
        (data("#1=A(--1);"), K::BadNumber),
        (data("#1=A(1E5E5);"), K::BadNumber),
        (data("#1=A(99999999999999999999);"), K::IntegerOutOfRange),
        (data("#1=A(-9223372036854775809);"), K::IntegerOutOfRange),
        (data("#1=A(1.E999999);"), K::RealOutOfRange),
        (data("#1=A(-1.8E308);"), K::RealOutOfRange),
        (data("#1=A(.);"), K::BadEnumeration),
        (data("#1=A(..);"), K::BadEnumeration),
        (data("#1=A(.T);"), K::BadEnumeration),
        (data("#1=A(.T .);"), K::BadEnumeration),
        (data("#1=A(. T.);"), K::BadEnumeration),
        (data("#1=A(.T-.);"), K::BadEnumeration),
        (data("#1=A(\"\");"), K::BadBinary),
        (data("#1=A(\"4F\");"), K::BadBinary),
        (data("#1=A(\"0f\");"), K::BadBinary),
        (data("#1=A(\"0G\");"), K::BadBinary),
        (data("#1=A(\"0 \");"), K::BadBinary),
        (
            b"ISO-10303-21;HEADER;ENDSEC;DATA;#1=A(\"0F".to_vec(),
            K::UnexpectedEnd,
        ),
        (
            b"ISO-10303-21;HEADER;ENDSEC;DATA;#1=A('x".to_vec(),
            K::UnterminatedString,
        ),
        (
            b"ISO-10303-21;HEADER;ENDSEC;DATA;#1=A(/*".to_vec(),
            K::UnterminatedComment,
        ),
        (
            b"ISO-10303-21;HEADER;ENDSEC;DATA;#1=A(1/2);ENDSEC;".to_vec(),
            K::UnexpectedByte(b'/'),
        ),
        (data("#1=A('\\X2\\00E\\X0\\');"), K::BadText),
        (
            b"ISO-10303-21;HEADER;ENDSEC;DATA;#1=A('\xe9');ENDSEC;END-ISO-10303-21;".to_vec(),
            K::BadText,
        ),
    ];
    let mut wrong = Vec::new();
    for (text, kind) in cases {
        let e = refusal(&text);
        let shown = String::from_utf8_lossy(&text).into_owned();
        if e.kind != kind {
            wrong.push(format!("{shown}: {:?}, not {kind:?}", e.kind));
        }
        assert!(e.offset <= text.len() as u64, "{shown}");
        let newlines = text.iter().filter(|c| **c == b'\n').count() as u64;
        assert!((1..=newlines + 1).contains(&e.line), "{shown}");
        assert!(!e.to_string().is_empty());
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn errors_name_their_line_and_offset() {
    let text =
        b"ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n#1=A(1,\n 2,\n ?);\nENDSEC;\nEND-ISO-10303-21;\n";
    let e = refusal(text);
    assert_eq!(e.kind, ErrorKind::UnexpectedByte(b'?'));
    assert_eq!(e.line, 7);
    assert_eq!(
        e.offset,
        text.iter().position(|c| *c == b'?').unwrap() as u64
    );
    assert_eq!(
        e.to_string(),
        format!(
            "Part 21 error at line 7 (byte {}): unexpected `?`",
            e.offset
        )
    );
    // A header without its `ENDSEC;` stops at `DATA`, on its line.
    // With parameters, `DATA(...)` reads as a header record, and the instance after it stops
    // the read.
    for (data, line) in [("DATA;", 4), ("DATA('d', ('AUTOMOTIVE_DESIGN'));", 5)] {
        let t = format!(
            "ISO-10303-21;\nHEADER;\nBAD();\n{data}\n#1=A();\nENDSEC;\nEND-ISO-10303-21;\n"
        );
        let e = refusal(t.as_bytes());
        assert_eq!(e.line, line, "{e}");
    }
    // Line feeds inside comments and strings count.
    let e = refusal(b"ISO-10303-21;HEADER;/*\n\n*/A('\n\n',?);");
    assert_eq!((e.kind, e.line), (ErrorKind::UnexpectedByte(b'?'), 5));
}

#[test]
fn nesting_is_read_to_its_bound_and_refused_past_it() {
    let nested = |depth: usize| {
        data(&format!(
            "#1=A({}{});",
            "(".repeat(depth),
            ")".repeat(depth)
        ))
    };
    let (events, error) = events(&nested(geom_p21::MAX_DEPTH));
    assert_eq!(error, None);
    let records = &events[3].1;
    assert_eq!(records[0].params[0].depth(), geom_p21::MAX_DEPTH);
    assert_eq!(
        refusal(&nested(geom_p21::MAX_DEPTH + 1)).kind,
        ErrorKind::TooDeep
    );
    assert_eq!(refusal(&nested(10_000)).kind, ErrorKind::TooDeep);
    let typed = data(&format!("#1=A({}1{});", "T(".repeat(65), ")".repeat(65)));
    assert_eq!(refusal(&typed).kind, ErrorKind::TooDeep);
    // A caller's list has the same bound.
    let mut deep = list(vec![]);
    for _ in 1..geom_p21::MAX_DEPTH {
        deep = list(vec![deep]);
    }
    assert!(List::new(vec![deep]).is_err());
}

#[test]
fn one_record_is_held_to_the_options() {
    let read = |text: &[u8], options| common::events_of(SliceSource::new(text), options).1;
    let t = data("#1=A($,($),T(1.));");
    // The record's name, `$`, the list, its `$`, its end, the typed value, `1.`, its end and
    // the record's end: nine items.
    let items = |items| Options {
        items,
        ..Options::DEFAULT
    };
    assert_eq!(read(&t, items(9)), None);
    assert_eq!(read(&t, items(8)).unwrap().kind, ErrorKind::TooManyItems);
    assert_eq!(read(&t, items(0)).unwrap().kind, ErrorKind::TooManyItems);
    // The count is of one instance, not the file.
    assert_eq!(
        read(&data("#1=A($);#2=A($);#3=(A($)B($));"), items(6)),
        None
    );
    let text_bytes = |text_bytes| Options {
        text_bytes,
        buffer_bytes: 3,
        ..Options::DEFAULT
    };
    // The cap holds the section words too: `ISO-10303-21` is 12 bytes.
    let t = data("#1=ABCDEFGHI('efgh',.IJ.,\"0\");");
    assert_eq!(read(&t, text_bytes(16)), None);
    for short in [0, 11, 12, 13, 15] {
        assert_eq!(
            read(&t, text_bytes(short)).unwrap().kind,
            ErrorKind::TooMuchText,
            "{short}"
        );
    }
    // A string of any length is refused after one buffer more than the cap, not held.
    let long = data(&format!("#1=A('{}');", "x".repeat(100_000)));
    let options = Options {
        text_bytes: 1000,
        buffer_bytes: 64,
        ..Options::DEFAULT
    };
    let e = read(&long, options).unwrap();
    assert_eq!(e.kind, ErrorKind::TooMuchText);
    assert!(e.offset < 1200, "{e}");
}

#[test]
fn text_that_is_not_unicode_is_refused_or_replaced_and_counted() {
    let text = b"ISO-10303-21;HEADER;ENDSEC;DATA;#1=A('caf\xe9','\\X2\\D83D\\X0\\');ENDSEC;END-ISO-10303-21;";
    assert_eq!(exchange(text).unwrap_err().kind, ErrorKind::BadText);
    let lossy = Options {
        lossy_text: true,
        ..Options::DEFAULT
    };
    let ex = Exchange::read(SliceSource::new(text), lossy, Totals::DEFAULT, &mut meter()).unwrap();
    assert_eq!(ex.replaced, 2);
    assert_eq!(
        ex.get(1).unwrap().params(),
        [
            Param::Str("caf\u{fffd}".into()),
            Param::Str("\u{fffd}".into())
        ]
    );
}

#[test]
fn a_whole_file_is_held_to_its_totals() {
    let read = |text: &[u8], totals| {
        Exchange::read(
            SliceSource::new(text),
            Options::DEFAULT,
            totals,
            &mut meter(),
        )
        .map_err(|e| e.kind)
    };
    let t = data("#1=A($,($),T(1.));");
    // `$`, the list, its `$`, the typed value and its `1.`: five parameters.
    let params = |params| Totals {
        params,
        ..Totals::DEFAULT
    };
    assert!(read(&t, params(5)).is_ok());
    assert_eq!(read(&t, params(4)), Err(ErrorKind::TooManyParams));
    assert_eq!(
        read(&data("#1=A($,$);#2=A($,$);"), params(3)),
        Err(ErrorKind::TooManyParams)
    );
    // The header's parameters count too.
    let header = b"ISO-10303-21;HEADER;A(1,2,3);ENDSEC;END-ISO-10303-21;";
    assert!(read(header, params(3)).is_ok());
    assert_eq!(read(header, params(2)), Err(ErrorKind::TooManyParams));
    let records = |records| Totals {
        records,
        ..Totals::DEFAULT
    };
    let t = data("#1=(A()B()C());");
    assert!(read(&t, records(3)).is_ok());
    assert_eq!(read(&t, records(2)), Err(ErrorKind::TooManyRecords));
    assert_eq!(
        read(&data("#1=(A()B());#2=C();"), records(2)),
        Err(ErrorKind::TooManyRecords)
    );
    // A repeated id counts as an instance.
    let instances = |instances| Totals {
        instances,
        ..Totals::DEFAULT
    };
    assert!(read(&data("#1=A();#2=A();"), instances(2)).is_ok());
    assert_eq!(
        read(&data("#1=A();#1=A();#1=A();"), instances(2)),
        Err(ErrorKind::TooManyInstances)
    );
    let t = data("#1=A();");
    let bytes = |bytes| Totals {
        bytes,
        ..Totals::DEFAULT
    };
    assert!(read(&t, bytes(t.len() as u64)).is_ok());
    assert_eq!(
        read(&t, bytes(t.len() as u64 - 1)),
        Err(ErrorKind::TooLarge)
    );
    assert_eq!(read(&t, bytes(0)), Err(ErrorKind::TooLarge));
}

#[test]
fn a_repeated_id_keeps_the_first_and_is_reported() {
    let ex = exchange(&data("#1=A(1);#1=B();#2=C();#1=D();")).unwrap();
    assert_eq!(ex.duplicates, vec![1, 1]);
    assert_eq!(ex.get(1).unwrap().name(), "A");
    assert_eq!(ex.entities.len(), 2);
}

#[test]
fn the_header_is_collected_and_its_other_records_kept() {
    let text = b"ISO-10303-21;HEADER;FILE_DESCRIPTION(('a','b',3),'2;1');FILE_NAME('n','t',('x','y'),('o'),'p','s','z');FILE_SCHEMA(('S1','S2'));FILE_POPULATION('g','d',$);FILE_NAME('m');ENDSEC;END-ISO-10303-21;";
    let ex = exchange(text).unwrap();
    assert_eq!(
        ex.header,
        Header {
            descriptions: vec!["a".into(), "b".into()],
            implementation_level: "2;1".into(),
            // The later record replaces the earlier one whole.
            file_name: "m".into(),
            schemas: vec!["S1".into(), "S2".into()],
            other: vec![Record {
                name: "FILE_POPULATION".into(),
                params: vec![Param::Str("g".into()), Param::Str("d".into()), Param::Null],
            }],
            ..Header::default()
        }
    );
    let written = String::from_utf8(ex.write(Vec::new()).unwrap()).unwrap();
    assert_eq!(
        written,
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('a','b'),'2;1');\nFILE_NAME('m','',(),(),'','','');\nFILE_SCHEMA(('S1','S2'));\nFILE_POPULATION('g','d',$);\nENDSEC;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n"
    );
    assert_eq!(exchange(written.as_bytes()).unwrap(), ex);
}

#[test]
fn the_writer_writes_the_canonical_form() {
    let ex = exchange(SAMPLE.as_bytes()).unwrap();
    let written = String::from_utf8(ex.write(Vec::new()).unwrap()).unwrap();
    assert_eq!(
        written,
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\nFILE_NAME('a','',(''),(''),'','sys','');\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN'));\nENDSEC;\nDATA;\n#1=CARTESIAN_POINT('p',(0.0,15.0,-2));\n#2=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));\n#3=X('it''s','\\X2\\00E9\\X0\\',$,#1,LENGTH_MEASURE(0.001),\"0F\");\nENDSEC;\nEND-ISO-10303-21;\n"
    );
    // Write, read, write gives the same bytes, and the same file.
    let back = exchange(written.as_bytes()).unwrap();
    assert_eq!(back, ex);
    assert_eq!(back.write(Vec::new()).unwrap(), written.as_bytes());
    // A complex instance's records are written in the order of their names.
    let mut w = Writer::new(Vec::new());
    w.begin().unwrap();
    w.end_section().unwrap();
    w.data().unwrap();
    let record = |name: &str| Record {
        name: name.into(),
        params: vec![],
    };
    w.instance(5, &[record("B"), record("A"), record("!C")])
        .unwrap();
    w.end_section().unwrap();
    w.data().unwrap();
    w.instance(
        6,
        &[Record {
            name: "N".into(),
            params: vec![
                list(vec![]),
                list(vec![list(vec![Param::Int(1)]), Param::Null]),
                Param::Ref(5),
            ],
        }],
    )
    .unwrap();
    w.end_section().unwrap();
    let out = String::from_utf8(w.finish().unwrap()).unwrap();
    assert_eq!(
        out,
        "ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n#5=(!C()A()B());\nENDSEC;\nDATA;\n#6=N((),((1),$),#5);\nENDSEC;\nEND-ISO-10303-21;\n"
    );
    assert_eq!(events(out.as_bytes()).1, None);
}

#[test]
fn the_writer_refuses_what_part_21_cannot_hold_and_writes_none_of_it() {
    let record = |name: &str, params| Record {
        name: name.into(),
        params,
    };
    let bad: Vec<(Record, WriteError)> = vec![
        (record("", vec![]), WriteError::BadName),
        (record("a", vec![]), WriteError::BadName),
        (record("1A", vec![]), WriteError::BadName),
        (record("A B", vec![]), WriteError::BadName),
        (record("!", vec![]), WriteError::BadName),
        (record("A!", vec![]), WriteError::BadName),
        (
            record("A", vec![Param::Typed("b".into(), List::empty())]),
            WriteError::BadName,
        ),
        (
            record("A", vec![Param::Real(f64::NAN)]),
            WriteError::NonFiniteReal,
        ),
        (
            record(
                "A",
                vec![Param::Int(1), list(vec![Param::Real(f64::INFINITY)])],
            ),
            WriteError::NonFiniteReal,
        ),
        (
            record("A", vec![Param::Enum(String::new())]),
            WriteError::BadEnumeration,
        ),
        (
            record("A", vec![Param::Enum("t".into())]),
            WriteError::BadEnumeration,
        ),
        (
            record("A", vec![Param::Enum("T.".into())]),
            WriteError::BadEnumeration,
        ),
        (
            record("A", vec![Param::Enum("1T".into())]),
            WriteError::BadEnumeration,
        ),
        (
            record("A", vec![Param::Binary(String::new())]),
            WriteError::BadBinary,
        ),
        (
            record("A", vec![Param::Binary("4".into())]),
            WriteError::BadBinary,
        ),
        (
            record("A", vec![Param::Binary("0a".into())]),
            WriteError::BadBinary,
        ),
        (
            record("A", vec![Param::Binary("0\"".into())]),
            WriteError::BadBinary,
        ),
    ];
    for (record, error) in bad {
        let mut w = Writer::new(Vec::new());
        w.begin().unwrap();
        assert_eq!(w.header_record(&record), Err(error), "{record:?}");
        w.end_section().unwrap();
        w.data().unwrap();
        assert_eq!(w.instance(1, core::slice::from_ref(&record)), Err(error));
        assert_eq!(w.instance(1, &[record.clone(), record.clone()]), Err(error));
        // The id was not used up, and nothing of the refused records was written.
        w.instance(
            1,
            &[Record {
                name: "OK".into(),
                params: vec![],
            }],
        )
        .unwrap();
        w.end_section().unwrap();
        assert_eq!(
            String::from_utf8(w.finish().unwrap()).unwrap(),
            "ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n#1=OK();\nENDSEC;\nEND-ISO-10303-21;\n"
        );
        assert!(!error.to_string().is_empty());
    }
}

#[test]
fn the_writer_keeps_the_files_order() {
    let ok = Record {
        name: "A".into(),
        params: vec![],
    };
    let o = Err(WriteError::OutOfOrder);
    let mut w = Writer::new(Vec::new());
    assert_eq!(w.header_record(&ok), o);
    assert_eq!(w.end_section(), o);
    assert_eq!(w.data(), o);
    assert_eq!(w.instance(1, core::slice::from_ref(&ok)), o);
    w.begin().unwrap();
    assert_eq!(w.begin(), o);
    assert_eq!(w.data(), o);
    assert_eq!(w.instance(1, core::slice::from_ref(&ok)), o);
    w.end_section().unwrap();
    assert_eq!(w.end_section(), o);
    assert_eq!(w.header_record(&ok), o);
    w.data().unwrap();
    assert_eq!(w.data(), o);
    assert_eq!(w.instance(3, &[]), Err(WriteError::EmptyInstance));
    w.instance(3, core::slice::from_ref(&ok)).unwrap();
    for id in [0, 2, 3] {
        assert_eq!(
            w.instance(id, core::slice::from_ref(&ok)),
            Err(WriteError::InstanceOrder { id, last: 3 })
        );
    }
    // The order runs through the whole file, not one section.
    w.end_section().unwrap();
    w.data().unwrap();
    assert_eq!(
        w.instance(3, core::slice::from_ref(&ok)),
        Err(WriteError::InstanceOrder { id: 3, last: 3 })
    );
    w.instance(u64::MAX, core::slice::from_ref(&ok)).unwrap();
    let unfinished = Writer::new(Vec::new());
    assert_eq!(unfinished.finish().unwrap_err(), WriteError::OutOfOrder);
    let mut open = Writer::new(Vec::new());
    open.begin().unwrap();
    assert_eq!(open.finish().unwrap_err(), WriteError::OutOfOrder);
    w.end_section().unwrap();
    assert!(w.finish().is_ok());
}

/// A sink that fails after taking `left` calls, and records the sizes it was handed.
#[derive(Debug)]
struct Failing {
    left: usize,
    sizes: Vec<usize>,
}

impl geom_p21::ByteSink for Failing {
    fn put(&mut self, bytes: &[u8]) -> Result<(), u32> {
        self.sizes.push(bytes.len());
        match self.left.checked_sub(1) {
            Some(left) => {
                self.left = left;
                Ok(())
            }
            None => Err(7),
        }
    }
}

#[test]
fn the_writer_hands_the_sink_bounded_pieces_and_reports_its_failure() {
    let record = Record {
        name: "A".into(),
        params: vec![Param::Str("x".repeat(1000))],
    };
    let mut sink = Failing {
        left: usize::MAX,
        sizes: Vec::new(),
    };
    let mut w = Writer::new(&mut sink);
    w.begin().unwrap();
    w.end_section().unwrap();
    w.data().unwrap();
    for id in 1..=1000 {
        w.instance(id, core::slice::from_ref(&record)).unwrap();
    }
    w.end_section().unwrap();
    w.finish().unwrap();
    assert!(sink.sizes.len() > 10);
    assert!(
        sink.sizes.iter().all(|n| *n < (64 << 10) + 1100),
        "{:?}",
        sink.sizes
    );
    let mut sink = Failing {
        left: 0,
        sizes: Vec::new(),
    };
    let mut w = Writer::new(&mut sink);
    w.begin().unwrap();
    w.end_section().unwrap();
    assert_eq!(w.finish().unwrap_err(), WriteError::Sink(7));
    let mut sink = Failing {
        left: 0,
        sizes: Vec::new(),
    };
    let mut w = Writer::new(&mut sink);
    w.begin().unwrap();
    w.end_section().unwrap();
    w.data().unwrap();
    let mut failed = None;
    for id in 1..=100 {
        if let Err(e) = w.instance(id, core::slice::from_ref(&record)) {
            failed = Some(e);
            break;
        }
    }
    assert_eq!(failed, Some(WriteError::Sink(7)));
}

/// A source that fails, or claims more bytes than it was given room for.
struct Broken(Result<usize, SourceError>);

impl ByteSource for Broken {
    fn read(&mut self, _: &mut [u8]) -> Result<usize, SourceError> {
        self.0
    }
}

#[test]
fn a_failing_source_is_its_error() {
    for (source, kind) in [
        (
            Broken(Err(SourceError::Failed(3))),
            ErrorKind::Source(SourceError::Failed(3)),
        ),
        (
            Broken(Ok(usize::MAX)),
            ErrorKind::Source(SourceError::Overrun),
        ),
    ] {
        let e = common::events_of(source, Options::DEFAULT).1.unwrap();
        assert_eq!((e.kind, e.line, e.offset), (kind, 1, 0));
    }
}
