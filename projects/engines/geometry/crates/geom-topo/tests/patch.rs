//! Bodies patched by row (piece G139a): the patched body is the one an edit publishes with the
//! same values, it shares every chunk the patch does not write, and a row that is not there is
//! refused.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests: native arithmetic, and a failure stops the test"
)]

mod common;

use common::{face, ix, many_faces, meter, plane, wall, wall_outside};
use geom_math::Box3;
use geom_num::grid::P3;
use geom_num::plane::Plane;
use geom_topo::{Arena, BodyKind, Builder, CHUNK_ROWS, Column, Patch, PointKind, TopoError};

/// How many chunks of `a` are `b`'s very chunks, not copies.
fn shared<K, V>(a: &Column<K, V>, b: &Column<K, V>) -> usize {
    a.chunks()
        .zip(b.chunks())
        .filter(|(x, y)| x.as_ptr() == y.as_ptr())
        .count()
}

fn bound(at: i64) -> Box3 {
    Box3::new(
        P3::new(at, at, at).unwrap(),
        P3::new(at + 1, at + 1, at + 1).unwrap(),
    )
    .unwrap()
}

#[test]
fn a_patch_writes_its_rows_and_shares_every_other_chunk() {
    let n = 3 * CHUNK_ROWS + 5;
    let body = many_faces(n);
    let row = CHUNK_ROWS + 7;
    let surface = plane([0.0, 0.0, 2.0], 0, 1);
    let patch = Patch {
        bounds: vec![(face(row), bound(-5))],
        surfaces: vec![(ix(1), surface.clone())],
        ..Patch::default()
    };
    let patched = body.patched(&patch).unwrap();
    let mut edit = Builder::edit(&body);
    edit.arenas.faces.bound.set(face(row), bound(-5)).unwrap();
    edit.arenas.surfaces.set(ix(1), surface).unwrap();
    let published = edit.publish(BodyKind::Sheet, ix(1), &mut meter()).unwrap();
    assert_eq!(patched, published);
    assert_eq!(
        (patched.kind(), patched.outside()),
        (body.kind(), body.outside())
    );
    assert_eq!(patched.names(), body.names());
    // The written chunk is copied; the rest of the column and every other column are shared.
    let (old, new) = (body.arenas(), patched.arenas());
    assert_eq!(new.faces.bound.chunk_count(), 4);
    assert_eq!(shared(&new.faces.bound, &old.faces.bound), 3);
    assert_eq!(shared(&new.faces.shell, &old.faces.shell), 4);
    assert_eq!(shared(&new.surfaces, &old.surfaces), 0);
    assert_ne!(old.faces.bound.get(face(row)), Some(&bound(-5)));
    assert!(!patch.is_empty() && Patch::default().is_empty());
    assert_eq!(body.patched(&Patch::default()).unwrap(), body);
}

#[test]
fn a_row_that_is_not_there_is_refused() {
    let body = wall();
    let rows = body.arenas().faces.len();
    let past = Patch {
        bounds: vec![(face(rows), bound(0))],
        ..Patch::default()
    };
    assert_eq!(
        body.patched(&past),
        Err(TopoError::NoSuchRow {
            arena: Arena::Face,
            row: rows
        })
    );
    // A point may meet planes of the pool, and none past it.
    let mut edit = Builder::edit(&body);
    for axis in 0..3 {
        let mut n = [0; 3];
        n[axis] = 1;
        edit.arenas.planes.push(Plane::new(n, 0).unwrap()).unwrap();
    }
    let body = edit
        .publish(BodyKind::Solid, wall_outside(), &mut meter())
        .unwrap();
    let planes = body.arenas().planes.len();
    let meet = |far: u32| Patch {
        points: vec![(ix(0), PointKind::Meet([ix(0), ix(1), ix(far)]))],
        ..Patch::default()
    };
    let on_planes = body.patched(&meet(2)).unwrap();
    assert_eq!(
        on_planes.arenas().points.get(ix(0)),
        Some(&PointKind::Meet([ix(0), ix(1), ix(2)]))
    );
    assert_eq!(
        body.patched(&meet(planes)),
        Err(TopoError::NoSuchRow {
            arena: Arena::Plane,
            row: planes
        })
    );
    for patch in [
        Patch {
            surfaces: vec![(ix(99), plane([0.0; 3], 0, 1))],
            ..Patch::default()
        },
        Patch {
            curves: vec![(ix(99), common::line([0, 0, 0], [1, 0, 0]))],
            ..Patch::default()
        },
        Patch {
            points: vec![(ix(99), PointKind::Grid(P3::new(0, 0, 0).unwrap()))],
            ..Patch::default()
        },
    ] {
        assert!(matches!(
            body.patched(&patch),
            Err(TopoError::NoSuchRow { row: 99, .. })
        ));
    }
}
