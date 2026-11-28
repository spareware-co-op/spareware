//! What the tests share: lengths, and the stairs the tests set out.

#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    reason = "test code; each test file uses part of this fixture"
)]

use construct_params::Length;
use construct_stairs::{Direction, LandingSpec, RunSpec, Settings, Stair, Turn};

/// Grid steps in a millimetre.
pub const MM: i64 = 6400;

/// Whole millimetres as a length.
pub fn mm(mm: i64) -> Length {
    Length::checked_from_mm(mm).unwrap()
}

/// A stair's settings: of `category` in New Zealand's rule set, starting at the origin and going
/// up towards +x, 1000 mm wide, the category's least going, a 150 mm waist, one run.
pub fn settings(category: &str) -> Settings {
    Settings {
        rules: "nz-d1-as1".to_owned(),
        category: category.to_owned(),
        start: [mm(0), mm(0)],
        direction: Direction::PlusX,
        width: mm(1000),
        going: None,
        waist: mm(150),
        runs: vec![RunSpec::default()],
        landings: Vec::new(),
    }
}

/// A straight stair of one run.
pub fn straight(category: &str) -> Stair {
    Stair::new(settings(category)).unwrap()
}

/// A stair of runs (each its count of risers, or none to share the rest) with the landings
/// between them, each `(length in mm, turn)`.
pub fn runs(category: &str, risers: &[Option<u32>], landings: &[(i64, Turn)]) -> Stair {
    let mut s = settings(category);
    s.runs = risers.iter().map(|&risers| RunSpec { risers }).collect();
    s.landings = landings
        .iter()
        .map(|&(length, turn)| LandingSpec {
            length: mm(length),
            turn,
        })
        .collect();
    Stair::new(s).unwrap()
}

/// The steps of a length.
pub fn steps(l: Length) -> i64 {
    l.steps()
}
