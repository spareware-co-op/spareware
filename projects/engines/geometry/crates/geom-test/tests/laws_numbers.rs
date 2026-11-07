// SPDX-License-Identifier: Apache-2.0
//! The law catalogue's row for numbers and frames (chapter 28.3; piece G155), on `geom-num`'s
//! grid and three-plane points and `geom-math`'s frames.
//!
//! - **Committing a point never flips a predicate that decided topology, or the implicit form is
//!   kept.** A three-plane point commits to the nearest grid point, ties to even, and says whether
//!   it rounded; an exact commit decides every plane as the point itself does, and a rounded one
//!   leaves at least one of its own planes, which is why it must stay implicit. On the bodies
//!   booleans make of slanted prisms, every vertex is exactly a corner of the true result, as the
//!   flat reference finds it (`geom_test::flat_reference`), and a vertex is kept as the meeting
//!   of its planes exactly when committing it would round.
//! - **Coarse to fine (×25) is exact, and fine to coarse is marked rounded.** Refining multiplies
//!   by 25 per rung or refuses a point past the frame; coarsening gives the nearest step and says
//!   whether it rounded; refining then coarsening gives the point back, exactly.
//! - **Neighbouring frames shift exactly.** Between lattice neighbours, between a frame and the
//!   same frame on a finer step, and between Ground frames on a shared lattice, the shift is the
//!   one the frame formula gives in exact rationals, a point moves by it exactly, the map sees the
//!   same place from both frames, and frames off a shared lattice are refused.
//! - **Map-projection round trips stay within commit rounding.** Each way is rounded once: map to
//!   frame to the nearest step, ties to even, and frame to map to the nearest doubles, both checked
//!   in exact arithmetic (rationals, and exact signs of `a − b√n` for a direction whose length is
//!   irrational). So a round trip moves a map point by at most a commit, and gives a grid point
//!   back unchanged wherever the doubles are finer than half a step.
//!
//! Seeds: each law prints `seed: <hex>` and replays with `SPAREKIT_SEED`; pull requests run the
//! fixed default seed and the nightly workflow a fresh one.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unwrap_used,
    reason = "tests: oracles written as the formulas read, and a failure stops the test"
)]

#[path = "catalogue/mod.rs"]
mod catalogue;

use core::cmp::Ordering;

use catalogue::flat::{big, corners, draw_prism, exact, operate, p};
use catalogue::{Draw, Findings};
use dashu_int::{IBig, UBig};
use dashu_ratio::RBig;
use geom_math::frame::steps_per_metre;
use geom_math::{
    CrsId, FrameMismatch, FrameRecord, LinearUnit, MapPoint, MathError, Rotation, Scale,
};
use geom_num::grid::{COORD_MAX, coarsen, refine};
use geom_num::plane::{Plane, Tpi};
use geom_num::{Coord, GridStep, NumError, Sign, Steps};
use geom_test::flat_reference::{Op, boolean};
use geom_topo::VertexIx;
use geom_topo::mass::Corner;
use geom_topo::query::corner;

type Q = RBig;

const STEPS: [GridStep; 3] = [GridStep::Q0, GridStep::Q1, GridStep::Q2];

fn q(v: i128) -> Q {
    Q::from(IBig::from(v))
}

fn qf(v: f64) -> Q {
    Q::try_from(v).unwrap()
}

fn magnitude(v: &IBig) -> IBig {
    if *v < IBig::ZERO { -v.clone() } else { v.clone() }
}

fn qabs(v: &Q) -> Q {
    if *v < Q::ZERO { -v.clone() } else { v.clone() }
}

fn half() -> Q {
    Q::from_parts(IBig::ONE, UBig::from(2_u8))
}

/// 25 to the power of the rungs from `coarse` down to `fine`.
fn factor(coarse: GridStep, fine: GridStep) -> i128 {
    25_i128.pow(fine.rung() - coarse.rung())
}

#[test]
fn committing_a_point_never_flips_a_decided_predicate() {
    let mut d = Draw::new("numbers: commits", 0x5001);
    let mut f = Findings::default();
    let (mut exact_commits, mut rounded_commits) = (0_usize, 0_usize);
    for case in 0..600 {
        let base = if d.one_in(2) {
            [0; 3]
        } else {
            [0; 3].map(|_: i64| d.between(-(1 << 30), 1 << 30))
        };
        let at = |d: &mut Draw| {
            p(
                base[0] + d.between(-12, 12),
                base[1] + d.between(-12, 12),
                base[2] + d.between(-12, 12),
            )
        };
        // Half the cases are three planes through one grid point, so exact commits come up.
        let common = at(&mut d);
        let through = |d: &mut Draw| {
            if case % 2 == 0 {
                Plane::through(common, at(d), at(d)).ok()
            } else {
                Plane::through(at(d), at(d), at(d)).ok()
            }
        };
        let (Some(a), Some(b), Some(c)) = (through(&mut d), through(&mut d), through(&mut d)) else {
            continue;
        };
        let Ok(t) = Tpi::new(a, b, c) else {
            continue;
        };
        f.ran();
        let (x, w) = t.homogeneous().unwrap();
        let (x, w) = (x.map(big), big(w));
        let commit = match t.to_grid() {
            Ok(r) => r,
            Err(e) => {
                f.add("a commit inside the frame refused", || format!("{e:?}"));
                continue;
            }
        };
        let g = commit.value();
        let at_grid = [g.x.get(), g.y.get(), g.z.get()].map(IBig::from);
        for i in 0..3 {
            let off = &x[i] - &at_grid[i] * &w;
            let twice = magnitude(&(off * IBig::from(2)));
            let width = magnitude(&w);
            f.check(twice <= width, "a commit goes to the nearest step", || {
                format!("{t:?} commits to {g:?}")
            });
            if twice == width {
                f.check(&at_grid[i] % IBig::from(2) == IBig::ZERO, "a tie commits to even", || {
                    format!("{t:?} commits to {g:?}")
                });
            }
        }
        let integral = (0..3).all(|i| x[i] == &at_grid[i] * &w);
        f.check(commit.is_exact() == integral, "a commit says whether it rounded", || {
            format!("{t:?} commits to {g:?}, exact {}", commit.is_exact())
        });
        // Other decided predicates: planes through the committed point, and planes anywhere near.
        let mut others: Vec<Plane> = (0..4)
            .filter_map(|_| Plane::through(g, at(&mut d), at(&mut d)).ok())
            .collect();
        others.extend((0..4).filter_map(|_| Plane::through(at(&mut d), at(&mut d), at(&mut d)).ok()));
        if commit.is_exact() {
            exact_commits += 1;
            for plane in [a, b, c].iter().chain(&others) {
                f.check(
                    plane.side(g).ok() == plane.side_of_tpi(&t).ok(),
                    "an exact commit decides every plane as the point does",
                    || format!("{t:?} at {g:?} against {plane:?}"),
                );
            }
        } else {
            rounded_commits += 1;
            f.check(
                [a, b, c].iter().any(|plane| plane.side(g) != Ok(Sign::Zero)),
                "a rounded commit leaves one of its own planes",
                || format!("{t:?} at {g:?}"),
            );
        }
    }
    f.check(exact_commits > 50 && rounded_commits > 50, "both kinds of commit were drawn", || {
        format!("{exact_commits} exact and {rounded_commits} rounded")
    });
    f.assert_none("numbers: commits", 400);
}

#[test]
fn a_boolean_keeps_each_vertex_on_the_grid_exactly_or_where_its_planes_meet() {
    let mut d = Draw::new("numbers: vertices of booleans", 0x5002);
    let mut f = Findings::default();
    let mut implicit = 0_usize;
    for _ in 0..24 {
        let pa = draw_prism(&mut d, None);
        let pb = draw_prism(&mut d, Some(&pa));
        let (Ok(a), Ok(b)) = (pa.body(), pb.body()) else {
            f.add("a prism the kernel would not build", || format!("{pa:?} {pb:?}"));
            continue;
        };
        f.ran();
        let op = [Op::Union, Op::Intersection, Op::Difference][d.pick(3)];
        let result = match operate(op, &a, &b) {
            Ok(r) => r,
            Err(why) => {
                f.add("refused", || why);
                continue;
            }
        };
        let truth = corners(&boolean(&pa.reference(), &pb.reference(), op).unwrap());
        for row in 0..result.arenas().vertices.len() {
            let v = VertexIx::new(row).unwrap();
            let place = match corner(&result, v) {
                Ok(c) => c,
                Err(e) => {
                    f.add("a vertex with no place", || format!("{e:?}"));
                    continue;
                }
            };
            let at = exact(&place).unwrap();
            f.check(
                truth.contains(&at),
                "every vertex is exactly a corner of the true result",
                || format!("{op:?} of {pa:?} and {pb:?}: {at:?}"),
            );
            match &place {
                Corner::Grid(_) => {}
                Corner::Tpi(t) => {
                    implicit += 1;
                    f.check(
                        t.to_grid().is_ok_and(|r| !r.is_exact()),
                        "a vertex kept implicit is one a commit would round",
                        || format!("{op:?} of {pa:?} and {pb:?}: {at:?}"),
                    );
                }
                Corner::Lpi(_) => f.add("a boolean's vertex is a line and a plane", || {
                    format!("{op:?} of {pa:?} and {pb:?}")
                }),
            }
        }
    }
    f.check(implicit > 0, "some vertex was kept implicit", || "none".to_owned());
    f.assert_none("numbers: vertices of booleans", 16);
}

#[test]
fn coarse_to_fine_is_exact_and_fine_to_coarse_is_marked_rounded() {
    let mut d = Draw::new("numbers: the grid ladder", 0x5003);
    let mut f = Findings::default();
    for _ in 0..3000 {
        f.ran();
        let (i, j) = (d.pick(3), d.pick(3));
        let (coarse, fine) = (STEPS[i.min(j)], STEPS[i.max(j)]);
        let k = factor(coarse, fine);
        let reach = match d.below(3) {
            0 => 40,
            1 => 30,
            _ => 8,
        };
        let v = d.between(-(1_i64 << reach) + 1, (1_i64 << reach) - 1);
        let c = Coord::new(v).unwrap();
        let up = i128::from(v) * k;
        match refine(c, coarse, fine) {
            Ok(r) => {
                f.check(i128::from(r.get()) == up, "coarse to fine multiplies by 25 a rung", || {
                    format!("{v} from {coarse:?} to {fine:?} gives {}", r.get())
                });
                match coarsen(r, fine, coarse) {
                    Ok(back) => f.check(
                        back.value() == c && back.is_exact(),
                        "fine to coarse undoes coarse to fine, exactly",
                        || format!("{v} through {fine:?}"),
                    ),
                    Err(e) => f.add("coarsening a refined coordinate refused", || format!("{e:?}")),
                }
            }
            Err(NumError::OutOfRange { .. }) => {
                f.check(up.abs() > i128::from(COORD_MAX), "a refinement is refused only past the frame", || {
                    format!("{v} from {coarse:?} to {fine:?}")
                });
            }
            Err(e) => f.add("a refinement refused", || format!("{v}: {e:?}")),
        }
        match coarsen(c, fine, coarse) {
            Ok(r) => {
                let near = i128::from(r.value().get());
                let off = i128::from(v) - near * k;
                f.check(2 * off.abs() < k || k == 1, "fine to coarse goes to the nearest step", || {
                    format!("{v} from {fine:?} to {coarse:?} gives {near}")
                });
                f.check(
                    r.is_exact() == (off == 0),
                    "fine to coarse is marked rounded exactly when it rounds",
                    || format!("{v} from {fine:?} to {coarse:?}: exact {}", r.is_exact()),
                );
            }
            Err(e) => f.add("a coarsening refused", || format!("{v}: {e:?}")),
        }
        if k > 1 {
            f.check(
                matches!(refine(c, fine, coarse), Err(NumError::WrongDirection))
                    && matches!(coarsen(c, coarse, fine), Err(NumError::WrongDirection)),
                "each way refuses the other direction",
                || format!("{coarse:?} and {fine:?}"),
            );
        }
        let len = d.below(1 << 40);
        f.check(
            Steps(len).refine(coarse, fine) == u64::try_from(i128::from(len) * k).ok().map(Steps),
            "a length refines by 25 a rung",
            || format!("{len} from {coarse:?} to {fine:?}"),
        );
    }
    f.assert_none("numbers: the grid ladder", 3000);
}

/// A rotation with an integer length (quarter turns and Pythagorean directions) or an irrational
/// one.
fn draw_rotation(d: &mut Draw) -> Rotation {
    let directions: [(i32, i32); 10] = [
        (1, 0),
        (0, 1),
        (-1, 0),
        (0, -1),
        (3, 4),
        (-5, 12),
        (8, -15),
        (-20, -21),
        (1, 2),
        (7, -3),
    ];
    let (c, s) = directions[d.pick(directions.len())];
    Rotation::direction(c, s).unwrap()
}

fn draw_scale(d: &mut Draw) -> Scale {
    let scales: [(u64, u64); 4] = [(1, 1), (1, 1), (9_999, 10_000), (4, 3)];
    let (n, m) = scales[d.pick(scales.len())];
    Scale::new(n, m).unwrap()
}

/// `kR·v` for an integer vector v whose every component is a multiple of `k_d·r`: an integer
/// vector, the offset between two origins on a shared lattice.
fn on_lattice(rotation: Rotation, scale: Scale, m: [i64; 2]) -> Option<([i64; 2], [i64; 2])> {
    let r = i64::from(rotation.exact_norm()?);
    let (c, s) = (i64::from(rotation.cos()), i64::from(rotation.sin()));
    let (kn, kd) = (i64::try_from(scale.num()).ok()?, i64::try_from(scale.den()).ok()?);
    let v = [kd * r * m[0], kd * r * m[1]];
    let image = [kn * (c * m[0] - s * m[1]), kn * (s * m[0] + c * m[1])];
    Some((v, image))
}

#[test]
fn neighbouring_frames_shift_exactly() {
    let mut d = Draw::new("frames: shifts", 0x5004);
    let mut f = Findings::default();
    let crs = CrsId::Epsg(2193);
    let mut moved = 0_usize;
    for case in 0..400 {
        f.ran();
        let (source, target) = if case % 3 == 0 {
            // Lattice neighbours of a regional network.
            let step = STEPS[d.pick(3)];
            let base = [
                d.between(-(1 << 45), 1 << 45),
                d.between(-(1 << 45), 1 << 45),
                d.between(-1000, 1000),
            ];
            let index = [d.between(-3, 3), d.between(-3, 3)];
            let next = [index[0] + d.between(-1, 1), index[1] + d.between(-1, 1)];
            (
                FrameRecord::lattice(crs, step, base, index).unwrap(),
                FrameRecord::lattice(crs, step, base, next).unwrap(),
            )
        } else {
            // A frame and another on the same or a finer step, rotated and scaled alike, its origin
            // on the shared lattice or off it by a few steps.
            let (i, j) = (d.pick(3), d.pick(3));
            let (from, to) = (STEPS[i.min(j)], STEPS[i.max(j)]);
            let rotation = draw_rotation(&mut d);
            let scale = draw_scale(&mut d);
            let origin = [
                d.between(-(1 << 45), 1 << 45),
                d.between(-(1 << 45), 1 << 45),
                d.between(-(1 << 20), 1 << 20),
            ];
            let k = factor(from, to) as i64;
            let m = [d.between(-1000, 1000), d.between(-1000, 1000)];
            let lattice = on_lattice(rotation, scale, m).map(|(_, image)| image);
            let shift_xy = match (lattice, d.below(3)) {
                (Some(image), 0 | 1) => image,
                (None, 0) => [0, 0],
                _ => [d.between(-7, 7), d.between(-7, 7)],
            };
            let target_origin = [
                k * origin[0] + shift_xy[0],
                k * origin[1] + shift_xy[1],
                k * origin[2] + d.between(-(1 << 20), 1 << 20),
            ];
            (
                FrameRecord::new(crs, from, origin, rotation, scale).unwrap(),
                FrameRecord::new(crs, to, target_origin, rotation, scale).unwrap(),
            )
        };
        let k = factor(source.step(), target.step());
        // The offset in target steps that keeps every map place: (kR)⁻¹(k·O_s − O_t)
        // horizontally, k·O_s − O_t in height, from the frame formula in rationals.
        let (os, ot) = (source.origin(), target.origin());
        let delta = [0, 1, 2].map(|i| i128::from(os[i]) * k - i128::from(ot[i]));
        let rotation = source.rotation();
        let (c, s) = (i128::from(rotation.cos()), i128::from(rotation.sin()));
        let (kn, kd) = (
            i128::from(source.scale().num()),
            i128::from(source.scale().den()),
        );
        let expected: Option<[i128; 3]> = match rotation.exact_norm() {
            Some(r) => {
                let whole = |a: i128| -> Option<i128> {
                    let v = Q::from_parts_signed(IBig::from(kd * a), IBig::from(kn * i128::from(r)));
                    if *v.denominator() == UBig::ONE {
                        i128::try_from(v.numerator().clone()).ok()
                    } else {
                        None
                    }
                };
                whole(c * delta[0] + s * delta[1])
                    .zip(whole(-s * delta[0] + c * delta[1]))
                    .map(|(x, y)| [x, y, delta[2]])
            }
            None => (delta[0] == 0 && delta[1] == 0).then_some([0, 0, delta[2]]),
        };
        match (source.shift_to(&target), expected) {
            (Ok(shift), Some(want)) => {
                f.check(
                    shift.refine() as i128 == k
                        && shift.offset().map(i128::from) == want,
                    "a shift is the offset the frame formula gives",
                    || format!("{source:?} to {target:?}: {shift:?}, want {want:?}"),
                );
                for _ in 0..4 {
                    let reach = 1_i64 << d.between(4, 38);
                    let x = [0; 3].map(|_: i64| d.between(-reach, reach));
                    let pt = p(x[0], x[1], x[2]);
                    let image = [0, 1, 2].map(|i| i128::from(x[i]) * k + want[i]);
                    let fits = image.iter().all(|v| v.abs() <= i128::from(COORD_MAX));
                    match shift.apply(pt) {
                        Ok(moved_to) => {
                            moved += 1;
                            f.check(
                                [moved_to.x.get(), moved_to.y.get(), moved_to.z.get()]
                                    .map(i128::from)
                                    == image,
                                "a point moves by the shift exactly",
                                || format!("{x:?} by {shift:?}"),
                            );
                            let (m1, m2) = (source.to_map(pt), target.to_map(moved_to));
                            f.check(
                                m1.as_ref().ok().map(|r| (r.value(), r.is_exact()))
                                    == m2.as_ref().ok().map(|r| (r.value(), r.is_exact())),
                                "both frames see the same place on the map",
                                || format!("{x:?}: {m1:?} and {m2:?}"),
                            );
                        }
                        Err(NumError::OutOfRange { .. }) => f.check(
                            !fits,
                            "a shift refuses only a point past the target frame",
                            || format!("{x:?} by {shift:?}"),
                        ),
                        Err(e) => f.add("a shift refused a point", || format!("{e:?}")),
                    }
                }
            }
            (Err(MathError::FrameMismatch(FrameMismatch::NotOnLattice)), None) => {}
            (got, want) => f.add("a shift is given exactly when the frames share a lattice", || {
                format!("{source:?} to {target:?}: {got:?}, want {want:?}")
            }),
        }
    }
    f.check(moved > 400, "points were moved", || format!("{moved}"));
    f.assert_none("frames: shifts", 400);
}

/// The sign of `a − b·√n`, exactly.
fn minus_root(a: &Q, b: &Q, n: u64) -> Ordering {
    let root = n.isqrt();
    if root * root == n {
        return a.cmp(&(b * q(i128::from(root))));
    }
    let zero = Q::ZERO;
    let square = |v: &Q| v * v;
    match (a.cmp(&zero), b.cmp(&zero)) {
        (_, Ordering::Equal) => a.cmp(&zero),
        (Ordering::Equal | Ordering::Greater, Ordering::Less) => Ordering::Greater,
        (Ordering::Equal | Ordering::Less, Ordering::Greater) => Ordering::Less,
        // Both positive: a − b√n has the sign of a² − b²n, which is never zero.
        (Ordering::Greater, Ordering::Greater) => square(a).cmp(&(square(b) * q(i128::from(n)))),
        // Both negative: a − b√n = |b|√n − |a|.
        _ => (square(b) * q(i128::from(n))).cmp(&square(a)),
    }
}

/// A frame's constants in rationals: steps per map unit, the scale's parts, the rotation and its
/// squared length.
struct Formula {
    per_unit: Q,
    kn: Q,
    kd: Q,
    c: Q,
    s: Q,
    n: u64,
    origin: [Q; 3],
}

impl Formula {
    fn of(frame: &FrameRecord) -> Formula {
        let unit = frame.linear_unit();
        let per_unit = Q::from_parts(
            IBig::from(steps_per_metre(frame.step())) * IBig::from(unit.num()),
            UBig::from(unit.den()),
        );
        let rotation = frame.rotation();
        Formula {
            per_unit,
            kn: q(i128::from(frame.scale().num())),
            kd: q(i128::from(frame.scale().den())),
            c: q(i128::from(rotation.cos())),
            s: q(i128::from(rotation.sin())),
            n: rotation.norm2(),
            origin: frame.origin().map(|o| q(i128::from(o))),
        }
    }

    /// Whether the integer `x` is the nearest whole step to `a/√n`, ties to even.
    fn nearest_step(&self, a: &Q, x: i64) -> bool {
        let xq = q(i128::from(x));
        let above = minus_root(a, &(&xq + half()), self.n);
        let below = minus_root(a, &(&xq - half()), self.n);
        let inside = above != Ordering::Greater && below != Ordering::Less;
        let tie = above == Ordering::Equal || below == Ordering::Equal;
        inside && (!tie || x % 2 == 0)
    }

    /// Whether `x` is exactly `a/√n`.
    fn equals_step(&self, a: &Q, x: i64) -> bool {
        minus_root(a, &q(i128::from(x)), self.n) == Ordering::Equal
    }

    /// The horizontal frame images of map values in steps, as `a/√n`: `a` for x and for y.
    fn frame_numerators(&self, map: MapPoint) -> [Q; 2] {
        let de = qf(map.easting) * &self.per_unit - &self.origin[0];
        let dn = qf(map.northing) * &self.per_unit - &self.origin[1];
        let ratio = &self.kd / &self.kn;
        [
            &ratio * (&self.c * &de + &self.s * &dn),
            &ratio * (&self.c * &dn - &self.s * &de),
        ]
    }

    /// For a grid point, `T` with map value `(O + T/√n)/per_unit` for easting and northing.
    fn map_numerators(&self, x: i64, y: i64) -> [Q; 2] {
        let (x, y) = (q(i128::from(x)), q(i128::from(y)));
        let ratio = &self.kn / &self.kd;
        [
            &ratio * (&self.c * &x - &self.s * &y),
            &ratio * (&self.s * &x + &self.c * &y),
        ]
    }

    /// Whether `value` is the nearest double to `(origin + t/√n)/per_unit`, ties to even, and
    /// whether it equals it.
    fn nearest_double(&self, value: f64, origin: &Q, t: &Q) -> (bool, bool) {
        let n = q(i128::from(self.n));
        let tn = t / &n;
        // value·per_unit − origin − t/√n, as (a − b√n), for a value v.
        let against = |v: &Q| minus_root(&(v * &self.per_unit - origin), &tn, self.n);
        let v = qf(value);
        let two = q(2);
        let up = (&v + qf(value.next_up())) / &two;
        let down = (&v + qf(value.next_down())) / &two;
        let (above, below) = (against(&up), against(&down));
        let inside = above != Ordering::Less && below != Ordering::Greater;
        let tie = above == Ordering::Equal || below == Ordering::Equal;
        (inside && (!tie || value.to_bits() % 2 == 0), against(&v) == Ordering::Equal)
    }
}

/// A frame for the round trips: any step, a Map or Ground frame, metres or international feet, an
/// origin at a site in a projected system or near zero.
fn draw_frame(d: &mut Draw) -> FrameRecord {
    let step = STEPS[d.pick(3)];
    let per_metre = i64::try_from(steps_per_metre(step)).unwrap();
    let origin = if d.one_in(2) {
        // About central Auckland in NZTM2000, in steps.
        [
            1_757_000 * per_metre + d.between(-1000, 1000),
            5_921_000 * per_metre + d.between(-1000, 1000),
            d.between(-10_000, 10_000),
        ]
    } else {
        [0; 3].map(|_: i64| d.between(-(1 << 20), 1 << 20))
    };
    let rotation = if d.one_in(2) { Rotation::IDENTITY } else { draw_rotation(d) };
    let frame = FrameRecord::new(CrsId::Epsg(2193), step, origin, rotation, draw_scale(d)).unwrap();
    if d.one_in(4) {
        frame.with_linear_unit(LinearUnit::INTERNATIONAL_FOOT)
    } else {
        frame
    }
}

#[test]
fn map_round_trips_stay_within_commit_rounding() {
    let mut d = Draw::new("frames: map round trips", 0x5005);
    let mut f = Findings::default();
    let mut identities = 0_usize;
    for _ in 0..500 {
        f.ran();
        let frame = draw_frame(&mut d);
        let form = Formula::of(&frame);
        let reach = 1_i64 << d.between(2, 34);
        let x = [0; 3].map(|_: i64| d.between(-reach, reach));
        let grid = p(x[0], x[1], x[2]);
        // Frame to map: the nearest doubles, ties to even.
        let map = match frame.to_map(grid) {
            Ok(m) => m,
            Err(e) => {
                f.add("a grid point has no map value", || format!("{e:?}"));
                continue;
            }
        };
        let m = map.value();
        let [te, tn] = form.map_numerators(x[0], x[1]);
        let (ne, ee) = form.nearest_double(m.easting, &form.origin[0], &te);
        let (nn, en) = form.nearest_double(m.northing, &form.origin[1], &tn);
        let (nh, eh) = form.nearest_double(m.height, &(&form.origin[2] + q(i128::from(x[2]))), &Q::ZERO);
        f.check(ne && nn && nh, "frame to map gives the nearest doubles", || {
            format!("{grid:?} in {frame:?} gives {m:?}")
        });
        f.check(map.is_exact() == (ee && en && eh), "frame to map says whether it rounded", || {
            format!("{grid:?} in {frame:?} gives {m:?}, exact {}", map.is_exact())
        });
        // Map to frame: the nearest step, ties to even.
        let back = match frame.to_frame(m) {
            Ok(b) => b,
            Err(e) => {
                f.add("a map value of the frame has no grid point", || format!("{e:?}"));
                continue;
            }
        };
        let g = back.value();
        let [ax, ay] = form.frame_numerators(m);
        let az = qf(m.height) * &form.per_unit - &form.origin[2];
        let height = |z: i64| {
            let off = qabs(&(&az - q(i128::from(z))));
            (off < half() || (off == half() && z % 2 == 0), off == Q::ZERO)
        };
        let (hz, hz_exact) = height(g.z.get());
        f.check(
            form.nearest_step(&ax, g.x.get()) && form.nearest_step(&ay, g.y.get()) && hz,
            "map to frame gives the nearest step",
            || format!("{m:?} in {frame:?} gives {g:?}"),
        );
        f.check(
            back.is_exact()
                == (form.equals_step(&ax, g.x.get()) && form.equals_step(&ay, g.y.get()) && hz_exact),
            "map to frame says whether it rounded",
            || format!("{m:?} in {frame:?} gives {g:?}, exact {}", back.is_exact()),
        );
        // The round trip gives the grid point back while the doubles' rounding, carried into the
        // frame, is under half a step: (k_d/k_n)·per_unit·(ulp(E) + ulp(N))/2 < 1/2 horizontally,
        // per_unit·ulp(H)/2 < 1/2 in height.
        let ulp = |v: f64| qf(v.next_up()) - qf(v);
        let one_half = half();
        let fine_horizontal = &form.kd / &form.kn
            * &form.per_unit
            * (ulp(m.easting.abs()) + ulp(m.northing.abs()))
            * &one_half
            < one_half;
        let fine_height = &form.per_unit * ulp(m.height.abs()) * &one_half < one_half;
        if fine_horizontal && fine_height {
            identities += 1;
            f.check(g == grid, "frame to map to frame is the identity where the doubles are fine", || {
                format!("{grid:?} in {frame:?} comes back as {g:?}")
            });
        }
        // Map to frame to map moves a map value by at most a commit: half a step on each frame
        // axis, carried to the map by the scale and rotation, and the doubles' own rounding.
        let jitter = |d: &mut Draw, v: f64, unit_steps: f64| {
            v + (d.between(-1000, 1000) as f64 / 1000.0) * 3.0 / unit_steps
        };
        let unit_steps = steps_per_metre(frame.step()) as f64
            * frame.linear_unit().num() as f64
            / frame.linear_unit().den() as f64;
        let start = MapPoint {
            easting: jitter(&mut d, m.easting, unit_steps),
            northing: jitter(&mut d, m.northing, unit_steps),
            height: jitter(&mut d, m.height, unit_steps),
        };
        let Ok(there) = frame.to_frame(start) else {
            f.add("a jittered map value has no grid point", || format!("{start:?}"));
            continue;
        };
        // r rounded down, so the bound below is never too tight for an irrational length.
        let r_low = form.n.isqrt();
        let Ok(again) = frame.to_map(there.value()) else {
            f.add("a committed grid point has no map value", || format!("{there:?}"));
            continue;
        };
        let end = again.value();
        let spread = (&form.kn / &form.kd)
            * (qabs(&form.c) + qabs(&form.s))
            / q(i128::from(r_low))
            * &one_half;
        let within = |a: f64, b: f64, carried: &Q| {
            let gap = (qf(a) - qf(b)) * &form.per_unit;
            let bound = carried + ulp(a.abs()) * &form.per_unit * &one_half;
            gap <= bound && -gap <= bound
        };
        f.check(
            within(end.easting, start.easting, &spread)
                && within(end.northing, start.northing, &spread)
                && within(end.height, start.height, &one_half),
            "map to frame to map moves a value by at most a commit",
            || format!("{start:?} in {frame:?} comes back as {end:?}"),
        );
    }
    f.check(identities > 100, "round trips where the doubles are fine were drawn", || {
        format!("{identities}")
    });
    f.assert_none("frames: map round trips", 500);
}
