//! The stored form: a dimension is an element record; one spelling, the same bytes for the same
//! data, and anything writing would not give is refused on reading. Also the rules a string of
//! references keeps and the lookups a document of dimensions gives.

#![allow(
    clippy::unwrap_used,
    clippy::assert_is_empty,
    clippy::too_many_lines,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_dimensions::{
    Dimension, DimensionError, Dimensions, Direction, Edge, Feature, Layout, Reference, Side,
};
use construct_grids::Point;
use construct_model::element::{Element, Want};
use construct_params::Length;
use kit_core::Id;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(mm: i32) -> Length {
    Length::from_mm(mm)
}

fn face(wall: &str, side: Side) -> Reference {
    Reference::new(id(wall), Feature::WallFace(side))
}

fn row(element: &Element) -> String {
    serde_json::to_string(element).unwrap()
}

fn plain() -> Dimension {
    Dimension::new(
        id("D-1"),
        id("V-1"),
        Layout::horizontal(mm(5000)).unwrap(),
        vec![face("W-1", Side::Exterior), face("W-2", Side::Interior)],
    )
    .unwrap()
}

const PLAIN_ROW: &str = concat!(
    r#"{"id":"D-1","document":"DOC-1","kind":"dimension","name":"Dimension","level":0,"#,
    r#""placement":{"x":0,"y":0,"z":0},"#,
    r#""recipe":{"owner":"construct","schema":1,"op":"dimension","op_version":1,"args":{}},"#,
    r#""props":{"@category":"5000","@dimension.layout":"choice:horizontal","#,
    r#""@dimension.view":"V-1","@dimension.y":"length:q0:32000000"},"#,
    r#""references":[{"doc":null,"element":"W-1","name":"wall.face.exterior","#,
    r#""kind":"dimension.reference","want":"carrier","signature":null},"#,
    r#"{"doc":null,"element":"W-2","name":"wall.face.interior","#,
    r#""kind":"dimension.reference","want":"carrier","signature":null}]}"#
);

#[test]
fn a_dimension_is_written_in_one_spelling_and_read_back() {
    let record = plain().to_record(&id("DOC-1"));
    assert_eq!(row(&record), PLAIN_ROW);
    let read = Dimension::from_record(&record).unwrap();
    assert_eq!(read, plain());
    // The record holds no value: nothing that could be out of date.
    assert!(!row(&record).contains("value"));
}

#[test]
fn every_layout_and_an_equality_round_trip() {
    let refs = vec![
        Reference::new(id("G-1"), Feature::GridLine),
        Reference::new(id("DR-1"), Feature::Edge(Edge::Start)),
        Reference::new(id("DR-1"), Feature::Centre),
        Reference::new(id("DR-1"), Feature::Edge(Edge::End)),
        Reference::new(id("L-1"), Feature::Level),
        Reference::new(id("W-1"), Feature::WallLine),
    ];
    let layouts = [
        Layout::vertical(mm(-250)).unwrap(),
        Layout::aligned(
            Point::new(mm(1), Length::from_steps(80_001)),
            Direction::new(mm(3), mm(-4)).unwrap(),
        )
        .unwrap(),
    ];
    for layout in layouts {
        let dimension = Dimension::new(id("D-9"), id("V-2"), layout, refs.clone())
            .unwrap()
            .with_equalities(vec![vec![0, 1], vec![3, 4]])
            .unwrap();
        let record = dimension.to_record(&id("DOC-1"));
        assert_eq!(
            record.props.get("@dimension.equal").map(String::as_str),
            Some("text:0,1;3,4")
        );
        assert_eq!(Dimension::from_record(&record).unwrap(), dimension);
        // JSON text back to a record gives the same dimension.
        let text = row(&record);
        let again: Element = serde_json::from_str(&text).unwrap();
        assert_eq!(Dimension::from_record(&again).unwrap(), dimension);
    }
}

#[test]
fn reading_refuses_what_writing_would_not_give() {
    let good = plain().to_record(&id("DOC-1"));
    let refused = |edit: &dyn Fn(&mut Element)| {
        let mut record = good.clone();
        edit(&mut record);
        let error = Dimension::from_record(&record).unwrap_err();
        assert!(matches!(error, DimensionError::Malformed(_)), "{error}");
    };
    refused(&|r| r.kind = "viewport".to_owned());
    refused(&|r| r.name = "Other".to_owned());
    refused(&|r| r.level = 1);
    refused(&|r| {
        r.props
            .insert("@dimension.stray".to_owned(), "x".to_owned());
    });
    refused(&|r| {
        r.props
            .insert("@dimension.y".to_owned(), "length:5000".to_owned());
    });
    refused(&|r| {
        r.props.remove("@dimension.view");
    });
    refused(&|r| {
        r.props
            .insert("@dimension.layout".to_owned(), "choice:diagonal".to_owned());
    });
    refused(&|r| {
        r.props
            .insert("@dimension.equal".to_owned(), "text:0,9".to_owned());
    });
    refused(&|r| {
        r.props
            .insert("@dimension.equal".to_owned(), "text:0,x".to_owned());
    });
    refused(&|r| r.references[0].want = Want::One);
    refused(&|r| r.references[0].name = "wall.face.top".to_owned());
    refused(&|r| r.references[0].signature = Some("old".to_owned()));
    refused(&|r| {
        r.references.pop();
    });
    refused(&|r| r.references[1] = r.references[0].clone());
}

#[test]
fn a_string_keeps_at_least_two_references_and_no_neighbours_the_same() {
    let layout = Layout::horizontal(mm(0)).unwrap();
    let one = Dimension::new(
        id("D-1"),
        id("V-1"),
        layout,
        vec![face("W-1", Side::Exterior)],
    );
    assert_eq!(one.unwrap_err(), DimensionError::TooFewReferences(1));
    let same = Dimension::new(
        id("D-1"),
        id("V-1"),
        layout,
        vec![
            face("W-1", Side::Exterior),
            face("W-2", Side::Exterior),
            face("W-2", Side::Exterior),
        ],
    );
    assert_eq!(same.unwrap_err(), DimensionError::RepeatedReference(1));
    let many: Vec<Reference> = (0..65)
        .map(|n| face(&format!("W-{n}"), Side::Exterior))
        .collect();
    assert_eq!(
        Dimension::new(id("D-1"), id("V-1"), layout, many).unwrap_err(),
        DimensionError::TooManyReferences(65)
    );
    // The same wall twice is fine when something lies between.
    assert!(
        Dimension::new(
            id("D-1"),
            id("V-1"),
            layout,
            vec![
                face("W-1", Side::Exterior),
                face("W-1", Side::Interior),
                face("W-1", Side::Exterior),
            ],
        )
        .is_ok()
    );
}

#[test]
fn references_are_added_and_taken_away_by_position() {
    let dimension = plain();
    let third = Reference::new(id("G-1"), Feature::GridLine);
    let longer = dimension
        .clone()
        .with_reference_added(1, third.clone())
        .unwrap();
    assert_eq!(longer.references()[1], third);
    assert_eq!(longer.segments(), 2);
    let last = dimension
        .clone()
        .with_reference_added(2, third.clone())
        .unwrap();
    assert_eq!(last.references()[2], third);
    assert_eq!(
        dimension
            .clone()
            .with_reference_added(3, third.clone())
            .unwrap_err(),
        DimensionError::NoSuchReference(3)
    );
    assert_eq!(longer.clone().with_reference_removed(1).unwrap(), dimension);
    assert_eq!(
        dimension.clone().with_reference_removed(0).unwrap_err(),
        DimensionError::TooFewReferences(1)
    );
    assert_eq!(
        dimension.clone().with_reference_removed(5).unwrap_err(),
        DimensionError::NoSuchReference(5)
    );
    // Adding a neighbour the same is refused.
    assert_eq!(
        dimension
            .clone()
            .with_reference_added(1, dimension.references()[0].clone())
            .unwrap_err(),
        DimensionError::RepeatedReference(0)
    );
}

#[test]
fn segments_held_equal_keep_their_references() {
    let three = plain()
        .with_reference_added(2, Reference::new(id("G-1"), Feature::GridLine))
        .unwrap()
        .with_equalities(vec![vec![0, 1]])
        .unwrap();
    assert_eq!(
        three
            .clone()
            .with_reference_added(0, Reference::new(id("G-2"), Feature::GridLine))
            .unwrap_err(),
        DimensionError::HasEqualities
    );
    assert_eq!(
        three.clone().with_reference_removed(2).unwrap_err(),
        DimensionError::HasEqualities
    );
    // Clearing the equalities frees them.
    let free = three.with_equalities(Vec::new()).unwrap();
    assert!(free.with_reference_removed(2).is_ok());
}

#[test]
fn equalities_name_real_segments_once_each() {
    let four = Dimension::new(
        id("D-1"),
        id("V-1"),
        Layout::horizontal(mm(0)).unwrap(),
        (0..5)
            .map(|n| face(&format!("W-{n}"), Side::Exterior))
            .collect(),
    )
    .unwrap();
    assert_eq!(four.segments(), 4);
    let bad = |groups: Vec<Vec<usize>>| four.clone().with_equalities(groups).unwrap_err();
    assert!(matches!(bad(vec![vec![0]]), DimensionError::BadEquality(_)));
    assert!(matches!(
        bad(vec![vec![1, 1]]),
        DimensionError::BadEquality(_)
    ));
    assert!(matches!(
        bad(vec![vec![2, 1]]),
        DimensionError::BadEquality(_)
    ));
    assert!(matches!(
        bad(vec![vec![0, 4]]),
        DimensionError::BadEquality(_)
    ));
    assert!(matches!(
        bad(vec![vec![0, 1], vec![1, 2]]),
        DimensionError::BadEquality(_)
    ));
    assert!(matches!(
        bad(vec![vec![2, 3], vec![0, 1]]),
        DimensionError::BadEquality(_)
    ));
    assert!(
        four.clone()
            .with_equalities(vec![vec![0, 1], vec![2, 3]])
            .is_ok()
    );
    assert!(four.with_equalities(vec![vec![0, 3]]).is_ok());
}

#[test]
fn the_offset_line_moves_by_its_own_layout() {
    let dimension = plain();
    let moved = dimension.layout().with_line(mm(7000)).unwrap();
    assert_eq!(moved, Layout::horizontal(mm(7000)).unwrap());
    assert!(matches!(
        dimension.layout().with_through(Point::mm(0, 0)),
        Err(DimensionError::WrongLayout(_))
    ));
    let aligned = Layout::aligned(Point::mm(0, 0), Direction::new(mm(1), mm(0)).unwrap()).unwrap();
    assert!(matches!(
        aligned.with_line(mm(1)),
        Err(DimensionError::WrongLayout(_))
    ));
    assert_eq!(
        aligned.with_through(Point::mm(10, 20)).unwrap(),
        Layout::aligned(Point::mm(10, 20), Direction::new(mm(1), mm(0)).unwrap()).unwrap()
    );
    assert_eq!(
        Direction::new(mm(0), mm(0)).unwrap_err(),
        DimensionError::ZeroDirection
    );
    let past = Length::from_steps(1_099_511_627_776);
    assert!(matches!(
        Layout::horizontal(past),
        Err(DimensionError::OutOfRange(_))
    ));
    assert!(matches!(
        Layout::aligned(
            Point::new(past, mm(0)),
            Direction::new(mm(1), mm(0)).unwrap()
        ),
        Err(DimensionError::OutOfRange(_))
    ));
    assert!(matches!(
        Direction::new(past, mm(0)),
        Err(DimensionError::OutOfRange(_))
    ));
}

#[test]
fn a_document_of_dimensions_finds_by_view_and_by_element() {
    let other = Dimension::new(
        id("D-2"),
        id("V-2"),
        Layout::vertical(mm(0)).unwrap(),
        vec![face("W-2", Side::Exterior), face("W-3", Side::Exterior)],
    )
    .unwrap();
    let mut all = Dimensions::new();
    all.add(plain()).unwrap();
    all.add(other.clone()).unwrap();
    assert_eq!(
        all.add(plain()).unwrap_err(),
        DimensionError::Duplicate(id("D-1"))
    );
    assert_eq!(all.all().len(), 2);
    assert_eq!(all.of_view(&id("V-2")), vec![&other]);
    let naming: Vec<&Id> = all.referencing(&id("W-2")).iter().map(|d| d.id()).collect();
    assert_eq!(naming, vec![&id("D-1"), &id("D-2")]);
    assert!(all.referencing(&id("W-9")).is_empty());
    let moved = plain().with_layout(Layout::horizontal(mm(1)).unwrap());
    all.replace(moved.clone()).unwrap();
    assert_eq!(all.get(&id("D-1")), Some(&moved));
    assert_eq!(
        all.replace(
            Dimension::new(
                id("D-7"),
                id("V-2"),
                Layout::vertical(mm(0)).unwrap(),
                other.references().to_vec()
            )
            .unwrap()
        )
        .unwrap_err(),
        DimensionError::Unknown(id("D-7"))
    );
    assert_eq!(all.remove(&id("D-2")).unwrap(), other);
    assert_eq!(
        all.remove(&id("D-2")).unwrap_err(),
        DimensionError::Unknown(id("D-2"))
    );
    // From records: the dimensions among every kind of record.
    let records = [plain().to_record(&id("DOC-1")), {
        let mut wall = plain().to_record(&id("DOC-1"));
        wall.id = id("W-1");
        wall.kind = "wall.straight".to_owned();
        wall
    }];
    let found = Dimensions::from_records(records.iter().take(1)).unwrap();
    assert_eq!(found.all().len(), 1);
    let mut broken = records[0].clone();
    broken.name = "x".to_owned();
    assert!(Dimensions::from_records([&broken]).is_err());
}

#[test]
fn a_feature_is_named_by_its_key_and_every_key_is_distinct() {
    let mut keys: Vec<&str> = Feature::ALL.iter().map(|f| f.key()).collect();
    let sorted = {
        let mut copy = keys.clone();
        copy.sort_unstable();
        copy
    };
    assert_eq!(keys, sorted, "ALL is in key order");
    keys.dedup();
    assert_eq!(keys.len(), Feature::ALL.len());
    for feature in Feature::ALL {
        assert_eq!(Feature::from_key(feature.key()), Some(feature));
    }
    assert_eq!(Feature::from_key("wall.face.top"), None);
    assert_eq!(
        face("W-1", Side::Interior).describe(),
        "the interior face of W-1"
    );
}
