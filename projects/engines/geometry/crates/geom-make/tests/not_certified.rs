//! A body the checker cannot certify is refused with a typed error that names the piece it waits
//! for, never as a defect: also when the transform is exact and an earlier one left the body's
//! data less than exact (chapter 28.16: the make fuzz target's first two findings).
//!
//! What the fuzzer found: a prism scaled by 5/2 twice, which rounds its corners and leaves its
//! planes off the integers, then scaled by 5; and a prism scaled by (2, 7/3, 1), then by
//! (3, 1, 1). The last scale of each is exact, so it rounded nothing, and the checker could not
//! certify its result: it tests two planes not given by integers for being parallel in interval
//! arithmetic, which says so only while every product in their normals is exact in doubles. The
//! operation reported that as `ResultInvalid`, a defect. The tests here hold the rule that
//! replaced it, arm by arm, on the smallest cases found and on a seeded sweep.
//!
//! Covers: `transform.mirror`, `transform.move`, `transform.pattern`, `transform.rotate`,
//! `transform.scale`.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::panic,
    reason = "test code: a failure should stop loudly, in a helper as in a test"
)]

use std::num::NonZeroU64;

use geom_check::{FaultKind, Level, check};
use geom_make::{
    BoxInput, Instance, Made, Moved, PrismInput, RoundInput, SphereInput, TorusInput, cone,
    cylinder, make_box, mirror, move_by, pattern, prism, rotate, scale, sphere, torus,
};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Scale, TypedTransform};
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_op::{
    ErrorKind, MakeDetail, OpContext, OpError, OpErrorDetail, OpId, OpOutput, OpPath, Remedy,
};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, SurfaceKind};
use geom_test::{Seed, SplitMix64};
use geom_topo::{Body, Builder, Ix, Sense};

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([7; 16])), Budget::UNLIMITED)
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn certified(bound: u64) -> Exactness {
    Exactness::Certified {
        bound: Steps(bound),
        step: GridStep::Q0,
    }
}

fn s(num: u64, den: u64) -> Scale {
    Scale::new(num, den).unwrap()
}

/// The full checker's report on `body`, recorded `exactness`.
fn report(body: &Body, exactness: Exactness) -> geom_check::Report {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    check(body, exactness, Level::Full, &mut meter).unwrap()
}

/// Asserts `out` passes every checker group at its record, and returns it.
fn valid(name: &str, out: OpOutput<Made>) -> OpOutput<Made> {
    let found = report(&out.result.body, out.exactness);
    assert!(found.passed(), "{name}: {:?}", found.faults());
    out
}

/// `body` to transform, recorded `exactness`, any bound accepted in its class.
fn moved(body: &Body, exactness: Exactness) -> Moved<'_> {
    let accept = match exactness {
        Exactness::Approximate { step, .. } => Exactness::Approximate {
            deviation: Steps(1 << 40),
            step,
        },
        _ => certified(1 << 40),
    };
    Moved {
        body,
        exactness,
        accept: Some(accept),
    }
}

/// Asserts `e` is the typed refusal that waits for `piece`, with the checker's words.
fn waits(what: &str, e: &OpError, piece: &'static str) {
    assert_eq!(
        *e.detail(),
        OpErrorDetail::Make(MakeDetail::NotCertifiedYet { piece }),
        "{what}: {}",
        e.message()
    );
    assert_eq!(e.kind(), ErrorKind::Unsupported, "{what}");
    assert_eq!(e.remedies(), [Remedy::AwaitPiece { piece }], "{what}");
    assert!(
        e.message().contains("no certified"),
        "{what}: {}",
        e.message()
    );
}

/// A unit cube scaled by 7/3 along y, which rounds its far corners from 7/3 to 2: a body the
/// checker passes, recorded Certified, its planes no longer given by integers.
fn a_rounded_box() -> OpOutput<Made> {
    let c = ctx();
    let cube = make_box(
        &c,
        &BoxInput {
            min: p(0, 0, 0),
            size: [Steps(1); 3],
        },
    )
    .unwrap();
    let wide = scale(
        &c,
        moved(&cube.result.body, cube.exactness),
        [s(1, 1), s(7, 3), s(1, 1)],
        p(0, 0, 0),
    );
    let wide = valid("a rounded box", wide.unwrap());
    assert!(matches!(wide.exactness, Exactness::Certified { .. }));
    wide
}

/// `body` published again with `edit` applied to its rows, under the same names.
fn edited(body: &Body, edit: impl FnOnce(&mut Builder)) -> Body {
    let mut builder = Builder::edit(body);
    edit(&mut builder);
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    builder
        .publish_named(
            body.names().unwrap(),
            body.kind(),
            body.outside(),
            &mut meter,
        )
        .unwrap()
}

/// The smallest case: one scale that rounds, then one by whole numbers. The second rounds
/// nothing and moves every plane exactly, and the checker cannot certify the caps apart: their
/// normals' one coordinate, 3 × 7/3 in doubles, is no longer a double.
#[test]
fn an_exact_scale_of_a_rounded_box_waits_for_the_checker_and_is_no_defect() {
    let c = ctx();
    let wide = a_rounded_box();
    let input = moved(&wide.result.body, wide.exactness);
    let by_three = [s(3, 1), s(1, 1), s(1, 1)];
    let e = scale(&c, input, by_three, p(0, 0, 0)).unwrap_err();
    waits("scale", &e, "G134");
    // The same in a pattern, whose copies go through the same check.
    let stretch = RationalTransform::new([[3, 0, 0], [0, 1, 0], [0, 0, 1]], [0; 3], 1).unwrap();
    let instance = Instance {
        key: [1; 16],
        transform: TypedTransform::from(stretch),
    };
    let e = pattern(&c, input, &[instance]).unwrap_err();
    waits("pattern", &e, "G134");
    // And for a body recorded Approximate, which the checker judges by the same bound.
    let Exactness::Certified { bound, step } = wide.exactness else {
        panic!("a rounded box is certified");
    };
    let measured = Exactness::Approximate {
        deviation: bound,
        step,
    };
    let e = scale(&c, moved(&wide.result.body, measured), by_three, p(0, 0, 0)).unwrap_err();
    waits("approximate", &e, "G134");
    // A scale by two keeps every product a double: the same body, certified.
    let doubled = scale(&c, input, [s(2, 1), s(1, 1), s(1, 1)], p(0, 0, 0));
    valid("doubled", doubled.unwrap());
    // And so do the transforms that only move, turn or mirror it.
    valid("moved", move_by(&c, input, [5, -7, 11]).unwrap());
    valid("mirrored", mirror(&c, input, Axis::X, p(3, 0, 0)).unwrap());
    let turn = Rotation::quarter_turns(1);
    valid(
        "turned",
        rotate(&c, input, Axis::Z, turn, p(1, 2, 3)).unwrap(),
    );
}

/// The fuzzer's first finding (crash-dc639a8b..., cut to four corners): two scales by 5/2, then
/// one by 5. Its sides on the lines of slope 3 lie on parallel planes off the integers.
#[test]
fn the_fuzzers_prism_scaled_by_five_halves_twice_then_by_five_waits_for_the_checker() {
    let c = ctx();
    let base = vec![p(-40, -11, 1), p(-39, -10, 1), p(-38, -7, 1), p(-37, -2, 1)];
    let height = Steps(41_472_000);
    let mut body = prism(&c, &PrismInput { base, height }).unwrap();
    let centre = p(50_368_000, 50_368_000, 50_368_000);
    for _ in 0..2 {
        let input = moved(&body.result.body, body.exactness);
        body = valid("5/2", scale(&c, input, [s(5, 2); 3], centre).unwrap());
    }
    assert_eq!(body.exactness, certified(7));
    let input = moved(&body.result.body, body.exactness);
    let e = scale(&c, input, [s(5, 1); 3], p(1_241_601, 0, 0)).unwrap_err();
    waits("scale by 5", &e, "G134");
    let five = RationalTransform::uniform_scale(s(5, 1)).unwrap();
    let instance = Instance {
        key: [0; 16],
        transform: TypedTransform::from(five),
    };
    let e = pattern(&c, input, &[instance]).unwrap_err();
    waits("pattern by 5", &e, "G134");
    // By 4 and by 100 its planes land on integers, where the checker is exact.
    for k in [4, 100] {
        valid("whole", scale(&c, input, [s(k, 1); 3], p(0, 0, 0)).unwrap());
    }
}

/// The fuzzer's second finding (crash-5c649c78...): a triangle prism scaled by (2, 7/3, 1), then
/// by (3, 1, 1).
#[test]
fn the_fuzzers_triangle_scaled_unevenly_then_by_three_waits_for_the_checker() {
    let c = ctx();
    let base = vec![
        p(4094, 2048, 64_000),
        p(4095, 2049, 64_000),
        p(4096, 2052, 64_000),
    ];
    let height = Steps(41_472_000);
    let made = prism(&c, &PrismInput { base, height }).unwrap();
    let first = scale(
        &c,
        moved(&made.result.body, made.exactness),
        [s(4, 2), s(7, 3), s(2, 2)],
        p(-12_799, 4_518_401, -22),
    );
    let first = valid("the scale that rounds", first.unwrap());
    assert_eq!(first.exactness, certified(2));
    let e = scale(
        &c,
        moved(&first.result.body, first.exactness),
        [s(6, 2), s(2, 2), s(1, 1)],
        p(2_880_001, 1, 0),
    )
    .unwrap_err();
    waits("the scale by whole numbers", &e, "G134");
}

/// A fault that says something is wrong stays a defect on a body that is less than exact: an
/// exact move of a body that passed breaks nothing, so only a body that was broken gives one.
#[test]
fn a_wrong_body_moved_exactly_is_still_a_defect_when_it_is_less_than_exact() {
    let wide = a_rounded_box();
    let broken = edited(&wide.result.body, |b| {
        let f = Ix::new(0).unwrap();
        let flipped = match *b.arenas.fins.sense.get(f).unwrap() {
            Sense::Forward => Sense::Reversed,
            Sense::Reversed => Sense::Forward,
        };
        b.arenas.fins.sense.set(f, flipped).unwrap();
    });
    let found = report(&broken, wide.exactness);
    assert!(!found.passed() && !found.uncertified());
    let c = ctx();
    let input = moved(&broken, wide.exactness);
    let by_three = [s(3, 1), s(1, 1), s(1, 1)];
    let stretch = RationalTransform::new([[3, 0, 0], [0, 1, 0], [0, 0, 1]], [0; 3], 1).unwrap();
    let instance = Instance {
        key: [1; 16],
        transform: TypedTransform::from(stretch),
    };
    for e in [
        move_by(&c, input, [1, 0, 0]).unwrap_err(),
        scale(&c, input, by_three, p(0, 0, 0)).unwrap_err(),
        pattern(&c, input, &[instance]).unwrap_err(),
    ] {
        assert_eq!(*e.detail(), OpErrorDetail::Make(MakeDetail::ResultInvalid));
        assert_eq!(e.kind(), ErrorKind::Internal);
        assert_eq!(e.remedies(), [Remedy::Report]);
    }
}

/// A box with one face's plane written from a point off the grid: the same plane, which the
/// checker follows only in a body that is not recorded Exact.
fn a_box_on_a_plane_off_the_grid() -> Body {
    let made = make_box(
        &ctx(),
        &BoxInput {
            min: p(0, 0, 0),
            size: [Steps(4), Steps(6), Steps(10)],
        },
    )
    .unwrap();
    edited(&made.result.body, |b| {
        let first = Ix::new(0).unwrap();
        let Some(SurfaceKind::Plane(flat)) = b.arenas.surfaces.get(first) else {
            panic!("a box's faces are planes");
        };
        let (origin, a, b_vector) = flat.vectors();
        let off = [0, 1, 2].map(|k| origin[k] + a[k] / 8.0);
        let domain = ((0.0, 1.0), (0.0, 1.0));
        let same = Plane::new(off, a, b_vector, domain).unwrap();
        b.arenas
            .surfaces
            .set(first, SurfaceKind::Plane(same))
            .unwrap();
    })
}

/// Exact data are followed in full, so a result recorded Exact that the checker does not pass is
/// a defect even when all the checker says is that it has no check: a plane off the grid in an
/// exact body is the body's fault. Recorded Certified, the same body waits for the checker.
#[test]
fn an_exact_record_the_checker_cannot_follow_is_a_defect_and_a_certified_one_waits() {
    let body = a_box_on_a_plane_off_the_grid();
    let c = ctx();
    // Recorded Exact: nothing found wrong, one carrier unchecked, and still a defect.
    let found = report(&body, Exactness::Exact);
    assert_eq!(
        found.faults().iter().map(|f| f.kind).collect::<Vec<_>>(),
        [FaultKind::Unchecked]
    );
    assert!(found.uncertified());
    let exact = Moved {
        body: &body,
        exactness: Exactness::Exact,
        accept: None,
    };
    let e = move_by(&c, exact, [1, 0, 0]).unwrap_err();
    assert_eq!(*e.detail(), OpErrorDetail::Make(MakeDetail::ResultInvalid));
    assert_eq!(e.kind(), ErrorKind::Internal);
    assert!(
        e.message().contains("no certified check"),
        "{}",
        e.message()
    );
    // Recorded Certified, the checker follows the plane, and the move passes.
    valid(
        "moved",
        move_by(&c, moved(&body, certified(1)), [1, 0, 0]).unwrap(),
    );
}

/// The rounded box with its caps' planes written from longer vectors, 3 along x: the same
/// planes, whose normals are then no longer doubles, so the checker cannot certify the caps
/// apart. A body made by hand that no operation returns.
fn a_box_the_checker_cannot_certify() -> (Body, Exactness) {
    let wide = a_rounded_box();
    let body = edited(&wide.result.body, |b| {
        for row in 0..6 {
            let at = Ix::new(row).unwrap();
            let Some(SurfaceKind::Plane(flat)) = b.arenas.surfaces.get(at) else {
                panic!("a box's faces are planes");
            };
            let (origin, along, across) = flat.vectors();
            if along[2] != 0.0 || across[2] != 0.0 {
                continue;
            }
            let longer = |v: [f64; 3]| [3.0 * v[0], v[1], v[2]];
            let domain = ((0.0, 1.0), (0.0, 1.0));
            let same = Plane::new(origin, longer(along), longer(across), domain).unwrap();
            b.arenas.surfaces.set(at, SurfaceKind::Plane(same)).unwrap();
        }
    });
    (body, wide.exactness)
}

/// The rule is the body's record and the checker's report, not which transform ran: a move or
/// a mirror of a body the checker cannot certify waits for the checker too.
#[test]
fn a_move_of_a_body_the_checker_cannot_certify_waits_for_the_checker() {
    let (body, record) = a_box_the_checker_cannot_certify();
    let found = report(&body, record);
    assert_eq!(
        found.faults().iter().map(|f| f.kind).collect::<Vec<_>>(),
        [FaultKind::PairUnchecked]
    );
    assert!(found.uncertified());
    let c = ctx();
    let input = moved(&body, record);
    waits("move", &move_by(&c, input, [1, 0, 0]).unwrap_err(), "G134");
    let mirrored = mirror(&c, input, Axis::Y, p(0, 0, 0));
    waits("mirror", &mirrored.unwrap_err(), "G134");
    let turn = Rotation::quarter_turns(1);
    let turned = rotate(&c, input, Axis::X, turn, p(0, 0, 0));
    waits("quarter turn", &turned.unwrap_err(), "G134");
}

/// A value below `n`.
fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    rng.below(NonZeroU64::new(n).unwrap())
}

/// A coordinate: a size class, then a value in it, either sign.
fn coordinate(rng: &mut SplitMix64) -> i64 {
    let most = [10, 1000, 1_000_000, 100_000_000][below(rng, 4) as usize];
    below(rng, 2 * most + 1) as i64 - most as i64
}

/// A length: a size class, then a value in it.
fn length(rng: &mut SplitMix64) -> Steps {
    let most = [10, 1000, 1_000_000, 100_000_000][below(rng, 4) as usize];
    Steps(below(rng, most) + 1)
}

fn point(rng: &mut SplitMix64) -> P3 {
    p(coordinate(rng), coordinate(rng), coordinate(rng))
}

fn axis(rng: &mut SplitMix64) -> Axis {
    [Axis::X, Axis::Y, Axis::Z][below(rng, 3) as usize]
}

/// A primitive, flat three times in four, and whether it is flat.
fn primitive(rng: &mut SplitMix64) -> (bool, Result<OpOutput<Made>, OpError>) {
    let c = ctx();
    let at = point(rng);
    let step = GridStep::Q0;
    match below(rng, 8) {
        0..=2 => {
            let size = [length(rng), length(rng), length(rng)];
            (true, make_box(&c, &BoxInput { min: at, size }))
        }
        3..=5 => {
            // Corners on a parabola: strictly convex, counter-clockwise, some sides parallel.
            let (count, dx, k) = (3 + below(rng, 6), 1 + below(rng, 4), 1 + below(rng, 3));
            let corner = |i: u64| {
                let (i, dx, k) = (i as i64, dx as i64, k as i64);
                p(at.x.get() + i * dx, at.y.get() + k * i * i, at.z.get())
            };
            let base = (0..count).map(corner).collect();
            let height = length(rng);
            (true, prism(&c, &PrismInput { base, height }))
        }
        6 => {
            let round = RoundInput {
                base: at,
                radius: length(rng),
                height: length(rng),
                step,
            };
            let made = if rng.coin() {
                cylinder(&c, &round)
            } else {
                cone(&c, &round)
            };
            (false, made)
        }
        _ => {
            let made = if rng.coin() {
                let ball = SphereInput {
                    center: at,
                    radius: length(rng),
                    step,
                };
                sphere(&c, &ball)
            } else {
                let minor = length(rng);
                let ring = TorusInput {
                    center: at,
                    major: Steps(minor.get() + length(rng).get()),
                    minor,
                    step,
                };
                torus(&c, &ring)
            };
            (false, made)
        }
    }
}

/// A transform that is not exact, of `body`: a scale by fractions (one factor for a curved
/// body, which takes no other), or a turn by a direction.
fn inexact(
    rng: &mut SplitMix64,
    flat: bool,
    body: &OpOutput<Made>,
) -> Result<OpOutput<Made>, OpError> {
    let c = ctx();
    let input = moved(&body.result.body, body.exactness);
    if rng.coin() {
        let mut factor = || s(1 + below(rng, 7), 2 + below(rng, 2));
        let first = factor();
        let factors = if flat {
            [first, factor(), factor()]
        } else {
            [first; 3]
        };
        scale(&c, input, factors, point(rng))
    } else {
        let (cos, sin) = (1 + below(rng, 9) as i32, 1 + below(rng, 9) as i32);
        let turn = Rotation::direction(cos, sin).unwrap();
        rotate(&c, input, axis(rng), turn, point(rng))
    }
}

/// A transform that is exact, of `body` recorded `record`: its name, and its result's record or
/// its refusal. A result is held to the full checker.
fn exact(
    rng: &mut SplitMix64,
    flat: bool,
    body: &Body,
    record: Exactness,
) -> (&'static str, Result<Exactness, OpError>) {
    let c = ctx();
    let input = moved(body, record);
    let whole =
        |rng: &mut SplitMix64| s([2, 3, 4, 5, 6, 7, 10, 25, 100][below(rng, 9) as usize], 1);
    let checked = |what: &str, out: Result<OpOutput<Made>, OpError>| {
        out.map(|out| valid(what, out).exactness)
    };
    let one = |transform: TypedTransform| {
        let instance = Instance {
            key: [9; 16],
            transform,
        };
        pattern(&c, input, &[instance]).map(|copies| {
            for (_, made) in &copies.result {
                let found = report(&made.body, copies.exactness);
                assert!(found.passed(), "a copy: {:?}", found.faults());
            }
            copies.exactness
        })
    };
    match below(rng, 8) {
        0 => {
            let by = [coordinate(rng), coordinate(rng), coordinate(rng)];
            ("move", checked("move", move_by(&c, input, by)))
        }
        1 => {
            let turn = Rotation::quarter_turns(1 + below(rng, 3) as i32);
            let turned = rotate(&c, input, axis(rng), turn, point(rng));
            ("quarter turn", checked("quarter turn", turned))
        }
        2 => {
            let mirrored = mirror(&c, input, axis(rng), point(rng));
            ("mirror", checked("mirror", mirrored))
        }
        3 | 4 => {
            let first = whole(rng);
            let factors = if flat && rng.coin() {
                [first, whole(rng), s(1, 1)]
            } else {
                [first; 3]
            };
            let scaled = scale(&c, input, factors, point(rng));
            ("scale", checked("scale", scaled))
        }
        5 => {
            let by = [coordinate(rng), coordinate(rng), coordinate(rng)];
            let shift = GridTransform::translation(by).unwrap();
            ("pattern of a move", one(TypedTransform::Exact(shift)))
        }
        6 => {
            let flip = GridTransform::mirror(axis(rng));
            ("pattern of a mirror", one(TypedTransform::Exact(flip)))
        }
        _ => {
            let grow = RationalTransform::uniform_scale(whole(rng)).unwrap();
            ("pattern of a scale", one(TypedTransform::from(grow)))
        }
    }
}

/// The sweep: primitives of every kind, moved inexactly up to twice, then by an exact transform
/// of every sort, as recorded and recorded Approximate. Whatever comes back passes the full
/// checker or is a typed refusal; none is a defect. On the default seed it reaches the rule on
/// both sides: exact transforms the checker certifies, and ones it cannot.
#[test]
fn exact_transforms_of_bodies_less_than_exact_pass_or_refuse_with_a_type() {
    let label = "exact transforms of rounded bodies";
    let seed = geom_test::seed::resolve(label, Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let (mut passed, mut waited) = (0_u32, 0_u32);
    for case in 0..600 {
        let (flat, made) = primitive(&mut rng);
        let Ok(mut body) = made else {
            let e = made.unwrap_err();
            assert_ne!(e.kind(), ErrorKind::Internal, "case {case}: {e:?}");
            continue;
        };
        for _ in 0..below(&mut rng, 3) {
            match inexact(&mut rng, flat, &body) {
                Ok(out) => body = valid("an inexact transform", out),
                Err(e) => assert_ne!(e.kind(), ErrorKind::Internal, "case {case}: {e:?}"),
            }
        }
        for _ in 0..3 {
            let record = match body.exactness {
                Exactness::Certified { bound, step } if below(&mut rng, 4) == 0 => {
                    Exactness::Approximate {
                        deviation: bound,
                        step,
                    }
                }
                other => other,
            };
            let rounded = !matches!(record, Exactness::Exact);
            let (what, out) = exact(&mut rng, flat, &body.result.body, record);
            match out {
                Ok(result) => {
                    assert!(result.rank() >= record.rank(), "case {case}, {what}");
                    passed += u32::from(rounded);
                }
                Err(e) => {
                    assert_ne!(
                        e.kind(),
                        ErrorKind::Internal,
                        "case {case}, {what} of a body recorded {record:?}: {e:?}"
                    );
                    if let OpErrorDetail::Make(MakeDetail::NotCertifiedYet { piece }) = e.detail() {
                        assert!(
                            rounded,
                            "case {case}, {what}: an exact body waits for nothing"
                        );
                        assert_eq!(*piece, if flat { "G134" } else { "G177" }, "case {case}");
                        waited += 1;
                    }
                }
            }
        }
    }
    println!(
        "{passed} exact transforms of bodies less than exact passed; {waited} wait for the checker"
    );
    if seed == Seed::DEFAULT {
        assert!(passed > 100, "{passed}");
        assert!(waited > 10, "{waited}");
    }
}
