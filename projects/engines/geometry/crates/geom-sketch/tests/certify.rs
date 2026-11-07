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
//! block; and a certificate is the same every time.

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
use geom_sketch::certify::{Block, Certificate, Certified, Reason, certify};
use geom_sketch::commit::commit;
use geom_sketch::model::{
    Amount, Constraint, ConstraintKind, Entity, EntityKind, ItemId, Sketch, Standing,
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
