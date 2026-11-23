//! What the tests share: the project library a generated recipe is checked against, lengths and
//! areas from millimetres, a family of a template, and the quantities a spec should measure,
//! worked out here by hand from the spec's sizes (never read from the generator).

#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::manual_midpoint,
    reason = "test code; each test file uses part of this fixture"
)]

use construct_categories::CategoryTable;
use construct_doorgen::{Family, Generated, Quantities, Refusal, Size, Spec, Template, generate};
use construct_families::Library;
use construct_materials::Materials;
use construct_params::{Area, Length};
use kit_core::Id;

/// The built-in categories and the default metric materials.
pub struct Fixture {
    pub categories: CategoryTable,
    pub materials: Materials,
}

impl Fixture {
    pub fn new() -> Self {
        Self {
            categories: CategoryTable::new(),
            materials: Materials::default_metric(),
        }
    }

    pub fn library(&self) -> Library<'_> {
        Library {
            categories: &self.categories,
            materials: &self.materials,
        }
    }
}

/// Whole millimetres.
pub fn mm(mm: i32) -> Length {
    Length::from_mm(mm)
}

/// Tenths of a millimetre (640 steps each), for sizes off the whole millimetre.
pub fn tenths(tenths: i64) -> Length {
    Length::from_steps(tenths * 640)
}

/// A length a number of grid steps from another.
pub fn plus(length: Length, steps: i64) -> Length {
    Length::from_steps(length.steps() + steps)
}

/// Whole square millimetres.
pub fn mm2(mm2: i64) -> Area {
    Area::from_mm2(mm2)
}

/// An overall size.
pub fn overall(width: Length, height: Length) -> Size {
    Size::Overall { width, height }
}

/// A family of `template` with a test id.
pub fn family(template: Template) -> Family {
    Family {
        id: Id::parse(&format!("project.test.{}", template.stem())).unwrap(),
        name: format!("Generated {}", template.stem()),
        template,
    }
}

/// Generates one family of `specs`' template.
pub fn make(specs: &[Spec]) -> Result<Generated, Refusal> {
    let fixture = Fixture::new();
    generate(&family(specs[0].template), specs, &fixture.library())
}

/// The template's own defaults, in steps: frame, frame depth, leaf, sash, sash depth, track gap,
/// glass.
pub struct Parts {
    pub frame: i64,
    pub depth: i64,
    pub leaf: i64,
    pub sash: i64,
    pub glass: i64,
}

/// The parts a spec measures with: its own or the template's defaults.
pub fn parts(spec: &Spec) -> Parts {
    let or = |given: Option<Length>, default: i32| given.unwrap_or(mm(default)).steps();
    match spec.template {
        Template::SingleSwingDoor => Parts {
            frame: or(spec.frame, 40),
            depth: or(spec.frame_depth, 100),
            leaf: or(spec.leaf, 40),
            sash: 0,
            glass: 0,
        },
        Template::FixedWindow => Parts {
            frame: or(spec.frame, 50),
            depth: or(spec.frame_depth, 70),
            leaf: 0,
            sash: 0,
            glass: or(spec.glass, 24),
        },
        Template::SlidingWindow => Parts {
            frame: or(spec.frame, 50),
            depth: or(spec.frame_depth, 100),
            leaf: 0,
            sash: or(spec.sash, 50),
            glass: or(spec.glass, 20),
        },
    }
}

/// What an overall `w` by `h` (steps) should measure, by hand:
/// - a swing door: clear width w - 2f - leaf, clear height h - f, leaf (w - 2f)(h - f);
/// - a fixed light: clear and glass (w - 2f) by (h - 2f);
/// - a sliding window, each sash spanning s' = (w - 2f + s) / 2: clear width s' - s, clear height
///   h - 2f, leaves 2 s' (h - 2f), glass 2 (s' - 2s)(h - 2f - 2s).
pub fn expected(spec: &Spec, w: i64, h: i64) -> Quantities {
    let p = parts(spec);
    let sq = |a: i64, b: i64| Some(Area::from_square_steps(i128::from(a) * i128::from(b)));
    let l = Length::from_steps;
    match spec.template {
        Template::SingleSwingDoor => Quantities {
            width: l(w),
            height: l(h),
            clear_width: l(w - 2 * p.frame - p.leaf),
            clear_height: l(h - p.frame),
            leaf_area: sq(w - 2 * p.frame, h - p.frame),
            glass_area: None,
            depth: l(p.depth),
        },
        Template::FixedWindow => Quantities {
            width: l(w),
            height: l(h),
            clear_width: l(w - 2 * p.frame),
            clear_height: l(h - 2 * p.frame),
            leaf_area: None,
            glass_area: sq(w - 2 * p.frame, h - 2 * p.frame),
            depth: l(p.depth),
        },
        Template::SlidingWindow => {
            let span = (w - 2 * p.frame + p.sash) / 2;
            Quantities {
                width: l(w),
                height: l(h),
                clear_width: l(span - p.sash),
                clear_height: l(h - 2 * p.frame),
                leaf_area: sq(2 * span, h - 2 * p.frame),
                glass_area: sq(2 * (span - 2 * p.sash), h - 2 * p.frame - 2 * p.sash),
                depth: l(p.depth),
            }
        }
    }
}
