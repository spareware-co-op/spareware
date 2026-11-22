//! The envelope's whole-triangle test, [`covers_within`], against an exact oracle written apart
//! from the code (piece G300): a triangle a, a + D·u, a + D·v has the grid points a + i·u + j·v
//! (i + j ≤ D) on it, and [`TriangleEnvelope::contains`] decides each exactly. A triangle the test
//! calls covered must have every such point (inside the region, when there is one) in some body.
//! A hand-built hole shows what the lattice cannot: a point bodies only touch is not covered.
//! The file's name sorts after the other test files', so a run that stops at its first failure
//! (cargo test runs the files in that order) reaches the quick tests of `envelope.rs` and
//! `model.rs` before these slow ones.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "test oracles and generators; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::num::NonZeroU64;

use geom_mesh::envelope::{EnvelopeError, TriangleEnvelope, covers_within};
use geom_num::NumError;
use geom_num::grid::P3;
use geom_run::{Budget, WorkMeter};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn below(g: &mut SplitMix64, n: u64) -> i64 {
    g.below(NonZeroU64::new(n).unwrap()) as i64
}

fn small(g: &mut SplitMix64, r: i64) -> i64 {
    below(g, (2 * r + 1) as u64) - r
}

/// The triangles of a bent sheet of `n` × `n` cells of side `s`, heights 0 to `bend`.
fn sheet(g: &mut SplitMix64, n: i64, s: i64, bend: i64) -> Vec<[P3; 3]> {
    let side = n + 1;
    let mut points = Vec::new();
    for y in 0..side {
        for x in 0..side {
            points.push(p(x * s, y * s, below(g, (bend + 1) as u64)));
        }
    }
    let at = |x: i64, y: i64| points[(y * side + x) as usize];
    let mut out = Vec::new();
    for y in 0..n {
        for x in 0..n {
            out.push([at(x, y), at(x + 1, y), at(x + 1, y + 1)]);
            out.push([at(x, y), at(x + 1, y + 1), at(x, y + 1)]);
        }
    }
    out
}

/// A fan of triangles round a centre: what a collapse or a move replaces.
fn fan(g: &mut SplitMix64, s: i64, bend: i64) -> Vec<[P3; 3]> {
    let ring = [
        (s, 0),
        (s / 2, s),
        (-s / 2, s),
        (-s, 0),
        (-s / 2, -s),
        (s / 2, -s),
    ];
    let c = p(
        2 * s + small(g, 2),
        2 * s + small(g, 2),
        below(g, (bend + 1) as u64),
    );
    let rim: Vec<P3> = ring
        .iter()
        .map(|(x, y)| {
            p(
                2 * s + x + small(g, 1),
                2 * s + y + small(g, 1),
                below(g, (bend + 1) as u64),
            )
        })
        .collect();
    (0..6).map(|k| [c, rim[k], rim[(k + 1) % 6]]).collect()
}

/// Triangles anywhere in a box.
fn soup(g: &mut SplitMix64, count: i64, reach: i64, height: i64) -> Vec<[P3; 3]> {
    (0..count)
        .map(|_| {
            [0, 1, 2].map(|_| {
                p(
                    below(g, reach as u64),
                    below(g, reach as u64),
                    below(g, height as u64),
                )
            })
        })
        .collect()
}

#[derive(Default, Debug)]
struct Tally {
    cases: u64,
    flat: u64,
    covered: u64,
    /// Covered, and no one body holds all three corners.
    covered_by_several: u64,
    /// Covered with a region that holds some lattice point but not all.
    covered_part: u64,
    /// Covered, with holes in the surface and the corners pinned.
    covered_pinned: u64,
    /// Refused rightly, with holes in the surface and the corners pinned.
    refused_pinned: u64,
    /// Refused, and a lattice point is in no body.
    refused_rightly: u64,
    /// Refused, with every lattice point in a body: cautious, or a gap between lattice points.
    refused_cautiously: u64,
}

/// One of three parts of the oracle's cases: every case is drawn from one generator, in order, so
/// the cases are the same however they are split, and part k asks the runs of 18 cases (each kind
/// of surface with each kind of region) numbered k modulo 3. Three tests run the parts side by
/// side.
fn lattice_part(part: u64) {
    let seed = resolve("geom-mesh covers lattice oracle", Seed::DEFAULT).unwrap();
    let mut g = SplitMix64::new(seed);
    let cases: u64 = std::env::var("COVERS_ORACLE_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60_000);
    let mut tally = Tally::default();
    let mut wrong: Vec<String> = Vec::new();
    for case in 0..cases {
        let kind = case % 6;
        let d = [4_i64, 8, 12][below(&mut g, 3) as usize];
        // The pinned kinds need a body to hold a corner strictly, so a cube with an inside.
        let h = if kind >= 4 {
            1 + below(&mut g, 6) as u64
        } else {
            below(&mut g, 7) as u64
        };
        let s = 10 + below(&mut g, 9);
        let bend = below(&mut g, 4);
        // The triangle asked about: a, a + d·u, a + d·v, often level.
        let span = 36 / d;
        let level = below(&mut g, 2) == 0;
        let a = [
            below(&mut g, 40),
            below(&mut g, 40),
            small(&mut g, 2) + bend / 2,
        ];
        let step = |g: &mut SplitMix64| {
            [
                small(g, span),
                small(g, span),
                if level { 0 } else { small(g, 1) },
            ]
        };
        let (u, v) = (step(&mut g), step(&mut g));
        let corner = |i: i64, j: i64| {
            p(
                a[0] + i * u[0] + j * v[0],
                a[1] + i * u[1] + j * v[1],
                a[2] + i * u[2] + j * v[2],
            )
        };
        let t = [corner(0, 0), corner(d, 0), corner(0, d)];
        let mut triangles = match kind {
            0 | 4 => sheet(&mut g, 2, s, bend),
            1 => fan(&mut g, s, bend),
            2 | 5 => {
                let count = 2 + below(&mut g, 5);
                soup(&mut g, count, 40, 8)
            }
            _ => {
                let mut both = fan(&mut g, s, bend);
                both.extend(sheet(&mut g, 2, s, bend));
                both
            }
        };
        if kind >= 4 {
            // Holes in the surface, and a small body round each corner of the triangle: the
            // corners are held, so only the crossings can show what lies bare between them.
            triangles.retain(|_| below(&mut g, 3) != 0);
            for c in t {
                let (x, y, z) = (c.x.get(), c.y.get(), c.z.get());
                triangles.push([p(x - 1, y - 1, z), p(x + 2, y - 1, z), p(x - 1, y + 2, z)]);
            }
        }
        // The region: none, the body of a triangle near one of the surface's, or of any triangle.
        let region = match (case / 6) % 3 {
            0 => None,
            1 => {
                let near = triangles[below(&mut g, triangles.len() as u64) as usize];
                Some(near.map(|c| {
                    p(
                        c.x.get() + small(&mut g, 3),
                        c.y.get() + small(&mut g, 3),
                        c.z.get() + small(&mut g, 2),
                    )
                }))
            }
            _ => Some(soup(&mut g, 1, 40, 8)[0]),
        };
        if (case / 18) % 3 != part {
            continue;
        }
        let region = region.map(|r| TriangleEnvelope::new(r, h).unwrap());
        let bodies: Vec<TriangleEnvelope> = triangles
            .iter()
            .map(|t| TriangleEnvelope::new(*t, h).unwrap())
            .collect();
        let refs: Vec<&TriangleEnvelope> = bodies.iter().collect();
        tally.cases += 1;
        let mut meter = WorkMeter::new(Budget::of_work(1 << 40));
        let answer = match covers_within(&refs, t, region.as_ref(), &mut meter) {
            Ok(answer) => answer,
            Err(EnvelopeError::Num(NumError::Degenerate)) => {
                tally.flat += 1;
                continue;
            }
            Err(e) => panic!("{seed:?} case {case}: {e:?}"),
        };
        let mut asked = 0_u64;
        let mut all = 0_u64;
        let mut bare: Option<P3> = None;
        for i in 0..=d {
            for j in 0..=(d - i) {
                let q = corner(i, j);
                all += 1;
                if region.as_ref().is_some_and(|r| !r.contains(q).unwrap()) {
                    continue;
                }
                asked += 1;
                if bare.is_none() && !bodies.iter().any(|b| b.contains(q).unwrap()) {
                    bare = Some(q);
                }
            }
        }
        if answer {
            tally.covered += 1;
            if !bodies
                .iter()
                .any(|b| t.iter().all(|c| b.contains(*c).unwrap()))
            {
                tally.covered_by_several += 1;
            }
            if region.is_some() && asked > 0 && asked < all {
                tally.covered_part += 1;
            }
            if kind >= 4 {
                tally.covered_pinned += 1;
            }
            if let Some(q) = bare
                && wrong.len() < 5
            {
                wrong.push(format!(
                    "case {case}: h {h}, t {t:?}, region {:?}, bodies {triangles:?}: {q:?} is in \
                     no body",
                    region.as_ref().map(|_| "yes")
                ));
            }
        } else if bare.is_some() {
            tally.refused_rightly += 1;
            if kind >= 4 {
                tally.refused_pinned += 1;
            }
        } else {
            tally.refused_cautiously += 1;
        }
    }
    eprintln!("{seed:?} part {part} {tally:?}");
    assert!(wrong.is_empty(), "{seed:?}: {wrong:#?}");
    assert!(
        tally.covered_by_several * 50 > tally.cases
            && tally.covered_part * 200 > tally.cases
            && tally.covered_pinned * 200 > tally.cases
            && tally.refused_pinned * 200 > tally.cases,
        "{seed:?}: too few covered cases to mean anything: {tally:?}"
    );
}

#[test]
fn a_covered_triangle_has_every_lattice_point_in_a_body() {
    lattice_part(0);
}

#[test]
fn a_covered_triangle_has_every_lattice_point_in_a_body_part_2() {
    lattice_part(1);
}

#[test]
fn a_covered_triangle_has_every_lattice_point_in_a_body_part_3() {
    lattice_part(2);
}

/// A hole only strictness and the two-body crossings can see. Four long bodies leave the square
/// 0 < x, y < 2 of the plane z = 0 bare, and four small bodies each have a corner of their
/// outline on a corner of the hole. The triangle's corners and every crossing on its edges are
/// strictly inside a body. Each corner of the hole is where two bodies' facet lines cross, and
/// lies on a third body, but strictly inside none: a test that took "inside or on" for covered,
/// or that left out the crossings of two bodies' facets, would call the triangle covered.
#[test]
fn a_hole_whose_corners_other_bodies_only_touch_is_not_covered() {
    let h = 2;
    let surface = [
        // Long bodies: their facets x = 0, y = 0, x = 2 and y = 2 bound the hole.
        [p(-2, -6, 0), p(-2, 8, 0), p(-3, 1, 0)],
        [p(-6, -2, 0), p(8, -2, 0), p(1, -3, 0)],
        [p(4, -6, 0), p(4, 8, 0), p(5, 1, 0)],
        [p(-6, 4, 0), p(8, 4, 0), p(1, 5, 0)],
        // Small bodies: an outline corner at (0, 0), (2, 0), (2, 2) and (0, 2).
        [p(-2, -2, 0), p(-3, -2, 0), p(-2, -3, 0)],
        [p(4, -2, 0), p(5, -2, 0), p(4, -3, 0)],
        [p(4, 4, 0), p(5, 4, 0), p(4, 5, 0)],
        [p(-2, 4, 0), p(-3, 4, 0), p(-2, 5, 0)],
    ];
    let bodies: Vec<TriangleEnvelope> = surface
        .iter()
        .map(|t| TriangleEnvelope::new(*t, h).unwrap())
        .collect();
    let refs: Vec<&TriangleEnvelope> = bodies.iter().collect();
    let t = [p(-3, -3, 0), p(5, -3, 0), p(1, 5, 0)];
    // The oracle: the middle of the hole is on the triangle and in no body, and the triangle's
    // corners and the hole's corners are each in one.
    let inside = |q: P3| bodies.iter().any(|b| b.contains(q).unwrap());
    assert!(!inside(p(1, 1, 0)));
    for q in t {
        assert!(inside(q));
    }
    for q in [p(0, 0, 0), p(2, 0, 0), p(2, 2, 0), p(0, 2, 0)] {
        assert_eq!(bodies.iter().filter(|b| b.contains(q).unwrap()).count(), 3);
    }
    let mut meter = WorkMeter::new(Budget::of_work(1 << 40));
    assert_eq!(covers_within(&refs, t, None, &mut meter), Ok(false));
    // The same with a region that holds the hole: the part inside it is not covered either.
    let region = TriangleEnvelope::new([p(1, 1, 0), p(1, 1, 0), p(1, 1, 0)], h).unwrap();
    assert_eq!(
        covers_within(&refs, t, Some(&region), &mut meter),
        Ok(false)
    );
    // With the hole filled, the triangle is covered.
    let plug = TriangleEnvelope::new([p(0, 0, 0), p(2, 0, 0), p(1, 2, 0)], h).unwrap();
    let mut filled = refs.clone();
    filled.push(&plug);
    assert_eq!(covers_within(&filled, t, None, &mut meter), Ok(true));
    assert_eq!(
        covers_within(&filled, t, Some(&region), &mut meter),
        Ok(true)
    );
}

/// The same hole with the long bodies alone: each corner of the hole is where two bodies' facet
/// lines cross and is a corner of neither body's outline, so only the crossings of two different
/// bodies' facets find it.
#[test]
fn a_hole_only_two_bodies_crossing_facets_bound_is_not_covered() {
    let h = 2;
    let surface = [
        [p(-2, -6, 0), p(-2, 8, 0), p(-3, 1, 0)],
        [p(-6, -2, 0), p(8, -2, 0), p(1, -3, 0)],
        [p(4, -6, 0), p(4, 8, 0), p(5, 1, 0)],
        [p(-6, 4, 0), p(8, 4, 0), p(1, 5, 0)],
    ];
    let bodies: Vec<TriangleEnvelope> = surface
        .iter()
        .map(|t| TriangleEnvelope::new(*t, h).unwrap())
        .collect();
    let refs: Vec<&TriangleEnvelope> = bodies.iter().collect();
    let t = [p(-3, -3, 0), p(5, -3, 0), p(1, 5, 0)];
    assert!(!bodies.iter().any(|b| b.contains(p(1, 1, 0)).unwrap()));
    let mut meter = WorkMeter::new(Budget::of_work(1 << 40));
    assert_eq!(covers_within(&refs, t, None, &mut meter), Ok(false));
    let plug = TriangleEnvelope::new([p(0, 0, 0), p(2, 0, 0), p(1, 2, 0)], h).unwrap();
    let mut filled = refs.clone();
    filled.push(&plug);
    assert_eq!(covers_within(&filled, t, None, &mut meter), Ok(true));
}
