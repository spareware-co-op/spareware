//! The stored form: a sheet and a viewport are element records; one spelling, the same bytes for
//! the same data, and anything writing would not give is refused on reading.

#![allow(
    clippy::unwrap_used,
    clippy::too_many_lines,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_model::element::{Element, Placement, Reference, Want};
use construct_params::Length;
use construct_sheets::{
    Alignment, Anchor, Orientation, Paper, PaperSize, Sheet, SheetError, Sheets, Viewport,
};
use kit_core::Id;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(mm: i32) -> Length {
    Length::from_mm(mm)
}

const SHEET_ROW: &str = concat!(
    r#"{"id":"S-1","document":"DOC-1","kind":"sheet","name":"Ground floor plan","level":0,"#,
    r#""placement":{"x":0,"y":0,"z":0},"#,
    r#""recipe":{"owner":"construct","schema":1,"op":"sheet","op_version":1,"args":{}},"#,
    r#""props":{"@category":"7100","@sheet.number":"A101","#,
    r#""@sheet.orientation":"choice:landscape","@sheet.paper":"choice:a1"},"references":[]}"#
);

const CUSTOM_ROW: &str = concat!(
    r#"{"id":"S-2","document":"DOC-1","kind":"sheet","name":"Detail","level":0,"#,
    r#""placement":{"x":0,"y":0,"z":0},"#,
    r#""recipe":{"owner":"construct","schema":1,"op":"sheet","op_version":1,"args":{}},"#,
    r#""props":{"@category":"7100","@sheet.height":"length:q0:1280000","@sheet.number":"D-7","#,
    r#""@sheet.paper":"choice:custom","@sheet.width":"length:q0:1904000"},"references":[]}"#
);

const VIEWPORT_ROW: &str = concat!(
    r#"{"id":"P-1","document":"DOC-1","kind":"viewport","name":"Viewport","level":0,"#,
    r#""placement":{"x":0,"y":0,"z":0},"#,
    r#""recipe":{"owner":"construct","schema":1,"op":"viewport","op_version":1,"args":{}},"#,
    r#""props":{"@category":"7400","@viewport.align.x":"P-0","#,
    r#""@viewport.align.x.anchor":"choice:centre","@viewport.sheet":"S-1","#,
    r#""@viewport.title":"yesno:no","@viewport.view":"V-1","#,
    r#""@viewport.x":"length:q0:1920000","@viewport.y":"length:q0:1280001"},"references":[]}"#
);

fn row(element: &Element) -> String {
    serde_json::to_string(element).unwrap()
}

fn sheet() -> Sheet {
    Sheet::new(
        id("S-1"),
        "A101",
        "Ground floor plan",
        Paper::Standard {
            size: PaperSize::A1,
            orientation: Orientation::Landscape,
        },
    )
    .unwrap()
}

fn custom() -> Sheet {
    let paper = Paper::Custom {
        width: Length::from_steps(1_904_000),
        height: mm(200),
    };
    Sheet::new(id("S-2"), "D-7", "Detail", paper).unwrap()
}

fn viewport() -> Viewport {
    Viewport::new(
        id("P-1"),
        id("S-1"),
        id("V-1"),
        mm(300),
        Length::from_steps(1_280_001),
    )
    .with_title(false)
    .with_align_x(Some(Alignment {
        to: id("P-0"),
        anchor: Anchor::Centre,
    }))
}

#[test]
fn a_sheet_is_written_as_pinned_and_read_back() {
    let doc = id("DOC-1");
    assert_eq!(row(&sheet().to_record(&doc)), SHEET_ROW);
    assert_eq!(row(&custom().to_record(&doc)), CUSTOM_ROW);
    for (text, want) in [(SHEET_ROW, sheet()), (CUSTOM_ROW, custom())] {
        let record: Element = serde_json::from_str(text).unwrap();
        assert_eq!(Sheet::from_record(&record).unwrap(), want);
    }
}

#[test]
fn a_viewport_is_written_as_pinned_and_read_back() {
    let record: Element = serde_json::from_str(VIEWPORT_ROW).unwrap();
    assert_eq!(row(&viewport().to_record(&id("DOC-1"))), VIEWPORT_ROW);
    assert_eq!(Viewport::from_record(&record).unwrap(), viewport());
    // With nothing aligned the two alignment keys are not there at all.
    let plain = Viewport::new(id("P-2"), id("S-1"), id("V-2"), mm(1), mm(2));
    let record = plain.to_record(&id("DOC-1"));
    assert!(!record.props.keys().any(|key| key.contains("align")));
    assert_eq!(Viewport::from_record(&record).unwrap(), plain);
}

#[test]
fn parameter_values_in_a_sheets_props_are_left_alone_and_kept_by_over() {
    let mut record = sheet().to_record(&id("DOC-1"));
    record
        .props
        .insert("sheet.drawn-by".to_owned(), "text:Cara".to_owned());
    assert_eq!(Sheet::from_record(&record).unwrap(), sheet());
    // A command that renames the sheet writes the record over this one and keeps the values.
    let renamed = sheet().with_name("Level 1 plan").unwrap();
    let over = renamed.over(&record);
    assert_eq!(over.props.get("sheet.drawn-by").unwrap(), "text:Cara");
    assert_eq!(over.name, "Level 1 plan");
    assert_eq!(Sheet::from_record(&over).unwrap(), renamed);
    // A key of ours that writing would not give is still refused.
    record
        .props
        .insert("@sheet.extra".to_owned(), "x".to_owned());
    assert!(Sheet::from_record(&record).is_err());
}

#[test]
fn a_record_that_is_not_as_written_is_refused_naming_it() {
    let bad = |edit: &dyn Fn(&mut Element)| {
        let mut record: Element = serde_json::from_str(SHEET_ROW).unwrap();
        edit(&mut record);
        match Sheet::from_record(&record).unwrap_err() {
            SheetError::Malformed(why) => why,
            other => panic!("{other:?}"),
        }
    };
    assert!(bad(&|r| r.kind = "view".to_owned()).starts_with("The sheet record S-1 "));
    assert!(bad(&|r| r.level = 1).contains("level"));
    assert!(bad(&|r| r.placement = Placement { x: 1, y: 0, z: 0 }).contains("placement"));
    assert!(
        bad(&|r| r.references.push(Reference {
            doc: None,
            element: id("E"),
            name: "n".to_owned(),
            kind: "face".to_owned(),
            want: Want::One,
            signature: None,
        }))
        .contains("references")
    );
    assert!(bad(&|r| r.name = "  ".to_owned()).contains("name"));
    assert!(
        bad(&|r| {
            r.props.remove("@sheet.number");
        })
        .contains("number")
    );
    assert!(
        bad(&|r| {
            r.props
                .insert("@sheet.number".to_owned(), " A101".to_owned());
        })
        .contains("not as a sheet record")
    );
    assert!(
        bad(&|r| {
            r.props
                .insert("@sheet.paper".to_owned(), "choice:a9".to_owned());
        })
        .contains("paper size")
    );
    assert!(
        bad(&|r| {
            r.props.insert("@sheet.paper".to_owned(), "a1".to_owned());
        })
        .contains("paper")
    );
    assert!(
        bad(&|r| {
            r.props
                .insert("@sheet.orientation".to_owned(), "choice:upside".to_owned());
        })
        .contains("orientation")
    );
    assert!(
        bad(&|r| {
            r.props
                .insert("@sheet.width".to_owned(), "length:q0:1".to_owned());
        })
        .contains("not as a sheet record")
    );
    // Old whole-millimetre spelling of a custom length is not accepted: no sheet was written so.
    let mut record: Element = serde_json::from_str(CUSTOM_ROW).unwrap();
    record
        .props
        .insert("@sheet.height".to_owned(), "length:200".to_owned());
    assert!(Sheet::from_record(&record).is_err());
    // A custom paper outside the limits does not read as a sheet.
    record
        .props
        .insert("@sheet.height".to_owned(), "length:q0:1".to_owned());
    assert!(Sheet::from_record(&record).is_err());

    let bad_viewport = |edit: &dyn Fn(&mut Element)| {
        let mut record: Element = serde_json::from_str(VIEWPORT_ROW).unwrap();
        edit(&mut record);
        match Viewport::from_record(&record).unwrap_err() {
            SheetError::Malformed(why) => why,
            other => panic!("{other:?}"),
        }
    };
    assert!(bad_viewport(&|r| r.name = "Other".to_owned()).contains("not as a viewport"));
    assert!(
        bad_viewport(&|r| {
            r.props.remove("@viewport.align.x.anchor");
        })
        .contains("anchor")
    );
    assert!(
        bad_viewport(&|r| {
            r.props.insert(
                "@viewport.align.x.anchor".to_owned(),
                "choice:left".to_owned(),
            );
        })
        .contains("anchor")
    );
    assert!(
        bad_viewport(&|r| {
            r.props.remove("@viewport.sheet");
        })
        .contains("@viewport.sheet")
    );
    assert!(
        bad_viewport(&|r| {
            r.props
                .insert("@viewport.title".to_owned(), "true".to_owned());
        })
        .contains("title")
    );
    assert!(
        bad_viewport(&|r| {
            r.props
                .insert("@viewport.x".to_owned(), "length:q0:+5".to_owned());
        })
        .contains("position")
    );
    assert!(
        bad_viewport(&|r| {
            r.props.insert("@viewport.stray".to_owned(), "1".to_owned());
        })
        .contains("not as a viewport")
    );
}

#[test]
fn a_documents_records_round_trip_in_any_order_and_old_documents_still_read() {
    let doc = id("DOC-1");
    let mut sheets = Sheets::new();
    sheets.add_sheet(sheet()).unwrap();
    sheets.add_sheet(custom()).unwrap();
    sheets
        .add_viewport(Viewport::new(
            id("P-0"),
            id("S-1"),
            id("V-0"),
            mm(10),
            mm(10),
        ))
        .unwrap();
    sheets
        .add_viewport(Viewport::new(
            id("P-1"),
            id("S-1"),
            id("V-1"),
            mm(300),
            mm(100),
        ))
        .unwrap();
    let records = sheets.to_records(&doc);
    assert_eq!(records.len(), 4);
    let mut reversed = records.clone();
    reversed.reverse();
    assert_eq!(Sheets::from_records(&reversed).unwrap(), sheets);
    assert_eq!(
        Sheets::from_records(&records).unwrap().to_records(&doc),
        records
    );
    // A document with no sheets at all (every document made before this format) reads as none,
    // and records of other kinds are passed over.
    let other: Element = serde_json::from_str(
        r#"{"id":"W-1","document":"DOC-1","kind":"wall.straight","name":"w","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"wall.straight","op_version":1,"args":{}},"props":{},"references":[]}"#,
    )
    .unwrap();
    let none = Sheets::from_records([&other]).unwrap();
    assert!(none.is_empty());
    // Two sheets of one number are both read: the rule binds commands, not reading.
    let mut twin = custom().to_record(&doc);
    twin.id = id("S-3");
    let both = Sheets::from_records(records.iter().chain([&twin])).unwrap();
    assert_eq!(both.len(), 3);
    // A malformed record in the set is named.
    let mut broken = records[0].clone();
    broken.level = 5;
    assert!(Sheets::from_records([&broken]).is_err());
}
