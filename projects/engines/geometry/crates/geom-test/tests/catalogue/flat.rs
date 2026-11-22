// SPDX-License-Identifier: Apache-2.0
//! Flat solids for the law catalogue's rows on numbers, booleans and mass (chapter 28.3; piece
//! G155): prisms over triangles, kites and rectangles drawn on a small grid, each as the kernel's
//! body and as the flat reference's solid (`geom_test::flat_reference`), and the exact rational
//! place of every vertex the kernel stores.
//!
//! A triangle's or a kite's sides are slanted planes, so two prisms cross where no grid point is:
//! the cases where a boolean must keep a vertex as the meeting of three planes.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    reason = "tests: native arithmetic on small numbers, corners named as the geometry names them, and a failure stops the test"
)]

use std::collections::BTreeSet;

use dashu_int::{IBig, UBig};
use dashu_ratio::RBig;
use geom_bool::combine::{intersect, subtract, unite};
use geom_bool::operand::{Operand, OperandKey, OperandKind};
use geom_make::{BoxInput, PrismInput, make_box, prism};
use geom_num::{Exactness, P3, Steps, Wide};
use geom_op::{OpContext, OpId, OpPath};
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::flat_reference::{Op, Point, Region, Solid, int};
use geom_topo::Body;
use geom_topo::mass::Corner;

use super::Draw;

/// One thread: the laws compare results, not schedules.
pub const SERIAL: Exec<'static> = Exec::serial();

/// The context every catalogue operation runs in.
pub fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([5; 16])), Budget::UNLIMITED)
}

/// A meter with no limit.
pub fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// A grid point; the catalogue's coordinates are far inside the frame.
pub fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// An operand key.
pub fn key(n: u8) -> OperandKey {
    OperandKey([n; 16])
}

/// A solid operand.
pub fn solid(n: u8, body: &Body) -> Operand<'_> {
    Operand {
        key: key(n),
        body,
        kind: OperandKind::Solid,
        exactness: Exactness::Exact,
    }
}

/// A prism: a strictly convex base, counter-clockwise from above, from `z` up by `height`.
#[derive(Clone, Debug)]
pub struct Prism {
    /// The base's corners.
    pub base: Vec<[i64; 2]>,
    /// The base's height.
    pub z: i64,
    /// How far the top is above the base.
    pub height: u64,
}

impl Prism {
    /// The kernel's body: a box for a rectangle with sides along the axes, a prism otherwise.
    pub fn body(&self) -> Result<Body, String> {
        let corners: Vec<P3> = self.base.iter().map(|c| p(c[0], c[1], self.z)).collect();
        let made = prism(
            &ctx(),
            &PrismInput {
                base: corners,
                height: Steps(self.height),
            },
        );
        made.map(|out| out.result.body)
            .map_err(|e| format!("prism {self:?}: {e:?}"))
    }

    /// The flat reference's solid.
    pub fn reference(&self) -> Solid {
        let base: Vec<(RBig, RBig)> = self.base.iter().map(|c| (int(c[0]), int(c[1]))).collect();
        Solid::prism(&base, &int(self.z), &int(self.z + self.height as i64)).unwrap()
    }
}

/// A box, the axis-aligned case, as the kernel's body.
pub fn cuboid(low: [i64; 3], size: [u64; 3]) -> Result<Body, String> {
    make_box(
        &ctx(),
        &BoxInput {
            min: p(low[0], low[1], low[2]),
            size: size.map(Steps),
        },
    )
    .map(|out| out.result.body)
    .map_err(|e| format!("box {low:?} {size:?}: {e:?}"))
}

/// Twice the signed area of the triangle `a`, `b`, `c`.
fn turn(a: [i64; 2], b: [i64; 2], c: [i64; 2]) -> i64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

/// A prism over a triangle, a kite or a rectangle, near `near`'s centre when given, so that two
/// prisms overlap, touch or cross most of the time.
pub fn draw_prism(d: &mut Draw, near: Option<&Prism>) -> Prism {
    let centre = match near {
        Some(n) => {
            let c = n.base[0];
            [c[0] + d.between(-3, 3), c[1] + d.between(-3, 3)]
        }
        None => [d.between(-4, 4), d.between(-4, 4)],
    };
    let base = match d.below(3) {
        0 => loop {
            let at = |d: &mut Draw| [centre[0] + d.between(-6, 6), centre[1] + d.between(-6, 6)];
            let (a, b, c) = (at(d), at(d), at(d));
            match turn(a, b, c) {
                0 => {}
                t if t > 0 => break vec![a, b, c],
                _ => break vec![a, c, b],
            }
        },
        1 => {
            let [a, b, c, e] = [0; 4].map(|_: i64| d.between(1, 6));
            vec![
                [centre[0] + a, centre[1]],
                [centre[0], centre[1] + b],
                [centre[0] - c, centre[1]],
                [centre[0], centre[1] - e],
            ]
        }
        _ => {
            let (w, h) = (d.between(1, 8), d.between(1, 8));
            vec![
                centre,
                [centre[0] + w, centre[1]],
                [centre[0] + w, centre[1] + h],
                [centre[0], centre[1] + h],
            ]
        }
    };
    let z = match near {
        Some(n) if d.one_in(3) => n.z,
        _ => d.between(-4, 4),
    };
    Prism {
        base,
        z,
        height: d.between(1, 8) as u64,
    }
}

/// The kernel's boolean `op` of two bodies.
pub fn operate(op: Op, a: &Body, b: &Body) -> Result<Body, String> {
    let both = [solid(1, a), solid(2, b)];
    match op {
        Op::Union => unite(&ctx(), &SERIAL, &both),
        Op::Intersection => intersect(&ctx(), &SERIAL, &both),
        _ => subtract(&ctx(), &SERIAL, key(1), &both),
    }
    .map(|out| out.result.body)
    .map_err(|e| format!("{op:?}: {e:?}"))
}

/// A wide two's-complement integer as a big integer.
pub fn big<const L: usize>(w: Wide<L>) -> IBig {
    let mut magnitude = UBig::ZERO;
    for limb in w.to_le_limbs().iter().rev() {
        magnitude = (magnitude << 64_usize) + UBig::from(*limb);
    }
    let value = IBig::from(magnitude);
    if w.is_negative() {
        value - (IBig::ONE << (64 * L))
    } else {
        value
    }
}

/// The exact place of a vertex the kernel stores.
pub fn exact(corner: &Corner) -> Result<Point, String> {
    let ratio = |x: IBig, w: &IBig| -> RBig { RBig::from_parts_signed(x, w.clone()) };
    match corner {
        Corner::Grid(q) => Ok([q.x.get(), q.y.get(), q.z.get()].map(int)),
        Corner::Tpi(t) => {
            let (x, w) = t.homogeneous().map_err(|e| format!("{e:?}"))?;
            let w = big(w);
            Ok(x.map(|xi| ratio(big(xi), &w)))
        }
        Corner::Lpi(l) => {
            let (x, w) = l.homogeneous().map_err(|e| format!("{e:?}"))?;
            let w = IBig::from(w);
            Ok(x.map(|xi| ratio(big(xi), &w)))
        }
    }
}

/// Every corner of a reference region's boundary pieces.
pub fn corners(region: &Region) -> BTreeSet<Point> {
    region
        .boundary()
        .iter()
        .flat_map(|piece| piece.corners().to_vec())
        .collect()
}
