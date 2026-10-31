//! Mesh booleans through the public interface (piece G148), against what 19.5 says of them: exact
//! volumes against counted cells, conforming closed meshes, the refusals and their remedies, one
//! arrangement for many results, and run-level history.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    reason = "test checks and generators; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::num::{NonZeroU64, NonZeroUsize};

use geom_mesh::boolean::{Outcome, boolean};
use geom_mesh::exact::build::{Corner, Polygons, build};
use geom_mesh::exact::mass::Volume;
use geom_mesh::exact::{ExactMesh, MeshError, MeshKind};
use geom_num::wide::I256;
use geom_op::{NameId, NameSet, OpId, OpPath, Origin};
use geom_poly::winding::{Term, WindingTest};
use geom_pool::native::Rayon;
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

mod shapes;

use shapes::{cuboid, g, soup, source};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 26))
}

fn step() -> OpPath {
    OpPath::new(OpId::from_bytes([7; 16]))
}

/// The box from `low` to `high`, of one run named by `name`.
fn solid(low: [i64; 3], high: [i64; 3], name: u8) -> ExactMesh {
    let mut input: Polygons = cuboid(high[0] - low[0], high[1] - low[1], high[2] - low[2]);
    for v in &mut input.vertices {
        let Corner::Grid(p) = *v else {
            panic!("a box is on the grid");
        };
        *v = g(p.x.get() + low[0], p.y.get() + low[1], p.z.get() + low[2]);
    }
    input.runs = vec![source(name)];
    build(&input, &Exec::serial(), &mut meter()).unwrap()
}

fn one(operands: &[&ExactMesh], test: WindingTest) -> Outcome {
    boolean(operands, &[test], &step(), &Exec::serial(), &mut meter())
        .unwrap()
        .pop()
        .unwrap()
}

fn six_volume(m: &ExactMesh) -> I256 {
    match m.volume(&mut meter()).unwrap() {
        Volume::Exact(six) => six,
        Volume::Certified(_) => panic!("every corner here is on the grid"),
    }
}

fn six(v: i128) -> I256 {
    I256::from_i128(6 * v)
}

#[test]
fn two_overlapping_boxes() {
    let a = solid([0, 0, 0], [4, 4, 4], 1);
    let b = solid([2, 2, 2], [6, 6, 6], 2);
    let united = one(&[&a, &b], WindingTest::unite());
    assert_eq!(six_volume(&united.mesh), six(120));
    assert_eq!(united.mesh.kind(), MeshKind::Closed);
    let met = one(&[&a, &b], WindingTest::intersect(2));
    assert_eq!(six_volume(&met.mesh), six(8));
    assert_eq!(met.mesh.faces(), 6);
    let less = one(&[&a, &b], WindingTest::subtract(0));
    assert_eq!(six_volume(&less.mesh), six(56));
    assert_eq!(less.mesh.kind(), MeshKind::Closed);
}

#[test]
fn boxes_on_one_face_merge_into_one_box() {
    let a = solid([0, 0, 0], [4, 4, 4], 1);
    let b = solid([4, 0, 0], [8, 4, 4], 2);
    let united = one(&[&a, &b], WindingTest::unite());
    assert_eq!(six_volume(&united.mesh), six(128));
    assert_eq!(united.mesh.faces(), 6);
    assert_eq!(united.mesh.parts().verts.len(), 8);
}

#[test]
fn a_small_box_on_a_big_one_leaves_no_t_junction() {
    let a = solid([0, 0, 0], [8, 8, 2], 1);
    let b = solid([2, 2, 2], [6, 6, 4], 2);
    let united = one(&[&a, &b], WindingTest::unite());
    assert_eq!(six_volume(&united.mesh), six(128 + 32));
    // The build's checker pairs every half-edge, so a corner in the middle of an edge is refused.
    assert_eq!(united.mesh.kind(), MeshKind::Closed);
    // The top of the big box keeps a hole where the small one stands.
    assert_eq!(united.mesh.faces(), 6 + 5);
}

#[test]
fn volumes_agree_with_counted_cells() {
    let seed = resolve("geom-mesh boolean cells", Seed(0x148)).unwrap();
    let mut rng = SplitMix64::new(seed);
    let side = NonZeroU64::new(6).unwrap();
    for _ in 0..40 {
        let mut pick = || {
            let mut low = [0; 3];
            let mut high = [0; 3];
            for k in 0..3 {
                let a = rng.below(side) as i64;
                let b = rng.below(side) as i64;
                low[k] = a.min(b);
                high[k] = a.max(b) + 1;
            }
            (low, high)
        };
        let boxes = [pick(), pick(), pick()];
        let meshes: Vec<ExactMesh> = boxes
            .iter()
            .zip(1..)
            .map(|(b, n)| solid(b.0, b.1, n))
            .collect();
        let refs: Vec<&ExactMesh> = meshes.iter().collect();
        let tests = [
            WindingTest::unite(),
            WindingTest::intersect(2),
            WindingTest::intersect(3),
            WindingTest::subtract(1),
        ];
        let got = boolean(&refs, &tests, &step(), &Exec::serial(), &mut meter()).unwrap();
        let mut cells = [0_i128; 4];
        for x in 0..8 {
            for y in 0..8 {
                for z in 0..8 {
                    let inside: Vec<bool> = boxes
                        .iter()
                        .map(|(l, h)| {
                            x >= l[0] && x < h[0] && y >= l[1] && y < h[1] && z >= l[2] && z < h[2]
                        })
                        .collect();
                    let count = inside.iter().filter(|i| **i).count();
                    cells[0] += i128::from(count >= 1);
                    cells[1] += i128::from(count >= 2);
                    cells[2] += i128::from(count >= 3);
                    cells[3] += i128::from(inside[1] && count == 1);
                }
            }
        }
        for (result, cell) in got.iter().zip(cells) {
            assert_eq!(six_volume(&result.mesh), six(cell));
        }
    }
}

#[test]
fn many_openings_are_one_call() {
    let plate = solid([0, 0, 0], [440, 440, 8], 1);
    let holes: Vec<ExactMesh> = (0..40)
        .map(|i| {
            let (x, y) = (4 + (i % 8) * 54, 4 + (i / 8) * 54);
            solid([x, y, -1], [x + 40, y + 40, 9], 2 + i as u8)
        })
        .collect();
    let mut refs = vec![&plate];
    refs.extend(holes.iter());
    let cut = one(&refs, WindingTest::subtract(0));
    assert_eq!(six_volume(&cut.mesh), six(440 * 440 * 8 - 40 * 40 * 40 * 8));
    assert_eq!(cut.mesh.kind(), MeshKind::Closed);
}

#[test]
fn several_tests_share_one_arrangement() {
    let a = solid([0, 0, 0], [4, 4, 4], 1);
    let b = solid([2, 0, 0], [6, 4, 4], 2);
    let tests = [WindingTest::unite(), WindingTest::subtract(1)];
    let both = boolean(&[&a, &b], &tests, &step(), &Exec::serial(), &mut meter()).unwrap();
    assert_eq!(both[0], one(&[&a, &b], WindingTest::unite()));
    assert_eq!(both[1], one(&[&a, &b], WindingTest::subtract(1)));
}

#[test]
fn an_open_operand_is_refused() {
    let open = build(
        &soup(vec![g(0, 0, 0), g(4, 0, 0), g(0, 4, 0)], &[&[0, 1, 2]]),
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap();
    let a = solid([0, 0, 0], [4, 4, 4], 1);
    let refused = boolean(
        &[&a, &open],
        &[WindingTest::unite()],
        &step(),
        &Exec::serial(),
        &mut meter(),
    );
    assert_eq!(refused.unwrap_err(), MeshError::NotClosed);
}

#[test]
fn a_result_with_no_outside_is_refused() {
    let a = solid([0, 0, 0], [4, 4, 4], 1);
    let not_a = WindingTest::new(vec![Term::Inside(0), Term::Not]).unwrap();
    let refused = boolean(
        &[&a],
        &[WindingTest::unite(), not_a],
        &step(),
        &Exec::serial(),
        &mut meter(),
    );
    assert_eq!(refused.unwrap_err(), MeshError::UnboundedResult { test: 1 });
}

fn origin_of(o: &Outcome, name: NameId) -> &Origin {
    o.history.origin(name).unwrap()
}

fn name_of(n: u8) -> NameId {
    NameId::from_bytes([n; 16])
}

fn inputs(names: &[u8]) -> NameSet {
    NameSet::new(names.iter().map(|n| name_of(*n))).unwrap()
}

#[test]
fn untouched_runs_are_unchanged() {
    let a = solid([0, 0, 0], [2, 2, 2], 1);
    let b = solid([5, 0, 0], [7, 2, 2], 2);
    let united = one(&[&a, &b], WindingTest::unite());
    assert_eq!(*origin_of(&united, name_of(1)), Origin::Unchanged);
    assert_eq!(*origin_of(&united, name_of(2)), Origin::Unchanged);
    assert!(united.history.deleted().names().is_empty());
    united.history.check(&inputs(&[1, 2])).unwrap();
}

#[test]
fn a_trimmed_run_is_modified_and_a_lost_one_deleted() {
    let a = solid([0, 0, 0], [4, 4, 4], 1);
    let b = solid([2, 2, 2], [6, 6, 6], 2);
    let less = one(&[&a, &b], WindingTest::subtract(0));
    assert!(matches!(
        origin_of(&less, name_of(1)),
        Origin::Modified { .. }
    ));
    // The tool's faces that stay are modified too: turned over and trimmed.
    assert!(matches!(
        origin_of(&less, name_of(2)),
        Origin::Modified { .. }
    ));
    less.history.check(&inputs(&[1, 2])).unwrap();
    let inner = solid([1, 1, 1], [3, 3, 3], 2);
    let whole = one(&[&a, &inner], WindingTest::unite());
    assert_eq!(whole.history.deleted(), &inputs(&[2]));
    whole.history.check(&inputs(&[1, 2])).unwrap();
}

#[test]
fn a_cut_through_splits_a_run_into_named_pieces() {
    let a = solid([0, 0, 0], [9, 3, 3], 1);
    let slot = solid([4, -1, -1], [5, 4, 4], 2);
    let less = one(&[&a, &slot], WindingTest::subtract(0));
    assert_eq!(six_volume(&less.mesh), six(8 * 9));
    let splits = less
        .history
        .records()
        .iter()
        .filter(|r| matches!(r.origin, Origin::Split { .. }))
        .count();
    assert_eq!(splits, 4);
    let of_box = less
        .history
        .records()
        .iter()
        .filter(|r| matches!(r.origin, Origin::Split { of, .. } if of == name_of(1)))
        .count();
    assert_eq!(of_box, 2);
    less.history.check(&inputs(&[1, 2])).unwrap();
    assert_eq!(less.mesh.parts().runs.len(), less.history.records().len());
}

#[test]
fn coplanar_runs_merge() {
    let a = solid([0, 0, 0], [4, 4, 4], 1);
    let b = solid([4, 0, 0], [8, 4, 4], 2);
    let united = one(&[&a, &b], WindingTest::unite());
    assert!(
        united
            .history
            .records()
            .iter()
            .any(|r| matches!(&r.origin, Origin::Merged { of } if *of == inputs(&[1, 2])))
    );
    united.history.check(&inputs(&[1, 2])).unwrap();
}

#[test]
fn the_threads_and_the_order_do_not_show() {
    let a = solid([0, 0, 0], [40, 40, 40], 1);
    let b = solid([20, 20, 20], [60, 60, 60], 2);
    let c = solid([10, 30, 5], [70, 35, 50], 3);
    let test = WindingTest::unite();
    let plain = one(&[&a, &b, &c], test.clone());
    let pool = Rayon::new(NonZeroUsize::new(4).unwrap()).unwrap();
    let threaded = boolean(
        &[&a, &b, &c],
        std::slice::from_ref(&test),
        &step(),
        &Exec::with_pool(&pool),
        &mut meter(),
    )
    .unwrap();
    assert_eq!(threaded[0], plain);
    let turned = one(&[&c, &a, &b], test);
    assert_eq!(turned.mesh, plain.mesh);
}

#[test]
fn the_meter_stops_a_boolean() {
    let a = solid([0, 0, 0], [4, 4, 4], 1);
    let b = solid([2, 2, 2], [6, 6, 6], 2);
    let mut short = WorkMeter::new(Budget::of_work(10));
    assert!(matches!(
        boolean(
            &[&a, &b],
            &[WindingTest::unite()],
            &step(),
            &Exec::serial(),
            &mut short
        ),
        Err(MeshError::Stopped(_) | MeshError::BudgetExceeded(_))
    ));
}
