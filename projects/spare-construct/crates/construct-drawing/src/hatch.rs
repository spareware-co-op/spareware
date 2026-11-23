//! Hatches: a cut region filled with its material's cut pattern, the pattern's lines clipped
//! exactly to the region (note A12's build-order step 4; chapter 14.6 of the geometry
//! architecture).
//!
//! What it does: a [`FillPattern`] is a set of [`Family`]s of parallel lines, each with a whole
//! direction and a spacing on paper. [`hatch`] lays every family over the region at the view's
//! scale and keeps each line's pieces inside it, exactly: a hole is not hatched, and a line along
//! an edge or touching a corner draws nothing. The lines are counted before they are made and a
//! hatch of more than [`MOST_HATCH_LINES`] is refused ([`DrawingError::Budget`]), never cut short.
//! Hatch lines are derived each time a drawing is printed and never stored.
//!
//! A family's lines, exactly: with direction `d = (dx, dy)`, line `k` (every whole `k`) runs along
//! `d` through the sheet point `(k s, 0)`, or `(0, k s)` for a family along x (`dy` = 0), where `s`
//! is the spacing at the scale, in grid steps: the printed micrometres times the scale's
//! denominator times 6,400 steps a millimetre over 1,000. So a family's spacing is measured along
//! the sheet's x axis (y for a family along x), a 45 degree family's lines are `s / √2` apart, and
//! every point is an exact fraction. Every family starts at the view's origin, so the hatches of
//! neighbouring regions line up.
//!
//! The patterns: [`FillPatterns::default_metric`] has one for each cut fill construct-materials'
//! default library names. They are our own: thin continuous lines at 45 degrees as the general
//! hatching of a cut (the convention of ISO 128-50, "Technical drawings: general principles of
//! presentation: basic conventions for representing areas on cuts and sections"), told apart by
//! spacing and by a second family. Each prints with model pen 1 (the thinnest) in the material's
//! cut colour. The table lives here until construct-styles holds fill patterns as project data
//! beside its line patterns.
//!
//! Not yet: dashed and dotted family lines, a pattern's own origin and angle per region, solid
//! fills, model patterns at true size (chapter 18.8), and the island rule (every hole is left
//! unhatched).
//!
//! Interim: the clipping is the crate's own exact overlay ([`crate::overlay`]); the kernel's
//! hatches (`geom-2d`, piece G210, with its pattern library G211) replace it with the same lines.

use construct_params::Length;
use construct_styles::{Micrometres, Scale};

use crate::DrawingError;
use crate::drawing::ViewPoint;
use crate::overlay::Overlay;

/// The most lines one hatch may draw, counted before any is made.
pub const MOST_HATCH_LINES: u64 = 100_000;

/// The pen the default patterns print with: model pen 1, the thinnest.
pub const HATCH_PEN: u16 = 1;

/// One family of parallel lines of a fill pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Family {
    /// The lines' direction on the sheet: a whole vector, not zero, `[1, 1]` for 45 degrees.
    pub direction: [i32; 2],
    /// The printed distance between neighbouring lines along the sheet's x axis (along y for a
    /// family along x).
    pub spacing: Micrometres,
}

/// A fill pattern: its name, as a material's cut fill names it, its families and its pen.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FillPattern {
    /// The name a material's cut fill refers to it by.
    pub name: String,
    /// Its families, drawn in this order.
    pub families: Vec<Family>,
    /// The model pen its lines print with (construct-styles' pen table, by number).
    pub pen: u16,
}

/// The fill patterns a drawing is printed with, by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FillPatterns {
    patterns: Vec<FillPattern>,
}

/// A default family: its direction's dx and dy, and its spacing in printed micrometres.
type DefaultFamily = (i32, i32, u32);

/// Our default patterns: a name and its families.
const DEFAULTS: [(&str, &[DefaultFamily]); 9] = [
    ("concrete", &[(1, 1, 1500), (1, -1, 1500)]),
    ("earth", &[(1, -1, 3000), (1, 1, 6000)]),
    ("glass", &[(1, 1, 500)]),
    ("insulation", &[(1, 0, 1000), (0, 1, 1000)]),
    ("membrane", &[(1, 0, 500)]),
    ("metal", &[(1, 1, 1000)]),
    ("plaster", &[(1, 1, 3000)]),
    ("steel", &[(1, 1, 750)]),
    ("timber", &[(1, -1, 2000)]),
];

impl FillPatterns {
    /// The table of `patterns`, in order of name.
    ///
    /// # Errors
    ///
    /// [`DrawingError::BadShape`] for two patterns of one name, a pattern with no family, or a
    /// family with no direction.
    pub fn new(mut patterns: Vec<FillPattern>) -> Result<Self, DrawingError> {
        patterns.sort();
        for pair in patterns.windows(2) {
            if let [a, b] = pair
                && a.name == b.name
            {
                return Err(DrawingError::BadShape(format!(
                    "two fill patterns named {}",
                    a.name
                )));
            }
        }
        for pattern in &patterns {
            if pattern.families.is_empty() {
                return Err(DrawingError::BadShape(format!(
                    "the fill pattern {} has no family",
                    pattern.name
                )));
            }
            if pattern.families.iter().any(|f| f.direction == [0, 0]) {
                return Err(DrawingError::BadShape(format!(
                    "a family of the fill pattern {} has no direction",
                    pattern.name
                )));
            }
        }
        Ok(Self { patterns })
    }

    /// Our default metric patterns, one for each cut fill of construct-materials' default
    /// library.
    #[must_use]
    pub fn default_metric() -> Self {
        let patterns = DEFAULTS
            .iter()
            .map(|(name, families)| FillPattern {
                name: (*name).to_owned(),
                families: families
                    .iter()
                    .filter_map(|&(dx, dy, um)| {
                        Some(Family {
                            direction: [dx, dy],
                            spacing: Micrometres::new(um).ok()?,
                        })
                    })
                    .collect(),
                pen: HATCH_PEN,
            })
            .collect();
        Self { patterns }
    }

    /// Every pattern, in order of name.
    #[must_use]
    pub fn patterns(&self) -> &[FillPattern] {
        &self.patterns
    }

    /// The pattern named `name`.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&FillPattern> {
        self.patterns.iter().find(|p| p.name == name)
    }
}

/// The distance between family lines, `c_k = k · step`, as the fraction `(num, den)` in the
/// units of `n · p` (grid steps times `n`, `n` the direction turned a quarter counter-clockwise).
fn step(family: Family, scale: Scale) -> Result<(i128, i128), DrawingError> {
    let [dx, dy] = family.direction;
    // Line k passes through (k s, 0), or (0, k s) for a family along x: n · that point.
    let along = if dy == 0 {
        i128::from(dx)
    } else {
        i128::from(dy).checked_neg().ok_or(DrawingError::TooBig)?
    };
    let num = i128::from(family.spacing.get())
        .checked_mul(i128::from(scale.denominator()))
        .and_then(|v| v.checked_mul(i128::from(Length::STEPS_PER_MM)))
        .and_then(|v| v.checked_mul(along))
        .ok_or(DrawingError::TooBig)?;
    Ok((num, 1000))
}

/// The lines of `pattern` inside the region bounded by `loops` (an outline counter-clockwise and
/// its holes clockwise, in view points) at `scale`: family by family, line by line, each piece
/// from its end lower along the family's direction.
///
/// # Errors
///
/// [`DrawingError::Budget`] for a family of more than [`MOST_HATCH_LINES`] lines across the
/// region; [`DrawingError::TooBig`] for a number past 128 bits.
pub fn hatch(
    loops: &[Vec<ViewPoint>],
    pattern: &FillPattern,
    scale: Scale,
) -> Result<Vec<[ViewPoint; 2]>, DrawingError> {
    let mut out = Vec::new();
    for family in &pattern.families {
        let [dx, dy] = family.direction;
        out.extend(Overlay::family(
            loops,
            [i128::from(dx), i128::from(dy)],
            step(*family, scale)?,
            MOST_HATCH_LINES,
        )?);
    }
    Ok(out)
}
