//! The generator's checks (CON-A7-F4-P3): a type is built through the family evaluator, then
//! judged on what was built, never on the spec's own arithmetic.
//!
//! 1. **Valid solids.** The recipe evaluates for the type (construct-families' evaluator, within
//!    its default limits) over the measuring engine, and every solid is one extrusion along one
//!    axis, so it can be measured exactly.
//! 2. **A clean opening cut.** The opening the wall is cut by (construct-families' `opening()`) is
//!    one closed rectangle, square to the wall, cut through the wall's whole thickness; it is
//!    exactly the frame's outside (no gap round the frame, no overlap); every solid lies within it
//!    along the wall and up, and within the frame's depth across it, so the unit fits any wall at
//!    least as thick as its frame.
//! 3. **Symbols inside.** Every elevation symbol lies within the opening, every plan symbol within
//!    its width (a door's swing reaches into the room, so plan symbols are judged along the wall
//!    only).
//! 4. **Quantities.** The overall size is the opening's; the clear opening, the leaf area and the
//!    glass area are measured from the solids ([`Quantities`]).
//! 5. **Tags** (for a stored type, [`crate::verify`]): the values the type's row holds for its
//!    tags are the quantities its solids measure.

use crate::measure::{Box3, Measure, Measured, Prism};
use crate::refusal::{Refusal, Rule};
use crate::template::Template;
use construct_families::{
    Depth, DrawnSymbol, Edge, Host, Point2, Recipe, SolvedOpening, TypeValues, View, evaluate,
    opening, symbols,
};
use construct_params::{Area, Length};
use std::collections::BTreeMap;

/// What a generated type measures, exactly: lengths in grid steps, areas in square steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quantities {
    /// The overall width: the opening's, the outside of the frame.
    pub width: Length,
    /// The overall height.
    pub height: Length,
    /// The clear width (spec, `Size::Clear`).
    pub clear_width: Length,
    /// The clear height.
    pub clear_height: Length,
    /// The face area of the leaf, or of both sashes added, their outlines whole (a sash's glass
    /// counted in): `None` for a unit without one.
    pub leaf_area: Option<Area>,
    /// The face area of all the glass: `None` for a unit without any.
    pub glass_area: Option<Area>,
    /// The frame's depth: the thinnest wall it fits.
    pub depth: Length,
}

/// A built type, judged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Judged {
    pub(crate) quantities: Quantities,
    pub(crate) opening: SolvedOpening,
}

/// One measured solid.
struct Solid {
    prism: Prism,
    bounds: Box3,
}

impl Solid {
    fn span(&self, axis: usize, step: &str) -> Result<Length, Refusal> {
        self.bounds
            .span(axis)
            .ok_or_else(|| Refusal::new(Rule::Evaluation, step, "the solid is too large"))
    }
}

/// Builds the type whose values are `values` and judges what was built (this module's header,
/// checks 1 to 4).
pub(crate) fn judge_built(
    template: Template,
    recipe: &Recipe,
    values: &TypeValues,
) -> Result<Judged, Refusal> {
    let built = evaluate(&mut Measure, recipe, values)
        .map_err(|e| Refusal::new(Rule::Evaluation, "solids", e.to_string()))?;
    let mut solids = BTreeMap::new();
    for b in built.solids {
        let step = b.step.as_str().to_owned();
        let Measured::Prism(prism) = b.solid else {
            return Err(Refusal::new(
                Rule::Evaluation,
                step,
                "the solid is not one extrusion, so it cannot be measured",
            ));
        };
        if prism.axis().is_none() {
            return Err(Refusal::new(
                Rule::Evaluation,
                step,
                "the solid is not swept along one axis",
            ));
        }
        let Some(bounds) = prism.bounds() else {
            return Err(Refusal::new(Rule::Evaluation, step, "the solid is empty"));
        };
        solids.insert(step, Solid { prism, bounds });
    }
    let cut = opening(recipe, values)
        .map_err(|e| Refusal::new(Rule::Opening, "opening", e.to_string()))?;
    let frame = solid(&solids, "frame")?;
    let room = clean(&cut, frame.bounds, &solids)?;
    let drawn = symbols(recipe, values)
        .map_err(|e| Refusal::new(Rule::Symbol, "symbols", e.to_string()))?;
    for symbol in &drawn.symbols {
        inside(symbol, &room)?;
    }
    let quantities = measure(template, &solids, &room)?;
    Ok(Judged {
        quantities,
        opening: cut,
    })
}

fn solid<'s>(solids: &'s BTreeMap<String, Solid>, step: &str) -> Result<&'s Solid, Refusal> {
    solids.get(step).ok_or_else(|| {
        Refusal::new(
            Rule::Evaluation,
            step,
            "the template builds no solid by this step",
        )
    })
}

/// Check 2: the opening is clean. Gives its box: along the wall and up, the opening's; across the
/// wall, the frame's.
fn clean(
    cut: &SolvedOpening,
    frame: Box3,
    solids: &BTreeMap<String, Solid>,
) -> Result<Box3, Refusal> {
    let refuse = |reason: String| Err(Refusal::new(Rule::Opening, "opening", reason));
    if cut.host != Host::Wall || cut.depth != Depth::ThroughHost {
        return refuse("the opening is not cut through a wall's whole thickness".to_owned());
    }
    let corners = &cut.outline;
    if corners.len() != 4 {
        return refuse(format!(
            "the opening has {} corners; a clean cut is a rectangle of 4",
            corners.len()
        ));
    }
    for (i, a) in corners.iter().enumerate() {
        let next = i.checked_add(1).and_then(|n| corners.get(n));
        let Some(b) = next.or_else(|| corners.first()) else {
            return refuse("the opening has no corners".to_owned());
        };
        let along = a[0] != b[0];
        let up = a[1] != b[1];
        if along == up {
            return refuse(format!(
                "the opening's side from ({}, {}) to ({}, {}) is not square to the wall",
                a[0], a[1], b[0], b[1]
            ));
        }
    }
    let low = |k: usize| corners.iter().filter_map(|p| p.get(k)).min().copied();
    let high = |k: usize| corners.iter().filter_map(|p| p.get(k)).max().copied();
    let (Some(x0), Some(x1), Some(z0), Some(z1)) = (low(0), high(0), low(1), high(1)) else {
        return refuse("the opening has no corners".to_owned());
    };
    if x0 >= x1 || z0 >= z1 {
        return refuse("the opening has no area".to_owned());
    }
    let room = Box3 {
        low: [x0, frame.low[1], z0],
        high: [x1, frame.high[1], z1],
    };
    if frame.low[0] != x0 || frame.high[0] != x1 || frame.low[2] != z0 || frame.high[2] != z1 {
        return refuse(format!(
            "the opening runs {x0} to {x1} along the wall and {z0} to {z1} up, and the frame's \
             outside {} to {} and {} to {}: they must be the same",
            frame.low[0], frame.high[0], frame.low[2], frame.high[2]
        ));
    }
    for (step, s) in solids {
        for (axis, way) in [(0, "along the wall"), (1, "across the wall"), (2, "up")] {
            if !room.holds_along(&s.bounds, axis) {
                return Err(Refusal::new(
                    Rule::Opening,
                    step.as_str(),
                    format!("the solid reaches past the opening {way}"),
                ));
            }
        }
    }
    Ok(room)
}

/// Check 3: a symbol lies within the opening.
fn inside(symbol: &DrawnSymbol, room: &Box3) -> Result<(), Refusal> {
    let mut points: Vec<Point2> = Vec::new();
    for edge in &symbol.edges {
        match edge {
            Edge::Line { start, end, .. } => points.extend([*start, *end]),
            Edge::Arc {
                centre, start, end, ..
            } => points.extend([*centre, *start, *end]),
            Edge::Circle { centre, radius, .. } => {
                let (Some(left), Some(right)) = (
                    centre[0].checked_sub(*radius),
                    centre[0].checked_add(*radius),
                ) else {
                    return Err(Refusal::new(
                        Rule::Symbol,
                        symbol.symbol.as_str(),
                        "a circle too large to hold",
                    ));
                };
                points.extend([[left, centre[1]], [right, centre[1]]]);
            }
        }
    }
    for p in points {
        let along = room.low[0] <= p[0] && p[0] <= room.high[0];
        let up = room.low[2] <= p[1] && p[1] <= room.high[2];
        let within = match symbol.view {
            View::Elevation => along && up,
            View::Plan => along,
        };
        if !within {
            return Err(Refusal::new(
                Rule::Symbol,
                symbol.symbol.as_str(),
                format!(
                    "the symbol reaches ({}, {}), outside the opening",
                    p[0], p[1]
                ),
            ));
        }
    }
    Ok(())
}

/// Check 4: the quantities, measured from the solids.
fn measure(
    template: Template,
    solids: &BTreeMap<String, Solid>,
    room: &Box3,
) -> Result<Quantities, Refusal> {
    let frame = solid(solids, "frame")?;
    let area = |s: &Solid, step: &str, outline: bool| {
        let a = if outline {
            s.prism.outline_area()
        } else {
            s.prism.face_area()
        };
        a.ok_or_else(|| {
            Refusal::new(
                Rule::Quantity,
                step,
                "the face is not whole square grid steps",
            )
        })
    };
    let sum = |a: Area, b: Area, what: &str| {
        a.checked_add(b)
            .ok_or_else(|| Refusal::new(Rule::Quantity, what, "the area is too large"))
    };
    let too_large = || Refusal::new(Rule::Quantity, "clear_width", "the size is too large");
    let (clear_width, clear_height, leaf_area, glass_area) = match template {
        Template::SingleSwingDoor => {
            let leaf = solid(solids, "leaf")?;
            // From the open leaf's face to the far jamb: the leaf's width less its thickness.
            let clear = leaf
                .span(0, "leaf")?
                .checked_sub(leaf.span(1, "leaf")?)
                .ok_or_else(too_large)?;
            (
                clear,
                leaf.span(2, "leaf")?,
                Some(area(leaf, "leaf", true)?),
                None,
            )
        }
        Template::FixedWindow => {
            let glass = solid(solids, "glass")?;
            (
                glass.span(0, "glass")?,
                glass.span(2, "glass")?,
                None,
                Some(area(glass, "glass", false)?),
            )
        }
        Template::SlidingWindow => {
            let fixed = solid(solids, "sash_fixed")?;
            let sliding = solid(solids, "sash_sliding")?;
            // Slid fully across, the sliding sash leaves open what lies between the fixed sash's
            // meeting edge and the sliding sash's far edge.
            let clear = sliding.bounds.high[0]
                .checked_sub(fixed.bounds.high[0])
                .ok_or_else(too_large)?;
            let leaves = sum(
                area(fixed, "sash_fixed", true)?,
                area(sliding, "sash_sliding", true)?,
                "leaf_area",
            )?;
            let glass = sum(
                area(solid(solids, "glass_fixed")?, "glass_fixed", false)?,
                area(solid(solids, "glass_sliding")?, "glass_sliding", false)?,
                "glass_area",
            )?;
            (
                clear,
                sliding.span(2, "sash_sliding")?,
                Some(leaves),
                Some(glass),
            )
        }
    };
    let span = |axis: usize, what: &str| {
        room.span(axis)
            .ok_or_else(|| Refusal::new(Rule::Opening, what, "the opening is too large"))
    };
    Ok(Quantities {
        width: span(0, "width")?,
        height: span(2, "height")?,
        clear_width,
        clear_height,
        leaf_area,
        glass_area,
        depth: frame.span(1, "frame")?,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]
    use super::*;
    use construct_categories::CategoryId;
    use construct_families::{Key, Visibility};

    fn mm(mm: i32) -> Length {
        Length::from_mm(mm)
    }

    /// An opening 1,000 wide and 2,000 high, 100 deep.
    fn room() -> Box3 {
        Box3 {
            low: [mm(-500), mm(-50), mm(0)],
            high: [mm(500), mm(50), mm(2000)],
        }
    }

    fn line(view: View, start: [i32; 2], end: [i32; 2]) -> DrawnSymbol {
        DrawnSymbol {
            symbol: Key::parse("mark").unwrap(),
            view,
            subcategory: CategoryId(301),
            visibility: Visibility {
                coarse: true,
                medium: true,
                fine: true,
            },
            when: None,
            edges: vec![Edge::Line {
                key: Key::parse("l").unwrap(),
                start: start.map(mm),
                end: end.map(mm),
            }],
        }
    }

    #[test]
    fn a_symbol_past_the_opening_is_refused_by_the_symbol_rule() {
        let r = inside(&line(View::Elevation, [0, 0], [600, 100]), &room()).unwrap_err();
        assert_eq!((r.rule, r.what.as_str()), (Rule::Symbol, "mark"));
        let r = inside(&line(View::Elevation, [0, 0], [100, 2100]), &room()).unwrap_err();
        assert_eq!(r.rule, Rule::Symbol);
        // A plan symbol may reach into the room (a door's swing), never past the jambs.
        assert!(inside(&line(View::Plan, [0, 0], [100, 2100]), &room()).is_ok());
        assert!(inside(&line(View::Plan, [-501, 0], [0, 0]), &room()).is_err());
    }

    #[test]
    fn a_face_that_is_not_whole_square_steps_is_refused_by_the_quantity_rule() {
        let triangle = Prism {
            outer: vec![[0, 0, 0], [1, 0, 0], [0, 0, 1]],
            holes: vec![],
            by: [0, 64_000, 0],
        };
        let square = Prism {
            outer: vec![[-640, 0, 0], [640, 0, 0], [640, 0, 640], [-640, 0, 640]],
            holes: vec![],
            by: [0, 64_000, 0],
        };
        let mut solids = BTreeMap::new();
        for (step, prism) in [("frame", square), ("glass", triangle)] {
            let bounds = prism.bounds().unwrap();
            solids.insert(step.to_owned(), Solid { prism, bounds });
        }
        let r = measure(Template::FixedWindow, &solids, &room()).unwrap_err();
        assert_eq!((r.rule, r.what.as_str()), (Rule::Quantity, "glass"));
    }
}
