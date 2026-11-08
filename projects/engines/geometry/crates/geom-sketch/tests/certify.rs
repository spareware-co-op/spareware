//! Tests of `geom_sketch::certify` (piece G227): chained Krawczyk certificates.
//!
//! The board's done-when: "Certification in plan order with upstream boxes as inputs, block
//! Krawczyk on geom-linalg, redundant groups certified without their dependent equations, Unproven
//! with reasons; proven radii contain high-precision solutions; a 200-constraint family in 2 ms."
//! The benchmark is `benches/certify.rs`.
//!
//! What this file proves: a point at two distances from two fixed points is certified, and its
//! proven box holds the exact solution (computed to 30 digits with `dashu` integers); a duplicated
//! equation is set aside and the block is still certified; a second point placed from the first is
//! certified in its own block on the first's proven box; a double root (two circles touching) is not
//! certified, and says why; the 200-constraint family of the solver's benchmark is certified in every
//! block; a certificate is the same every time; and a block of more than `BLOCK_LIMIT` unknowns is
//! refused as too large, charged only for reading it, while one of exactly the limit is attempted.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "tests, written as the definitions read; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::fmt::Write;

use dashu_int::UBig;
use dashu_ratio::RBig;
use geom_num::interval::Interval;
use geom_num::{GridStep, P3};
use geom_run::{Budget, WorkMeter};
use geom_sketch::certify::{
    BLOCK_LIMIT, Block, Certificate, Certified, Certifier, Reason, certify,
};
use geom_sketch::commit::commit;
use geom_sketch::equation::Assembly;
use geom_sketch::model::{
    Amount, Constraint, ConstraintKind, Entity, EntityKind, Extent, ItemId, Sketch, SketchError,
    Standing,
};
use geom_sketch::plane::{Lattice, PlaneFrame, SketchPlane};
use geom_sketch::solve::solve;
use geom_sketch::text::parse;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn pid(key: u64) -> ItemId {
    ItemId::new(1, key)
}

fn cid(key: u64) -> ItemId {
    ItemId::new(3, key)
}

fn reference(key: u64, x: f64, y: f64) -> Entity {
    Entity {
        id: pid(key),
        kind: EntityKind::Reference { x, y },
        construction: false,
    }
}

fn point(key: u64, x: f64, y: f64) -> Entity {
    Entity {
        id: pid(key),
        kind: EntityKind::Point { x, y },
        construction: false,
    }
}

fn distance(key: u64, a: u64, b: u64, d: f64) -> Constraint {
    Constraint {
        id: cid(key),
        kind: ConstraintKind::Distance {
            a: pid(a),
            b: pid(b),
            distance: Amount::Value(d),
        },
        standing: Standing::Driving,
        priority: 0,
    }
}

fn build(entities: Vec<Entity>, constraints: Vec<Constraint>) -> Sketch {
    let lattice = Lattice::new(P3::new(0, 0, 0).unwrap(), [1, 0, 0], [0, 1, 0]).unwrap();
    let mut s = Sketch::new(SketchPlane::new(PlaneFrame::Lattice(lattice), GridStep::Q0));
    for e in entities {
        s.add_entity(e).unwrap();
    }
    for c in constraints {
        s.add_constraint(c).unwrap();
    }
    s
}

/// Solves a sketch, commits it and certifies the commit.
fn certified(s: &Sketch) -> Certified {
    let solved = solve(s, &mut meter()).unwrap().sketch;
    let committed = commit(&solved, &mut meter()).unwrap();
    certify(&solved, &committed, &mut meter()).unwrap()
}

fn proven(b: &Block) -> f64 {
    match b.certificate {
        Certificate::Proven { radius } => radius,
        other @ Certificate::Unproven(_) => panic!("not proven: {other:?}"),
    }
}

fn box_of(c: &Certified, owner: ItemId, slot: u8) -> Interval {
    c.boxes
        .iter()
        .find(|(o, s, _)| *o == owner && *s == slot)
        .map_or_else(
            || panic!("no proven box for {owner:?} slot {slot}"),
            |(_, _, b)| *b,
        )
}

/// Whether the interval holds the rational `[lo, hi]`.
fn holds(b: Interval, lo: &RBig, hi: &RBig) -> bool {
    RBig::try_from(b.lo()).unwrap() <= *lo && RBig::try_from(b.hi()).unwrap() >= *hi
}

/// The integer square root, by bisection: the largest `s` with `s * s <= n`.
fn isqrt(n: &UBig) -> UBig {
    let (mut lo, mut hi) = (UBig::ZERO, n.clone() + UBig::ONE);
    while hi.clone() - lo.clone() > UBig::ONE {
        let middle = (lo.clone() + hi.clone()) / UBig::from(2_u32);
        if middle.clone() * middle.clone() <= *n {
            lo = middle;
        } else {
            hi = middle;
        }
    }
    lo
}

/// sqrt(12.75) to 30 digits: the interval `[s / 10^30, (s + 1) / 10^30]` holding it.
fn root_of_12_75() -> (RBig, RBig) {
    let ten30 = UBig::from(10_u32).pow(30);
    let scaled = UBig::from(1275_u32) * UBig::from(10_u32).pow(58);
    let s = isqrt(&scaled);
    let lo = RBig::from_parts(s.clone().into(), ten30.clone());
    let hi = RBig::from_parts((s + UBig::ONE).into(), ten30);
    (lo, hi)
}

#[test]
fn a_point_at_two_distances_is_certified_and_its_box_holds_the_exact_solution() {
    // P is 5 from (0, 0) and from (7, 0): P = (3.5, sqrt(12.75)). The commit rounds P to the grid,
    // about a step from the solution; the proof still finds the one exact solution near it.
    let s = build(
        vec![
            reference(1, 0.0, 0.0),
            reference(2, 7.0, 0.0),
            point(3, 3.4, 3.7),
        ],
        vec![distance(1, 1, 3, 5.0), distance(2, 2, 3, 5.0)],
    );
    let got = certified(&s);
    assert_eq!(got.blocks.len(), 1, "{got:?}");
    let radius = proven(&got.blocks[0]);
    assert!(radius > 0.0 && radius < 2.0, "{radius}");
    assert_eq!(got.blocks[0].unknowns, 2);
    assert_eq!(got.blocks[0].set_aside, Vec::new());
    let x = RBig::from_parts(7_i32.into(), UBig::from(2_u32));
    assert!(holds(box_of(&got, pid(3), 0), &x, &x));
    let (lo, hi) = root_of_12_75();
    assert!(holds(box_of(&got, pid(3), 1), &lo, &hi));
}

#[test]
fn a_duplicated_equation_is_set_aside_and_the_block_is_still_certified() {
    let s = build(
        vec![
            reference(1, 0.0, 0.0),
            reference(2, 7.0, 0.0),
            point(3, 3.4, 3.7),
        ],
        vec![
            distance(1, 1, 3, 5.0),
            distance(2, 2, 3, 5.0),
            distance(3, 1, 3, 5.0),
        ],
    );
    let got = certified(&s);
    let block = got
        .blocks
        .iter()
        .find(|b| !b.set_aside.is_empty())
        .unwrap_or_else(|| panic!("no equation set aside: {got:?}"));
    assert_eq!(block.unknowns, 2);
    assert_eq!(block.set_aside.len(), 1);
    assert!(proven(block) < 2.0);
    let (lo, hi) = root_of_12_75();
    assert!(holds(box_of(&got, pid(3), 1), &lo, &hi));
}

#[test]
fn a_second_point_is_certified_on_the_first_ones_proven_box() {
    // At a scale where a grid step is small against the lengths, as it is in a real sketch: P is
    // 500 from (0, 0) and from (700, 0), and Q is 300 from P and 600 from (700, 0). The equations
    // form one coupled block of four unknowns, certified together on the committed geometry; and a
    // third point R, 200 from Q only, hangs from it in a block of its own that takes Q's proven box
    // as its input.
    let s = build(
        vec![
            reference(1, 0.0, 0.0),
            reference(2, 700.0, 0.0),
            point(3, 340.0, 370.0),
            point(4, 800.0, 800.0),
            point(5, 900.0, 900.0),
        ],
        vec![
            distance(1, 1, 3, 500.0),
            distance(2, 2, 3, 500.0),
            distance(3, 3, 4, 300.0),
            distance(4, 2, 4, 600.0),
            distance(5, 4, 5, 200.0),
        ],
    );
    let got = certified(&s);
    assert!(got.blocks.len() >= 2, "{got:?}");
    let radii: Vec<f64> = got.blocks.iter().map(proven).collect();
    assert!(radii.iter().all(|r| r.is_finite() && *r < 5.0), "{radii:?}");
    // R is under-determined by one equation (a circle about Q): its block fixes it on the circle
    // through its committed point, and the box it proves is a point's width wide at most.
    let r = box_of(&got, pid(5), 0);
    assert!(r.hi() - r.lo() < 1.0, "{r:?}");
}

#[test]
fn two_circles_that_touch_are_not_certified_and_the_reason_is_given() {
    // Distances 5 and 5 from (0, 0) and (10, 0): the circles touch at (5, 0), a double root, where
    // the Jacobian is singular.
    let s = build(
        vec![
            reference(1, 0.0, 0.0),
            reference(2, 10.0, 0.0),
            point(3, 5.0, 0.3),
        ],
        vec![distance(1, 1, 3, 5.0), distance(2, 2, 3, 5.0)],
    );
    let solved = solve(&s, &mut meter()).map(|r| r.sketch);
    // Whether or not the solver reaches the double root, no certificate may claim it.
    if let Ok(sketch) = solved {
        let committed = commit(&sketch, &mut meter()).unwrap();
        let got = certify(&sketch, &committed, &mut meter()).unwrap();
        for block in &got.blocks {
            assert!(
                matches!(
                    block.certificate,
                    Certificate::Unproven(Reason::Singular | Reason::NoInclusion | Reason::Refused)
                ),
                "{block:?}"
            );
        }
    }
}

fn family() -> Sketch {
    const HEAD: &str = "sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n";
    let mut text = HEAD.to_owned();
    for i in 0..40_u32 {
        let (a, b) = (10 + 2 * i, 11 + 2 * i);
        let at = f64::from(i) * 1000.0;
        for (key, (x, y)) in [(0.5, 0.5), (401.0, 3.0), (399.0, 298.0), (2.0, 301.0)]
            .iter()
            .enumerate()
        {
            writeln!(text, "entity {a}.{key} point {} {y}", at + x).unwrap();
        }
        for (key, (p, q)) in [(0, 1), (1, 2), (3, 2), (0, 3)].iter().enumerate() {
            writeln!(text, "entity {b}.{key} line {a}.{p} {a}.{q} segment").unwrap();
        }
    }
    for i in 0..40_u32 {
        let (a, b) = (10 + 2 * i, 11 + 2 * i);
        let at = f64::from(i) * 1000.0;
        let c = 100 + i;
        writeln!(text, "constraint {c}.0 driving 0 horizontal {b}.0").unwrap();
        writeln!(text, "constraint {c}.1 driving 0 vertical {b}.1").unwrap();
        writeln!(text, "constraint {c}.2 driving 0 parallel {b}.0 {b}.2").unwrap();
        writeln!(text, "constraint {c}.3 driving 0 fix {a}.0 0 {at}").unwrap();
        writeln!(text, "constraint {c}.4 driving 0 distance {a}.0 {a}.1 400").unwrap();
    }
    parse(&text).unwrap()
}

#[test]
fn a_family_of_rectangles_is_certified_in_every_block_and_the_certificate_repeats() {
    let s = family();
    assert_eq!(s.constraints().len(), 200);
    let first = certified(&s);
    assert_ne!(first.blocks.len(), 0);
    for block in &first.blocks {
        assert!(
            matches!(block.certificate, Certificate::Proven { .. }),
            "{block:?}"
        );
    }
    assert_eq!(certified(&s), first);
}

#[test]
fn a_proven_radius_reaches_past_the_solution_and_is_pinned() {
    // The same point: P = (3.5, 3.5707...) solved, its x a tie that rounds to even, so it is
    // committed at (4, 4). The proven box holds P, so the radius is at least the farther of 0.5
    // and 0.4293..., and reaches past it by what the interval Jacobian over the box leaves. Its
    // value for this sketch is pinned, bit for bit, as the certificate is the same on every
    // target: a change to the epsilon-inflation, the narrowing or the radius moves it.
    let s = build(
        vec![
            reference(1, 0.0, 0.0),
            reference(2, 7.0, 0.0),
            point(3, 3.4, 3.7),
        ],
        vec![distance(1, 1, 3, 5.0), distance(2, 2, 3, 5.0)],
    );
    let got = certified(&s);
    let committed = commit(&solve(&s, &mut meter()).unwrap().sketch, &mut meter()).unwrap();
    let p = committed.point(pid(3)).unwrap().rational();
    let ([x, y], w) = p.coordinates();
    assert_eq!((x, y, w), (4, 4, 1), "the commit rounds P to (4, 4)");
    let radius = proven(&got.blocks[0]);
    assert!((0.5..1.0).contains(&radius), "{radius}");
    assert_eq!(
        radius.to_bits(),
        0.813_245_103_167_033_f64.to_bits(),
        "{radius}"
    );
    for slot in 0..2 {
        let b = box_of(&got, pid(3), slot);
        assert!(
            b.lo() >= 4.0 - radius && b.hi() <= 4.0 + radius,
            "slot {slot}: {b:?}"
        );
    }
    // The boxes' ends are pinned too: each refinement narrows both ends of every box, so a box
    // not narrowed from above (or below) moves them even where the radius, set by the farther
    // end, stays.
    let ends = [
        (0x4009_9d6f_b33f_c113_u64, 0x400e_6290_4cc0_3eed_u64),
        (0x4009_7e79_59f2_1fa3_u64, 0x400d_aaf5_e042_8dc3_u64),
    ];
    for (slot, (lo, hi)) in (0_u8..).zip(ends) {
        let b = box_of(&got, pid(3), slot);
        assert_eq!(
            (b.lo().to_bits(), b.hi().to_bits()),
            (lo, hi),
            "slot {slot}: {b:?}"
        );
    }
}

#[test]
fn a_box_that_leaves_the_committed_point_behind_is_still_judged_over_the_segment_to_it() {
    // P is 35.8 from (-42, 46) and 27.6 from (-44, -17), drawn at (-46.2, 10.5) and committed at
    // (-46, 10), about half a step from the solution near (-46.24, 10.49). The first box is not
    // mapped into itself; the image the operator gives moves far enough in one coordinate that
    // the next box no longer holds the committed point there. The Jacobian is then enclosed over
    // the box and the committed point together (the mean value theorem takes it on the segment
    // between them), so the box proved on a later inflation, and its radius, are those of that
    // hull; a Jacobian over the box alone would prove a narrower box with no theorem behind it.
    let s = build(
        vec![
            reference(1, -42.0, 46.0),
            reference(2, -44.0, -17.0),
            point(3, -46.2, 10.5),
        ],
        vec![distance(1, 1, 3, 35.8), distance(2, 2, 3, 27.6)],
    );
    let committed = commit(&s, &mut meter()).unwrap();
    let got = certify(&s, &committed, &mut meter()).unwrap();
    assert_eq!(got.blocks.len(), 1, "{got:?}");
    let radius = proven(&got.blocks[0]);
    assert_eq!(radius.to_bits(), 0x3feb_13d1_d688_9c81_u64, "{radius}");
    // The exact solution, to within a double's rounding, lies in the box.
    for (slot, want) in [(0_u8, -46.511_900_596_575_2), (1, 10.485_457_161_796_04)] {
        let b = box_of(&got, pid(3), slot);
        assert!(b.lo() < want && want < b.hi(), "slot {slot}: {b:?}");
    }
}

#[test]
fn a_point_no_solution_is_near_is_not_certified() {
    // P is to be 10 from (0, 0) and from (100, 0): the two circles are 80 apart and never meet.
    // Committed where it is drawn, at (50, 5), the Jacobian is regular, but no box around it is
    // mapped into itself, so nothing is proved and no box is given.
    let s = build(
        vec![
            reference(1, 0.0, 0.0),
            reference(2, 100.0, 0.0),
            point(3, 50.0, 5.0),
        ],
        vec![distance(1, 1, 3, 10.0), distance(2, 2, 3, 10.0)],
    );
    let committed = commit(&s, &mut meter()).unwrap();
    let got = certify(&s, &committed, &mut meter()).unwrap();
    assert_eq!(got.blocks.len(), 1, "{got:?}");
    assert_eq!(
        got.blocks[0].certificate,
        Certificate::Unproven(Reason::NoInclusion),
        "{got:?}"
    );
    assert!(
        got.boxes.iter().all(|(owner, _, _)| *owner != pid(3)),
        "{got:?}"
    );
}

#[test]
fn the_assembly_counts_its_rules() {
    let s = build(
        vec![
            reference(1, 0.0, 0.0),
            reference(2, 7.0, 0.0),
            point(3, 3.4, 3.7),
        ],
        vec![distance(1, 1, 3, 5.0), distance(2, 2, 3, 5.0)],
    );
    assert_eq!(Assembly::new(&s).unwrap().rule_count(), 2);
    let one = build(
        vec![reference(1, 0.0, 0.0), point(3, 3.4, 3.7)],
        vec![distance(1, 1, 3, 5.0)],
    );
    assert_eq!(Assembly::new(&one).unwrap().rule_count(), 1);
}

/// A line from (0, 0) to (10, 1), and a second line from (3, 2) to (7, -1) both of whose ends are to
/// be on the first: one rule of two residuals, each of which depends on the first line's ends.
fn collinear_pair() -> Sketch {
    let line = |key: u64, a: u64, b: u64| Entity {
        id: ItemId::new(2, key),
        kind: EntityKind::Line {
            start: pid(a),
            end: pid(b),
            extent: Extent::Segment,
        },
        construction: false,
    };
    build(
        vec![
            point(1, 0.0, 0.0),
            point(2, 10.0, 1.0),
            point(3, 3.0, 2.0),
            point(4, 7.0, -1.0),
            line(1, 1, 2),
            line(2, 3, 4),
        ],
        vec![Constraint {
            id: cid(1),
            kind: ConstraintKind::Collinear {
                a: ItemId::new(2, 1),
                b: ItemId::new(2, 2),
            },
            standing: Standing::Driving,
            priority: 0,
        }],
    )
}

#[test]
fn a_rules_derivatives_are_kept_apart_by_residual_and_match_its_differences() {
    // Central differences of the rule's own residuals, which are quadratic in the coordinates, so
    // the difference is the derivative to rounding. Each (variable, residual) comes once, in the
    // doubles the solver uses and in the enclosures the certificate uses, and the residual a slope
    // belongs to is the one whose difference it matches.
    let s = collinear_pair();
    let a = Assembly::new(&s).unwrap();
    let x = a.start().unwrap();
    let point_box =
        |x: &[f64]| -> Vec<Interval> { x.iter().map(|v| Interval::point(*v).unwrap()).collect() };
    let values = |x: &[f64]| -> Vec<f64> {
        a.rule_values(0, &point_box(x))
            .unwrap()
            .iter()
            .map(|r| f64::midpoint(r.lo(), r.hi()))
            .collect()
    };
    let n = x.len();
    let slopes = a.slopes(0, &x).unwrap();
    let all: Vec<usize> = (0..n).collect();
    let jet = a.jet(0, &point_box(&x), &all).unwrap();
    assert_eq!(jet.residuals.len(), 2);
    let mut shared = 0;
    for i in 0..n {
        let h = 1.0e-3;
        let (mut up, mut down) = (x.clone(), x.clone());
        up[i] += h;
        down[i] -= h;
        let (vu, vd) = (values(&up), values(&down));
        let mut both = 0;
        for e in 0..2 {
            let want = (vu[e] - vd[e]) / (2.0 * h);
            let found: Vec<f64> = slopes
                .iter()
                .filter(|(v, f, _)| *v == i && *f == e)
                .map(|(_, _, d)| *d)
                .collect();
            assert!(found.len() <= 1, "variable {i}, residual {e}: {found:?}");
            let got = found.first().copied().unwrap_or(0.0);
            assert!(
                (got - want).abs() < 1e-6,
                "variable {i}, residual {e}: {got} against {want}"
            );
            let enclosed: Vec<Interval> = jet
                .partials
                .iter()
                .filter(|(v, f, _)| *v == i && *f == e)
                .map(|(_, _, d)| *d)
                .collect();
            assert!(
                enclosed.len() <= 1,
                "variable {i}, residual {e}: {enclosed:?}"
            );
            let got = enclosed
                .first()
                .copied()
                .unwrap_or_else(|| Interval::point(0.0).unwrap());
            assert!(
                got.lo() - 1e-6 <= want && want <= got.hi() + 1e-6,
                "variable {i}, residual {e}: {got:?} against {want}"
            );
            if want.abs() > 1e-9 {
                both += 1;
            }
        }
        if both == 2 {
            shared += 1;
        }
    }
    // The first line's four coordinates enter both residuals.
    assert_eq!(shared, 4);
}

#[test]
fn a_commit_the_proof_cannot_reach_is_left_unproven_with_no_box() {
    // The same two distances from (0, 0) and (7, 0), committed unsolved where the point is drawn,
    // at (3, 3): 0.5707... steps from P in y. The interval Jacobian over a box that wide does not
    // map it into itself, so the block is refused as NoInclusion and no box is given for P, never
    // a box that might miss it.
    let s = build(
        vec![
            reference(1, 0.0, 0.0),
            reference(2, 7.0, 0.0),
            point(3, 3.0, 3.0),
        ],
        vec![distance(1, 1, 3, 5.0), distance(2, 2, 3, 5.0)],
    );
    let committed = commit(&s, &mut meter()).unwrap();
    let got = certify(&s, &committed, &mut meter()).unwrap();
    assert_eq!(got.blocks.len(), 1, "{got:?}");
    assert_eq!(
        got.blocks[0].certificate,
        Certificate::Unproven(Reason::NoInclusion),
        "{got:?}"
    );
    assert!(
        got.boxes.iter().all(|(owner, _, _)| *owner != pid(3)),
        "{got:?}"
    );
}

/// A ring of `n` points, each at a distance from one fixed point and from the next, the last joined
/// to the first. Its `2n` equations in `2n` unknowns are one square block (each point's place
/// leans on its neighbours' all the way round). Only its structure matters here: the points sit in
/// a row 10 apart and the stated distances are round numbers, not a solved figure.
fn ring(n: u64) -> Sketch {
    let mut entities = vec![reference(1000, 0.0, 1000.0)];
    let mut constraints = Vec::new();
    for k in 1..=n {
        #[allow(clippy::cast_precision_loss, reason = "a small count")]
        let x = 10.0 * k as f64;
        entities.push(point(k, x, 0.0));
        constraints.push(distance(k, 1000, k, 1000.0));
        constraints.push(distance(2000 + k, k, k % n + 1, 10.0));
    }
    build(entities, constraints)
}

#[test]
fn a_block_past_the_limit_is_not_attempted_and_is_charged_only_for_reading_it() {
    // 101 points: a square block of 202 equations in 202 unknowns, past BLOCK_LIMIT. It is left
    // Unproven(TooLarge) with no box, and the meter is charged for its equations and unknowns
    // only, 202 + 202, with no factorisation: a budget of 404 covers the certificate and one of
    // 403 stops it.
    let s = ring(101);
    let committed = commit(&s, &mut meter()).unwrap();
    let certifier = Certifier::new(&s, &mut meter()).unwrap();
    let got = certifier
        .certify(&s, &committed, &mut WorkMeter::new(Budget::of_work(404)))
        .unwrap();
    assert_eq!(got.blocks.len(), 1, "{got:?}");
    assert_eq!(got.blocks[0].unknowns, 202);
    assert_eq!(got.blocks[0].rules.len(), 202);
    assert_eq!(
        got.blocks[0].certificate,
        Certificate::Unproven(Reason::TooLarge)
    );
    assert_eq!(got.boxes, Vec::new());
    assert_eq!(
        certifier.certify(&s, &committed, &mut WorkMeter::new(Budget::of_work(403))),
        Err(SketchError::Stopped)
    );
}

#[test]
fn a_block_of_exactly_the_limit_is_attempted() {
    // 201 points in a row, 10 apart, each 10 from the next: 200 equations in 402 unknowns, one
    // under-determined part. Its independent subsystem is the 200 equations and 200 of the
    // points' x coordinates, exactly BLOCK_LIMIT unknowns, so it is certified, not refused.
    let n = 201_u64;
    let mut entities = Vec::new();
    let mut constraints = Vec::new();
    for k in 0..n {
        #[allow(clippy::cast_precision_loss, reason = "a small count")]
        let x = 10.0 * k as f64;
        entities.push(point(k + 1, x, 0.0));
        if k + 1 < n {
            constraints.push(distance(k + 1, k + 1, k + 2, 10.0));
        }
    }
    let s = build(entities, constraints);
    let committed = commit(&s, &mut meter()).unwrap();
    let got = certify(&s, &committed, &mut meter()).unwrap();
    assert_eq!(got.blocks.len(), 1, "{got:?}");
    assert_eq!(got.blocks[0].unknowns, BLOCK_LIMIT);
    let radius = proven(&got.blocks[0]);
    assert!(radius < 1.0, "{radius}");
}
