// SPDX-License-Identifier: Apache-2.0
//! The law catalogue's row for mass properties of flat solids (chapter 28.3; piece G155), on
//! `geom-topo`'s exact and certified queries (piece G102).
//!
//! - **Exact rationals for flat solids.** A body whose vertices are grid points has six times its
//!   volume and twice each face's area vector as whole numbers, and they are exactly the flat
//!   reference's (`geom_test::flat_reference`, written from the definitions alone): the volume,
//!   and the area on every plane, compared plane by plane. Each shell's area vectors sum to zero.
//! - **Certified where a vertex is implicit.** A boolean of slanted prisms keeps vertices where
//!   three planes meet off the grid (chapter 6.3); its volume is a certified interval, rounded
//!   outward (chapter 10.2), and the interval holds the reference's exact rational volume.
//! - **Grid moves keep them.** A translation, a quarter turn or a mirror leaves six times the
//!   volume and the total of the area vectors' absolute components unchanged, exactly.
//!
//! Curved solids' mass properties, certified against closed forms, are piece G283's row, and the
//! centroid and moments join with the kernel's mass-property query (piece G247), which this
//! branch does not hold. Seeds: each law prints `seed: <hex>` and replays with `SPAREKIT_SEED`;
//! pull requests run the fixed default seed and the nightly workflow a fresh one.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    clippy::unwrap_used,
    reason = "tests: native arithmetic on small numbers, and a failure stops the test"
)]

#[path = "catalogue/mod.rs"]
mod catalogue;

use std::collections::BTreeMap;

use catalogue::flat::{Prism, ctx, draw_prism, meter, operate, p};
use catalogue::{Draw, Findings};
use dashu_int::IBig;
use dashu_ratio::RBig;
use geom_make::{Moved, mirror, move_by, rotate};
use geom_math::{Axis, Rotation};
use geom_num::Exactness;
use geom_test::flat_reference::{Op, Plane as RefPlane, Region, boolean, int, region};
use geom_topo::query::{certified_shell_volume, face_area, point, shell_volume};
use geom_topo::{Body, FaceIx, PointKind, ShellIx, VertexIx};

type Q = RBig;

/// Whether every vertex of `body` is a grid point.
fn on_grid(body: &Body) -> bool {
    let a = body.arenas();
    (0..a.vertices.len()).all(|row| {
        VertexIx::new(row)
            .and_then(|v| a.vertices.get(v))
            .and_then(|vertex| a.points.get(vertex.point).copied())
            .is_some_and(|place| matches!(place, PointKind::Grid(_)))
    })
}

/// Six times the volume, exactly: the sum of every shell's, a cavity's counting negative. A
/// regularised boolean bounds its material by closed shells even where it is not manifold.
fn six(body: &Body) -> Result<i128, String> {
    let mut sum = 0_i128;
    for row in 0..body.arenas().shells.len() {
        let s = ShellIx::new(row).ok_or("shell index")?;
        sum += shell_volume(body, s, &mut meter())
            .map_err(|e| format!("shell_volume: {e:?}"))?
            .to_i128()
            .ok_or("shell volume: out of i128")?;
    }
    Ok(sum)
}

/// The certified interval of six times the volume, as its two rational ends.
fn certified_six(body: &Body) -> Result<(Q, Q), String> {
    let (mut lo, mut hi) = (Q::ZERO, Q::ZERO);
    for row in 0..body.arenas().shells.len() {
        let s = ShellIx::new(row).ok_or("shell index")?;
        let v = certified_shell_volume(body, s, &mut meter())
            .map_err(|e| format!("certified_shell_volume: {e:?}"))?;
        lo += Q::try_from(v.lo()).map_err(|e| format!("{e:?}"))?;
        hi += Q::try_from(v.hi()).map_err(|e| format!("{e:?}"))?;
    }
    Ok((lo, hi))
}

/// A vertex of `face`: where its first loop's first fin starts.
fn a_vertex_of(body: &Body, face: FaceIx) -> Option<VertexIx> {
    let a = body.arenas();
    let l = a.faces.get(face)?.first_loop?;
    let fin = a.loops.get(l)?.first_fin?;
    let edge = a.edges.get(a.fins.get(fin)?.edge)?;
    edge.start
}

/// Twice the area on each plane of a body whose vertices are grid points, keyed by the reference's
/// canonical plane: for a face's twice-area vector A, the plane through its vertex with normal A,
/// and A's first nonzero component taken positive (the reference's measure along a normal whose
/// first nonzero component is one). Also each shell's sum of area vectors.
fn areas(body: &Body) -> Result<(BTreeMap<RefPlane, Q>, Vec<[i128; 3]>), String> {
    let a = body.arenas();
    let mut by_plane: BTreeMap<RefPlane, Q> = BTreeMap::new();
    let mut shells = vec![[0_i128; 3]; usize::try_from(a.shells.len()).unwrap()];
    for row in 0..a.faces.len() {
        let face = FaceIx::new(row).ok_or("face index")?;
        let vector = face_area(body, face, &mut meter()).map_err(|e| format!("face_area: {e:?}"))?;
        let shell = a.faces.get(face).ok_or("face row")?.shell;
        let total = &mut shells[shell.to_usize()];
        for i in 0..3 {
            total[i] += vector[i];
        }
        let v = a_vertex_of(body, face).ok_or("a face with no vertex")?;
        let at = point(body, v).map_err(|e| format!("point: {e:?}"))?;
        let normal = vector.map(|c| Q::from(IBig::from(c)));
        let offset = Q::from(IBig::from(
            vector[0] * i128::from(at.x.get())
                + vector[1] * i128::from(at.y.get())
                + vector[2] * i128::from(at.z.get()),
        ));
        let plane = RefPlane::new(normal, offset)
            .map_err(|e| format!("{e:?}"))?
            .canonical();
        let lead = vector.iter().copied().find(|c| *c != 0).unwrap_or(0);
        *by_plane.entry(plane).or_insert(Q::ZERO) += Q::from(IBig::from(lead.abs()));
    }
    Ok((by_plane, shells))
}

/// A body drawn for the mass laws: a prism, or a boolean of two, with the reference's region.
fn draw_body(d: &mut Draw) -> Result<(String, Body, Region), String> {
    let pa = draw_prism(d, None);
    let a = pa.body()?;
    if d.one_in(3) {
        let r = region(&pa.reference()).map_err(|e| format!("{e:?}"))?;
        return Ok((format!("{pa:?}"), a, r));
    }
    let pb: Prism = draw_prism(d, Some(&pa));
    let b = pb.body()?;
    let op = [Op::Union, Op::Intersection, Op::Difference][d.pick(3)];
    let body = operate(op, &a, &b)?;
    let r = boolean(&pa.reference(), &pb.reference(), op).map_err(|e| format!("{e:?}"))?;
    Ok((format!("{op:?} of {pa:?} and {pb:?}"), body, r))
}

#[test]
fn flat_volumes_are_exact_rationals_and_certified_where_a_vertex_is_implicit() {
    let mut d = Draw::new("mass: volumes", 0x8001);
    let mut f = Findings::default();
    let (mut exact, mut certified) = (0_usize, 0_usize);
    for _ in 0..30 {
        let (what, body, truth) = match draw_body(&mut d) {
            Ok(drawn) => drawn,
            Err(why) => {
                f.add("a body the kernel would not build", || why);
                continue;
            }
        };
        f.ran();
        let want = truth.volume() * int(6);
        if on_grid(&body) {
            exact += 1;
            let got = six(&body).map(|v| Q::from(IBig::from(v)));
            f.check(got.as_ref() == Ok(&want), "six times the volume is exact", || {
                format!("{what}: {got:?} against {want}")
            });
        }
        match certified_six(&body) {
            Ok((lo, hi)) => {
                if !on_grid(&body) {
                    certified += 1;
                }
                f.check(lo <= want && want <= hi, "the certified volume holds the exact one", || {
                    format!("{what}: [{lo}, {hi}] against {want}")
                });
                f.check(
                    &hi - &lo <= (&want + int(1)) * Q::from_parts(IBig::ONE, (1_u64 << 30).into()),
                    "the certified volume is tight",
                    || format!("{what}: [{lo}, {hi}]"),
                );
            }
            Err(why) => f.add("a certified volume refused", || format!("{what}: {why}")),
        }
    }
    f.check(exact > 5 && certified > 3, "both kinds of body were drawn", || {
        format!("{exact} on the grid, {certified} with implicit vertices")
    });
    f.assert_none("mass: volumes", 20);
}

#[test]
fn flat_areas_are_exact_plane_by_plane_and_close_up() {
    let mut d = Draw::new("mass: areas", 0x8002);
    let mut f = Findings::default();
    for _ in 0..30 {
        let (what, body, truth) = match draw_body(&mut d) {
            Ok(drawn) => drawn,
            Err(why) => {
                f.add("a body the kernel would not build", || why);
                continue;
            }
        };
        if !on_grid(&body) {
            continue;
        }
        f.ran();
        match areas(&body) {
            Ok((by_plane, shells)) => {
                let want = truth.areas();
                f.check(by_plane == want, "every plane's area is the reference's", || {
                    format!("{what}: {by_plane:?} against {want:?}")
                });
                f.check(
                    shells.iter().all(|s| *s == [0; 3]),
                    "a closed shell's area vectors sum to zero",
                    || format!("{what}: {shells:?}"),
                );
            }
            Err(why) => f.add("an area refused", || format!("{what}: {why}")),
        }
    }
    f.assert_none("mass: areas", 8);
}

#[test]
fn grid_moves_keep_volume_and_area() {
    let mut d = Draw::new("mass: grid moves", 0x8003);
    let mut f = Findings::default();
    let measure = |body: &Body| -> Result<(i128, i128), String> {
        let a = body.arenas();
        let mut total = 0_i128;
        for row in 0..a.faces.len() {
            let face = FaceIx::new(row).ok_or("face index")?;
            let v = face_area(body, face, &mut meter()).map_err(|e| format!("{e:?}"))?;
            total += v.iter().map(|c| c.abs()).sum::<i128>();
        }
        Ok((six(body)?, total))
    };
    for _ in 0..60 {
        let prism = draw_prism(&mut d, None);
        let Ok(body) = prism.body() else {
            f.add("a prism the kernel would not build", || format!("{prism:?}"));
            continue;
        };
        f.ran();
        let moved = Moved {
            body: &body,
            exactness: Exactness::Exact,
            accept: None,
        };
        let axis = [Axis::X, Axis::Y, Axis::Z][d.pick(3)];
        let centre = p(d.between(-5, 5), d.between(-5, 5), d.between(-5, 5));
        let (how, out) = match d.below(3) {
            0 => {
                let by = [0; 3].map(|_: i64| d.between(-50, 50));
                (format!("moved by {by:?}"), move_by(&ctx(), moved, by))
            }
            1 => {
                let turns = i32::try_from(d.between(1, 3)).unwrap();
                (
                    format!("{turns} quarter turns about {axis:?}"),
                    rotate(&ctx(), moved, axis, Rotation::quarter_turns(turns), centre),
                )
            }
            _ => (format!("mirrored in {axis:?}"), mirror(&ctx(), moved, axis, centre)),
        };
        let out = match out {
            Ok(o) => o.result.body,
            Err(e) => {
                f.add("a grid move refused", || format!("{prism:?} {how}: {e:?}"));
                continue;
            }
        };
        let (before, after) = (measure(&body), measure(&out));
        f.check(before.is_ok() && before == after, "a grid move keeps volume and area", || {
            format!("{prism:?} {how}: {before:?} then {after:?}")
        });
    }
    f.assert_none("mass: grid moves", 50);
}
