//! The stored form: a view is an element record of kind `view`; one spelling, the same bytes for
//! the same view, and anything writing would not give is refused on reading.

#![allow(
    clippy::unwrap_used,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_levels::{Tie, tied_to};
use construct_model::element::{Element, Placement, Reference, Want};
use construct_params::Length;
use construct_phases::{DesignOptions, ElementStaging, PhaseFilter, PhaseList, ViewSettings};
use construct_views::{
    AreaPlan, CATEGORY_KEY, CutLine, DetailLevel, PlanSetup, Point, RANGE_KEYS, Scale, Side,
    VIEW_KIND, View, ViewError, ViewKind, ViewRange, Views,
};
use kit_core::Id;

const PLAN_ROW: &str = concat!(
    r#"{"id":"V-1","document":"DOC-1","kind":"view","name":"Ground floor plan","level":0,"#,
    r#""placement":{"x":0,"y":0,"z":0},"#,
    r#""recipe":{"owner":"construct","schema":1,"op":"view","op_version":1,"args":{}},"#,
    r#""props":{"@category":"7000","@level":"L-1","@level.offset":"length:q0:0","#,
    r#""@staging":"{\"created\":\"ph-a\",\"demolished\":null,\"design_option\":null}","#,
    r#""@view.detail":"choice:medium","@view.kind":"choice:floor_plan","#,
    r#""@view.range.bottom":"L-1","@view.range.bottom.offset":"length:q0:0","#,
    r#""@view.range.cut":"L-1","@view.range.cut.offset":"length:q0:7680000","#,
    r#""@view.range.depth":"L-1","@view.range.depth.offset":"length:q0:-3200000","#,
    r#""@view.range.top":"L-1","@view.range.top.offset":"length:q0:14720000","#,
    r#""@view.scale":"whole:100"},"references":[]}"#
);

const SECTION_ROW: &str = concat!(
    r#"{"id":"V-2","document":"DOC-1","kind":"view","name":"Section A","level":0,"#,
    r#""placement":{"x":0,"y":0,"z":0},"#,
    r#""recipe":{"owner":"construct","schema":1,"op":"view","op_version":1,"args":{}},"#,
    r#""props":{"@category":"7000","#,
    r#""@staging":"{\"created\":\"ph-a\",\"demolished\":null,\"design_option\":null}","#,
    r#""@view.detail":"choice:fine","@view.far":"length:q0:32000000","@view.kind":"choice:section","#,
    r#""@view.line.x0":"length:q0:0","@view.line.x1":"length:q0:64000000","#,
    r#""@view.line.y0":"length:q0:0","@view.line.y1":"length:q0:0","@view.look":"choice:left","#,
    r#""@view.phasing":"{\"phase\":\"ph-b\",\"filter\":\"show_new\",\"choices\":[]}","#,
    r#""@view.scale":"whole:50"},"references":[]}"#
);

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(mm: i32) -> Length {
    Length::from_mm(mm)
}

fn staging() -> ElementStaging {
    let phases = PhaseList::new()
        .add(id("ph-a"), "Existing")
        .unwrap()
        .add(id("ph-b"), "New works")
        .unwrap();
    ElementStaging::new(&phases, &DesignOptions::new(), id("ph-a"), None, None).unwrap()
}

fn plan() -> View {
    View::new(
        id("V-1"),
        "Ground floor plan",
        Scale::new(100).unwrap(),
        DetailLevel::Medium,
        ViewKind::FloorPlan(PlanSetup {
            level: id("L-1"),
            range: ViewRange::on_level(&id("L-1"), mm(2300), mm(1200), mm(0), mm(-500)),
        }),
        staging(),
    )
    .unwrap()
}

fn line() -> CutLine {
    CutLine::new(
        Point { x: mm(0), y: mm(0) },
        Point {
            x: Length::from_steps(64_000_000),
            y: mm(0),
        },
        Side::Left,
        mm(5000),
    )
    .unwrap()
}

fn section() -> View {
    View::new(
        id("V-2"),
        "Section A",
        Scale::new(50).unwrap(),
        DetailLevel::Fine,
        ViewKind::Section(line()),
        staging(),
    )
    .unwrap()
    .with_phasing(Some(ViewSettings {
        phase: id("ph-b"),
        filter: PhaseFilter::ShowNew,
        choices: Vec::new(),
    }))
}

/// A view's record, byte for byte, and back.
#[test]
fn view_record_bytes_pinned() {
    for (view, row) in [(plan(), PLAN_ROW), (section(), SECTION_ROW)] {
        let record = view.to_record(&id("DOC-1"));
        assert_eq!(record.kind, VIEW_KIND);
        assert_eq!(serde_json::to_string(&record).unwrap(), row);
        let read: Element = serde_json::from_str(row).unwrap();
        assert_eq!(View::from_record(&read).unwrap(), view);
    }
}

#[test]
fn every_kind_round_trips() {
    let line = line();
    let range = ViewRange::on_level(&id("L-1"), mm(2300), mm(1200), mm(0), mm(-500));
    let kinds = [
        ViewKind::FloorPlan(PlanSetup {
            level: id("L-2"),
            range: range.clone(),
        }),
        ViewKind::CeilingPlan(PlanSetup {
            level: id("L-1"),
            range,
        }),
        ViewKind::Section(line),
        ViewKind::Elevation(line),
        ViewKind::ThreeD,
        ViewKind::Drafting,
        ViewKind::Legend,
    ];
    for kind in kinds {
        let view = View::new(
            id("V-9"),
            "A view",
            Scale::new(200).unwrap(),
            DetailLevel::Coarse,
            kind,
            staging(),
        )
        .unwrap();
        let record = view.to_record(&id("DOC-1"));
        assert_eq!(record.props.get(CATEGORY_KEY).unwrap(), "7000");
        assert_eq!(View::from_record(&record).unwrap(), view);
    }
}

#[test]
fn off_grid_values_survive() {
    // A cut plane at 12.5 mm and a section 7.25 mm deep: neither is a whole millimetre.
    let mut view = plan();
    if let ViewKind::FloorPlan(setup) = view.kind().clone() {
        let mut setup = setup;
        setup.range.cut.offset = Length::from_steps(80_000);
        view = view.with_kind(ViewKind::FloorPlan(setup));
    }
    let record = view.to_record(&id("DOC-1"));
    assert_eq!(
        record.props.get("@view.range.cut.offset").unwrap(),
        "length:q0:80000"
    );
    assert_eq!(View::from_record(&record).unwrap(), view);
}

fn read_changed(change: impl FnOnce(&mut Element)) -> Result<View, ViewError> {
    let mut record = plan().to_record(&id("DOC-1"));
    change(&mut record);
    View::from_record(&record)
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "reads as a call on the result"
)]
fn refused(result: Result<View, ViewError>) {
    assert!(matches!(result, Err(ViewError::Malformed(_))), "{result:?}");
}

#[test]
fn a_record_that_is_not_as_written_is_refused_never_repaired() {
    read_changed(|_| {}).unwrap();
    refused(read_changed(|r| r.kind = "level".to_owned()));
    refused(read_changed(|r| r.recipe.op_version = 2));
    refused(read_changed(|r| r.level = 3));
    refused(read_changed(|r| {
        r.placement = Placement { x: 1, y: 0, z: 0 }
    }));
    refused(read_changed(|r| {
        r.references.push(Reference {
            doc: None,
            element: id("x"),
            name: "n".to_owned(),
            kind: "face".to_owned(),
            want: Want::One,
            signature: None,
        });
    }));
    refused(read_changed(|r| r.name = "  ".to_owned()));
    refused(read_changed(|r| {
        r.props.insert("@extra".to_owned(), "1".to_owned());
    }));
    refused(read_changed(|r| {
        r.props.insert(CATEGORY_KEY.to_owned(), "7100".to_owned());
    }));
    for key in [
        "@view.kind",
        "@view.scale",
        "@view.detail",
        "@staging",
        "@level",
        "@level.offset",
        "@view.range.top",
        "@view.range.depth.offset",
    ] {
        refused(read_changed(|r| {
            r.props.remove(key);
        }));
    }
}

#[test]
fn a_value_not_in_its_one_spelling_is_refused() {
    let set = |key: &'static str, value: &'static str| {
        read_changed(move |r| {
            r.props.insert(key.to_owned(), value.to_owned());
        })
    };
    refused(set("@view.scale", "whole:0"));
    refused(set("@view.scale", "whole:1000001"));
    refused(set("@view.scale", "whole:-5"));
    refused(set("@view.scale", "whole:0100"));
    refused(set("@view.scale", "number:100"));
    refused(set("@view.detail", "choice:ultra"));
    refused(set("@view.kind", "choice:perspective"));
    refused(set("@view.kind", "floor_plan"));
    // The old whole-millimetre spelling reads as the length it is, but is not the one spelling.
    refused(set("@view.range.cut.offset", "length:1200"));
    refused(set("@view.range.cut.offset", "length:q0:01"));
    refused(set("@view.range.cut.offset", "number:1"));
    refused(set("@view.range.cut", "not an id"));
    refused(set("@staging", "{}"));
    refused(set(
        "@staging",
        r#"{"demolished":null,"created":"ph-a","design_option":null}"#,
    ));
    refused(set("@level.offset", "length:q0:640"));
}

#[test]
fn a_section_line_and_phase_settings_are_read_strictly() {
    let changed = |change: fn(&mut Element)| {
        let mut record = section().to_record(&id("DOC-1"));
        change(&mut record);
        View::from_record(&record)
    };
    assert!(changed(|_| {}).is_ok());
    refused(changed(|r| {
        r.props
            .insert("@view.line.x1".to_owned(), "length:q0:0".to_owned());
    }));
    refused(changed(|r| {
        r.props
            .insert("@view.far".to_owned(), "length:q0:0".to_owned());
    }));
    refused(changed(|r| {
        r.props
            .insert("@view.look".to_owned(), "choice:up".to_owned());
    }));
    refused(changed(|r| {
        r.props.remove("@view.line.y0");
    }));
    refused(changed(|r| {
        r.props.insert("@view.phasing".to_owned(), "{}".to_owned());
    }));
    refused(changed(|r| {
        r.props.insert(
            "@view.phasing".to_owned(),
            r#"{"phase":"ph-b","filter":"show_new","choices":[],"x":1}"#.to_owned(),
        );
    }));
    // A section holds no level.
    refused(changed(|r| {
        r.props.insert("@level".to_owned(), "L-1".to_owned());
        r.props
            .insert("@level.offset".to_owned(), "length:q0:0".to_owned());
    }));
}

#[test]
fn a_plan_stands_on_its_level_as_an_element_does() {
    let record = plan().to_record(&id("DOC-1"));
    assert_eq!(tied_to(&id("L-1"), [&record]).unwrap(), 1);
    assert_eq!(tied_to(&id("L-2"), [&record]).unwrap(), 0);
    let record = section().to_record(&id("DOC-1"));
    assert_eq!(tied_to(&id("L-1"), [&record]).unwrap(), 0);
}

/// A level a view's range is tied to is counted, so it is not deleted behind the view's back; a
/// plan tied to a level twice (its own and its range) counts once.
#[test]
fn a_level_a_views_range_is_tied_to_counts_as_tied() {
    let top = Tie {
        level: id("L-2"),
        offset: mm(0),
    };
    let on_two = View::new(
        id("V-3"),
        "Ground floor, seen from the roof",
        Scale::new(100).unwrap(),
        DetailLevel::Medium,
        ViewKind::FloorPlan(PlanSetup {
            level: id("L-1"),
            range: ViewRange {
                top,
                ..ViewRange::on_level(&id("L-1"), mm(2300), mm(1200), mm(0), mm(-500))
            },
        }),
        staging(),
    )
    .unwrap()
    .to_record(&id("DOC-1"));
    assert_eq!(tied_to(&id("L-2"), [&on_two]).unwrap(), 1);
    assert_eq!(tied_to(&id("L-1"), [&on_two]).unwrap(), 1);
    assert_eq!(tied_to(&id("L-3"), [&on_two]).unwrap(), 0);
    // The keys are construct-levels' own list: one list, so the two cannot drift.
    assert_eq!(RANGE_KEYS, construct_levels::RANGE_TIES);
}

#[test]
fn views_read_from_a_documents_records_in_any_order() {
    let document = id("DOC-1");
    let a = plan().to_record(&document);
    let b = section().to_record(&document);
    let mut wall = a.clone();
    wall.id = id("W-1");
    wall.kind = "wall.straight".to_owned();
    let forward = Views::from_records([&a, &wall, &b]).unwrap();
    let backward = Views::from_records([&b, &wall, &a]).unwrap();
    assert_eq!(forward, backward);
    assert_eq!(forward.len(), 2);
    assert_eq!(forward.to_records(&document), vec![a.clone(), b]);
    let mut bad = a;
    bad.props.remove("@view.scale");
    assert!(matches!(
        Views::from_records([&bad]),
        Err(ViewError::Malformed(_))
    ));
}

#[test]
fn the_same_view_is_the_same_bytes_every_time() {
    let one = serde_json::to_vec(&section().to_record(&id("DOC-1"))).unwrap();
    let two = serde_json::to_vec(&section().to_record(&id("DOC-1"))).unwrap();
    assert_eq!(one, two);
}

/// An area plan is a plan of a level with the area scheme it shows: stored beside the plan's own
/// keys, read back as written, and its level tie counted like a plan's.
#[test]
fn an_area_plan_holds_its_scheme_and_reads_back_as_written() {
    let view = View::new(
        id("V-3"),
        "Ground gross areas",
        Scale::new(100).unwrap(),
        DetailLevel::Medium,
        ViewKind::AreaPlan(AreaPlan {
            plan: PlanSetup {
                level: id("L-1"),
                range: ViewRange::on_level(&id("L-1"), mm(2300), mm(1200), mm(0), mm(-500)),
            },
            scheme: id("S-gross"),
        }),
        staging(),
    )
    .unwrap();
    let record = view.to_record(&id("DOC-1"));
    assert_eq!(
        record.props.get("@area.scheme").map(String::as_str),
        Some("S-gross")
    );
    assert_eq!(
        record.props.get("@view.kind").map(String::as_str),
        Some("choice:area_plan")
    );
    assert_eq!(View::from_record(&record).unwrap(), view);
    assert_eq!(
        view.kind().plan().map(|plan| plan.level.clone()),
        Some(id("L-1"))
    );
    assert_eq!(view.kind().area_scheme(), Some(&id("S-gross")));
    assert_eq!(plan().kind().area_scheme(), None);
    // Without its scheme the record is refused, never repaired.
    let mut bare = record;
    bare.props.remove("@area.scheme");
    assert!(View::from_record(&bare).is_err());
}
