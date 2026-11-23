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
    Answer, Cut, DisplayMesh, Edition, Engine, EngineError, GridPlane, Profile, Section, Tolerance,
    Volume,
};
use engine_geometry::value::grid::GridStep;
use std::sync::{Arc, Mutex};

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
        // Its first side along x, or along y.
        let upright = (a[1] == b[1] && b[0] == c[0] && c[1] == d[1] && d[0] == a[0])
            || (a[0] == b[0] && b[1] == c[1] && c[0] == d[0] && d[1] == a[1]);
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

    fn subtract_pieces(&mut self, _: &Cuboid, _: &Cuboid) -> Result<Vec<Cuboid>, EngineError> {
        Err(unsupported("subtract_pieces"))
    }

    fn loft(&mut self, _: &[Section]) -> Result<Cuboid, EngineError> {
        Err(unsupported("loft"))
    }

    fn unite(&mut self, _: &Cuboid, _: &Cuboid) -> Result<Cuboid, EngineError> {
        Err(unsupported("unite"))
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

    fn section(&mut self, _: &Cuboid, _: &GridPlane) -> Result<Cut, EngineError> {
        Err(unsupported("section"))
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

/// A test engine that answers as [`Cuboids`] does, and records every subtraction it is asked for
/// by the tool's lowest corner, answering it with the target unchanged: it judges what the builder
/// asks and names, not the shape a cut gives.
#[derive(Clone, Default)]
struct Recording {
    tools: Arc<Mutex<Vec<[i64; 3]>>>,
}

impl Engine for Recording {
    type Solid = Cuboid;

    fn edition(&self) -> Edition {
        Cuboids.edition()
    }

    fn extrude(&mut self, profile: &Profile, by: [i64; 3]) -> Result<Cuboid, EngineError> {
        Cuboids.extrude(profile, by)
    }

    fn subtract(&mut self, target: &Cuboid, tool: &Cuboid) -> Result<Cuboid, EngineError> {
        self.tools.lock().unwrap().push(tool.lo);
        Ok(target.clone())
    }

    fn subtract_pieces(
        &mut self,
        target: &Cuboid,
        tool: &Cuboid,
    ) -> Result<Vec<Cuboid>, EngineError> {
        Cuboids.subtract_pieces(target, tool)
    }

    fn loft(&mut self, sections: &[Section]) -> Result<Cuboid, EngineError> {
        Cuboids.loft(sections)
    }

    fn unite(&mut self, a: &Cuboid, b: &Cuboid) -> Result<Cuboid, EngineError> {
        Cuboids.unite(a, b)
    }

    fn volume(&mut self, s: &Cuboid) -> Result<Volume, EngineError> {
        Cuboids.volume(s)
    }

    fn mesh(&mut self, s: &Cuboid) -> Result<DisplayMesh, EngineError> {
        Cuboids.mesh(s)
    }

    fn section(&mut self, s: &Cuboid, plane: &GridPlane) -> Result<Cut, EngineError> {
        Cuboids.section(s, plane)
    }
}

#[test]
fn the_walls_names_survive_its_openings() {
    kernel_suite::the_walls_names_survive_its_openings(Recording::default);
}

#[test]
fn an_opening_at_the_edge_is_refused_with_its_door_and_reason() {
    kernel_suite::an_opening_at_the_edge_is_refused_with_its_door_and_reason(|| Cuboids);
}

/// Each opening is asked of the engine as one cutter, one after another in the order of the
/// doors' keys whatever order they come in: the door's width and height at its place along the
/// wall, through the wall and 100 mm past each side, and from 100 mm under the base for a door to
/// the floor; a raised opening from its own foot.
#[test]
fn the_openings_are_cut_in_the_order_of_their_keys() {
    use construct_regen::kernel::{Hosted, Kernel};
    let engine = Recording::default();
    let tools = engine.tools.clone();
    let k = Kernel::new(engine).unwrap();
    let mm = |v: i64| v * 6400;
    let at = |x: i64, z: i64| k.units().point([x, -50, z]).unwrap();
    let (door, window) = (kernel_suite::door(), kernel_suite::door_of(800, 1000, "x"));
    let wall = kernel_suite::wall(4000, "x");
    let hosted = [
        Hosted {
            key: "D-b",
            recipe: &window,
            at: at(2500, 900),
        },
        Hosted {
            key: "D-a",
            recipe: &door,
            at: at(500, 0),
        },
    ];
    let built = k.build_hosting(&wall, &hosted).unwrap();
    assert_eq!(
        *tools.lock().unwrap(),
        [[mm(500), mm(-100), mm(-100)], [mm(2500), mm(-100), mm(900)]]
    );
    let roles: Vec<&str> = built.names.iter().map(|n| n.role.as_str()).collect();
    assert_eq!(roles.len(), 6 + 3 + 4, "{roles:?}");
}

/// A wall with no doors asks the engine for no subtraction, and its body's bytes are what the
/// engine was asked for the extrusion alone.
#[test]
fn a_wall_without_doors_asks_for_no_cut() {
    use construct_model::geometry::Geometry;
    let engine = Recording::default();
    let tools = engine.tools.clone();
    let k = construct_regen::kernel::Kernel::new(engine).unwrap();
    let built = k.build(&kernel_suite::wall(4000, "x")).unwrap();
    assert!(tools.lock().unwrap().is_empty());
    let body: serde_json::Value = serde_json::from_slice(&built.body).unwrap();
    assert_eq!(
        body.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["by", "extrude"]
    );
    assert_eq!(built.displays[0].mesh, built.displays[1].mesh);
    assert_eq!(built.displays[1].deviation, 0);
}

#[test]
fn a_layered_wall_is_one_solid_per_layer() {
    kernel_suite::a_layered_wall_is_one_solid_per_layer(|| Cuboids);
}

#[test]
fn a_door_in_a_layered_wall_is_refused_as_in_the_box() {
    kernel_suite::a_door_in_a_layered_wall_is_refused_as_in_the_box(|| Cuboids);
}

/// A door in a layered wall is asked of the engine as one cutter, extruded once and subtracted
/// from each layer that has a thickness, in the layers' order: the door's width and height at its
/// place along the wall, through the wall's whole thickness and 100 mm past each side, from 100
/// mm under the base. A membrane has no solid, so nothing is cut from it.
#[test]
fn a_door_is_cut_from_each_layer_of_a_layered_wall() {
    use construct_regen::kernel::{Hosted, Kernel};
    let engine = Recording::default();
    let tools = engine.tools.clone();
    let k = Kernel::new(engine).unwrap();
    let mm = |v: i64| v * 6400;
    let door = kernel_suite::door();
    let with_membrane = [
        kernel_suite::PARTITION[0],
        ("membrane", "membrane-polyethylene", 0),
        kernel_suite::PARTITION[1],
        kernel_suite::PARTITION[2],
    ];
    let wall = kernel_suite::layered_wall([0, 0], [4000, 0], 2700, &with_membrane);
    let hosted = [Hosted {
        key: "D-1",
        recipe: &door,
        at: k.units().point([1000, -150, 0]).unwrap(),
    }];
    let built = k.build_hosting(&wall, &hosted).unwrap();
    // The exterior face is 57.5 mm to the right of the line: at y = -57.5 mm along x.
    let foot = [mm(1000), -368_000 - mm(100), mm(-100)];
    assert_eq!(*tools.lock().unwrap(), [foot, foot, foot]);
    assert_eq!(built.displays[0].parts.len(), 3);
    let roles: Vec<&str> = built.names.iter().map(|n| n.role.as_str()).collect();
    assert_eq!(roles.len(), 6 + 3 + 3, "{roles:?}");
}

#[test]
fn a_layered_wall_at_an_angle_is_its_strips_extruded() {
    kernel_suite::a_layered_wall_at_an_angle_is_its_strips_extruded(|| Prisms);
}

/// A test engine that answers only an extrusion straight up of a strictly convex polygon, which
/// it keeps as the prism it is and meshes as its two caps (fans from the first corner) and a quad
/// of two triangles per side. It is judged by the suite like any engine; it is not a geometry
/// engine.
struct Prisms;

/// A prism: its floor's corners counter-clockwise from above, its floor's height and its top's.
#[derive(Clone, Debug)]
struct Prism {
    corners: Vec<[i64; 2]>,
    z: [i64; 2],
}

impl Engine for Prisms {
    type Solid = Prism;

    fn edition(&self) -> Edition {
        Edition {
            engine: "test.prisms".to_owned(),
            ..Cuboids.edition()
        }
    }

    #[allow(
        clippy::many_single_char_names,
        reason = "a polygon's corners, its count and its height"
    )]
    fn extrude(&mut self, profile: &Profile, by: [i64; 3]) -> Result<Prism, EngineError> {
        let [0, 0, h] = by else {
            return Err(unsupported("extrude"));
        };
        let z = profile.outer.first().map_or(0, |p| p.z.get());
        let mut corners: Vec<[i64; 2]> = profile
            .outer
            .iter()
            .map(|p| [p.x.get(), p.y.get()])
            .collect();
        let n = corners.len();
        let turn = |i: usize, c: &[[i64; 2]]| {
            let (a, b, d) = (c[i], c[(i + 1) % n], c[(i + 2) % n]);
            i128::from(b[0] - a[0]) * i128::from(d[1] - b[1])
                - i128::from(b[1] - a[1]) * i128::from(d[0] - b[0])
        };
        if n < 3
            || h <= 0
            || !profile.holes.is_empty()
            || profile.outer.iter().any(|p| p.z.get() != z)
        {
            return Err(unsupported("extrude"));
        }
        if (0..n).all(|i| turn(i, &corners) < 0) {
            corners.reverse();
        }
        if !(0..n).all(|i| turn(i, &corners) > 0) {
            return Err(unsupported("extrude"));
        }
        Ok(Prism {
            corners,
            z: [z, z + h],
        })
    }

    fn subtract(&mut self, _: &Prism, _: &Prism) -> Result<Prism, EngineError> {
        Err(unsupported("subtract"))
    }

    fn subtract_pieces(&mut self, _: &Prism, _: &Prism) -> Result<Vec<Prism>, EngineError> {
        Err(unsupported("subtract_pieces"))
    }

    fn loft(&mut self, _: &[Section]) -> Result<Prism, EngineError> {
        Err(unsupported("loft"))
    }

    fn unite(&mut self, _: &Prism, _: &Prism) -> Result<Prism, EngineError> {
        Err(unsupported("unite"))
    }

    fn volume(&mut self, _: &Prism) -> Result<Volume, EngineError> {
        Err(unsupported("volume"))
    }

    #[allow(
        clippy::many_single_char_names,
        reason = "a prism, its corners' count and two corners' numbers"
    )]
    fn mesh(&mut self, s: &Prism) -> Result<DisplayMesh, EngineError> {
        let n = s.corners.len() as u32;
        let mut mesh = DisplayMesh::default();
        for z in s.z {
            for c in &s.corners {
                mesh.positions.push([c[0] as f64, c[1] as f64, z as f64]);
                mesh.normals.push([0.0, 0.0, 0.0]);
            }
        }
        for i in 1..n - 1 {
            // The floor seen from below, the top from above.
            mesh.triangles.push([0, i + 1, i]);
            mesh.triangles.push([n, n + i, n + i + 1]);
        }
        for i in 0..n {
            let j = (i + 1) % n;
            mesh.triangles.push([i, j, n + j]);
            mesh.triangles.push([i, n + j, n + i]);
        }
        Ok(mesh)
    }

    fn section(&mut self, _: &Prism, _: &GridPlane) -> Result<Cut, EngineError> {
        Err(unsupported("section"))
    }
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
        fn subtract_pieces(&mut self, _: &Cuboid, _: &Cuboid) -> Result<Vec<Cuboid>, EngineError> {
            Err(unsupported("subtract_pieces"))
        }
        fn loft(&mut self, _: &[Section]) -> Result<Cuboid, EngineError> {
            Err(unsupported("loft"))
        }
        fn unite(&mut self, _: &Cuboid, _: &Cuboid) -> Result<Cuboid, EngineError> {
            Err(unsupported("unite"))
        }
        fn volume(&mut self, _: &Cuboid) -> Result<Volume, EngineError> {
            Err(unsupported("volume"))
        }
        fn mesh(&mut self, _: &Cuboid) -> Result<DisplayMesh, EngineError> {
            Err(unsupported("mesh"))
        }
        fn section(&mut self, _: &Cuboid, _: &GridPlane) -> Result<Cut, EngineError> {
            Err(unsupported("section"))
        }
    }
    let k = construct_regen::kernel::Kernel::new(Refusing).unwrap();
    let error = k.build(&kernel_suite::block()).unwrap_err();
    assert_eq!(error.code, "EngineRefused");
    assert_eq!(error.message, "extrude: the checker refused it");
}
