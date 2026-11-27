//! What the tests share: lengths, the railings the tests set out, and the stair a railing follows.

#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    reason = "test code; each test file uses part of this fixture"
)]

use construct_params::Length;
use construct_railings::{Layout, Path, Railing, RailingType, Settings, Side, StairHost, lay_out};
use construct_stairs::{Direction, Layout as StairLayout, RunSpec, Stair, Turn};
use kit_core::Id;

/// Grid steps in a millimetre.
pub const MM: i64 = 6400;

/// Whole millimetres as a length.
pub fn mm(mm: i64) -> Length {
    Length::checked_from_mm(mm).unwrap()
}

/// A plan point in whole millimetres.
pub fn at(x: i64, y: i64) -> [Length; 2] {
    [mm(x), mm(y)]
}

/// The standard type with its balusters `spacing` mm apart.
pub fn spaced(spacing: i64) -> RailingType {
    let mut kind = RailingType::standard();
    kind.balusters.spacing = mm(spacing);
    kind
}

/// A sketched railing of `kind` along `points`, of category `household-edge`.
pub fn sketched(kind: RailingType, points: Vec<[Length; 2]>) -> Railing {
    Railing::new(Settings {
        rules: "nz-f4-as1".to_owned(),
        category: "household-edge".to_owned(),
        openings: "children".to_owned(),
        kind,
        path: Path::Sketched { points },
        flipped: false,
    })
    .unwrap()
}

/// A railing's layout along its own lines, with `host` for one on a stair.
pub fn laid(railing: &Railing, host: Option<StairHost<'_>>) -> Layout {
    lay_out(railing.kind(), railing.lines(host).unwrap()).unwrap()
}

/// A stair of New Zealand's main private category from the origin towards +x, 1000 mm wide, on
/// its least going (280 mm), with runs of `risers` (none to share) and landings `(mm, turn)`.
pub fn stair(risers: &[Option<u32>], landings: &[(i64, Turn)]) -> Stair {
    Stair::new(construct_stairs::Settings {
        rules: "nz-d1-as1".to_owned(),
        category: "main-private".to_owned(),
        start: [mm(0), mm(0)],
        direction: Direction::PlusX,
        width: mm(1000),
        going: None,
        waist: mm(150),
        runs: risers.iter().map(|&risers| RunSpec { risers }).collect(),
        landings: landings
            .iter()
            .map(|&(length, turn)| construct_stairs::LandingSpec {
                length: mm(length),
                turn,
            })
            .collect(),
    })
    .unwrap()
}

/// A stair laid out for a rise of `rise` mm.
pub fn laid_stair(stair: &Stair, rise: i64) -> StairLayout {
    construct_stairs::layout(stair, mm(rise)).unwrap()
}

/// A railing of `kind` on stair `S-1`, on `side`, `inset` mm in, of category `household-stair`.
pub fn on_stair(kind: RailingType, side: Side, inset: i64) -> Railing {
    Railing::new(Settings {
        rules: "nz-f4-as1".to_owned(),
        category: "household-stair".to_owned(),
        openings: "children".to_owned(),
        kind,
        path: Path::OnStair {
            stair: Id::parse("S-1").unwrap(),
            side,
            inset: mm(inset),
        },
        flipped: false,
    })
    .unwrap()
}

/// The steps of a length.
pub fn steps(l: Length) -> i64 {
    l.steps()
}
