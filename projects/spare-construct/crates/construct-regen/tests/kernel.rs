//! The kernel builder's suite (`kernel_suite`) on a test engine of this file's own, so the builder's
//! side of the join is checked wherever this crate's tests run: what it asks the engine, how it
//! turns the answer into Construct's shapes, its names, its units and its edition. The server's
//! `tests/kernel.rs` runs the same suite on the engine the server links.

// The crate denies native arithmetic, lossy casts and unchecked indexing outside tests
// (Cargo.toml); a test's helpers may use them, and a broken helper must stop the test loudly.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests; the rules are for the crate's own code"
)]

mod kernel_suite;

use engine_geometry::value::engine::{
    Answer, DisplayMesh, Edition, Engine, EngineError, Profile, Tolerance, Volume,
};
use engine_geometry::value::grid::GridStep;

/// A test engine: it answers only an extrusion of an upright rectangle, level and swept straight
/// up, which it keeps as the box it is, and meshes as six faces of two triangles each. It is
/// judged by the suite like any engine; it is not a geometry engine.
struct Cuboids;

/// A cuboid's lowest and highest corners, in grid steps.
#[derive(Clone, Debug)]
struct Cuboid {
    lo: [i64; 3],
    hi: [i64; 3],
}

fn unsupported(operation: &'static str) -> EngineError {
    EngineError::Unsupported {
        operation,
        what: "anything but an upright rectangle swept straight up".to_owned(),
    }
}

impl Engine for Cuboids {
    type Solid = Cuboid;

    fn edition(&self) -> Edition {
        Edition {
            engine: "test.cuboids".to_owned(),
            version: "this test file".to_owned(),
            step: GridStep::Q0,
            tolerances: vec![Tolerance {
                name: "none".to_owned(),
                value: 0.0,
                unit: "m".to_owned(),
            }],
            answers: vec![Answer {
                operation: "extrude".to_owned(),
                case: "an upright rectangle swept straight up".to_owned(),
                by: "the test engine".to_owned(),
            }],
        }
    }

    #[allow(
        clippy::many_single_char_names,
        reason = "a rectangle's four corners and its height"
    )]
    fn extrude(&mut self, profile: &Profile, by: [i64; 3]) -> Result<Cuboid, EngineError> {
        let [0, 0, h] = by else {
            return Err(unsupported("extrude"));
        };
        if h <= 0 {
            return Err(EngineError::Invalid {
                operation: "extrude",
                what: "a height of zero or less".to_owned(),
            });
        }
        let corners: Vec<[i64; 3]> = profile
            .outer
            .iter()
            .map(|p| [p.x.get(), p.y.get(), p.z.get()])
            .collect();
        let [a, b, c, d] = corners[..] else {
            return Err(unsupported("extrude"));
        };
        let upright = a[1] == b[1] && b[0] == c[0] && c[1] == d[1] && d[0] == a[0];
        let level = [b, c, d].iter().all(|p| p[2] == a[2]);
        if !profile.holes.is_empty() || !upright || !level || a[0] == c[0] || a[1] == c[1] {
            return Err(unsupported("extrude"));
        }
        Ok(Cuboid {
            lo: [a[0].min(c[0]), a[1].min(c[1]), a[2]],
            hi: [a[0].max(c[0]), a[1].max(c[1]), a[2] + h],
        })
    }

    fn subtract(&mut self, _: &Cuboid, _: &Cuboid) -> Result<Cuboid, EngineError> {
        Err(unsupported("subtract"))
    }

    fn volume(&mut self, _: &Cuboid) -> Result<Volume, EngineError> {
        Err(unsupported("volume"))
    }

    fn mesh(&mut self, s: &Cuboid) -> Result<DisplayMesh, EngineError> {
        let ([x0, y0, z0], [x1, y1, z1]) = (s.lo, s.hi);
        // Each face's corners counter-clockwise seen from outside, and its outward normal.
        let faces = [
            (
                [[x0, y0, z0], [x0, y1, z0], [x1, y1, z0], [x1, y0, z0]],
                [0.0, 0.0, -1.0],
            ),
            (
                [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
                [0.0, 0.0, 1.0],
            ),
            (
                [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]],
                [0.0, -1.0, 0.0],
            ),
            (
                [[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]],
                [0.0, 1.0, 0.0],
            ),
            (
                [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]],
                [-1.0, 0.0, 0.0],
            ),
            (
                [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]],
                [1.0, 0.0, 0.0],
            ),
        ];
        let mut mesh = DisplayMesh::default();
        for (corners, normal) in faces {
            let base = mesh.positions.len() as u32;
            for p in corners {
                mesh.positions.push(p.map(|c| c as f64));
                mesh.normals.push(normal);
            }
            mesh.triangles.push([base, base + 1, base + 2]);
            mesh.triangles.push([base, base + 2, base + 3]);
        }
        Ok(mesh)
    }
}

#[test]
fn a_straight_wall_is_its_plan_extruded() {
    kernel_suite::a_straight_wall_is_its_plan_extruded(|| Cuboids);
}

#[test]
fn the_wall_has_its_boxs_volume_and_a_closed_mesh() {
    kernel_suite::the_wall_has_its_boxs_volume_and_a_closed_mesh(|| Cuboids);
}

#[test]
fn every_face_carries_a_persistent_role_name() {
    kernel_suite::every_face_carries_a_persistent_role_name(|| Cuboids);
}

#[test]
fn the_same_model_built_twice_gives_the_same_bytes() {
    kernel_suite::the_same_model_built_twice_gives_the_same_bytes(|| Cuboids);
}

#[test]
fn the_edition_carries_the_engines_report() {
    kernel_suite::the_edition_carries_the_engines_report(|| Cuboids);
}

#[test]
fn a_recipe_that_cant_build_says_why() {
    kernel_suite::a_recipe_that_cant_build_says_why(|| Cuboids);
}

/// An engine's refusal comes back as a shape that can't build, with the engine's words kept.
#[test]
fn an_engines_refusal_is_a_recipe_that_cant_build() {
    use construct_model::geometry::Geometry;
    struct Refusing;
    impl Engine for Refusing {
        type Solid = Cuboid;
        fn edition(&self) -> Edition {
            Cuboids.edition()
        }
        fn extrude(&mut self, _: &Profile, _: [i64; 3]) -> Result<Cuboid, EngineError> {
            Err(EngineError::Refused {
                operation: "extrude",
                reason: "the checker refused it".to_owned(),
            })
        }
        fn subtract(&mut self, _: &Cuboid, _: &Cuboid) -> Result<Cuboid, EngineError> {
            Err(unsupported("subtract"))
        }
        fn volume(&mut self, _: &Cuboid) -> Result<Volume, EngineError> {
            Err(unsupported("volume"))
        }
        fn mesh(&mut self, _: &Cuboid) -> Result<DisplayMesh, EngineError> {
            Err(unsupported("mesh"))
        }
    }
    let k = construct_regen::kernel::Kernel::new(Refusing).unwrap();
    let error = k.build(&kernel_suite::block()).unwrap_err();
    assert_eq!(error.code, "EngineRefused");
    assert_eq!(error.message, "extrude: the checker refused it");
}
