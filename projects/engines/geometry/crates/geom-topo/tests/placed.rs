//! The instance record (19.12, 23.9; piece G145): at most 88 bytes, every placement packed and
//! unpacked exactly, its box held once, and queries that answer in the shared frame. The queries
//! are held to their materialisation in `geom-make`'s tests.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "test code: a failure should stop loudly"
)]

mod common;

use common::{WALL, ix, meter, wall};
use geom_math::{Axis, Box3, GridTransform, Rotation, TypedTransform};
use geom_num::wide::I256;
use geom_num::{NumError, P3};
use geom_run::{Budget, LogicalSize, WorkMeter};
use geom_topo::Sense;
use geom_topo::placed::{
    Definition, Placed, PlacedError, Placement, face_area, faces, from_shared, side_of_shell,
    six_volume, to_shared,
};
use geom_topo::query::{self, QueryError, Side};

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn shared_box() -> Box3 {
    Box3::new(p(0, 0, 0), p(WALL[0], WALL[1], WALL[2])).unwrap()
}

fn all() -> Vec<GridTransform> {
    let mut out = Vec::new();
    for source in [
        [Axis::X, Axis::Y, Axis::Z],
        [Axis::X, Axis::Z, Axis::Y],
        [Axis::Y, Axis::X, Axis::Z],
        [Axis::Y, Axis::Z, Axis::X],
        [Axis::Z, Axis::X, Axis::Y],
        [Axis::Z, Axis::Y, Axis::X],
    ] {
        for bits in 0..8 {
            let negate = [bits & 1 != 0, bits & 2 != 0, bits & 4 != 0];
            out.push(GridTransform::from_parts(source, negate, [1, -2, 3]).unwrap());
        }
    }
    out
}

#[test]
fn the_record_is_at_most_88_bytes() {
    assert!(size_of::<Placed>() <= 88, "{}", size_of::<Placed>());
    assert_eq!(<Placed as LogicalSize>::LOGICAL_BYTES, 88);
}

#[test]
fn every_permutation_packs_and_unpacks_to_itself() {
    let transforms = all();
    assert_eq!(transforms.len(), 48);
    let mut packed: Vec<u8> = Vec::new();
    for t in &transforms {
        let placement = Placement::exact(t);
        assert_eq!(placement.grid().unwrap(), *t);
        let Placement::Exact { shift, axes } = placement else {
            panic!("exact")
        };
        assert_eq!(shift, [1, -2, 3]);
        packed.push(axes);
    }
    packed.sort_unstable();
    assert_eq!(packed, (0..48).collect::<Vec<u8>>());
    let past = Placement::Exact {
        shift: [0; 3],
        axes: 48,
    };
    assert_eq!(past.grid(), Err(PlacedError::Permutation(48)));
    assert_eq!(Placement::General(3).grid(), Err(PlacedError::General));
}

#[test]
fn an_exact_box_is_the_shared_box_moved() {
    for t in all() {
        let placed = Placed::exact(Definition(7), &t, &shared_box()).unwrap();
        let want = Box3::from_points([
            t.apply(shared_box().min()).unwrap(),
            t.apply(shared_box().max()).unwrap(),
        ])
        .unwrap();
        assert_eq!(*placed.bound(), want);
        assert_eq!(placed.definition(), Definition(7));
        assert_eq!(placed.placement(), Placement::exact(&t));
    }
    let far = GridTransform::translation([(1 << 40) - 1, 0, 0]).unwrap();
    assert!(matches!(
        Placed::exact(Definition(0), &far, &shared_box()),
        Err(PlacedError::Num(NumError::OutOfRange { .. }))
    ));
}

#[test]
fn a_general_box_holds_the_exact_image_one_step_out() {
    let t = TypedTransform::rotation(Axis::Z, Rotation::direction(3, 4).unwrap()).unwrap();
    let placed = Placed::general(Definition(1), 2, &t, &shared_box()).unwrap();
    assert_eq!(placed.placement(), Placement::General(2));
    // Each corner's rounded image, one step out each way, is inside.
    for corner in [
        p(0, 0, 0),
        p(4000, 200, 2700),
        p(4000, 0, 0),
        p(0, 200, 2700),
    ] {
        let image = t.apply(corner).unwrap().value();
        for d in [-1, 1] {
            assert!(placed.bound().contains(p(
                image.x.get() + d,
                image.y.get() + d,
                image.z.get() + d
            )));
        }
    }
    assert_eq!(placed.bound().min().z.get(), -1);
    assert_eq!(placed.bound().max().z.get(), 2701);
}

#[test]
fn a_general_placement_must_keep_lengths() {
    let scale = TypedTransform::from(
        geom_math::RationalTransform::uniform_scale(geom_math::Scale::new(2, 1).unwrap()).unwrap(),
    );
    assert_eq!(
        Placed::general(Definition(0), 0, &scale, &shared_box()),
        Err(PlacedError::NotRigid)
    );
}

#[test]
fn a_box_past_the_grid_is_refused() {
    let m = (1_i64 << 40) - 1;
    let edge = Box3::new(p(m - 1, 0, 0), p(m, 1, 1)).unwrap();
    let id = TypedTransform::Exact(GridTransform::translation([0, 0, 0]).unwrap());
    assert_eq!(
        Placed::general(Definition(0), 0, &id, &edge),
        Err(PlacedError::Num(NumError::OutOfRange {
            value: i128::from(m + 1)
        }))
    );
}

#[test]
fn queries_answer_in_the_shared_frame() {
    let w = wall();
    let t = GridTransform::translation([10, 20, 30]).unwrap();
    let placed = Placed::exact(Definition(0), &t, &shared_box()).unwrap();
    let [x, y, z] = WALL.map(i128::from);
    assert_eq!(
        six_volume(&w, &placed, &mut meter()).unwrap(),
        I256::from_i128(6 * x * y * z)
    );
    assert_eq!(
        faces(&w, &placed, &mut meter()).unwrap(),
        query::faces(&w, &mut meter()).unwrap()
    );
    assert_eq!(
        face_area(&w, &placed, ix(0), &mut meter()).unwrap(),
        query::face_area(&w, ix(0), &mut meter()).unwrap()
    );
    assert_eq!(
        side_of_shell(&w, &placed, ix(0), p(15, 25, 35), &mut meter()).unwrap(),
        Some(Side::Inside)
    );
    assert_eq!(
        side_of_shell(&w, &placed, ix(0), p(5, 25, 35), &mut meter()).unwrap(),
        Some(Side::Outside)
    );
    assert_eq!(to_shared(&placed, p(10, 20, 30)).unwrap(), p(0, 0, 0));
    assert_eq!(from_shared(&placed, p(0, 0, 0)).unwrap(), p(10, 20, 30));
}

#[test]
fn a_mirror_turns_senses_and_a_turn_does_not() {
    let w = wall();
    let plain = query::faces(&w, &mut meter()).unwrap();
    let mirror = GridTransform::mirror(Axis::X);
    let placed = Placed::exact(Definition(0), &mirror, &shared_box()).unwrap();
    let turned = faces(&w, &placed, &mut meter()).unwrap();
    for (a, b) in plain.iter().zip(&turned) {
        assert_ne!(a.sense, b.sense);
        assert_eq!(a.face, b.face);
    }
    let quarter = GridTransform::quarter_turns(Axis::Z, 1);
    let placed = Placed::exact(Definition(0), &quarter, &shared_box()).unwrap();
    assert_eq!(faces(&w, &placed, &mut meter()).unwrap(), plain);
    assert!(matches!(plain[0].sense, Sense::Forward | Sense::Reversed));
}

#[test]
fn an_area_turns_with_its_body_and_ignores_the_shift() {
    let w = wall();
    let area = query::face_area(&w, ix(0), &mut meter()).unwrap();
    let quarter = GridTransform::quarter_turns(Axis::Z, 1);
    let placed = Placed::exact(Definition(0), &quarter, &shared_box()).unwrap();
    assert_eq!(
        face_area(&w, &placed, ix(0), &mut meter()).unwrap(),
        [-area[1], area[0], area[2]]
    );
    let mirror = GridTransform::mirror(Axis::Y);
    let placed = Placed::exact(Definition(0), &mirror, &shared_box()).unwrap();
    assert_eq!(
        face_area(&w, &placed, ix(0), &mut meter()).unwrap(),
        [area[0], -area[1], area[2]]
    );
}

#[test]
fn a_general_placement_answers_nothing_here() {
    let w = wall();
    let t = TypedTransform::rotation(Axis::Z, Rotation::direction(3, 4).unwrap()).unwrap();
    let placed = Placed::general(Definition(0), 0, &t, &shared_box()).unwrap();
    assert_eq!(to_shared(&placed, p(0, 0, 0)), Err(PlacedError::General));
    assert_eq!(from_shared(&placed, p(0, 0, 0)), Err(PlacedError::General));
    assert_eq!(
        six_volume(&w, &placed, &mut meter()),
        Err(PlacedError::General)
    );
    assert_eq!(
        face_area(&w, &placed, ix(0), &mut meter()),
        Err(PlacedError::General)
    );
    assert_eq!(
        faces(&w, &placed, &mut meter()).map(|_| ()),
        Err(PlacedError::General)
    );
    assert_eq!(
        side_of_shell(&w, &placed, ix(0), p(0, 0, 0), &mut meter()),
        Err(PlacedError::General)
    );
}

#[test]
fn a_shared_querys_failures_come_through() {
    let w = wall();
    let t = GridTransform::translation([0, 0, 0]).unwrap();
    let placed = Placed::exact(Definition(0), &t, &shared_box()).unwrap();
    assert_eq!(
        face_area(&w, &placed, ix(99), &mut meter()),
        Err(PlacedError::Query(QueryError::NoSuchRow))
    );
    let mut tight = WorkMeter::new(Budget::of_work(1));
    assert!(matches!(
        six_volume(&w, &placed, &mut tight),
        Err(PlacedError::Query(QueryError::Stopped(_)))
    ));
    for e in [
        PlacedError::General,
        PlacedError::NotRigid,
        PlacedError::Permutation(50),
        PlacedError::Num(NumError::BudgetExceeded),
        PlacedError::Query(QueryError::Ring),
    ] {
        assert_ne!(e.to_string(), "");
    }
}
