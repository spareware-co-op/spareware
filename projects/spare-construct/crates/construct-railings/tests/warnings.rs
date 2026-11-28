//! A railing that breaks a limit of its rule set is a warning by name, naming the limit, the value
//! and its clause; one that keeps them has none. The figures are F4/AS1's (Table 1, 1.2.1, 1.2.2)
//! and D1/AS1's (6.0.6), worked by hand.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{at, laid, laid_stair, mm, on_stair, sketched, spaced, stair};
use construct_railings::{
    HANDRAIL, Handrail, LOW, OPENING, Path, Profile, Railing, RailingError, RailingType, Settings,
    Severity, Side, StairHost, Warning, warnings,
};
use kit_core::Id;

fn id() -> Id {
    Id::parse("R-1").unwrap()
}

fn of(railing: &Railing, host: Option<StairHost<'_>>) -> Vec<Warning> {
    warnings(&id(), "Railing R-1", railing, &laid(railing, host)).unwrap()
}

fn sketched_as(kind: RailingType, category: &str, openings: &str) -> Railing {
    Railing::new(Settings {
        rules: "nz-f4-as1".to_owned(),
        category: category.to_owned(),
        openings: openings.to_owned(),
        kind,
        path: Path::Sketched {
            points: vec![at(0, 0), at(3000, 0), at(3000, 2000)],
        },
        flipped: false,
    })
    .unwrap()
}

/// The standard type along a balcony's edge in a house keeps every limit: 1000 mm high (Table 1:
/// 1000 mm), 90 mm between balusters and 50 mm under them (1.2.1 b): a 100 mm sphere).
#[test]
fn the_standard_type_on_a_balcony_keeps_every_limit() {
    let railing = sketched(RailingType::standard(), vec![at(0, 0), at(3000, 0)]);
    assert_eq!(of(&railing, None), Vec::new());
}

/// Balusters 130 mm apart leave 110 mm between them, which a 100 mm sphere passes.
#[test]
fn an_opening_a_sphere_passes_is_a_warning() {
    let railing = sketched(spaced(130), vec![at(0, 0), at(3000, 0)]);
    let found = of(&railing, None);
    assert_eq!(found.len(), 1);
    let w = &found[0];
    assert_eq!((w.kind, w.severity), (OPENING, Severity::Warning));
    assert_eq!(w.elements, vec![id()]);
    assert_eq!(w.identity(), "railing.opening|R-1");
    assert_eq!(
        w.words,
        "Railing R-1: the opening between upright members on piece 1 of line 1 is 110 mm, and a sphere of 100 mm (F4/AS1 1.2.1 b)) must not pass it."
    );
}

/// The same 1000 mm railing in another building's location is below Table 1's 1100 mm.
#[test]
fn a_top_rail_below_its_category_is_a_warning() {
    let found = of(
        &sketched_as(RailingType::standard(), "other", "children"),
        None,
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].kind, LOW);
    assert_eq!(
        found[0].words,
        "Railing R-1: its top rail is 1000 mm high on the level, below the least barrier height there, 1100 mm (F4/AS1 1.1.1 and Table 1)."
    );
}

/// Where children are not expected (1.2.2), upright members may be 300 mm apart: 320 mm centres
/// less 20 mm is 300 mm and keeps it; 330 mm does not.
#[test]
fn where_no_children_are_the_openings_may_be_wider() {
    let mut kind = spaced(320);
    kind.top_rail.height = mm(1100);
    assert_eq!(
        of(&sketched_as(kind.clone(), "other", "no-children"), None),
        Vec::new()
    );
    kind.balusters.spacing = mm(330);
    let found = of(&sketched_as(kind, "other", "no-children"), None);
    assert_eq!(found.len(), 1);
    assert!(
        found[0]
            .words
            .ends_with("is 310 mm, wider than the most allowed, 300 mm (F4/AS1 1.2.2 a))."),
        "{}",
        found[0].words
    );
}

/// A house's category is always checked as one children use (1.2.1, Comment 1).
#[test]
fn a_house_has_no_openings_rule_without_children() {
    let refused = Railing::new(Settings {
        rules: "nz-f4-as1".to_owned(),
        category: "household-edge".to_owned(),
        openings: "no-children".to_owned(),
        kind: RailingType::standard(),
        path: Path::Sketched {
            points: vec![at(0, 0), at(1000, 0)],
        },
        flipped: false,
    })
    .unwrap_err();
    assert_eq!(
        refused.to_string(),
        "A barrier of category household-edge is checked by the openings rule children, not \"no-children\"."
    );
    assert!(matches!(refused, RailingError::UnknownOpenings { .. }));
}

/// On the stair of 17 risers (180 mm on 280 mm), the opening under balusters standing 50 mm above
/// the pitch line: its largest disc, in the corner of tread and riser, is (230 / 180) (460 -
/// sqrt(280^2 + 180^2)) = 162.45 mm (shown in whole mm), which a 150 mm sphere passes (1.2.1 c)); with their foot on
/// the pitch line it is 127.13 mm, which it does not. A handrail at 850 mm is below D1/AS1's
/// 900 mm; at 900 mm it keeps it.
#[test]
fn on_a_flight_the_opening_under_the_balusters_and_the_handrail_are_checked() {
    let s = stair(&[None], &[]);
    let laid_out = laid_stair(&s, 3060);
    let host = || {
        Some(StairHost {
            stair: &s,
            layout: &laid_out,
            shift: mm(0),
        })
    };
    let railing = on_stair(RailingType::standard(), Side::Right, 50);
    let found = of(&railing, host());
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].words,
        "Railing R-1: the opening under the balusters on the flight, piece 1 of line 1 is 162 mm, and a sphere of 150 mm (F4/AS1 1.2.1 c) and Figure 4) must not pass it."
    );
    let mut kind = RailingType::standard();
    kind.balusters.bottom = mm(0);
    kind.handrail = Some(Handrail {
        profile: Profile::Round { diameter: mm(40) },
        height: mm(850),
        offset: mm(60),
    });
    let found = of(&on_stair(kind.clone(), Side::Right, 50), host());
    assert_eq!(
        found.iter().map(|w| w.kind).collect::<Vec<_>>(),
        vec![HANDRAIL]
    );
    assert_eq!(
        found[0].words,
        "Railing R-1: its handrail's top is 850 mm above the pitch line; it stands from 900 mm (D1/AS1 6.0.6) to 1000 mm (D1/AS1 6.0.6)."
    );
    if let Some(h) = kind.handrail.as_mut() {
        h.height = mm(900);
    }
    assert_eq!(of(&on_stair(kind, Side::Right, 50), host()), Vec::new());
}
