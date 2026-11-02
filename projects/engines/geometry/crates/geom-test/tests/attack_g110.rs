//! Attacks on G110 (chapter 28.4, fault injection both ways): every valid body and closed-form
//! construction passes the full checker; every fault of the catalogue, on every body it applies
//! to, is caught with the right kind; every injection applies to some body; and the harness itself
//! catches a checker that sees nothing, one that names the wrong thing, and one that rejects too
//! much.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::too_many_lines,
    reason = "test code: a failure should stop loudly"
)]

use std::collections::BTreeSet;

use geom_check::FaultKind;
use geom_make::{
    BoxInput, Made, Moved, PrismInput, RoundInput, SphereInput, TorusInput, cone, cylinder,
    make_box, mirror, move_by, prism, rotate, scale, sphere, torus,
};
use geom_math::{Axis, Rotation, Scale};
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_op::{OpContext, OpId, OpOutput, OpPath};
use geom_run::Budget;
use geom_test::faults::{Applied, AttackError, CATALOGUE, attack, attack_with, full_check, missed};
use geom_test::plane_bodies::{CORPUS, meeting, scaled_jog};
use geom_topo::Body;

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([9; 16])), Budget::UNLIMITED)
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

/// The valid bodies: every primitive at several sizes and places, each moved, mirrored, turned
/// and scaled; closed forms all, since the corpus itself is G114's.
fn corpus() -> Vec<(String, Body, Exactness)> {
    let c = ctx();
    let mut bases: Vec<(String, OpOutput<Made>)> = Vec::new();
    for (i, at) in [p(0, 0, 0), p(-40, 17, 3), p(1 << 30, -(1 << 30), 5)]
        .into_iter()
        .enumerate()
    {
        let size = [Steps(4 << i), Steps(6 << i), Steps(8 << i)];
        let [ax, ay, az] = [at.x.get(), at.y.get(), at.z.get()];
        let base = vec![
            p(ax, ay, az),
            p(ax + 10, ay, az),
            p(ax + 12, ay + 7, az),
            p(ax + 3, ay + 11, az),
        ];
        let round = RoundInput {
            base: at,
            radius: Steps(5 << i),
            height: Steps(10 << i),
            step: GridStep::Q0,
        };
        let ball = SphereInput {
            center: at,
            radius: Steps(5 << i),
            step: GridStep::Q0,
        };
        let ring = TorusInput {
            center: at,
            major: Steps(10 << i),
            minor: Steps(3 << i),
            step: GridStep::Q0,
        };
        bases.push((
            format!("box {i}"),
            make_box(&c, &BoxInput { min: at, size }).unwrap(),
        ));
        bases.push((
            format!("prism {i}"),
            prism(
                &c,
                &PrismInput {
                    base,
                    height: Steps(9 << i),
                },
            )
            .unwrap(),
        ));
        bases.push((format!("cylinder {i}"), cylinder(&c, &round).unwrap()));
        bases.push((format!("cone {i}"), cone(&c, &round).unwrap()));
        bases.push((format!("sphere {i}"), sphere(&c, &ball).unwrap()));
        bases.push((format!("torus {i}"), torus(&c, &ring).unwrap()));
    }
    let mut all = Vec::new();
    for (name, out) in bases {
        let moved = Moved {
            body: &out.result.body,
            exactness: out.exactness,
            accept: Some(certified(64)),
        };
        let doubled = [Scale::new(2, 1).unwrap(); 3];
        let quarter = Rotation::quarter_turns(1);
        let at = p(1, 2, 3);
        for (how, result) in [
            ("moved", move_by(&c, moved, [3, -4, 5])),
            ("mirrored", mirror(&c, moved, Axis::X, at)),
            ("quarter turned", rotate(&c, moved, Axis::Z, quarter, at)),
            ("doubled", scale(&c, moved, doubled, at)),
        ] {
            let made = result.unwrap();
            all.push((format!("{name} {how}"), made.result.body, made.exactness));
        }
        all.push((name, out.result.body, out.exactness));
    }
    // A body of planes, for the faults only such a body has (G135).
    let tilted = meeting(CORPUS[0], &scaled_jog(1)).unwrap();
    all.push(("a tilted box".to_string(), tilted, Exactness::Exact));
    all
}

#[test]
fn every_valid_body_passes_and_every_fault_is_caught_with_its_kind() {
    let mut misses = Vec::new();
    for (name, body, exactness) in corpus() {
        let outcomes = attack(&body, exactness).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        misses.extend(
            outcomes
                .into_iter()
                .filter(|o| o.applied == Applied::Missed)
                .map(|o| format!("{name}: {} reported {:?}", o.name, o.reported)),
        );
    }
    assert!(
        misses.is_empty(),
        "{}",
        misses.join(
            "
"
        )
    );
}

#[test]
fn every_injection_applies_to_some_body() {
    let mut applied = BTreeSet::new();
    for (_, body, exactness) in corpus() {
        for o in attack(&body, exactness).unwrap() {
            if o.applied == Applied::Caught {
                applied.insert(o.name);
            }
        }
    }
    let all: BTreeSet<&str> = CATALOGUE.iter().map(|i| i.name).collect();
    assert_eq!(applied, all);
}

/// The catalogue holds what chapter 28.4 and the board's "done when" name.
#[test]
fn the_catalogue_holds_what_the_chapter_names() {
    let names: Vec<&str> = CATALOGUE.iter().map(|i| i.name).collect();
    for wanted in [
        "a flipped fin",
        "an open loop",
        "a vertex off its surface by just more than its bound",
        "two crossing faces",
        "a shell of a void region",
        "a stated bound lowered below the real error",
    ] {
        assert!(names.contains(&wanted), "{wanted}");
    }
    let unique: BTreeSet<&str> = names.iter().copied().collect();
    assert_eq!(unique.len(), names.len());
    for i in CATALOGUE {
        assert!(!i.kinds.is_empty(), "{}", i.name);
    }
}

fn a_box() -> (Body, Exactness) {
    let out = make_box(
        &ctx(),
        &BoxInput {
            min: p(0, 0, 0),
            size: [Steps(4), Steps(6), Steps(8)],
        },
    )
    .unwrap();
    (out.result.body, out.exactness)
}

#[test]
fn a_blind_checker_is_caught() {
    let (body, exactness) = a_box();
    let outcomes = attack_with(&body, exactness, |_, _| Ok(Vec::new())).unwrap();
    let applied = outcomes.iter().filter(|o| o.applied != Applied::No).count();
    assert!(applied > 10, "{outcomes:?}");
    assert_eq!(missed(&outcomes).len(), applied);
}

#[test]
fn a_checker_that_names_the_wrong_thing_is_caught() {
    let (body, exactness) = a_box();
    let valid = body.clone();
    // It reports every broken body, always as something the catalogue never expects of it.
    let outcomes = attack_with(&body, exactness, |b, _| {
        Ok(if *b == valid {
            Vec::new()
        } else {
            vec![FaultKind::Unchecked]
        })
    })
    .unwrap();
    let applied = outcomes.iter().filter(|o| o.applied != Applied::No).count();
    assert_eq!(missed(&outcomes).len(), applied);
    let first = outcomes
        .iter()
        .find(|o| o.applied == Applied::Missed)
        .unwrap();
    assert_eq!(first.reported, [FaultKind::Unchecked]);
}

#[test]
fn a_checker_that_rejects_a_valid_body_is_caught() {
    let (body, exactness) = a_box();
    assert_eq!(
        attack_with(&body, exactness, |_, _| Ok(vec![FaultKind::Euler])).unwrap_err(),
        AttackError::NotValid(vec![FaultKind::Euler])
    );
}

#[test]
fn a_body_that_does_not_pass_is_never_attacked() {
    // The body at a record it does not meet: a round body called Exact.
    let c = cylinder(
        &ctx(),
        &RoundInput {
            base: p(0, 0, 0),
            radius: Steps(5),
            height: Steps(10),
            step: GridStep::Q0,
        },
    )
    .unwrap();
    assert!(matches!(
        attack(&c.result.body, Exactness::Exact),
        Err(AttackError::NotValid(_))
    ));
    assert!(full_check(&c.result.body, c.exactness).unwrap().passed());
}
