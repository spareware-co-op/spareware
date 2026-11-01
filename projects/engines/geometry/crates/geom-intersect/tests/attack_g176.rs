//! Attacks on piece G176's claims: a sweep that crosses itself is caught, its crossing marched
//! once, never twice from its two sheets; a crossing that shrinks to a cusp is covered by branches
//! or flags to its end, never dropped; a cusp is flagged, never passed as clean; 1,000 clean
//! surfaces pass with nothing to report; and the analytic kinds have nothing to search.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::{Basis as Leaf, Frame, Trimmed};
use geom_intersect::IntersectError;
use geom_intersect::march::Branch;
use geom_intersect::result::CurveEnd;
use geom_intersect::selfcross::{SelfCrossing, self_intersect};
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Curve, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Extrusion, Sphere, SurfaceKind, Torus};
use geom_test::Seed;
use geom_test::rng::SplitMix64;

const FLOOR: f64 = 1.0 / 128.0;
const CEILING: f64 = 1.0 / 16.0;

/// One turn, in half-turns.
const TURN: f64 = 2.0;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

fn mid(x: Interval) -> f64 {
    x.lo() / 2.0 + x.hi() / 2.0
}

/// The symmetric cubic arch with control points (−a, 0), (b, h), (−b, h), (a, 0) in x and z: its
/// x is s·(3(a − b)/2 + (2a + 6b)·s²) at u = ½ + s, so it loops, crossing itself at x = 0, when
/// b > a, at s² = 3(b − a)/(2(2a + 6b)), and has a cusp at u = ½ when b = a.
fn profile(a: f64, b: f64, h: f64) -> [[f64; 2]; 4] {
    [[-a, 0.0], [b, h], [-b, h], [a, 0.0]]
}

/// How far from u = ½ the arch (a, b) crosses itself, if it does.
fn crossing(a: f64, b: f64) -> Option<f64> {
    (b > a).then(|| (1.5 * (b - a) / (2.0 * a + 6.0 * b)).sqrt())
}

/// The sheet whose section at y = v is the arch of `rows`' first entry at v = −1 and its second at
/// v = 1, blended linearly between: cubic in u over [0, 1], linear in v over [−1, 1].
fn sweep(rows: [(f64, f64, f64); 2]) -> SurfaceKind {
    let across = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let along = Basis::new(1, vec![-1.0, -1.0, 1.0, 1.0]).unwrap();
    let sections = rows.map(|(a, b, h)| profile(a, b, h));
    let mut net = Vec::new();
    for i in 0..4 {
        for (y, section) in [-1.0, 1.0].into_iter().zip(&sections) {
            let [x, z] = section[i];
            net.push([x, y, z]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(across, along, &net).unwrap())
}

/// The arch a = 9/8, b = 13/8, h = 2: it crosses itself at u = ¼ and u = ¾, exactly, at z = 9/8.
const LOOP: (f64, f64, f64) = (9.0 / 8.0, 13.0 / 8.0, 2.0);

/// Whether `branch` follows the crossing of the arches (a, b(v)) at `v`, from either sheet: its
/// chord there passes within `near` of u = ½ ∓ s, both sheets at the same v.
fn follows(branch: &Branch, v: f64, s: f64, near: f64) -> bool {
    let points: Vec<[f64; 4]> = branch.vertices.iter().map(|x| x.map(mid)).collect();
    points.windows(2).any(|w| {
        let (p, q) = (w[0], w[1]);
        let (lo, hi) = (p[1].min(q[1]), p[1].max(q[1]));
        if !(lo - near <= v && v <= hi + near) {
            return false;
        }
        let t = if q[1] == p[1] {
            0.0
        } else {
            ((v - p[1]) / (q[1] - p[1])).clamp(0.0, 1.0)
        };
        let at = |k: usize| p[k] + t * (q[k] - p[k]);
        let (u1, u2) = (at(0), at(2));
        let low = 0.5 - s;
        let high = 0.5 + s;
        ((u1 - low).abs() < near && (u2 - high).abs() < near)
            || ((u1 - high).abs() < near && (u2 - low).abs() < near)
    })
}

/// Whether a flagged or unproven pair holds the crossing at `v`, from either sheet.
fn held(found: &SelfCrossing, v: f64, s: f64, near: f64) -> bool {
    let holds = |((a, b), (c, d)): ((f64, f64), (f64, f64)), u: f64| {
        a - near <= u && u <= b + near && c - near <= v && v <= d + near
    };
    let both = |first, second| {
        (holds(first, 0.5 - s) && holds(second, 0.5 + s))
            || (holds(first, 0.5 + s) && holds(second, 0.5 - s))
    };
    found.flagged.iter().any(|p| both(p.first, p.second))
        || found.unproven.iter().any(|p| both(p.first, p.second))
}

/// Whether the crossing at `v`, `s` from u = ½ on either side, is covered: followed by a branch, or
/// held by a flagged or unproven pair.
fn covered(found: &SelfCrossing, v: f64, s: f64, near: f64) -> bool {
    found.branches.iter().any(|x| follows(x, v, s, near)) || held(found, v, s, near)
}

/// The crossing of the sweep of `LOOP` along v, at u = ¼ and ¾: found from both sheets, each
/// branch on it, covered all along, and anything left undecided within `ends` of where it meets
/// the domain's edges v = ±1 on both sheets at once.
fn check_line(found: &SelfCrossing, ends: f64) {
    assert!(found.unproven.is_empty(), "{:?}", found.unproven);
    let mut sides = [0, 0];
    for branch in &found.branches {
        let mut side = None;
        for x in &branch.vertices {
            let [u1, v1, u2, v2] = x.map(mid);
            assert!((v1 - v2).abs() < FLOOR, "{x:?}");
            let here = if (u1 - 0.25).abs() < FLOOR && (u2 - 0.75).abs() < FLOOR {
                0
            } else if (u1 - 0.75).abs() < FLOOR && (u2 - 0.25).abs() < FLOOR {
                1
            } else {
                panic!("{x:?} is off the crossing");
            };
            assert!(side.is_none_or(|k| k == here), "{branch:?}");
            side = Some(here);
        }
        sides[side.unwrap()] += 1;
    }
    // Each crossing from both sheets: as many branches one way as the other.
    assert!(sides[0] >= 1 && sides[0] == sides[1], "{sides:?}");
    for pair in &found.flagged {
        for (_, (lo, hi)) in [pair.first, pair.second] {
            assert!(hi - lo <= ends, "{pair:?}");
            assert!(lo <= -1.0 + ends || 1.0 - ends <= hi, "{pair:?}");
        }
    }
    for k in 0..=64 {
        let v = -1.0 + f64::from(k) / 32.0;
        assert!(covered(found, v, 0.25, FLOOR), "{v}: {found:?}");
    }
}

#[test]
fn a_looped_sweep_crosses_itself_along_one_line() {
    let found = self_intersect(&sweep([LOOP, LOOP]), FLOOR, CEILING, &mut meter()).unwrap();
    check_line(&found, FLOOR);
    // One branch from each sheet, ending where the crossing meets the domain's edges on both
    // sheets at once: there no start can be proven, and a box a floor across is flagged.
    assert_eq!(found.branches.len(), 2, "{:?}", found.branches);
    for branch in &found.branches {
        assert_eq!(branch.ends, [CurveEnd::Flagged, CurveEnd::Flagged]);
    }
    assert_eq!(found.flagged.len(), 4, "{:?}", found.flagged);
}

#[test]
fn the_extrusion_kind_is_caught_the_same() {
    let (a, b, h) = LOOP;
    let points: Vec<[f64; 3]> = profile(a, b, h)
        .iter()
        .map(|[x, z]| [*x, 0.0, *z])
        .collect();
    let basis = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let curve = Trimmed::whole(Leaf::Nurbs(Curve::new(basis, &points).unwrap())).unwrap();
    let swept =
        SurfaceKind::Extrusion(Extrusion::new(curve, [0.0, 1.0, 0.0], (-1.0, 1.0)).unwrap());
    let found = self_intersect(&swept, FLOOR, CEILING, &mut meter()).unwrap();
    // The kind's bounds are looser than the NURBS form's, so more is left a floor across near
    // the ends; the crossing is still followed from both sheets all along.
    check_line(&found, 1.0 / 16.0);
}

#[test]
fn a_crossing_that_shrinks_to_a_cusp_is_covered_to_its_end() {
    // b runs from 13/8 at v = −1 to 1/2 at v = 1: the loop closes up into a cusp at b = a, where
    // v = −1/9, and is gone beyond.
    let (a, _, h) = LOOP;
    let found = self_intersect(&sweep([LOOP, (a, 0.5, h)]), FLOOR, CEILING, &mut meter()).unwrap();
    assert!(!found.branches.is_empty(), "{found:?}");
    let b = |v: f64| 13.0 / 8.0 + (0.5 - 13.0 / 8.0) * (v + 1.0) / 2.0;
    for k in 0..=56 {
        let v = -1.0 + f64::from(k) / 64.0;
        let s = crossing(a, b(v)).unwrap();
        let near = 4.0 * FLOOR;
        assert!(
            covered(&found, v, s, near),
            "{v}: the crossing at s = {s} was dropped: {found:?}"
        );
    }
    // Every branch is that crossing, and nothing past the cusp is reported.
    for branch in &found.branches {
        for x in &branch.vertices {
            let [u1, v1, u2, v2] = x.map(mid);
            assert!((v1 - v2).abs() < 4.0 * FLOOR, "{x:?}");
            assert!((u1 + u2 - 1.0).abs() < 4.0 * FLOOR, "{x:?}");
            assert!(v1 < -1.0 / 9.0 + 4.0 * FLOOR, "{x:?}");
        }
    }
    // What is left undecided is at the apex, u = ½, where its radius 3(a − b)²/(8h) is under four
    // floors, from b = 1.533 (v = −0.836) through the cusp to b = 0.717 (v = 0.614): a feature
    // under the floor, rightly left undecided.
    let at_apex = |((a, b), (c, d)): ((f64, f64), (f64, f64))| {
        a - 1.0 / 8.0 <= 0.5 && 0.5 <= b + 1.0 / 8.0 && -0.836 <= d && c <= 0.614
    };
    // Or where the crossing meets the domain's edge v = −1 on both sheets at once, a floor across.
    let at_edge = |(_, (c, d)): ((f64, f64), (f64, f64))| c == -1.0 && d - c <= FLOOR;
    for pair in &found.flagged {
        assert!(
            (at_apex(pair.first) && at_apex(pair.second))
                || (at_edge(pair.first) && at_edge(pair.second)),
            "{pair:?}"
        );
    }
    for pair in &found.unproven {
        assert!(at_apex(pair.first) && at_apex(pair.second), "{pair:?}");
    }
}

#[test]
fn a_cusp_is_flagged_never_passed_as_clean() {
    // b = a: a cusp at u = ½ along the whole sheet, where S_u vanishes.
    let (a, _, h) = LOOP;
    let found =
        self_intersect(&sweep([(a, a, h), (a, a, h)]), FLOOR, CEILING, &mut meter()).unwrap();
    assert!(!found.flagged.is_empty(), "{found:?}");
    for pair in &found.flagged {
        for ((lo, hi), _) in [pair.first, pair.second] {
            assert!(lo - 1.0 / 4.0 <= 0.5 && 0.5 <= hi + 1.0 / 4.0, "{pair:?}");
        }
    }
}

fn rng(label: &str, seed: u64) -> SplitMix64 {
    SplitMix64::new(geom_test::seed::resolve(label, Seed(seed)).unwrap())
}

fn unit(rng: &mut SplitMix64) -> f64 {
    (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64
}

/// A bicubic graph z = f(x, y) over [−1, 1]², its heights in [−½, ½].
fn graph(g: &mut SplitMix64) -> SurfaceKind {
    let cubic = || Basis::new(3, vec![-1.0, -1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for i in 0..4 {
        for j in 0..4 {
            let at = |k: i32| -1.0 + 2.0 * f64::from(k) / 3.0;
            net.push([at(i), at(j), unit(g) - 0.5]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(cubic(), cubic(), &net).unwrap())
}

/// A sweep of arches that never loop, b at most a − 3/5 at both ends and so between, and h at
/// most 3/2: the arch's apex has radius 3(a − b)²/(8h) ≥ 0.09, so every feature is many floors
/// across (a sharper one is rightly left undecided at the floor).
fn arch(g: &mut SplitMix64) -> SurfaceKind {
    let mut row = || {
        let a = 1.0 + unit(g);
        (a, (a - 0.6) * unit(g), 0.5 + unit(g))
    };
    sweep([row(), row()])
}

#[test]
fn a_thousand_clean_surfaces_pass() {
    let mut g = rng("selfcross::clean", 0x6176_0001);
    for k in 0..1000 {
        let surface = if k % 2 == 0 {
            graph(&mut g)
        } else {
            arch(&mut g)
        };
        let found = self_intersect(&surface, FLOOR, CEILING, &mut meter()).unwrap();
        assert_eq!(
            found,
            SelfCrossing {
                branches: Vec::new(),
                unproven: Vec::new(),
                flagged: Vec::new(),
            },
            "{k}: {surface:?}"
        );
    }
}

#[test]
fn the_analytic_kinds_have_nothing_to_search() {
    let frame = || Frame::standard().unwrap();
    let kinds = [
        SurfaceKind::Sphere(
            Sphere::new([0.0; 3], 5.0, frame(), ((0.0, TURN), (-0.5, 0.5))).unwrap(),
        ),
        // A spindle torus over both turns: its apple and lemon meet only at the poles.
        SurfaceKind::Torus(
            Torus::new([0.0; 3], 1.0, 2.0, frame(), ((0.0, TURN), (-1.0, 1.0))).unwrap(),
        ),
        // A cone across its apex: two nappes meeting at a point.
        SurfaceKind::Cone(
            Cone::new([0.0; 3], 1.0, 0.5, frame(), ((0.0, TURN), (-4.0, 4.0))).unwrap(),
        ),
    ];
    for surface in kinds {
        // Nothing is read: even a budget of nothing is enough.
        let mut none = WorkMeter::new(Budget::of_work(0));
        let found = self_intersect(&surface, FLOOR, CEILING, &mut none).unwrap();
        assert!(
            found.branches.is_empty() && found.flagged.is_empty(),
            "{found:?}"
        );
    }
}

#[test]
fn a_budget_stops_it_and_bad_tolerances_are_refused() {
    let surface = sweep([LOOP, LOOP]);
    let mut small = WorkMeter::new(Budget::of_work(8));
    assert_eq!(
        self_intersect(&surface, FLOOR, CEILING, &mut small),
        Err(IntersectError::BudgetExceeded)
    );
    for (floor, ceiling) in [
        (0.0, 1.0),
        (-1.0, 1.0),
        (f64::NAN, 1.0),
        (1.0, 0.0),
        (1.0, f64::NAN),
    ] {
        assert_eq!(
            self_intersect(&surface, floor, ceiling, &mut meter()),
            Err(IntersectError::InvalidInput)
        );
    }
    let once = self_intersect(&surface, FLOOR, CEILING, &mut meter()).unwrap();
    let again = self_intersect(&surface, FLOOR, CEILING, &mut meter()).unwrap();
    assert_eq!(once, again);
}
