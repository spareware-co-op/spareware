//! The replay hash of piece G102: every core query over primitives from a fixed corpus, hashed with
//! SHA-256, must give the same digest on every target (chapters 21 and 28.13). It is also the end
//! to end test of faces with roles: a box's faces are named as chapter 10.2 says, through a move.
//!
//! The corpus, from a fixed `SplitMix64` stream: 32 boxes and 32 prisms of random sizes and places,
//! each moved by a random offset, and 8 cylinders. For each body: every face's row, name and
//! roles, the bounding box, each flat face's twice-area vector, six times the volume of a flat
//! solid, the exactness report's counts, the validity report's deferred count, and the work each
//! query charged. Every number is written in full, so a target that orders a row, rounds a value or
//! charges a unit differently changes the digest.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use geom_check::Level;
use geom_check::query::{exactness, validity};
use geom_make::{
    BoxInput, Made, Moved, PrismInput, RoundInput, cylinder, make_box, move_by, prism,
};
use geom_name::faces;
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_op::history::History;
use geom_op::{OpContext, OpId, OpOutput, OpPath};
use geom_run::Budget;
use geom_test::golden::{self, hex, sha256};
use geom_test::{Seed, SplitMix64};
use geom_topo::query_ops::{bounding_box, six_volume, twice_area};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-make/queries";

fn ctx(tag: u8) -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([tag; 16])), Budget::UNLIMITED)
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// Every query's answer on one body, written into `out`.
fn record(out: &mut Vec<u8>, made: &OpOutput<Made>, histories: &[&History]) {
    let (body, recorded) = (&made.result.body, made.exactness);
    let mut line = |text: String| {
        out.extend_from_slice(text.as_bytes());
        out.push(b'\n');
    };
    let listed = faces(&ctx(1), body, histories, recorded).unwrap();
    for f in &listed.result {
        let roles: Vec<&str> = f.roles.iter().map(|r| r.name()).collect();
        line(format!(
            "face {} shell {} surface {} {:?} loops {} name {:?} roles {roles:?}",
            f.row.face.row(),
            f.row.shell.row(),
            f.row.surface.row(),
            f.row.sense,
            f.row.loops,
            f.name.map(geom_op::NameId::to_bytes),
        ));
        if let Ok(area) = twice_area(&ctx(2), body, f.row.face, recorded) {
            line(format!("area {:?}", area.result));
        }
    }
    let boxed = bounding_box(&ctx(3), body, recorded)
        .unwrap()
        .result
        .unwrap();
    let (lo, hi) = (boxed.bound.min(), boxed.bound.max());
    line(format!(
        "box {} {} {} to {} {} {} tight {}",
        lo.x.get(),
        lo.y.get(),
        lo.z.get(),
        hi.x.get(),
        hi.y.get(),
        hi.z.get(),
        boxed.tight
    ));
    if let Ok(v) = six_volume(&ctx(4), body, recorded) {
        line(format!("volume {:?}", v.result.to_le_limbs()));
    }
    let e = exactness(&ctx(5), body, recorded).unwrap().result;
    line(format!(
        "exactness {:?} faces {} {} edges {} {}",
        e.recorded, e.flat_faces, e.curved_faces, e.straight_edges, e.curved_edges
    ));
    let v = validity(&ctx(6), body, recorded, Level::Full)
        .unwrap()
        .result;
    assert!(v.passed(), "{:?}", v.faults());
    line(format!("deferred {}", v.deferred()));
}

/// The bytes of the whole corpus.
fn corpus() -> Vec<u8> {
    let mut r = SplitMix64::new(geom_test::seed::fixed(GOLDEN, Seed::DEFAULT));
    let mut out = Vec::new();
    let mut coord = |lo: i64, hi: i64| r.range_i64(lo, hi).unwrap();
    for _ in 0..32 {
        let min = p(coord(-1000, 1000), coord(-1000, 1000), coord(-1000, 1000));
        let size = [
            Steps(coord(1, 500) as u64),
            Steps(coord(1, 500) as u64),
            Steps(coord(1, 500) as u64),
        ];
        let made = make_box(&ctx(10), &BoxInput { min, size }).unwrap();
        record(&mut out, &made, &[&made.result.history]);
        let moved = move_by(
            &ctx(11),
            Moved {
                body: &made.result.body,
                exactness: made.exactness,
                accept: None,
            },
            [coord(-99, 99), coord(-99, 99), coord(-99, 99)],
        )
        .unwrap();
        record(
            &mut out,
            &moved,
            &[&made.result.history, &moved.result.history],
        );
    }
    for _ in 0..32 {
        let (x, y, z) = (coord(-1000, 1000), coord(-1000, 1000), coord(-1000, 1000));
        let (w, d, skew) = (coord(2, 400), coord(2, 400), coord(1, 300));
        let base = vec![
            p(x, y, z),
            p(x + w, y, z),
            p(x + w + skew, y + d, z),
            p(x + skew, y + d, z),
        ];
        let made = prism(
            &ctx(12),
            &PrismInput {
                base,
                height: Steps(coord(1, 300) as u64),
            },
        )
        .unwrap();
        record(&mut out, &made, &[&made.result.history]);
    }
    for _ in 0..8 {
        let made = cylinder(
            &ctx(13),
            &RoundInput {
                base: p(coord(-100, 100), coord(-100, 100), coord(-100, 100)),
                radius: Steps(coord(100, 900) as u64),
                height: Steps(coord(1, 900) as u64),
                step: GridStep::Q0,
            },
        )
        .unwrap();
        assert!(matches!(made.exactness, Exactness::Certified { .. }));
        record(&mut out, &made, &[&made.result.history]);
    }
    out
}

#[test]
fn queries_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let bytes = corpus();
        (sha256(&bytes), bytes)
    });
    println!(
        "geom-make queries corpus: {} bytes, golden hash {}",
        checked.value.len(),
        hex(sha256(&checked.value))
    );
}

#[test]
fn a_boxs_faces_keep_their_roles_through_a_move() {
    let input = BoxInput {
        min: p(0, 0, 0),
        size: [Steps(10), Steps(20), Steps(30)],
    };
    let made = make_box(&ctx(20), &input).unwrap();
    let moved = move_by(
        &ctx(21),
        Moved {
            body: &made.result.body,
            exactness: made.exactness,
            accept: None,
        },
        [5, 6, 7],
    )
    .unwrap();
    let listed = faces(
        &ctx(22),
        &moved.result.body,
        &[&made.result.history, &moved.result.history],
        moved.exactness,
    )
    .unwrap()
    .result;
    let mut roles: Vec<&str> = listed
        .iter()
        .map(|f| {
            assert_eq!(f.roles.len(), 1, "{f:?}");
            f.roles[0].name()
        })
        .collect();
    roles.sort_unstable();
    assert_eq!(
        roles,
        ["max.x", "max.y", "max.z", "min.x", "min.y", "min.z"]
    );
    // Each role sits on the face its name says: the volume and the areas follow the box.
    for f in &listed {
        let area = twice_area(&ctx(23), &moved.result.body, f.row.face, moved.exactness)
            .unwrap()
            .result;
        let want = match f.roles[0].name() {
            "min.x" => [-2 * 20 * 30, 0, 0],
            "max.x" => [2 * 20 * 30, 0, 0],
            "min.y" => [0, -2 * 10 * 30, 0],
            "max.y" => [0, 2 * 10 * 30, 0],
            "min.z" => [0, 0, -2 * 10 * 20],
            _ => [0, 0, 2 * 10 * 20],
        };
        assert_eq!(area, want, "{f:?}");
    }
    // Without the history that made them, the names have no roles.
    let bare = faces(
        &ctx(24),
        &moved.result.body,
        &[&moved.result.history],
        moved.exactness,
    )
    .unwrap()
    .result;
    assert!(bare.iter().all(|f| f.roles.is_empty()));
}
