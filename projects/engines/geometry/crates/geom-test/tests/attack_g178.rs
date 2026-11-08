//! Piece G178's attack tests: the refutation checker for curved bodies (`geom_test::refute`,
//! chapter 28.4's third check). It must find nothing on every valid curved body, under every
//! transform the Make family offers, and find every curved fault of the catalogue with its kind;
//! what `geom-check` names on the same faults is recorded beside it.
//!
//! Seeds: `nightly_fixtures_have_nothing_found_and_every_fault_found` prints `seed: <hex>` and
//! replays with `SPAREKIT_SEED`; pull requests run the fixed default seed, and the nightly workflow
//! a fresh one (`kernel-nightly.yml`).

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::unnecessary_wraps,
    reason = "test code: a failure should stop loudly"
)]

use std::collections::BTreeSet;

use geom_check::FaultKind;
use geom_curve::{Basis, Circle, CurveKind, Frame, ParamCurve, Trimmed};
use geom_make::{
    BoxInput, Made, Moved, RoundInput, SphereInput, TorusInput, cone, cylinder, make_box, mirror,
    move_by, rotate, scale, sphere, torus,
};
use geom_math::{Axis, Rotation, Scale};
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_op::{OpContext, OpId, OpOutput, OpPath};
use geom_run::{Budget, Stopped, WorkMeter};
use geom_surface::{Extrusion, ParamSurface, Plane, SurfaceKind};
use geom_test::faults::{Applied, CATALOGUE, CURVED, Injection, attack_from, full_check};
use geom_test::refute::{Fault, Refutation, Unjudged, refute};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};
use geom_topo::ix::Arena;
use geom_topo::{Body, Builder, Face, Fin, Ix};

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

/// The checker's faults as `geom-check`'s kinds, for the catalogue's attack.
fn kinds(r: &Refutation) -> Vec<FaultKind> {
    r.faults()
        .into_iter()
        .map(|f| match f {
            Fault::VertexOffCurve { .. } => FaultKind::VertexOffCurve,
            Fault::VertexOffSurface { .. } => FaultKind::VertexOffSurface,
            Fault::EdgeOffSurface { .. } => FaultKind::EdgeOffSurface,
            Fault::PcurveOffCurve { .. } => FaultKind::PcurveOffCurve,
            Fault::FacesCross { .. } => FaultKind::FacesCross,
        })
        .collect()
}

fn refuter(b: &Body, e: Exactness) -> Result<Vec<FaultKind>, Stopped> {
    Ok(kinds(&refute(b, e)))
}

fn checker(b: &Body, e: Exactness) -> Result<Vec<FaultKind>, Stopped> {
    full_check(b, e).map(|r| r.faults().iter().map(|f| f.kind).collect())
}

/// The primitives of one size at one place.
fn primitives(at: P3, size: i64) -> Vec<(String, OpOutput<Made>)> {
    let c = ctx();
    let round = RoundInput {
        base: at,
        radius: Steps(5 * size as u64),
        height: Steps(10 * size as u64),
        step: GridStep::Q0,
    };
    vec![
        (
            "box".to_string(),
            make_box(
                &c,
                &BoxInput {
                    min: at,
                    size: [
                        Steps(4 * size as u64),
                        Steps(6 * size as u64),
                        Steps(8 * size as u64),
                    ],
                },
            )
            .unwrap(),
        ),
        ("cylinder".to_string(), cylinder(&c, &round).unwrap()),
        ("cone".to_string(), cone(&c, &round).unwrap()),
        (
            "sphere".to_string(),
            sphere(
                &c,
                &SphereInput {
                    center: at,
                    radius: Steps(5 * size as u64),
                    step: GridStep::Q0,
                },
            )
            .unwrap(),
        ),
        (
            "torus".to_string(),
            torus(
                &c,
                &TorusInput {
                    center: at,
                    major: Steps(10 * size as u64),
                    minor: Steps(3 * size as u64),
                    step: GridStep::Q0,
                },
            )
            .unwrap(),
        ),
    ]
}

/// Each primitive as made and under each transform of the Make family: a move, mirrors, and
/// quarter turns about each axis; a box also doubled, and turned to the direction (3, 4) about z
/// and x, which rounds. The Make family refuses such a turn of a curved body until `geom-check` can
/// certify its pairs of faces, and states a bound of about twice the radius for a doubled one
/// (162 steps for a cylinder of radius 80), past which a crossing near an edge cannot be told from
/// a crease; so neither is a curved fixture yet.
fn transformed(name: &str, out: &OpOutput<Made>) -> Vec<(String, Body, Exactness)> {
    let c = ctx();
    let moved = Moved {
        body: &out.result.body,
        exactness: out.exactness,
        accept: Some(certified(64)),
    };
    let at = p(1, 2, 3);
    let turned = Rotation::direction(3, 4).unwrap();
    let mut all = vec![(name.to_string(), out.result.body.clone(), out.exactness)];
    for (how, result) in [
        ("moved", move_by(&c, moved, [3, -4, 5])),
        ("mirrored", mirror(&c, moved, Axis::X, at)),
        ("mirrored in z", mirror(&c, moved, Axis::Z, at)),
        (
            "quarter turned",
            rotate(&c, moved, Axis::Z, Rotation::quarter_turns(1), at),
        ),
        (
            "quarter turned about x",
            rotate(&c, moved, Axis::X, Rotation::quarter_turns(1), at),
        ),
        (
            "half turned about y",
            rotate(&c, moved, Axis::Y, Rotation::quarter_turns(2), at),
        ),
        (
            "doubled",
            scale(&c, moved, [Scale::new(2, 1).unwrap(); 3], at),
        ),
        ("turned about z", rotate(&c, moved, Axis::Z, turned, at)),
        ("turned about x", rotate(&c, moved, Axis::X, turned, at)),
    ] {
        if (how.starts_with("turned") || how == "doubled") && !name.starts_with("box") {
            continue;
        }
        // A debug build takes a cut of the transforms, as the law catalogue takes an eighth of
        // its rounds (`tests/truth`): the 400-bit arithmetic is slow unoptimised.
        if cfg!(debug_assertions) && !DEBUG_TRANSFORMS.contains(&how) {
            continue;
        }
        let out = result.unwrap_or_else(|e| panic!("{name} {how}: {e:?}"));
        all.push((format!("{name} {how}"), out.result.body, out.exactness));
    }
    all
}

/// The transforms a debug build runs.
const DEBUG_TRANSFORMS: [&str; 3] = ["mirrored in z", "quarter turned about x", "turned about z"];

/// The plane-parameter circle a 3D circle maps to on plane c + u·a + v·b, for a and b
/// orthogonal and of one length, as the caps' are: the parameters of a point w from c are
/// (w·a, w·b)/|a|², and the radius r/|a|.
fn circle_on_plane(circle: &Circle<3>, (c, a, b): ([f64; 3], [f64; 3], [f64; 3])) -> CurveKind<2> {
    let dot = |x: [f64; 3], y: [f64; 3]| x[0] * y[0] + x[1] * y[1] + x[2] * y[2];
    let (aa, bb) = (dot(a, a), dot(b, b));
    assert!(
        dot(a, b) == 0.0 && aa == bb,
        "a cap's plane vectors: {a:?} {b:?}"
    );
    let (axes, _) = circle.frame().unit_vectors().unwrap();
    let [x, y, _] = axes;
    let center = circle.center();
    let w = [center[0] - c[0], center[1] - c[1], center[2] - c[2]];
    let q = [dot(w, a) / aa, dot(w, b) / aa];
    let (ux, uy) = (dot(x, a), dot(x, b));
    let (vx, vy) = (dot(y, a), dot(y, b));
    let turn = if ux * vy - uy * vx > 0.0 { 1.0 } else { -1.0 };
    let frame = Frame::new([ux, uy], [0.0, 0.0, turn]).unwrap();
    let radius = circle.radius() / geom_num::math::sqrt(aa).unwrap().get();
    CurveKind::Circle(Circle::new(q, radius, frame, circle.domain()).unwrap())
}

/// `body` with a pcurve on every fin whose edge is a circle on a plane: `geom-check` passes the
/// planar ones, and its curved pairs of faces would stay unchecked with pcurves on the round face.
fn with_pcurves(body: &Body) -> Body {
    let mut builder = Builder::edit(body);
    let a = &mut builder.arenas;
    for n in (0..a.rows(Arena::Fin)).filter_map(Ix::<Fin>::new) {
        let edge = *a.fins.edge.get(n).unwrap();
        let Some(CurveKind::Circle(circle)) =
            a.curves.get(*a.edges.curve.get(edge).unwrap()).cloned()
        else {
            continue;
        };
        let face: Ix<Face> = *a.loops.face.get(*a.fins.loop_.get(n).unwrap()).unwrap();
        let surface = a
            .surfaces
            .get(*a.faces.surface.get(face).unwrap())
            .unwrap()
            .clone();
        let SurfaceKind::Plane(plane) = &surface else {
            continue;
        };
        let pcurve = circle_on_plane(&circle, plane.vectors());
        // The cap's plane is stored over a nominal domain; it is widened to hold the pcurve and any
        // pcurve moved off it by a few bounds, the same plane, so `geom-check` evaluates them
        // inside their surface.
        let reach = circle.radius() + 64.0;
        let ((u0, u1), (v0, v1)) = plane.domain();
        let (low, high) = (
            |x: f64| if x < -reach { x } else { -reach },
            |x: f64| if x > reach { x } else { reach },
        );
        let (c, pa, pb) = plane.vectors();
        let wide = Plane::new(c, pa, pb, ((low(u0), high(u1)), (low(v0), high(v1)))).unwrap();
        a.surfaces
            .set(
                *a.faces.surface.get(face).unwrap(),
                SurfaceKind::Plane(wide),
            )
            .unwrap();
        let row = a.pcurves.push(pcurve).unwrap();
        a.fins.pcurve.set(n, Some(row)).unwrap();
    }
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    builder
        .publish(body.kind(), body.outside(), &mut meter)
        .unwrap()
}

fn corpus() -> Vec<(String, Body, Exactness)> {
    let mut all = Vec::new();
    for (i, at) in [p(0, 0, 0), p(1 << 30, -(1 << 30), 5), p(-40, 17, 3)]
        .into_iter()
        .enumerate()
        .take(if cfg!(debug_assertions) { 2 } else { 3 })
    {
        for (name, out) in primitives(at, 1 << (2 * i)) {
            all.extend(transformed(&format!("{name} {i}"), &out));
        }
    }
    let c = primitives(p(0, 0, 0), 1).swap_remove(1).1;
    all.push((
        "cylinder with pcurves on its caps".to_string(),
        with_pcurves(&c.result.body),
        c.exactness,
    ));
    all
}

#[test]
fn every_valid_curved_body_has_nothing_found_and_is_judged_whole() {
    let mut wrong = Vec::new();
    for (name, body, exactness) in corpus() {
        assert!(
            full_check(&body, exactness).unwrap().passed(),
            "{name} does not pass geom-check"
        );
        let r = refute(&body, exactness);
        if !r.found.is_empty() || !r.unjudged.is_empty() {
            wrong.push(format!(
                "{name} at {exactness:?}: {:?} unjudged {:?}",
                r.found, r.unjudged
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The catalogue's injections that break geometry, which the refutation checker must find too.
fn geometric() -> Vec<Injection> {
    let names = [
        "a vertex off its surface by just more than its bound",
        "a stated bound lowered below the real error",
        "a vertex off its curve by just more than its bound",
        "an edge off its surface by just more than its bound",
        "two crossing faces",
    ];
    CATALOGUE
        .iter()
        .filter(|i| names.contains(&i.name))
        .copied()
        .collect()
}

#[test]
fn every_curved_fault_is_found_with_its_kind() {
    let mut misses = Vec::new();
    let mut applied = BTreeSet::new();
    let geometric = geometric();
    for (name, body, exactness) in corpus() {
        for list in [CURVED, &geometric[..]] {
            for o in attack_from(list, &body, exactness, refuter).unwrap() {
                match o.applied {
                    Applied::Missed => {
                        misses.push(format!("{name}: {} reported {:?}", o.name, o.reported));
                    }
                    Applied::Caught => {
                        applied.insert(o.name);
                    }
                    Applied::No => {}
                }
            }
        }
    }
    assert!(misses.is_empty(), "{}", misses.join("\n"));
    let all: BTreeSet<&str> = CURVED.iter().chain(&geometric).map(|i| i.name).collect();
    assert_eq!(applied, all, "an injection applied to no body");
}

/// What `geom-check` names on the curved list: each fault it misses is a checker bug (28.4), and
/// this records which stand open so the list moves into the catalogue as each is fixed.
#[test]
fn geom_check_on_the_curved_faults() {
    let mut missed = BTreeSet::new();
    for (name, body, exactness) in corpus() {
        for o in attack_from(CURVED, &body, exactness, checker).unwrap() {
            if o.applied == Applied::Missed {
                missed.insert(format!("{}: {:?}", o.name, o.reported.first()));
                let _ = &name;
            }
        }
    }
    let missed: Vec<String> = missed.into_iter().collect();
    // G177 leaves a crossing it finds between curved faces unchecked until a point-in-face test
    // on a face's loops in its parameters lets it say FacesCross (its pull request's "Left").
    assert_eq!(
        missed,
        ["two crossing curved faces: Some(PairUnchecked)"],
        "geom-check misses"
    );
}

/// A bound met exactly is never a find; a step past it is.
#[test]
fn a_bound_met_exactly_is_not_found_and_a_step_past_it_is() {
    let made = make_box(
        &ctx(),
        &BoxInput {
            min: p(0, 0, 0),
            size: [Steps(4), Steps(6), Steps(8)],
        },
    )
    .unwrap();
    let body = made.result.body;
    let lifted = |by: i64| {
        let mut builder = Builder::edit(&body);
        let a = &mut builder.arenas;
        let row = (0..a.rows(Arena::Point))
            .filter_map(Ix::new)
            .find(|&r| a.points.get(r).unwrap().grid().unwrap().z.get() == 8)
            .unwrap();
        let q = a.points.get(row).unwrap().grid().unwrap();
        a.points
            .set(row, p(q.x.get(), q.y.get(), 8 + by).into())
            .unwrap();
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        builder
            .publish(body.kind(), body.outside(), &mut meter)
            .unwrap()
    };
    assert_eq!(refute(&lifted(2), certified(2)).found, []);
    let found = refute(&lifted(3), certified(2)).faults();
    assert!(
        found
            .iter()
            .any(|f| matches!(f, Fault::VertexOffSurface { .. })),
        "{found:?}"
    );
    assert!(
        found
            .iter()
            .any(|f| matches!(f, Fault::VertexOffCurve { .. })),
        "{found:?}"
    );
    let errors: Vec<f64> = refute(&lifted(3), certified(2))
        .found
        .iter()
        .filter_map(|f| f.error)
        .collect();
    assert!(errors.iter().all(|e| (*e - 3.0).abs() < 1e-9), "{errors:?}");
}

/// A kind without a definition here is listed, never passed in silence.
#[test]
fn a_kind_without_a_definition_is_listed_unjudged() {
    let made = primitives(p(0, 0, 0), 1).swap_remove(1).1;
    let body = made.result.body;
    let mut builder = Builder::edit(&body);
    let a = &mut builder.arenas;
    let (row, circle) = (0..a.rows(Arena::Curve))
        .filter_map(Ix::new)
        .find_map(|r| match a.curves.get(r) {
            Some(CurveKind::Circle(c)) => Some((r, c.clone())),
            _ => None,
        })
        .unwrap();
    let _ = row;
    let side = (0..a.rows(Arena::Surface))
        .filter_map(Ix::new)
        .find(|&r| matches!(a.surfaces.get(r), Some(SurfaceKind::Cylinder(_))))
        .unwrap();
    let swept = Extrusion::new(
        Trimmed::new(Basis::Circle(circle), (0.0, 2.0)).unwrap(),
        [0.0, 0.0, 10.0],
        (0.0, 1.0),
    )
    .unwrap();
    a.surfaces.set(side, SurfaceKind::Extrusion(swept)).unwrap();
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let swapped = builder
        .publish(body.kind(), body.outside(), &mut meter)
        .unwrap();
    let r = refute(&swapped, made.exactness);
    assert_eq!(r.found, []);
    assert!(
        r.unjudged.iter().any(|u| matches!(u, Unjudged::Surface(_))),
        "{:?}",
        r.unjudged
    );
}

/// Fresh fixtures each night: primitives of drawn sizes at drawn places, under drawn quarter turns
/// and mirrors (a box also turned to a drawn direction); nothing may be found on any, and every
/// curved fault on each.
#[test]
fn nightly_fixtures_have_nothing_found_and_every_fault_found() {
    let seed = resolve("attack_g178::nightly", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let mut draw = |lo: i64, hi: i64| lo + (rng.next_u64() % (hi - lo + 1) as u64) as i64;
    let mut wrong = Vec::new();
    for round in 0..if cfg!(debug_assertions) { 1 } else { 4 } {
        let at = p(
            draw(-1 << 20, 1 << 20),
            draw(-1 << 20, 1 << 20),
            draw(-1 << 20, 1 << 20),
        );
        let size = draw(1, 64);
        let (cos, sin) = (draw(-9, 9) as i32, draw(1, 9) as i32);
        let axis = [Axis::X, Axis::Y, Axis::Z][draw(0, 2) as usize];
        let quarters = draw(1, 3) as i32;
        for (name, out) in primitives(at, size) {
            let c = ctx();
            let moved = Moved {
                body: &out.result.body,
                exactness: out.exactness,
                accept: Some(certified(64)),
            };
            let turn = if name == "box" {
                Rotation::direction(cos, sin).unwrap()
            } else {
                Rotation::quarter_turns(quarters)
            };
            let mut bodies = vec![("made", out.result.body.clone(), out.exactness)];
            // No drawn scale: far from the origin the Make family states a bound of hundreds of
            // steps for one, past which no crossing near an edge can be told from a crease.
            for (how, result) in [
                ("turned", rotate(&c, moved, axis, turn, at)),
                ("mirrored", mirror(&c, moved, axis, at)),
            ] {
                let made = result.unwrap_or_else(|e| panic!("{name} {how}: {e:?}"));
                bodies.push((how, made.result.body, made.exactness));
            }
            for (how, body, exactness) in bodies {
                let label = format!(
                    "round {round}: {name} {how} at {at:?} size {size}, turn ({cos}, {sin}) or {quarters} quarters about {axis:?}"
                );
                let r = refute(&body, exactness);
                if !r.found.is_empty() || !r.unjudged.is_empty() {
                    wrong.push(format!("{label}: {:?} unjudged {:?}", r.found, r.unjudged));
                    continue;
                }
                for o in attack_from(CURVED, &body, exactness, refuter).unwrap() {
                    if o.applied == Applied::Missed {
                        wrong.push(format!(
                            "{label}: missed {} reported {:?}",
                            o.name, o.reported
                        ));
                    }
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
