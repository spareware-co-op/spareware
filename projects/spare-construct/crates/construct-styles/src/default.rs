//! Our default metric set of pens.
//!
//! **The widths.** The ladder of line widths is the one ISO 128-20 (Technical drawings: general
//! principles of presentation: basic conventions for lines) gives, each step the previous one
//! times the square root of two: 0.13, 0.18, 0.25, 0.35, 0.5, 0.7, 1.0, 1.4 and 2.0 mm. It is a
//! public standard; the pen numbers, the names and the rule below are ours.
//!
//! **Pen numbers.** There are nine pens, 1 to 9, and pen `n` is the `n`th rung of the ladder:
//! pen 1 is 0.13 mm and pen 9 is 2.0 mm at the largest scales.
//!
//! **How a pen steps down with scale.** At scales from 1:1 to 1:20 a pen prints at its own rung.
//! Beyond 1:20 it steps down one rung for each factor of 2.5 in the scale's denominator: one rung
//! down at 1:50 and 1:100, two at 1:200, three at 1:500 and four at 1:1000, and never below the
//! bottom rung, 0.13 mm. A rung is the square root of two thinner, so a drawing 2.5 times smaller
//! prints its lines about 1.4 times thinner. The rule is in [`steps_down`] and every width in the
//! table is pinned by a test.
//!
//! **Annotation pens.** Pen `n` prints at the `n`th rung at every scale.

use crate::pens::{AnnotationPen, AnnotationPens, Pen, PenTable};
use crate::units::{Micrometres, Scale};

/// ISO 128-20's preferred line widths in micrometres, thinnest first.
pub const ISO_128_WIDTHS_MICROMETRES: [u32; 9] = [130, 180, 250, 350, 500, 700, 1000, 1400, 2000];

/// The scales of the default table, as the `n` of 1:n, largest scale first.
pub const DEFAULT_SCALE_DENOMINATORS: [u32; 10] = [1, 2, 5, 10, 20, 50, 100, 200, 500, 1000];

/// The bottom rung of the ladder, the thinnest line printed.
const BOTTOM_RUNG_MICROMETRES: u32 = 130;

/// The scale at and above which (to 1:1) a pen prints at its own rung.
const REFERENCE_DENOMINATOR: u64 = 20;

/// The most rungs a pen can step down: the ladder has nine.
const MOST_STEPS: u32 = 8;

/// How many rungs of the ladder a pen steps down at `scale`: none up to 1:20, then one for each
/// factor of 2.5 in the denominator. Whole-number arithmetic only: the denominator `d` is at least
/// 20 times 2.5 to the power `k` exactly when `d * 2^k >= 20 * 5^k`.
#[must_use]
pub fn steps_down(scale: Scale) -> u32 {
    let denominator = u64::from(scale.denominator());
    let mut steps = 0_u32;
    while steps < MOST_STEPS {
        let next = steps.saturating_add(1);
        let lhs = denominator.checked_mul(2_u64.saturating_pow(next));
        let rhs = REFERENCE_DENOMINATOR.checked_mul(5_u64.saturating_pow(next));
        match (lhs, rhs) {
            (Some(lhs), Some(rhs)) if lhs >= rhs => steps = next,
            _ => break,
        }
    }
    steps
}

/// The width of the pen on rung `top` (from 0) after it has stepped down `steps` rungs, never below
/// the bottom rung.
fn rung(top: usize, steps: u32) -> Micrometres {
    // `steps` is at most 8 and a rung number at most 8: the cast-free conversion cannot fail.
    let drop = usize::try_from(steps).unwrap_or(usize::MAX);
    let index = top.saturating_sub(drop);
    let micrometres = ISO_128_WIDTHS_MICROMETRES
        .get(index)
        .copied()
        .unwrap_or(BOTTOM_RUNG_MICROMETRES);
    Micrometres::built_in(micrometres)
}

/// The default model-line pens: nine pens over ten scales, by the rule in this module.
#[must_use]
pub fn default_model_pens() -> PenTable {
    let scales: Vec<Scale> = DEFAULT_SCALE_DENOMINATORS
        .iter()
        .filter_map(|denominator| Scale::new(*denominator).ok())
        .collect();
    let pens = (0..ISO_128_WIDTHS_MICROMETRES.len())
        .map(|top| Pen {
            number: pen_number(top),
            name: format!("Pen {}", pen_number(top)),
            widths: scales
                .iter()
                .map(|scale| rung(top, steps_down(*scale)))
                .collect(),
        })
        .collect();
    PenTable::assemble(scales, pens)
}

/// The default annotation and perspective pens: nine pens, one width each, at the ladder's rungs.
#[must_use]
pub fn default_annotation_pens() -> AnnotationPens {
    let pens = (0..ISO_128_WIDTHS_MICROMETRES.len())
        .map(|top| AnnotationPen {
            number: pen_number(top),
            name: format!("Pen {}", pen_number(top)),
            width: rung(top, 0),
        })
        .collect();
    AnnotationPens::assemble(pens)
}

/// Pen numbers count from 1: the pen on rung `top` (from 0).
fn pen_number(top: usize) -> u16 {
    u16::try_from(top.saturating_add(1)).unwrap_or(u16::MAX)
}
