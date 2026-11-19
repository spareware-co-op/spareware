//! A cylinder's silhouette in parallel view whose domain ends on a ruling, by its closed form
//! (decision 32.NN). The normal of a cylinder is a multiple of e(u), so N·d is a multiple of
//! a·cos πu + b·sin πu (a = x̂·d, b = ŷ·d) along every ruling: its zero set is whole rulings, each
//! one branch across the domain in v. Whether a domain end is a ruling is decided exactly (Niven's
//! theorem and the exact signs of a, b, a + b and a − b); every other case stays with the search,
//! as it was. Before, a silhouette along a domain end could never be signed there, and the search
//! split the whole side to the floor in every box along it: the level fuzz target's three stored
//! timeouts stopped on its budget of 2^20, and on 2^24 too.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp,
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly, and the expected rulings are exact doubles"
)]

use geom_curve::Frame;
use geom_intersect::result::CurveEnd;
use geom_intersect::{Branch, IntersectError, LevelCurves, Silhouette, trace};
use geom_num::interval::Interval;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cylinder, SurfaceKind};

/// The floor and ceiling of the level fuzz target.
const FLOOR: f64 = 1e-5;
const CEILING: f64 = 1e-3;

/// Far past what a closed form spends and far short of what the search spends on a silhouette
/// along a domain end: a case the closed form leaves stops here instead of running on.
const BUDGET: u64 = 1 << 12;

/// The cylinder of radius `r` on `frame` over `u` (half-turns) and `v`.
fn cylinder(r: f64, frame: Frame<3>, u: (f64, f64), v: (f64, f64)) -> SurfaceKind {
    SurfaceKind::Cylinder(Cylinder::new([1.0, -2.0, 3.0], r, frame, (u, v)).unwrap())
}

fn standard() -> Frame<3> {
    Frame::standard().unwrap()
}

/// The silhouette along `d` on `surface`, and the work it took, under [`BUDGET`].
fn outline(surface: &SurfaceKind, d: [f64; 3]) -> (Result<LevelCurves, IntersectError>, u128) {
    let mut meter = WorkMeter::new(Budget::of_work(BUDGET));
    let found = trace(
        surface,
        &Silhouette { direction: d },
        FLOOR,
        CEILING,
        &mut meter,
    );
    (found, meter.used().work)
}

/// The ruling at `u` across v from `v0` to `v1`.
fn ruling(u: Interval, (v0, v1): (f64, f64)) -> Branch {
    Branch {
        vertices: vec![
            [u, Interval::point(v0).unwrap()],
            [u, Interval::point(v1).unwrap()],
        ],
        ends: [CurveEnd::Boundary, CurveEnd::Boundary],
        deviation: 0.0,
        passes: Vec::new(),
    }
}

/// The rulings at the exact doubles `us`, in order.
fn exactly(us: &[f64], v: (f64, f64)) -> LevelCurves {
    LevelCurves {
        branches: us
            .iter()
            .map(|&u| ruling(Interval::point(u).unwrap(), v))
            .collect(),
        flagged: Vec::new(),
    }
}

#[test]
fn the_level_targets_silhouette_along_its_domain_end_is_one_ruling() {
    // The level fuzz target's shape: a cylinder kept from 1/16 to 1/4 of a half-turn, seen along
    // (4, -4, -4), whose normal is square to the view exactly at u = 1/4, the domain's end.
    for r in [1.0, 1.5, 4.5] {
        let v = (-0.5, 0.5);
        let surface = cylinder(r, standard(), (0.0625, 0.25), v);
        let (found, used) = outline(&surface, [4.0, -4.0, -4.0]);
        assert_eq!(used, 1, "radius {r}");
        assert_eq!(found.unwrap(), exactly(&[0.25], v), "radius {r}");
    }
}

#[test]
fn every_eighth_of_a_turn_is_decided_exactly() {
    // A domain an eighth of a half-turn wide starting on k quarters, and a direction square to the
    // normal there: the start is a ruling and nothing else is, for each of the eight classes of
    // eighths of a turn, from above zero and from two half-turns below.
    let v = (-1.0, 2.0);
    for k in 0..8 {
        let d = match k % 4 {
            0 => [0.0, 3.0, 1.0],
            1 => [2.0, -2.0, 5.0],
            2 => [-0.5, 0.0, 7.0],
            _ => [0.75, 0.75, -1.0],
        };
        for start in [f64::from(k) / 4.0, f64::from(k) / 4.0 - 2.0] {
            let surface = cylinder(2.0, standard(), (start, start + 0.125), v);
            let (found, used) = outline(&surface, d);
            assert_eq!(used, 1, "{start}");
            assert_eq!(found.unwrap(), exactly(&[start], v), "{start}");
        }
    }
}

#[test]
fn every_ruling_is_an_end_ruling_plus_whole_half_turns() {
    let v = (0.0, 3.0);
    // A whole turn from 0, seen along y: rulings at 0, 1 and 2, the last the first across the seam.
    let whole = cylinder(1.0, standard(), (0.0, 2.0), v);
    let (found, used) = outline(&whole, [0.0, 1.0, 0.0]);
    assert_eq!(used, 1);
    assert_eq!(found.unwrap(), exactly(&[0.0, 1.0], v));
    // The same turn from -1/2 seen along x: -1/2, 1/2 and 3/2, which is -1/2 across the seam.
    let shifted = cylinder(1.0, standard(), (-0.5, 1.5), v);
    let (found, _) = outline(&shifted, [1.0, 0.0, 0.0]);
    assert_eq!(found.unwrap(), exactly(&[-0.5, 0.5], v));
    // Less than a turn, its upper end a ruling and its lower end none: the ruling a half-turn
    // below the upper end is found from it.
    let part = cylinder(1.0, standard(), (-0.9, 0.25), v);
    let (found, used) = outline(&part, [4.0, -4.0, -4.0]);
    assert_eq!(used, 1);
    assert_eq!(found.unwrap(), exactly(&[-0.75, 0.25], v));
    // Both ends rulings, a half-turn apart, on a domain that does not close.
    let half = cylinder(1.0, standard(), (0.25, 1.25), v);
    let (found, _) = outline(&half, [4.0, -4.0, 0.0]);
    assert_eq!(found.unwrap(), exactly(&[0.25, 1.25], v));
}

#[test]
fn a_ruling_inside_the_domain_stays_with_the_search() {
    // On an eighth inside the domain, and where tan πu = -2, off every eighth. Neither end is a
    // ruling, though for (1, 1, 0) the difference a - b is zero: only a whole number of quarters
    // reads it. The search answers each as it did before the closed form, at the same count: the
    // ruling's two crossings, each enclosed to the floor.
    let v = (-0.5, 0.5);
    let mut counts = Vec::new();
    for (d, exact) in [
        ([4.0, -4.0, -4.0], 0.25),
        ([1.0, 1.0, 0.0], 0.75),
        ([1.0, 0.5, 2.0], 1.0 - 2.0f64.atan() / core::f64::consts::PI),
    ] {
        let surface = cylinder(1.0, standard(), (0.0625, 1.0), v);
        let (found, used) = outline(&surface, d);
        let found = found.unwrap();
        assert!(found.flagged.is_empty(), "{found:?}");
        assert_eq!(found.branches.len(), 1, "{found:?}");
        let branch = &found.branches[0];
        let u = branch.vertices[0][0];
        let ends = [-0.5, 0.5].map(|w| [u, Interval::point(w).unwrap()]);
        assert_eq!(branch.vertices, ends, "{d:?}");
        assert_eq!(branch.ends, [CurveEnd::Boundary, CurveEnd::Boundary]);
        assert!(
            u.lo() - 1e-15 <= exact && exact <= u.hi() + 1e-15,
            "{u:?} {exact}"
        );
        // Within the floor in space: the speed across u is the radius times pi.
        assert!((u.hi() - u.lo()) * core::f64::consts::PI <= FLOOR, "{u:?}");
        counts.push(used);
    }
    assert_eq!(counts, [308, 384, 384]);
}

#[test]
fn what_the_closed_form_cannot_decide_stays_with_the_search() {
    let v = (-0.5, 0.5);
    let end = (0.0625, 0.25);
    // A view along the axis: g vanishes everywhere.
    let along = outline(&cylinder(1.0, standard(), end, v), [0.0, 0.0, 5.0]);
    // A factor too small for exact products, on the axis where it changes nothing.
    let small = outline(&cylinder(1.0, standard(), end, v), [4.0, -4.0, 1e-80]);
    // An end one step past the ruling: not a whole number of quarters, so not a ruling, and too
    // close to it for an enclosure to sign.
    let past = outline(
        &cylinder(1.0, standard(), (0.0625, 0.25f64.next_up()), v),
        [4.0, -4.0, -4.0],
    );
    // A domain end on a ruling, on frames whose unit vectors are not all doubles: x-hat is
    // (0.6, 0.8, 0) and y-hat (-0.8, 0.6, 0), so (7, 1, 0) is square to the normal at u = 1/4; and
    // about the axis (0, 1, 1), x-hat is (1, 0, 0) exactly and y-hat is not, and (1, 0, 0) is
    // square to the normal at u = 1/2.
    let turned = Frame::new([3.0, 4.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let rounded = outline(&cylinder(1.0, turned, end, v), [7.0, 1.0, 0.0]);
    let tilted = Frame::new([1.0, 0.0, 0.0], [0.0, 1.0, 1.0]).unwrap();
    let half = outline(&cylinder(1.0, tilted, (0.25, 0.5), v), [1.0, 0.0, 0.0]);
    // On the turned frame, a direction whose products with the lower ends of x-hat's enclosures
    // cancel exactly: read from those ends, a would be zero and u = 1 a ruling, but the true a is
    // about 2e-17, so the ruling lies within rounding of u = 1 and not on it.
    let (_, [low, _, _]) = turned.unit_vectors().unwrap();
    let ends = outline(
        &cylinder(1.0, turned, (0.875, 1.0), v),
        [low[1].lo(), -low[0].lo(), 0.0],
    );
    for (name, (found, used)) in [
        ("along", along),
        ("small", small),
        ("past", past),
        ("rounded", rounded),
        ("half", half),
        ("ends", ends),
    ] {
        assert_eq!(found, Err(IntersectError::BudgetExceeded), "{name}");
        assert!(used > u128::from(BUDGET), "{name}: {used}");
    }
}
