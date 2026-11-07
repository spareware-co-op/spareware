// SPDX-License-Identifier: Apache-2.0
//! The law catalogue's row for booleans and cells (chapter 28.3; piece G155), on flat solids and
//! `geom-bool`'s entry points.
//!
//! The laws, each a seeded property test over boxes and unions of boxes drawn on a small grid, so
//! that shared planes, touching faces, equal boxes and boxes inside boxes come up all the time:
//!
//! - **Volumes, exactly.** `vol(A ∪ B) + vol(A ∩ B) = vol A + vol B` and
//!   `vol(A − B) = vol A − vol(A ∩ B)`, on six times the volume, which is a whole number when every
//!   vertex is a grid point (chapter 28.3's first care).
//! - **`A − A` is empty, and `A ∪ A = A`.** The second as volume and as the same vertices.
//! - **Imprint leaves volume unchanged**, and **cells partition the space**: the cells of two
//!   operands (inside both, inside one only, inside the other only) have volumes that sum to the
//!   union's, and each is what its test says.
//! - **The membership law.** Every coordinate of the sampled points is odd and every box's is even,
//!   so no point is on a boundary and the three-answer classification has two answers there:
//!   a point is in `A op B` exactly when `op` applied to whether it is in `A` and in `B` says so.
//!   The inputs' answers come from the flat reference (`geom_test::flat_reference`), written
//!   from the definitions alone; the result's from the kernel's own shell queries.
//! - **As shapes.** Union and intersection commute; grid translations, quarter turns and mirrors
//!   commute with every boolean (the same vertices, the same volume); a batch cut equals the same
//!   cuts one at a time, in either order.
//! - **Against the reference.** On two boxes, the kernel's union, intersection and difference have
//!   exactly the volumes the reference's brute force over a full plane arrangement gives.
//!
//! The re-cut cache (G139, the line of G140) is not in this branch; its laws join once it is
//! integrated with G131. Seeds: each law prints `seed: <hex>`, replays with `SPAREKIT_SEED`;
//! pull requests run the fixed default seed and the nightly workflow a fresh one.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::panic,
    clippy::too_many_lines,
    clippy::unwrap_used,
    reason = "tests: native arithmetic on small numbers, and a failure stops the test"
)]

#[path = "catalogue/mod.rs"]
mod catalogue;

use std::collections::BTreeSet;

use catalogue::{Draw, Findings};
use geom_bool::combine::{combine, intersect, subtract, unite};
use geom_bool::operand::{BooleanInput, Operand, OperandKey, OperandKind, Selection, Term};
use geom_check::{Level, check};
use geom_make::{BoxInput, Moved, make_box, mirror, move_by, rotate};
use geom_math::{Axis, Rotation};
use geom_num::{Exactness, P3, Steps};
use geom_op::{OpContext, OpId, OpPath};
use geom_poly::merge::Merge;
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::flat_reference::{Op, Solid, Where, boolean, int, point as qpoint};
use geom_topo::query::{point as vertex_point, shell_volume, side_of_shell, volume, Side};
use geom_topo::{Body, BodyKind, ShellIx, VertexIx};

const SERIAL: Exec<'static> = Exec::serial();

/// The cases of each law on a pull request, and the least a law must run.
const CASES: usize = 60;

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([7; 16])), Budget::UNLIMITED)
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn key(n: u8) -> OperandKey {
    OperandKey([n; 16])
}

fn solid(n: u8, body: &Body) -> Operand<'_> {
    Operand {
        key: key(n),
        body,
        kind: OperandKind::Solid,
        exactness: Exactness::Exact,
    }
}

/// A box on the even grid.
#[derive(Clone, Copy, Debug)]
struct Cuboid {
    low: [i64; 3],
    size: [u64; 3],
}

impl Cuboid {
    fn high(&self) -> [i64; 3] {
        [0, 1, 2].map(|i| self.low[i] + self.size[i] as i64)
    }

    fn contains(&self, x: [i64; 3]) -> bool {
        let high = self.high();
        (0..3).all(|i| self.low[i] < x[i] && x[i] < high[i])
    }

    fn reference(&self) -> Solid {
        let h = self.high();
        Solid::cuboid(
            &qpoint(self.low[0], self.low[1], self.low[2]),
            &qpoint(h[0], h[1], h[2]),
        )
        .unwrap()
    }
}

/// How the second box is drawn against the first.
fn draw_cuboid(d: &mut Draw, near: Option<&Cuboid>) -> Cuboid {
    let mut c = Cuboid {
        low: [0; 3].map(|_: i64| 2 * d.between(0, 5)),
        size: [0; 3].map(|_: u64| 2 * d.between(1, 4) as u64),
    };
    let Some(n) = near else { return c };
    let axis = d.pick(3);
    match d.below(6) {
        // Touching the first at a face.
        0 => c.low[axis] = n.low[axis] + n.size[axis] as i64,
        1 => c.low[axis] = n.low[axis] - c.size[axis] as i64,
        // Equal.
        2 => c = *n,
        // Inside, flush on one side.
        3 => {
            for i in 0..3 {
                c.size[i] = 2 * d.between(1, (n.size[i] / 2) as i64) as u64;
                c.low[i] = n.low[i] + 2 * d.between(0, ((n.size[i] - c.size[i]) / 2) as i64);
            }
        }
        // Sharing a plane.
        4 => c.low[axis] = n.low[axis],
        _ => {}
    }
    c
}

/// A solid of one box or two overlapping or separate boxes, with its body and each box's own
/// body, which carries the names a transform needs.
struct Shape {
    parts: Vec<Cuboid>,
    pieces: Vec<Body>,
    body: Body,
}

impl Shape {
    fn contains(&self, x: [i64; 3]) -> bool {
        self.parts.iter().any(|c| c.contains(x))
    }
}

fn make(c: &Cuboid) -> Result<Body, String> {
    let input = BoxInput {
        min: p(c.low[0], c.low[1], c.low[2]),
        size: c.size.map(Steps),
    };
    make_box(&ctx(), &input)
        .map(|made| made.result.body)
        .map_err(|e| format!("make_box {c:?}: {e:?}"))
}

fn draw_shape(d: &mut Draw, near: Option<&Shape>) -> Result<Shape, String> {
    let first = draw_cuboid(d, near.and_then(|s| s.parts.first()));
    let mut parts = vec![first];
    if d.one_in(2) {
        parts.push(draw_cuboid(d, Some(&first)));
    }
    let pieces = parts.iter().map(make).collect::<Result<Vec<Body>, String>>()?;
    let body = joined(&pieces).map_err(|e| format!("building {parts:?}: {e}"))?;
    Ok(Shape {
        parts,
        pieces,
        body,
    })
}

/// The union of one or two boxes' bodies.
fn joined(pieces: &[Body]) -> Result<Body, String> {
    match pieces {
        [one] => Ok(one.clone()),
        [a, b] => unite(&ctx(), &SERIAL, &[solid(1, a), solid(2, b)])
            .map(|out| out.result)
            .map_err(|e| format!("{e:?}")),
        _ => Err(format!("{} pieces", pieces.len())),
    }
}

fn draw_pair(d: &mut Draw) -> Result<(Shape, Shape), String> {
    let a = draw_shape(d, None)?;
    let b = draw_shape(d, Some(&a))?;
    Ok((a, b))
}

/// Six times a body's volume, `0` for no body at all. A regularised boolean of solids bounds its
/// material by closed shells even where it is not manifold (two boxes sharing only an edge make a
/// general body, chapter 11.6), so a general body's volume is the sum of its shells', as a solid's
/// is.
fn six(body: &Body) -> Result<i128, String> {
    if body.arenas().edges.is_empty() {
        return Ok(0);
    }
    let total = if body.kind() == BodyKind::General {
        let mut sum = 0_i128;
        for row in 0..body.arenas().shells.len() {
            let s = ShellIx::new(row).ok_or("shell index")?;
            let v = shell_volume(body, s, &mut meter())
                .map_err(|e| format!("shell_volume: {e:?}"))?
                .to_i128()
                .ok_or("shell volume: out of i128")?;
            sum += v;
        }
        sum
    } else {
        volume(body, &mut meter())
            .map_err(|e| format!("volume: {e:?}"))?
            .to_i128()
            .ok_or("volume: out of i128")?
    };
    Ok(total)
}

fn vertices(body: &Body) -> BTreeSet<[i64; 3]> {
    (0..body.arenas().vertices.len())
        .filter_map(VertexIx::new)
        .filter_map(|v| vertex_point(body, v).ok())
        .map(|q| [q.x.get(), q.y.get(), q.z.get()])
        .collect()
}

fn checked(body: &Body) -> bool {
    check(body, Exactness::Exact, Level::Full, &mut meter()).is_ok_and(|r| r.passed())
}

/// Runs `law` on `CASES` cases drawn from the law's seed, collecting what it finds.
fn run(label: &str, default: u64, law: impl Fn(&mut Findings, &mut Draw, &Shape, &Shape)) {
    let mut d = Draw::new(label, default);
    let mut f = Findings::default();
    for _ in 0..CASES {
        match draw_pair(&mut d) {
            Ok((a, b)) => {
                f.ran();
                law(&mut f, &mut d, &a, &b);
            }
            Err(why) => f.add("a shape the kernel would not build", || why),
        }
    }
    f.assert_none(label, CASES / 2);
}

/// Adds the error of a refused boolean as a finding of `case`.
fn took<T, E: core::fmt::Debug>(
    f: &mut Findings,
    case: &str,
    what: &str,
    result: Result<T, E>,
) -> Option<T> {
    match result {
        Ok(v) => Some(v),
        Err(e) => {
            f.add(case, || format!("{what}: refused with {e:?}"));
            None
        }
    }
}

fn describe(a: &Shape, b: &Shape) -> String {
    format!("A {:?} and B {:?}", a.parts, b.parts)
}

#[test]
fn volumes_add_up_exactly() {
    run("booleans: volumes", 0x6001, |f, _, a, b| {
        let both = [solid(1, &a.body), solid(2, &b.body)];
        let (Some(u), Some(i), Some(s)) = (
            took(f, "refused", "union", unite(&ctx(), &SERIAL, &both)),
            took(f, "refused", "intersection", intersect(&ctx(), &SERIAL, &both)),
            took(f, "refused", "difference", subtract(&ctx(), &SERIAL, key(1), &both)),
        ) else {
            return;
        };
        let (Ok(va), Ok(vb), Ok(vu), Ok(vi), Ok(vs)) = (
            six(&a.body),
            six(&b.body),
            six(&u.result),
            six(&i.result),
            six(&s.result),
        ) else {
            f.add("volume", || describe(a, b));
            return;
        };
        f.check(vu + vi == va + vb, "vol(A u B) + vol(A n B) = vol A + vol B", || {
            format!("{} gives {vu} + {vi} against {va} + {vb}", describe(a, b))
        });
        f.check(vs == va - vi, "vol(A - B) = vol A - vol(A n B)", || {
            format!("{} gives {vs} against {va} - {vi}", describe(a, b))
        });
        for (name, body) in [("union", &u.result), ("intersection", &i.result), ("difference", &s.result)] {
            f.check(body.arenas().edges.is_empty() || checked(body), "full checker", || {
                format!("{name} of {}", describe(a, b))
            });
        }
    });
}

#[test]
fn a_solid_minus_itself_is_empty_and_united_with_itself_is_itself() {
    run("booleans: idempotence", 0x6002, |f, _, a, _| {
        let Some(copy) = took(f, "refused", "a copy", rebuild(a)) else {
            return;
        };
        let twin = [solid(1, &a.body), solid(2, &copy)];
        if let Some(none) = took(f, "refused", "A - A", subtract(&ctx(), &SERIAL, key(1), &twin)) {
            f.check(none.result.arenas().edges.is_empty(), "A - A is empty", || {
                format!("{:?} leaves {} edges", a.parts, none.result.arenas().edges.len())
            });
        }
        if let Some(same) = took(f, "refused", "A u A", unite(&ctx(), &SERIAL, &twin)) {
            f.check(six(&same.result) == six(&a.body), "A u A has A's volume", || {
                format!("{:?}", a.parts)
            });
            f.check(
                vertices(&same.result) == vertices(&a.body),
                "A u A has A's vertices",
                || format!("{:?}", a.parts),
            );
        }
    });
}

/// A second body of the same solid, built again from the same boxes.
fn rebuild(s: &Shape) -> Result<Body, String> {
    let pieces = s.parts.iter().map(make).collect::<Result<Vec<Body>, String>>()?;
    joined(&pieces)
}

#[test]
fn imprint_keeps_the_volume_and_the_cells_partition_the_union() {
    run("booleans: imprint and cells", 0x6003, |f, _, a, b| {
        let input = BooleanInput {
            operands: vec![solid(1, &a.body), solid(2, &b.body)],
            select: vec![
                Selection::Imprint { target: key(1) },
                Selection::Where(vec![Term::Inside(key(1)), Term::Inside(key(2)), Term::Not, Term::And]),
                Selection::Where(vec![Term::Inside(key(2)), Term::Inside(key(1)), Term::Not, Term::And]),
                Selection::Where(vec![Term::Inside(key(1)), Term::Inside(key(2)), Term::And]),
                Selection::Unite,
            ],
            merge: Merge::RemoveRedundant,
            accept: None,
        };
        let Some(out) = took(f, "refused", "one arrangement", combine(&ctx(), &SERIAL, &input)) else {
            return;
        };
        let volumes: Vec<Result<i128, String>> = out
            .result
            .iter()
            .map(|bodies| {
                bodies
                    .iter()
                    .try_fold(0_i128, |sum, body| six(body).map(|v| sum + v))
            })
            .collect();
        let [Ok(imprint), Ok(only_a), Ok(only_b), Ok(both), Ok(union)] = volumes.as_slice() else {
            f.add("volume", || format!("{volumes:?} for {}", describe(a, b)));
            return;
        };
        f.check(Ok(*imprint) == six(&a.body), "imprint leaves volume unchanged", || {
            format!("{imprint} against {:?} for {}", six(&a.body), describe(a, b))
        });
        f.check(only_a + only_b + both == *union, "the cells sum to the union", || {
            format!("{only_a} + {only_b} + {both} against {union} for {}", describe(a, b))
        });
        f.check(
            Ok(only_a + both) == six(&a.body) && Ok(only_b + both) == six(&b.body),
            "the cells of an operand sum to the operand",
            || format!("{only_a}, {only_b}, {both} for {}", describe(a, b)),
        );
    });
}

/// Whether the odd point `x` is inside `body`: inside a shell that bounds material, and in no cavity.
fn kernel_inside(body: &Body, x: [i64; 3]) -> Result<bool, String> {
    let mut in_material = false;
    let mut in_cavity = false;
    for row in 0..body.arenas().shells.len() {
        let s = ShellIx::new(row).ok_or("shell index")?;
        let side = side_of_shell(body, s, p(x[0], x[1], x[2]), &mut meter())
            .map_err(|e| format!("side_of_shell: {e:?}"))?;
        if side == Some(Side::Inside) {
            let six_v = shell_volume(body, s, &mut meter())
                .map_err(|e| format!("shell_volume: {e:?}"))?
                .to_i128()
                .ok_or("shell volume")?;
            if six_v > 0 {
                in_material = true;
            } else {
                in_cavity = true;
            }
        }
    }
    Ok(in_material && !in_cavity)
}

#[test]
fn a_point_is_in_the_result_when_the_operation_says_so_of_its_classes() {
    run("booleans: membership", 0x6004, |f, d, a, b| {
        let both = [solid(1, &a.body), solid(2, &b.body)];
        let (Some(u), Some(i), Some(s)) = (
            took(f, "refused", "union", unite(&ctx(), &SERIAL, &both)),
            took(f, "refused", "intersection", intersect(&ctx(), &SERIAL, &both)),
            took(f, "refused", "difference", subtract(&ctx(), &SERIAL, key(1), &both)),
        ) else {
            return;
        };
        for _ in 0..24 {
            let x = [0; 3].map(|_: i64| 2 * d.between(-1, 7) + 1);
            let (ia, ib) = (a.contains(x), b.contains(x));
            for (name, body, expect) in [
                ("union", &u.result, ia || ib),
                ("intersection", &i.result, ia && ib),
                ("difference", &s.result, ia && !ib),
            ] {
                match kernel_inside(body, x) {
                    Ok(got) => f.check(got == expect, name, || {
                        format!("point {x:?} is {got} in the result, {ia} in A and {ib} in B: {}", describe(a, b))
                    }),
                    Err(why) => f.add(name, || format!("{why} at {x:?}")),
                }
            }
        }
    });
}

#[test]
fn the_kernels_volumes_are_the_references() {
    let mut d = Draw::new("booleans: against the reference", 0x6005);
    let mut f = Findings::default();
    for _ in 0..CASES {
        let ca = draw_cuboid(&mut d, None);
        let cb = draw_cuboid(&mut d, Some(&ca));
        let (Ok(ba), Ok(bb)) = (make(&ca), make(&cb)) else {
            f.add("a box the kernel would not build", || format!("{ca:?} {cb:?}"));
            continue;
        };
        f.ran();
        let both = [solid(1, &ba), solid(2, &bb)];
        let (Some(u), Some(i), Some(s)) = (
            took(&mut f, "refused", "union", unite(&ctx(), &SERIAL, &both)),
            took(&mut f, "refused", "intersection", intersect(&ctx(), &SERIAL, &both)),
            took(&mut f, "refused", "difference", subtract(&ctx(), &SERIAL, key(1), &both)),
        ) else {
            continue;
        };
        let (ra, rb) = (ca.reference(), cb.reference());
        for (name, kernel, op) in [
            ("union", &u.result, Op::Union),
            ("intersection", &i.result, Op::Intersection),
            ("difference", &s.result, Op::Difference),
        ] {
            let reference = boolean(&ra, &rb, op).unwrap().volume() * int(6);
            let got = six(kernel).map(|v| int(i64::try_from(v).unwrap()));
            f.check(got.as_ref() == Ok(&reference), name, || {
                format!("kernel {got:?} against reference {reference} for {ca:?} and {cb:?}")
            });
        }
        // The reference's answers about the point classes the membership law uses.
        let x = [0; 3].map(|_: i64| 2 * d.between(-1, 7) + 1);
        f.check(
            (ra.locate(&qpoint(x[0], x[1], x[2])) == Where::In) == ca.contains(x),
            "the reference and the box agree on a point",
            || format!("{x:?} in {ca:?}"),
        );
    }
    f.assert_none("booleans: against the reference", CASES / 2);
}

/// The kernel's boolean of `a` and `b` under `op`, as a body.
fn operate(op: &str, a: &Body, b: &Body) -> Result<Body, String> {
    let both = [solid(1, a), solid(2, b)];
    match op {
        "union" => unite(&ctx(), &SERIAL, &both).map(|o| o.result),
        "intersection" => intersect(&ctx(), &SERIAL, &both).map(|o| o.result),
        _ => subtract(&ctx(), &SERIAL, key(1), &both).map(|o| o.result),
    }
    .map_err(|e| format!("{op}: {e:?}"))
}

#[test]
fn union_and_intersection_commute() {
    run("booleans: commutation", 0x6006, |f, _, a, b| {
        for op in ["union", "intersection"] {
            let (Some(ab), Some(ba)) = (
                took(f, "refused", op, operate(op, &a.body, &b.body)),
                took(f, "refused", op, operate(op, &b.body, &a.body)),
            ) else {
                continue;
            };
            f.check(six(&ab) == six(&ba), &format!("{op} commutes in volume"), || {
                format!("{} gives {:?} and {:?}", describe(a, b), six(&ab), six(&ba))
            });
            f.check(vertices(&ab) == vertices(&ba), &format!("{op} commutes in vertices"), || {
                format!("{}", describe(a, b))
            });
        }
    });
}

/// A grid move of a body by the kernel's own transform operations, and of a point by hand.
#[derive(Clone, Copy, Debug)]
enum Move {
    Shift([i64; 3]),
    Turn(Axis, i64),
    Mirror(Axis),
}

fn axis_index(axis: Axis) -> usize {
    match axis {
        Axis::X => 0,
        Axis::Y => 1,
        Axis::Z => 2,
    }
}

impl Move {
    fn draw(d: &mut Draw) -> Move {
        let axis = [Axis::X, Axis::Y, Axis::Z][d.pick(3)];
        match d.below(3) {
            0 => Move::Shift([0; 3].map(|_: i64| 2 * d.between(-3, 3))),
            1 => Move::Turn(axis, d.between(1, 3)),
            _ => Move::Mirror(axis),
        }
    }

    /// The shape moved: each of its boxes moved by the kernel's transform, and united again.
    fn shape(self, s: &Shape) -> Result<Body, String> {
        let moved = s
            .pieces
            .iter()
            .map(|piece| self.body(piece))
            .collect::<Result<Vec<Body>, String>>()?;
        joined(&moved)
    }

    fn body(self, b: &Body) -> Result<Body, String> {
        let moved = Moved {
            body: b,
            exactness: Exactness::Exact,
            accept: None,
        };
        match self {
            Move::Shift(by) => move_by(&ctx(), moved, by),
            Move::Turn(axis, n) => rotate(&ctx(), moved, axis, Rotation::quarter_turns(i32::try_from(n).unwrap()), p(0, 0, 0)),
            Move::Mirror(axis) => mirror(&ctx(), moved, axis, p(0, 0, 0)),
        }
        .map(|o| o.result.body)
        .map_err(|e| format!("{self:?}: {e:?}"))
    }

    fn point(self, x: [i64; 3]) -> [i64; 3] {
        match self {
            Move::Shift(by) => [0, 1, 2].map(|i| x[i] + by[i]),
            Move::Mirror(axis) => {
                let mut y = x;
                y[axis_index(axis)] = -y[axis_index(axis)];
                y
            }
            Move::Turn(axis, n) => {
                let k = axis_index(axis);
                let (i, j) = ((k + 1) % 3, (k + 2) % 3);
                let mut y = x;
                for _ in 0..n {
                    let (u, v) = (y[i], y[j]);
                    y[i] = -v;
                    y[j] = u;
                }
                y
            }
        }
    }
}

#[test]
fn grid_moves_commute_with_every_boolean() {
    run("booleans: grid moves", 0x6007, |f, d, a, b| {
        let m = Move::draw(d);
        let (Some(ma), Some(mb)) = (
            took(f, "refused", "moving A", m.shape(a)),
            took(f, "refused", "moving B", m.shape(b)),
        ) else {
            return;
        };
        for op in ["union", "intersection", "difference"] {
            let (Some(first), Some(moved_first)) = (
                took(f, "refused", op, operate(op, &a.body, &b.body)),
                took(f, "refused", op, operate(op, &ma, &mb)),
            ) else {
                continue;
            };
            let expected: BTreeSet<[i64; 3]> = vertices(&first).into_iter().map(|q| m.point(q)).collect();
            let name = format!("{op} commutes with {m:?}");
            f.check(six(&first) == six(&moved_first), &format!("{name} (volume)"), || {
                format!("{} gives {:?} and {:?}", describe(a, b), six(&first), six(&moved_first))
            });
            f.check(vertices(&moved_first) == expected, &format!("{name} (vertices)"), || {
                format!("{}", describe(a, b))
            });
        }
    });
}

#[test]
fn a_batch_cut_equals_the_cuts_one_at_a_time_in_any_order() {
    let mut d = Draw::new("booleans: batch cuts", 0x6008);
    let mut f = Findings::default();
    for _ in 0..CASES {
        let drawn = (|| -> Result<(Shape, Shape, Shape), String> {
            let a = draw_shape(&mut d, None)?;
            let t1 = draw_shape(&mut d, Some(&a))?;
            let t2 = draw_shape(&mut d, Some(&a))?;
            Ok((a, t1, t2))
        })();
        let Ok((a, t1, t2)) = drawn else {
            f.add("a shape the kernel would not build", || "a batch".to_owned());
            continue;
        };
        f.ran();
        let all = [solid(1, &a.body), solid(2, &t1.body), solid(3, &t2.body)];
        let Some(batch) = took(&mut f, "refused", "batch", subtract(&ctx(), &SERIAL, key(1), &all)) else {
            continue;
        };
        for (first, second, name) in [(&t1, &t2, "1 then 2"), (&t2, &t1, "2 then 1")] {
            let step1 = operate("difference", &a.body, &first.body);
            let step2 = step1.and_then(|s| operate("difference", &s, &second.body));
            let Some(one_by_one) = took(&mut f, "refused", name, step2) else {
                continue;
            };
            f.check(six(&batch.result) == six(&one_by_one), &format!("batch = one at a time, {name} (volume)"), || {
                format!("A {:?}, tools {:?} and {:?}", a.parts, t1.parts, t2.parts)
            });
            f.check(
                vertices(&batch.result) == vertices(&one_by_one),
                &format!("batch = one at a time, {name} (vertices)"),
                || format!("A {:?}, tools {:?} and {:?}", a.parts, t1.parts, t2.parts),
            );
        }
    }
    f.assert_none("booleans: batch cuts", CASES / 2);
}
