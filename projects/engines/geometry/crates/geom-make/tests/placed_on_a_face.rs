//! A point on a face of a placed shell (19.12, 23.9; piece G145): on an exact placement,
//! `geom_topo::placed::side_of_shell` gives the same side as the same query on the materialised
//! body, for every grid point in and round the instance's box, faces, edges and corners included,
//! and for all 48 signed permutations.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    reason = "test code: a failure should stop loudly"
)]

use geom_make::placed::{Library, Shared, materialise};
use geom_make::{BoxInput, Made, Moved, PrismInput, make_box, prism};
use geom_math::{Axis, GridTransform, TypedTransform};
use geom_num::{P3, Steps};
use geom_op::{OpContext, OpId, OpOutput, OpPath};
use geom_run::{Budget, WorkMeter};
use geom_topo::Ix;
use geom_topo::placed::{self, Definition, Placed};
use geom_topo::query;

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([12; 16])), Budget::UNLIMITED)
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn a_box() -> OpOutput<Made> {
    let input = BoxInput {
        min: p(2, -3, 5),
        size: [Steps(3), Steps(4), Steps(5)],
    };
    make_box(&ctx(), &input).unwrap()
}

/// A prism whose two slanted sides hold grid points away from its corners.
fn a_prism() -> OpOutput<Made> {
    let input = PrismInput {
        base: vec![p(0, 0, 1), p(6, 0, 1), p(8, 4, 1), p(2, 4, 1)],
        height: Steps(3),
    };
    prism(&ctx(), &input).unwrap()
}

/// All 48 signed permutations, each with a shift.
fn permutations() -> Vec<GridTransform> {
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
            out.push(GridTransform::from_parts(source, negate, [7, -11, 13]).unwrap());
        }
    }
    out
}

#[test]
fn a_point_on_a_face_has_the_side_it_has_on_the_materialised_body() {
    let s = Ix::new(0).unwrap();
    let table: [TypedTransform; 0] = [];
    for (name, made) in [("box", a_box()), ("prism", a_prism())] {
        let shared = &made.result.body;
        let bound = query::bounding_box(shared, &mut meter())
            .unwrap()
            .unwrap()
            .0;
        let key = Shared {
            key: 1_u8,
            body: shared,
            exactness: made.exactness,
            accept: None,
        };
        for (n, t) in permutations().iter().enumerate() {
            let instance = Placed::exact(Definition(1), t, &bound).unwrap();
            let moved = Moved {
                body: shared,
                exactness: made.exactness,
                accept: None,
            };
            let body = materialise(&ctx(), moved, &TypedTransform::Exact(*t)).unwrap();
            let body = &body.result.body;
            let mut library = Library::new(&table);
            let b = instance.bound();
            let (lo, hi) = (b.min(), b.max());
            for x in lo.x.get() - 1..=hi.x.get() + 1 {
                for y in lo.y.get() - 1..=hi.y.get() + 1 {
                    for z in lo.z.get() - 1..=hi.z.get() + 1 {
                        let q = p(x, y, z);
                        let want = query::side_of_shell(body, s, q, &mut meter()).unwrap();
                        assert_eq!(
                            placed::side_of_shell(shared, &instance, s, q, &mut meter()).unwrap(),
                            want,
                            "{name} {n}: {q:?}"
                        );
                        assert_eq!(
                            library
                                .side_of_shell(&ctx(), &key, &instance, s, q)
                                .unwrap(),
                            want,
                            "{name} {n}: {q:?}"
                        );
                    }
                }
            }
            assert!(library.is_empty());
        }
    }
}
