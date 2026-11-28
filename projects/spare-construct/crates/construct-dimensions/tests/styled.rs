//! The stored form of the new kinds, of dimension styles and of overrides: each written once in
//! one spelling, read back to the same value, and anything the writer would not write refused.
//! A linear dimension of piece A81 keeps its bytes: none of the new keys is written for it.

#![allow(
    clippy::unwrap_used,
    clippy::too_many_lines,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_dimensions::{
    ARROWS_ID, Dimension, DimensionError, DimensionStyle, Direction, Feature, Layout, Look,
    MAX_OVERRIDE, Marker, Overrides, Ray, Reference, STYLE_KIND, Styles, TICKS_ID, TextPlace,
    is_builtin,
};
use construct_model::element::Element;
use construct_params::{Format, Length, Unit, Units};
use kit_core::Id;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(mm: i32) -> Length {
    Length::from_mm(mm)
}

fn grid(name: &str) -> Reference {
    Reference::new(id(name), Feature::GridLine)
}

fn json(record: &Element) -> String {
    serde_json::to_string(record).unwrap()
}

#[test]
fn each_new_kind_is_one_spelling_and_reads_back() {
    let doc = id("DOC-1");
    let angular = Dimension::new(
        id("D-1"),
        id("V-1"),
        Layout::angular(Ray::Backward, Ray::Forward, mm(800)).unwrap(),
        vec![grid("G-1"), grid("G-2")],
    )
    .unwrap();
    let record = angular.to_record(&doc);
    assert_eq!(
        json(&record),
        r#"{"id":"D-1","document":"DOC-1","kind":"dimension","name":"Dimension","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"dimension","op_version":1,"args":{}},"props":{"@category":"5000","@dimension.first":"choice:backward","@dimension.layout":"choice:angular","@dimension.radius":"length:q0:5120000","@dimension.second":"choice:forward","@dimension.view":"V-1"},"references":[{"doc":null,"element":"G-1","name":"grid.line","kind":"dimension.reference","want":"carrier","signature":null},{"doc":null,"element":"G-2","name":"grid.line","kind":"dimension.reference","want":"carrier","signature":null}]}"#
    );
    assert_eq!(Dimension::from_record(&record).unwrap(), angular);
    let toward = Direction::new(mm(3), mm(-4)).unwrap();
    let styled = Dimension::new(
        id("D-2"),
        id("V-1"),
        Layout::diameter(toward),
        vec![grid("G-R")],
    )
    .unwrap()
    .with_style(Some(id(ARROWS_ID)))
    .with_overrides(
        Overrides::new(None, Some("Ø".to_owned()), Some("typical".to_owned())).unwrap(),
    );
    let record = styled.to_record(&doc);
    let props: Vec<(&str, &str)> = record
        .props
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    assert_eq!(
        props,
        [
            ("@category", "5000"),
            ("@dimension.layout", "choice:diameter"),
            ("@dimension.prefix", "text:Ø"),
            ("@dimension.style", "dimension-style.arrows"),
            ("@dimension.suffix", "text:typical"),
            ("@dimension.toward.x", "length:q0:19200"),
            ("@dimension.toward.y", "length:q0:-25600"),
            ("@dimension.view", "V-1"),
        ]
    );
    assert_eq!(Dimension::from_record(&record).unwrap(), styled);
    let ordinate = Dimension::ordinate(
        id("D-3"),
        id("V-1"),
        Layout::vertical(mm(-500)).unwrap(),
        vec![grid("G-1"), grid("G-2"), grid("G-3")],
    )
    .unwrap();
    let record = ordinate.to_record(&doc);
    assert_eq!(record.props["@dimension.ordinate"], "yesno:yes");
    assert_eq!(Dimension::from_record(&record).unwrap(), ordinate);
    let arc = Dimension::new(
        id("D-4"),
        id("V-1"),
        Layout::arc_length(mm(-250)).unwrap(),
        vec![grid("G-R")],
    )
    .unwrap();
    let record = arc.to_record(&doc);
    assert_eq!(record.props["@dimension.offset"], "length:q0:-1600000");
    assert_eq!(Dimension::from_record(&record).unwrap(), arc);
    // A linear dimension writes none of the new keys.
    let linear = Dimension::new(
        id("D-5"),
        id("V-1"),
        Layout::horizontal(mm(10)).unwrap(),
        vec![grid("G-1"), grid("G-2")],
    )
    .unwrap();
    let written = linear.to_record(&doc);
    let keys: Vec<&String> = written.props.keys().collect();
    assert_eq!(
        keys,
        [
            "@category",
            "@dimension.layout",
            "@dimension.view",
            "@dimension.y"
        ]
    );
}

#[test]
fn a_record_the_writer_would_not_write_is_refused() {
    let doc = id("DOC-1");
    let good = Dimension::new(
        id("D-1"),
        id("V-1"),
        Layout::radial(Direction::new(mm(1), mm(0)).unwrap()),
        vec![grid("G-R")],
    )
    .unwrap()
    .to_record(&doc);
    let refused = |change: &dyn Fn(&mut Element)| {
        let mut record = good.clone();
        change(&mut record);
        Dimension::from_record(&record).unwrap_err()
    };
    let malformed = |e: DimensionError| matches!(e, DimensionError::Malformed(_));
    assert!(malformed(refused(&|r| {
        r.props
            .insert("@dimension.ordinate".to_owned(), "yesno:yes".to_owned());
    })));
    assert!(malformed(refused(&|r| {
        r.props
            .insert("@dimension.ordinate".to_owned(), "yesno:no".to_owned());
    })));
    assert!(malformed(refused(&|r| {
        r.props
            .insert("@dimension.text".to_owned(), "text: wide".to_owned());
    })));
    assert!(malformed(refused(&|r| {
        r.props
            .insert("@dimension.style".to_owned(), "not an id".to_owned());
    })));
    assert!(malformed(refused(&|r| {
        r.props
            .insert("@dimension.radius".to_owned(), "length:q0:1".to_owned());
    })));
    assert!(malformed(refused(&|r| {
        r.props
            .insert("@dimension.toward.x".to_owned(), "length:q0:0".to_owned());
        r.props
            .insert("@dimension.toward.y".to_owned(), "length:q0:0".to_owned());
    })));
    // A radius has one reference.
    assert!(malformed(refused(&|r| {
        let again = r.references[0].clone();
        r.references.push(again);
    })));
    assert!(malformed(refused(&|r| {
        r.props
            .insert("@dimension.layout".to_owned(), "choice:spiral".to_owned());
    })));
}

#[test]
fn overrides_keep_one_spelling_and_the_true_value_is_shown_when_asked() {
    let none = Overrides::none();
    assert!(none.is_none());
    assert_eq!(none.apply("2400 mm"), "2400 mm");
    let around = Overrides::new(None, Some("about".to_owned()), Some("clear".to_owned())).unwrap();
    assert_eq!(around.apply("2400 mm"), "about 2400 mm clear");
    let replaced =
        Overrides::new(Some("VARIES".to_owned()), None, Some("(site)".to_owned())).unwrap();
    assert_eq!(replaced.apply("2400 mm"), "VARIES (site)");
    assert_eq!(replaced.text(), Some("VARIES"));
    for bad in ["", " lead", "trail ", "two\nlines"] {
        assert!(matches!(
            Overrides::new(Some(bad.to_owned()), None, None),
            Err(DimensionError::BadOverride(_))
        ));
    }
    let long = "x".repeat(MAX_OVERRIDE + 1);
    assert!(Overrides::new(None, Some(long), None).is_err());
    assert!(Overrides::new(None, Some("x".repeat(MAX_OVERRIDE)), None).is_ok());
}

#[test]
fn the_built_in_styles_are_there_and_cannot_be_changed() {
    let builtin = DimensionStyle::builtin();
    let names: Vec<&str> = builtin.iter().map(DimensionStyle::name).collect();
    assert_eq!(names, ["Ticks", "Arrows"]);
    assert_eq!(builtin[0].id().as_str(), TICKS_ID);
    assert_eq!(builtin[1].look().marker, Marker::Arrow);
    assert!(builtin.iter().all(DimensionStyle::is_builtin));
    assert!(is_builtin(&id(TICKS_ID)) && !is_builtin(&id("S-1")));
    assert!(matches!(
        builtin[0].renamed("Mine"),
        Err(DimensionError::BadStyle(_))
    ));
    assert!(matches!(
        DimensionStyle::new(id(ARROWS_ID), "Mine", Look::ticks()),
        Err(DimensionError::BadStyle(_))
    ));
    // The first is the look of a dimension that names none, or one that is not there.
    let styles = Styles::new();
    assert_eq!(styles.look_of(None), Look::ticks());
    assert_eq!(styles.look_of(Some(&id("S-gone"))), Look::ticks());
    assert_eq!(styles.look_of(Some(&id(ARROWS_ID))).marker, Marker::Arrow);
    let ticks = Look::ticks();
    assert_eq!(
        (ticks.gap, ticks.extension, ticks.height),
        (Length::from_steps(9_600), mm(2), Length::from_steps(16_000))
    );
}

#[test]
fn a_persons_own_style_is_a_record_with_one_spelling() {
    let look = Look {
        marker: Marker::Dot,
        text: TextPlace::Inline,
        length: Some(Format {
            unit: Unit::Metre,
            places: 3,
        }),
        angle_places: Some(0),
        gap: mm(1),
        extension: mm(3),
        height: mm(3),
        centre_mark: false,
    };
    let style = DimensionStyle::new(id("S-1"), "Site, metres", look).unwrap();
    let record = style.to_record(&id("DOC-1"));
    assert_eq!(record.kind, STYLE_KIND);
    assert_eq!(
        json(&record),
        r#"{"id":"S-1","document":"DOC-1","kind":"dimension.style","name":"Site, metres","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"dimension.style","op_version":1,"args":{}},"props":{"@category":"5000","@dimension.style.angle.places":"whole:0","@dimension.style.centre":"yesno:no","@dimension.style.extension":"length:q0:19200","@dimension.style.gap":"length:q0:6400","@dimension.style.height":"length:q0:19200","@dimension.style.length.places":"whole:3","@dimension.style.length.unit":"choice:m","@dimension.style.marker":"choice:dot","@dimension.style.text":"choice:inline"},"references":[]}"#
    );
    assert_eq!(DimensionStyle::from_record(&record).unwrap(), style);
    // Its units are the project's with its own lengths and angles.
    let units = look.units(&Units::metric());
    assert_eq!(
        units.format(construct_params::Kind::Length),
        Some(Format {
            unit: Unit::Metre,
            places: 3
        })
    );
    assert_eq!(
        units.format(construct_params::Kind::Area),
        Units::metric().format(construct_params::Kind::Area)
    );
    // Refused: what the writer would not write, and a look out of range.
    let mut odd = record.clone();
    odd.props.remove("@dimension.style.length.places");
    assert!(DimensionStyle::from_record(&odd).is_err());
    let mut odd = record.clone();
    odd.props.insert(
        "@dimension.style.marker".to_owned(),
        "choice:star".to_owned(),
    );
    assert!(DimensionStyle::from_record(&odd).is_err());
    let mut odd = record.clone();
    odd.id = id(TICKS_ID);
    assert!(DimensionStyle::from_record(&odd).is_err());
    for bad in [
        Look {
            height: mm(0),
            ..look
        },
        Look {
            gap: mm(51),
            ..look
        },
        Look {
            angle_places: Some(10),
            ..look
        },
        Look {
            length: Some(Format {
                unit: Unit::Millimetre,
                places: 3,
            }),
            ..look
        },
        Look {
            length: Some(Format {
                unit: Unit::Degree,
                places: 0,
            }),
            ..look
        },
    ] {
        assert!(matches!(
            DimensionStyle::new(id("S-2"), "Bad", bad),
            Err(DimensionError::BadStyle(_))
        ));
    }
    assert!(DimensionStyle::new(id("S-2"), " padded", look).is_err());
    // A document's styles: names are unique among them and the built-in ones, case aside.
    let mut styles = Styles::from_records([&record]).unwrap();
    assert_eq!(styles.all().len(), 3);
    assert_eq!(styles.get(&id("S-1")), Some(&style));
    let clash = DimensionStyle::new(id("S-2"), "ticks", look).unwrap();
    assert!(styles.add(clash).is_err());
    let other = DimensionStyle::new(id("S-2"), "Detail", look).unwrap();
    styles.add(other.clone()).unwrap();
    assert!(styles.add(other).is_err());
    let renamed = style.renamed("Detail").unwrap();
    assert!(styles.replace(renamed).is_err());
    let renamed = style.renamed("Site").unwrap();
    styles.replace(renamed).unwrap();
    assert_eq!(styles.named("SITE").unwrap().id(), &id("S-1"));
}
