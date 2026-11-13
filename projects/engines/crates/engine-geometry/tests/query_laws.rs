//! The Query family's laws (chapter 22, "Queries are first-class operations"; piece G102), on
//! every primitive, moved, turned and mirrored, recorded at every class.
//!
//! What this file proves: a query reads its body and changes nothing; the same body gives the same
//! answer twice, and the same answer again after the body and its histories are saved to bytes
//! and read back (24.4); every answer states the class its body is recorded with, and
//! `query.exactness` reports that record and the body's geometry as they are; `query.validity` is
//! the checker's own report at the level asked. Beside the laws, each answer is held to an oracle
//! that does not share the query's code: a flat solid's six volume against its sizes, its faces'
//! twice-area vectors summing to zero over its closed shell, its box against its vertices, and its
//! faces' names against the body's. `proptest!` draws the bodies; the unit tests break each law
//! once with a query that gets it wrong, so none of them passes by being blind.
//!
//! Covers: `query.area`, `query.bounding_box`, `query.exactness`, `query.faces`,
//! `query.validity`, `query.volume`.
//!
//! Family laws: Query.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::similar_names,
    reason = "test code: a failure should stop loudly"
)]

use core::cell::Cell;

use engine_geometry::operations::{
    BoundingBox, BoxInput, ExactnessReport, Level, Made, Moved, NamedFace, PrismInput, Report,
    RoundInput, SphereInput, TorusInput, bounding_box, cone, cylinder, exactness, faces, make_box,
    mirror, move_by, prism, rotate, six_volume, sphere, torus, twice_area, validity,
};
use geom_format::body::{StoredBody, read_body, write_body};
use geom_format::history::{history_record, read_history};
use geom_format::record::Record;
use geom_math::{Axis, Rotation};
use geom_num::{Exactness, GridStep, I256, P3, Steps};
use geom_op::history::History;
use geom_op::{NameId, OpContext, OpError, OpId, OpOutput, OpPath};
use geom_run::{Budget, WorkMeter};
use geom_topo::ix::Arena;
use geom_topo::{Body, CurveKind, Ix, SurfaceKind};
use proptest::prelude::*;

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([0x51; 16])), Budget::UNLIMITED)
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// A body asked about: what a program holds of it, the body, the class it is recorded with
/// (beside the body, 24.4) and the histories of the operations that made it, oldest first.
#[derive(Clone, Debug)]
struct Asked {
    body: Body,
    exactness: Exactness,
    histories: Vec<History>,
}

/// Every query's answer on one body: each face's area, the volume, the box, the exactness report,
/// the faces, and the validity report at both levels.
#[derive(Clone, Debug, PartialEq)]
struct Answers {
    areas: Vec<Result<OpOutput<[i128; 3]>, OpError>>,
    volume: Result<OpOutput<I256>, OpError>,
    bounding_box: Result<OpOutput<Option<BoundingBox>>, OpError>,
    exactness: Result<OpOutput<ExactnessReport>, OpError>,
    faces: Result<OpOutput<Vec<NamedFace>>, OpError>,
    validity: [Result<OpOutput<Report>, OpError>; 2],
}

/// The six queries, each on `asked`, as a program calls them.
fn ask(asked: &Asked) -> Answers {
    let c = ctx();
    let (body, class) = (&asked.body, asked.exactness);
    let histories: Vec<&History> = asked.histories.iter().collect();
    Answers {
        areas: (0..body.arenas().rows(Arena::Face))
            .map(|f| twice_area(&c, body, Ix::new(f).unwrap(), class))
            .collect(),
        volume: six_volume(&c, body, class),
        bounding_box: bounding_box(&c, body, class),
        exactness: exactness(&c, body, class),
        faces: faces(&c, body, &histories, class),
        validity: [Level::Cheap, Level::Full].map(|level| validity(&c, body, class, level)),
    }
}

/// How a law's test asks: the real queries, or a broken one standing in for them.
type Ask<'a> = &'a dyn Fn(&Asked) -> Answers;

/// The body and histories of `asked` written as records, each record to its bytes, and read back:
/// a save and a reload. The class is kept beside the body, as 24.4 stores it.
fn reloaded(asked: &Asked) -> Asked {
    let mut m = meter();
    let mut through_bytes = |record: &Record| {
        let bytes = record.encode(&mut m).unwrap();
        let (back, read) = Record::decode(&bytes, &mut m).unwrap();
        assert_eq!(read, bytes.len());
        back
    };
    let stored = write_body(&asked.body, &mut meter()).unwrap();
    let back = StoredBody {
        index: through_bytes(&stored.index),
        chunks: stored.chunks.iter().map(&mut through_bytes).collect(),
    };
    let path = OpPath::new(OpId::from_bytes([0x52; 16]));
    let histories = asked
        .histories
        .iter()
        .map(|h| {
            let record = through_bytes(&history_record(&path, h).unwrap());
            let (at, history) = read_history(&record.payload, &mut meter()).unwrap();
            assert_eq!(at, path);
            history
        })
        .collect();
    Asked {
        body: read_body(&back, &mut meter()).unwrap(),
        exactness: asked.exactness,
        histories,
    }
}

/// The class of every answer that is one.
fn classes(answers: &Answers) -> Vec<Exactness> {
    let mut all: Vec<Exactness> = answers
        .areas
        .iter()
        .filter_map(|a| a.as_ref().ok().map(|o| o.exactness))
        .collect();
    all.extend(answers.volume.as_ref().ok().map(|o| o.exactness));
    all.extend(answers.bounding_box.as_ref().ok().map(|o| o.exactness));
    all.extend(answers.exactness.as_ref().ok().map(|o| o.exactness));
    all.extend(answers.faces.as_ref().ok().map(|o| o.exactness));
    for v in &answers.validity {
        all.extend(v.as_ref().ok().map(|o| o.exactness));
    }
    all
}

/// The faces on planes and the edges on lines of `body`, counted here from its arenas.
fn flat_and_straight(body: &Body) -> (u32, u32, u32, u32) {
    let a = body.arenas();
    let (faces, edges) = (a.rows(Arena::Face), a.rows(Arena::Edge));
    let flat = (0..faces)
        .filter(|&f| {
            let face = a.faces.get(Ix::new(f).unwrap()).unwrap();
            matches!(a.surfaces.get(face.surface), Some(SurfaceKind::Plane(_)))
        })
        .count() as u32;
    let straight = (0..edges)
        .filter(|&e| {
            let edge = a.edges.get(Ix::new(e).unwrap()).unwrap();
            matches!(a.curves.get(edge.curve), Some(CurveKind::Line(_)))
        })
        .count() as u32;
    (flat, faces - flat, straight, edges - straight)
}

/// Chapter 22's laws for a query, on `asked`, through `ask`: the first broken one, named.
fn laws(asked: &Asked, ask: Ask<'_>) -> Result<Answers, String> {
    let before = write_body(&asked.body, &mut meter()).unwrap();
    let first = ask(asked);
    // A query reads its body and changes nothing: the body writes to the same records after.
    if write_body(&asked.body, &mut meter()).unwrap() != before {
        return Err("a query changed its body".to_owned());
    }
    // The same body gives the same answer twice, and through a save and a reload.
    if ask(asked) != first {
        return Err("a second asking gave another answer".to_owned());
    }
    if ask(&reloaded(asked)) != first {
        return Err("the body read back gave another answer".to_owned());
    }
    // Each answer states its body's record, no more exact and no less.
    if let Some(other) = classes(&first).iter().find(|c| **c != asked.exactness) {
        return Err(format!(
            "an answer stated {other:?} for a body recorded {:?}",
            asked.exactness
        ));
    }
    // The exactness report says what the record claims and what the geometry is, and judges the
    // record by the checker's cheap groups.
    let report = first.exactness.as_ref().map_err(|e| format!("{e:?}"))?;
    let r = &report.result;
    let counted = (
        r.flat_faces,
        r.curved_faces,
        r.straight_edges,
        r.curved_edges,
    );
    if r.recorded != asked.exactness || counted != flat_and_straight(&asked.body) {
        return Err(format!("the exactness report is not the body's: {r:?}"));
    }
    let checker =
        |level| geom_check::check(&asked.body, asked.exactness, level, &mut meter()).unwrap();
    if r.report != checker(Level::Cheap) {
        return Err("the exactness report's judgement is not the checker's".to_owned());
    }
    // The validity report is the checker's own, at each level.
    for (got, level) in first.validity.iter().zip([Level::Cheap, Level::Full]) {
        let got = got
            .as_ref()
            .map(|o| &o.result)
            .map_err(|e| format!("{e:?}"))?;
        if *got != checker(level) {
            return Err(format!("validity at {level:?} is not the checker's report"));
        }
    }
    Ok(first)
}

/// The oracles, for a body every face of which is a polygon on a plane: six times its volume is
/// `six`, its faces' twice-area vectors sum to zero over its closed shell, its box is its
/// vertices' and tight, and its faces carry the body's names in arena order.
fn flat_oracles(asked: &Asked, answers: &Answers, six: i128) -> Result<(), String> {
    let volume = answers.volume.as_ref().map_err(|e| format!("{e:?}"))?;
    if volume.result != I256::from_i128(six) {
        return Err(format!("six volume {:?}, not {six}", volume.result));
    }
    let mut sum = [0i128; 3];
    for area in &answers.areas {
        let area = area.as_ref().map_err(|e| format!("{e:?}"))?;
        for (s, a) in sum.iter_mut().zip(area.result) {
            *s += a;
        }
    }
    if sum != [0; 3] {
        return Err(format!("a closed shell's area vectors sum to {sum:?}"));
    }
    let a = asked.body.arenas();
    let corners: Vec<P3> = (0..a.rows(Arena::Vertex))
        .map(|v| {
            let vertex = a.vertices.get(Ix::new(v).unwrap()).unwrap();
            *a.points.get(vertex.point).unwrap()
        })
        .collect();
    let axis = |get: &dyn Fn(&P3) -> i64| {
        let values = corners.iter().map(get);
        (values.clone().min().unwrap(), values.max().unwrap())
    };
    let ((x0, x1), (y0, y1), (z0, z1)) = (
        axis(&|q| q.x.get()),
        axis(&|q| q.y.get()),
        axis(&|q| q.z.get()),
    );
    let want = (p(x0, y0, z0), p(x1, y1, z1));
    let boxed = answers
        .bounding_box
        .as_ref()
        .map_err(|e| format!("{e:?}"))?
        .result
        .ok_or("a solid has no box")?;
    if !boxed.tight || (boxed.bound.min(), boxed.bound.max()) != want {
        return Err(format!("box {boxed:?}, not {want:?}"));
    }
    let listed = &answers.faces.as_ref().map_err(|e| format!("{e:?}"))?.result;
    let names = asked
        .body
        .names()
        .map(|n| n.faces.clone())
        .unwrap_or_default();
    let named: Vec<Option<NameId>> = listed.iter().map(|f| f.name).collect();
    let want: Vec<Option<NameId>> = names
        .iter()
        .map(|k| Some(NameId::from_bytes(k.0)))
        .collect();
    if named != want {
        return Err("the faces are not the body's, by name, in arena order".to_owned());
    }
    Ok(())
}

/// The primitive `kind` (0 a box, 1 a prism on a right triangle, 2 a cylinder, 3 a cone, 4 a
/// sphere, 5 a torus) at `at`, of sizes `a`, `b` and `h`; with six times its volume if it is flat.
fn primitive(kind: u8, at: P3, a: u64, b: u64, h: u64) -> (OpOutput<Made>, Option<i128>) {
    let c = ctx();
    let (ia, ib, ih) = (i128::from(a), i128::from(b), i128::from(h));
    let (x, y, z) = (at.x.get(), at.y.get(), at.z.get());
    let round = RoundInput {
        base: at,
        radius: Steps(a),
        height: Steps(h),
        step: GridStep::Q0,
    };
    match kind % 6 {
        0 => {
            let input = BoxInput {
                min: at,
                size: [Steps(a), Steps(b), Steps(h)],
            };
            (make_box(&c, &input).unwrap(), Some(6 * ia * ib * ih))
        }
        1 => {
            let (sa, sb) = (a as i64, b as i64);
            let base = vec![p(x, y, z), p(x + sa, y, z), p(x, y + sb, z)];
            let input = PrismInput {
                base,
                height: Steps(h),
            };
            // A prism's volume is its base's area times its height: here a·b/2 · h.
            (prism(&c, &input).unwrap(), Some(3 * ia * ib * ih))
        }
        2 => (cylinder(&c, &round).unwrap(), None),
        3 => (cone(&c, &round).unwrap(), None),
        4 => {
            let input = SphereInput {
                center: at,
                radius: Steps(a),
                step: GridStep::Q0,
            };
            (sphere(&c, &input).unwrap(), None)
        }
        _ => {
            let input = TorusInput {
                center: at,
                major: Steps(a.max(b) + 1),
                minor: Steps(a.min(b)),
                step: GridStep::Q0,
            };
            (torus(&c, &input).unwrap(), None)
        }
    }
}

/// `out` after `motion` (0 none, 1 a move, 2 a quarter turn about z, 3 a mirror across x = 7),
/// as a program holds it, recorded as `record` gives (0 as made, 1 a looser Certified, 2
/// Approximate).
fn asked(out: OpOutput<Made>, motion: u8, offset: [i64; 3], record: u8, loose: u64) -> Asked {
    let c = ctx();
    let moved = Moved {
        body: &out.result.body,
        exactness: out.exactness,
        accept: None,
    };
    let after = match motion % 4 {
        0 => None,
        1 => Some(move_by(&c, moved, offset).unwrap()),
        2 => Some(rotate(&c, moved, Axis::Z, Rotation::quarter_turns(1), p(3, 4, 5)).unwrap()),
        _ => Some(mirror(&c, moved, Axis::X, p(7, 0, 0)).unwrap()),
    };
    let mut histories = vec![out.result.history.clone()];
    let (body, made) = match after {
        Some(next) => {
            histories.push(next.result.history);
            (next.result.body, next.exactness)
        }
        None => (out.result.body, out.exactness),
    };
    let exactness = match record % 3 {
        0 => made,
        1 => made
            .combine(Exactness::Certified {
                bound: Steps(loose),
                step: GridStep::Q0,
            })
            .unwrap(),
        _ => Exactness::Approximate {
            deviation: Steps(loose),
            step: GridStep::Q0,
        },
    };
    Asked {
        body,
        exactness,
        histories,
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    #[test]
    fn every_query_keeps_its_laws_on_any_body(
        kind in 0u8..6, motion in 0u8..4, record in 0u8..3,
        at in prop::array::uniform3(-100_000i64..100_000),
        a in 1u64..400, b in 1u64..400, h in 1u64..400,
        offset in prop::array::uniform3(-1000i64..1000),
        loose in 1u64..100,
    ) {
        let (out, six) = primitive(kind, p(at[0], at[1], at[2]), a, b, h);
        let asked = asked(out, motion, offset, record, loose);
        let answers = laws(&asked, &ask).map_err(TestCaseError::fail)?;
        match six {
            Some(six) => flat_oracles(&asked, &answers, six).map_err(TestCaseError::fail)?,
            // A curved body's mass properties wait for piece G177: refused with a type, never
            // a number.
            None => prop_assert!(answers.volume.is_err()),
        }
    }
}

/// A box, its six volume, and the body asked about, as made.
fn a_box() -> (Asked, i128) {
    let (out, six) = primitive(0, p(-5, 0, 3), 10, 20, 30);
    (asked(out, 0, [0; 3], 0, 1), six.unwrap())
}

#[test]
fn the_laws_hold_on_every_primitive_and_motion() {
    for kind in 0..6 {
        for motion in 0..4 {
            for record in 0..3 {
                let (out, six) = primitive(kind, p(1, -2, 3), 7, 5, 9);
                let asked = asked(out, motion, [4, 5, 6], record, 11);
                let answers = laws(&asked, &ask).unwrap();
                if let Some(six) = six {
                    flat_oracles(&asked, &answers, six).unwrap();
                }
            }
        }
    }
}

#[test]
fn a_query_that_answers_differently_twice_is_caught() {
    let (box_, _) = a_box();
    let calls = Cell::new(0u32);
    let drifting = |asked: &Asked| {
        calls.set(calls.get() + 1);
        let mut answers = ask(asked);
        if calls.get() == 2 {
            answers.areas.pop();
        }
        answers
    };
    assert_eq!(
        laws(&box_, &drifting).unwrap_err(),
        "a second asking gave another answer"
    );
}

#[test]
fn a_query_whose_answer_does_not_survive_a_reload_is_caught() {
    // A query reading something a save does not keep: here, which asking it is.
    let (box_, _) = a_box();
    let calls = Cell::new(0u32);
    let forgetful = |asked: &Asked| {
        calls.set(calls.get() + 1);
        let mut answers = ask(asked);
        if calls.get() >= 3 {
            answers.faces = Ok(OpOutput::new(Vec::new(), asked.exactness));
        }
        answers
    };
    assert_eq!(
        laws(&box_, &forgetful).unwrap_err(),
        "the body read back gave another answer"
    );
}

#[test]
fn a_query_that_claims_a_class_of_its_own_is_caught() {
    let (box_, _) = a_box();
    let loose = Asked {
        exactness: Exactness::Approximate {
            deviation: Steps(3),
            step: GridStep::Q0,
        },
        ..box_
    };
    let boasting = |asked: &Asked| {
        let mut answers = ask(asked);
        if let Ok(v) = &mut answers.volume {
            v.exactness = Exactness::Exact;
        }
        answers
    };
    assert!(
        laws(&loose, &boasting)
            .unwrap_err()
            .contains("stated Exact")
    );
}

#[test]
fn a_report_that_is_not_the_bodys_or_the_checkers_is_caught() {
    let (box_, _) = a_box();
    let miscounting = |asked: &Asked| {
        let mut answers = ask(asked);
        if let Ok(r) = &mut answers.exactness {
            r.result.curved_faces += 1;
        }
        answers
    };
    assert!(
        laws(&box_, &miscounting)
            .unwrap_err()
            .starts_with("the exactness report is not the body's")
    );
    // A validity report from the other level is not the checker's at this one.
    let lazy = |asked: &Asked| {
        let mut answers = ask(asked);
        answers.validity[1] = answers.validity[0].clone();
        answers
    };
    assert_eq!(
        laws(&box_, &lazy).unwrap_err(),
        "validity at Full is not the checker's report"
    );
    let unjudged = |asked: &Asked| {
        let mut answers = ask(asked);
        if let (Ok(r), Ok(full)) = (&mut answers.exactness, &answers.validity[1]) {
            r.result.report = full.result.clone();
        }
        answers
    };
    assert_eq!(
        laws(&box_, &unjudged).unwrap_err(),
        "the exactness report's judgement is not the checker's"
    );
}

#[test]
fn a_wrong_volume_area_box_or_face_list_is_caught() {
    let (box_, six) = a_box();
    let answers = laws(&box_, &ask).unwrap();
    flat_oracles(&box_, &answers, six).unwrap();
    assert!(
        flat_oracles(&box_, &answers, six + 6)
            .unwrap_err()
            .starts_with("six volume")
    );
    let mut lopsided = answers.clone();
    if let Some(Ok(a)) = lopsided.areas.first_mut() {
        a.result[0] += 2;
    }
    assert!(
        flat_oracles(&box_, &lopsided, six)
            .unwrap_err()
            .starts_with("a closed shell's")
    );
    let mut loose = answers.clone();
    if let Ok(b) = &mut loose.bounding_box {
        b.result = b.result.map(|bb| BoundingBox { tight: false, ..bb });
    }
    assert!(
        flat_oracles(&box_, &loose, six)
            .unwrap_err()
            .starts_with("box")
    );
    let mut shuffled = answers;
    if let Ok(f) = &mut shuffled.faces {
        f.result.swap(0, 1);
    }
    assert!(
        flat_oracles(&box_, &shuffled, six)
            .unwrap_err()
            .starts_with("the faces are not the body's")
    );
}

#[test]
fn the_box_read_back_is_the_box() {
    // The reload the laws use is a real one: the body comes back equal, with its names.
    let (box_, _) = a_box();
    let back = reloaded(&box_);
    assert_eq!(back.body, box_.body);
    assert_eq!(back.body.names(), box_.body.names());
    assert_eq!(back.histories, box_.histories);
}
