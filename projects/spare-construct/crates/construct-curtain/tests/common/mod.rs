//! What the tests share: lengths, materials, profiles, panels and specs.

#![allow(
    dead_code,
    clippy::unwrap_used,
    reason = "test code; each test file uses part of this fixture"
)]

use construct_curtain::{
    Crossing, Curtain, CurtainSpec, Edits, Infill, Justify, Layout, Mullions, Panel, Profile,
};
use construct_materials::MaterialId;
use construct_params::Length;

/// Whole millimetres as a length.
pub fn mm(mm: i64) -> Length {
    Length::checked_from_mm(mm).unwrap()
}

/// Whole millimetres as lengths.
pub fn mms(list: &[i64]) -> Vec<Length> {
    list.iter().map(|value| mm(*value)).collect()
}

/// A material of the default library.
pub fn material(id: &str) -> MaterialId {
    MaterialId::parse(id).unwrap()
}

/// A mullion `width` by `depth` whole millimetres, centred on the wall's line.
pub fn mullion(width: i64, depth: i64) -> Profile {
    Profile::rectangle(mm(width), mm(depth), mm(0), material("timber-douglas-fir")).unwrap()
}

/// Glazing `thickness` whole millimetres thick, centred on the line.
pub fn glazed(thickness: i64) -> Panel {
    Panel::Glazed(Infill::new(mm(thickness), mm(0), material("glass-plate")).unwrap())
}

/// A solid panel `thickness` whole millimetres thick, centred on the line.
pub fn solid(thickness: i64) -> Panel {
    Panel::Solid(Infill::new(mm(thickness), mm(0), material("plaster-gypsum")).unwrap())
}

/// A fixed spacing of whole millimetres from the start.
pub fn every(spacing: i64) -> Layout {
    Layout::Spacing {
        spacing: mm(spacing),
        justify: Justify::Start,
    }
}

/// The same mullion on every kind of line.
pub fn all(profile: &Profile) -> Mullions {
    Mullions {
        interior_vertical: Some(profile.clone()),
        interior_horizontal: Some(profile.clone()),
        border_vertical: Some(profile.clone()),
        border_horizontal: Some(profile.clone()),
    }
}

/// A spec of these layouts, 24 mm glazing, these mullions and this crossing.
pub fn spec(
    vertical: Layout,
    horizontal: Layout,
    mullions: Mullions,
    crossing: Crossing,
) -> CurtainSpec {
    CurtainSpec::new(vertical, horizontal, glazed(24), mullions, crossing).unwrap()
}

/// A wall of whole millimetres with no edits.
pub fn wall(length: i64, height: i64, spec: CurtainSpec) -> Curtain {
    Curtain::new(mm(length), mm(height), spec, Edits::new()).unwrap()
}

/// The 6 m by 3 m wall of the worked examples: vertical lines every 1.5 m and horizontal lines
/// every 1 m, both from the start, with these mullions, the vertical ones running through.
pub fn six_by_three(mullions: Mullions) -> Curtain {
    wall(
        6000,
        3000,
        spec(every(1500), every(1000), mullions, Crossing::Vertical),
    )
}

/// The places of a grid's lines in one direction, borders and all, in whole millimetres.
pub fn places(lines: &[construct_curtain::GridLine]) -> Vec<i64> {
    lines.iter().map(|l| l.at.whole_mm().unwrap()).collect()
}
