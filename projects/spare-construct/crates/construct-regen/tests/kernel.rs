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

#[test]
fn a_datum_is_built_as_the_stand_in_builds_it() {
    kernel_suite::a_datum_is_built_as_the_stand_in_builds_it(|| Cuboids);
}

#[test]
fn a_rectangle_floor_is_its_layers() {
    kernel_suite::a_rectangle_floor_is_its_layers(|| Cuboids);
}

/// A sloped floor, or one with slab edges, on an engine that takes only level boxes is the
/// engine's refusal, never a flat floor or one without its edges: the server's engine builds
/// both (its tests/kernel.rs runs the suite's `a_sloped_floor_is_its_layers` and
/// `a_floor_with_slab_edges_is_one_solid`).
#[test]
fn a_shaped_floor_the_engine_cannot_build_is_refused() {
    use construct_model::geometry::Geometry;
    let k = construct_regen::kernel::Kernel::new(Cuboids).unwrap();
    let flat = kernel_suite::floor_of(&[&kernel_suite::RECTANGLE], &[("structure", 200)]);
    let tops = serde_json::json!([[0, -40 * 6400, -40 * 6400, 0]]);
    let sloped = kernel_suite::shaped_floor(flat.clone(), &[("tops", tops)]);
    assert!(k.build(&sloped).is_err());
    let square = [[0, 0], [4000, 0], [4000, 300], [0, 300]];
    let down = kernel_suite::slab_edge("e0", "downturn", (square, -200), (square, -600));
    let edged = kernel_suite::shaped_floor(flat, &[("edges", serde_json::json!([down]))]);
    assert!(k.build(&edged).is_err());
}

#[test]
fn an_opening_in_a_wall_is_refused_by_name() {
    kernel_suite::an_opening_in_a_wall_is_refused_by_name(|| Cuboids);
}

#[test]
fn an_opening_in_a_floor_is_refused_by_name() {
    kernel_suite::an_opening_in_a_floor_is_refused_by_name(|| Cuboids);
}

#[test]
fn a_shaped_floor_that_is_cut_is_refused_by_name() {
    kernel_suite::a_shaped_floor_that_is_cut_is_refused_by_name(|| Cuboids);
}

#[test]
fn a_room_is_its_loop_extruded() {
    kernel_suite::a_room_is_its_loop_extruded(|| Cuboids, false);
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

#[test]
fn walls_joined_at_an_l_and_a_t_fill_the_joined_shape() {
    kernel_suite::walls_joined_at_an_l_and_a_t_fill_the_joined_shape(|| Prisms);
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

/// A drawing cuts a layered wall one layer at a time: a solid for each layer with a thickness,
/// its material, and a plane for each of its six named faces, named as the wall names them (the
/// two layers that meet call their shared face alike). Every side's plane holds the two corners
/// of the edge that sweeps it and has the other two inside it, so a cut edge is found by its
/// plane; the old box and the wall at an angle are both answered.
#[test]
fn a_layered_walls_layers_are_solids_with_their_faces_named() {
    use construct_regen::kernel::Kernel;
    use std::collections::BTreeSet;
    let k = Kernel::new(Prisms).unwrap();
    let flat = kernel_suite::partition("x");
    let angled = kernel_suite::layered_wall([0, 0], [2400, 3200], 2700, &kernel_suite::PARTITION);
    for wall in [flat, angled] {
        let layers = k.layers(&wall).unwrap();
        assert_eq!(layers.len(), 3);
        let materials: Vec<&str> = layers
            .iter()
            .map(|l| l.material.as_deref().unwrap())
            .collect();
        assert_eq!(
            materials,
            [
                "plasterboard-gypsum",
                "timber-douglas-fir",
                "plasterboard-gypsum"
            ]
        );
        for (at, layer) in layers.iter().enumerate() {
            let outside = if at == 0 {
                "exterior".to_owned()
            } else {
                format!("face.{at}")
            };
            let inside = if at == 2 {
                "interior".to_owned()
            } else {
                format!("face.{}", at + 1)
            };
            let faces = &layer.solid.faces;
            let roles: BTreeSet<&str> = faces.iter().map(|f| f.name.role.as_str()).collect();
            let expected: BTreeSet<&str> = [
                "base",
                "top",
                "end0",
                "end1",
                outside.as_str(),
                inside.as_str(),
            ]
            .into();
            assert_eq!(roles, expected, "layer {at}");
            let top = faces.iter().find(|f| f.name.role == "top").unwrap();
            assert_eq!(
                (top.plane.normal, top.plane.offset),
                ([0, 0, 1], 2700 * 6400)
            );
            for face in faces.iter().filter(|f| f.plane.normal[2] == 0) {
                let [nx, ny, _] = face.plane.normal;
                let held = |c: &[i64; 2]| i128::from(c[0]) * nx + i128::from(c[1]) * ny;
                let corners = &layer.solid.solid.corners;
                let on = corners
                    .iter()
                    .filter(|c| held(c) == face.plane.offset)
                    .count();
                let outside_it = corners
                    .iter()
                    .filter(|c| held(c) > face.plane.offset)
                    .count();
                assert_eq!((on, outside_it), (2, 0), "layer {at}, {}", face.name.role);
            }
        }
    }
    // Any other recipe is its one solid, with no material.
    let boxed = k.layers(&kernel_suite::wall(3000, "x")).unwrap();
    assert_eq!(boxed.len(), 1);
    assert!(boxed[0].material.is_none());
}

/// A drawing cuts a curtain wall one part at a time: a solid for each panel with a slab and each
/// mullion, in the order the wall's display lists them, each with its part's word and material and
/// a plane for each of its six faces, named by its key, so no two faces of the wall are called
/// alike. Every side's plane holds the two corners of the edge that sweeps it, and its base and
/// top stand at the heights the part's box does. A door panel adds none.
#[test]
fn a_curtain_walls_parts_are_solids_with_their_faces_named() {
    use construct_regen::kernel::Kernel;
    use std::collections::BTreeSet;
    let k = Kernel::new(Prisms).unwrap();
    let wall = kernel_suite::curtain(
        [6000, 0],
        3000,
        &kernel_suite::curtain_spec(1500, 1000, true),
        None,
    );
    let layers = k.layers(&wall).unwrap();
    assert_eq!(layers.len(), 12 + 31);
    let built = construct_regen::geometry().build(&wall).unwrap();
    let mut names = BTreeSet::new();
    for (at, layer) in layers.iter().enumerate() {
        let part = &built.displays[0].parts[at];
        assert_eq!(layer.function, part.function, "part {at}");
        assert_eq!(layer.material.as_deref(), Some(part.material.as_str()));
        let faces = &layer.solid.faces;
        assert_eq!(faces.len(), 6, "part {at}");
        for face in faces {
            assert!(
                names.insert(face.name.name.clone()),
                "part {at}: a name twice"
            );
        }
        let corners = &layer.solid.solid.corners;
        for face in faces.iter().filter(|f| f.plane.normal[2] == 0) {
            let [nx, ny, _] = face.plane.normal;
            let held = |c: &[i64; 2]| i128::from(c[0]) * nx + i128::from(c[1]) * ny;
            let on = corners
                .iter()
                .filter(|c| held(c) == face.plane.offset)
                .count();
            let outside = corners
                .iter()
                .filter(|c| held(c) > face.plane.offset)
                .count();
            assert_eq!((on, outside), (2, 0), "part {at}, {}", face.name.role);
        }
    }
    assert_eq!(names.len(), 43 * 6);
}

/// A test engine that answers an extrusion as [`Cuboids`] does, and a loft of two sections the
/// second of which is the first moved by one vector, which it keeps as the prism it is and meshes
/// as its two ends (each cut into triangles by clipping ears, so a section need not be convex) and
/// a quad of two triangles per side, every triangle facing out. It is judged by the suite like any
/// engine; it is not a geometry engine.
struct Lofts;

/// A solid of [`Lofts`]: a cuboid, or a loft's first section and the vector to its second.
#[derive(Clone, Debug)]
enum Lofted {
    Cuboid(Cuboid),
    Prism {
        section: Vec<[i64; 3]>,
        by: [i64; 3],
    },
}

fn less(a: [i64; 3], b: [i64; 3]) -> [i128; 3] {
    [0, 1, 2].map(|i| i128::from(a[i]) - i128::from(b[i]))
}

fn cross(a: [i128; 3], b: [i128; 3]) -> [i128; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot3(a: [i128; 3], b: [i128; 3]) -> i128 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The polygon's corners, by number, turned anticlockwise seen from the side `up` points to, and
/// the polygon cut into triangles by clipping ears, each triangle facing `up`.
#[allow(
    clippy::many_single_char_names,
    reason = "a polygon's corners and an ear's three, as the method names them"
)]
fn ears(polygon: &[[i64; 3]], up: [i128; 3]) -> (Vec<usize>, Vec<[usize; 3]>) {
    let n = polygon.len();
    let mut newell = [0_i128; 3];
    for i in 0..n {
        let (p, q) = (polygon[i], polygon[(i + 1) % n]);
        newell = [0, 1, 2].map(|k| {
            let (a, b) = ((k + 1) % 3, (k + 2) % 3);
            newell[k]
                + (i128::from(p[a]) - i128::from(q[a])) * (i128::from(p[b]) + i128::from(q[b]))
        });
    }
    let mut left: Vec<usize> = (0..n).collect();
    if dot3(newell, up) < 0 {
        left.reverse();
    }
    let ring = left.clone();
    let turn = |a: usize, b: usize, c: usize| {
        dot3(
            cross(less(polygon[b], polygon[a]), less(polygon[c], polygon[b])),
            up,
        )
    };
    let mut out = Vec::new();
    while left.len() > 3 {
        let m = left.len();
        let ear = (0..m).find(|&i| {
            let (a, b, c) = (left[(i + m - 1) % m], left[i], left[(i + 1) % m]);
            turn(a, b, c) > 0
                && left.iter().all(|&p| {
                    [a, b, c].contains(&p)
                        || !(turn(a, b, p) >= 0 && turn(b, c, p) >= 0 && turn(c, a, p) >= 0)
                })
        });
        let Some(i) = ear else {
            return (ring, Vec::new());
        };
        out.push([left[(i + m - 1) % m], left[i], left[(i + 1) % m]]);
        left.remove(i);
    }
    out.push([left[0], left[1], left[2]]);
    (ring, out)
}

impl Engine for Lofts {
    type Solid = Lofted;

    fn edition(&self) -> Edition {
        Edition {
            engine: "test.lofts".to_owned(),
            ..Cuboids.edition()
        }
    }

    fn extrude(&mut self, profile: &Profile, by: [i64; 3]) -> Result<Lofted, EngineError> {
        Cuboids.extrude(profile, by).map(Lofted::Cuboid)
    }

    fn subtract(&mut self, _: &Lofted, _: &Lofted) -> Result<Lofted, EngineError> {
        Err(unsupported("subtract"))
    }

    fn subtract_pieces(&mut self, _: &Lofted, _: &Lofted) -> Result<Vec<Lofted>, EngineError> {
        Err(unsupported("subtract_pieces"))
    }

    fn loft(&mut self, sections: &[Section]) -> Result<Lofted, EngineError> {
        let point = |p: &engine_geometry::value::grid::P3| [p.x.get(), p.y.get(), p.z.get()];
        let [first, second] = sections else {
            return Err(unsupported("loft"));
        };
        let a: Vec<[i64; 3]> = first.corners.iter().map(point).collect();
        let b: Vec<[i64; 3]> = second.corners.iter().map(point).collect();
        if a.len() < 3 || a.len() != b.len() {
            return Err(unsupported("loft"));
        }
        let by = [0, 1, 2].map(|k| b[0][k] - a[0][k]);
        let moved = a
            .iter()
            .zip(&b)
            .all(|(p, q)| [0, 1, 2].map(|k| q[k] - p[k]) == by);
        if !moved || by == [0, 0, 0] {
            return Err(unsupported("loft"));
        }
        Ok(Lofted::Prism { section: a, by })
    }

    fn unite(&mut self, _: &Lofted, _: &Lofted) -> Result<Lofted, EngineError> {
        Err(unsupported("unite"))
    }

    fn volume(&mut self, _: &Lofted) -> Result<Volume, EngineError> {
        Err(unsupported("volume"))
    }

    #[allow(
        clippy::many_single_char_names,
        reason = "a prism's corners and a triangle's three"
    )]
    fn mesh(&mut self, s: &Lofted) -> Result<DisplayMesh, EngineError> {
        let (section, by) = match s {
            Lofted::Cuboid(c) => return Cuboids.mesh(c),
            Lofted::Prism { section, by } => (section, *by),
        };
        let n = section.len();
        let up = by.map(i128::from);
        let (ring, caps) = ears(section, up);
        if caps.len() + 2 != n {
            return Err(unsupported("mesh"));
        }
        let mut mesh = DisplayMesh::default();
        for shift in [[0, 0, 0], by] {
            for p in section {
                mesh.positions
                    .push([0, 1, 2].map(|k| (p[k] + shift[k]) as f64));
                mesh.normals.push([0.0, 0.0, 0.0]);
            }
        }
        let far = n as u32;
        for [i, j, k] in &caps {
            let (i, j, k) = (*i as u32, *j as u32, *k as u32);
            // The first end faces back along the loft, the second along it.
            mesh.triangles.push([i, k, j]);
            mesh.triangles.push([far + i, far + j, far + k]);
        }
        // The sides, round the section the way the ends' triangles turn.
        for at in 0..n {
            let (i, j) = (ring[at] as u32, ring[(at + 1) % n] as u32);
            mesh.triangles.push([i, j, far + j]);
            mesh.triangles.push([i, far + j, far + i]);
        }
        Ok(mesh)
    }

    fn section(&mut self, _: &Lofted, _: &GridPlane) -> Result<Cut, EngineError> {
        Err(unsupported("section"))
    }
}

#[test]
fn a_gable_roof_is_its_layers() {
    kernel_suite::a_gable_roof_is_its_layers(|| Lofts);
}

#[test]
fn a_mono_roof_is_its_layers() {
    kernel_suite::a_mono_roof_is_its_layers(|| Lofts);
}

#[test]
fn a_flat_roof_is_its_layers() {
    kernel_suite::a_flat_roof_is_its_layers(|| Lofts);
}

/// A solved roof (job 6.2) on a test engine that keeps each extruded piece: each layer's pieces
/// are extruded and united in the recipe's order, the body lists them, and the pieces hold the
/// layer's plan area (24 m2) by its upright height. An engine that cannot unite them says so.
/// A stair's run is lofted and its landing extruded on the test engine that keeps each loft.
#[test]
fn a_stair_is_its_pieces() {
    kernel_suite::a_stair_is_its_pieces(|| Lofts);
}

#[test]
fn a_solved_roof_unites_its_pieces_on_the_engine() {
    use construct_model::geometry::Geometry;
    let roof = kernel_suite::hip_roof(&[300, 250, 0]);
    let k = construct_regen::kernel::Kernel::new(Pieces).unwrap();
    let b = k.build(&roof).unwrap();
    let body: serde_json::Value = serde_json::from_slice(&b.body).unwrap();
    assert_eq!(
        body["layers"][0]["unite"],
        roof.args["layers"][0]["solid"]["unite"]
    );
    assert_eq!(
        body["layers"][0]["by"],
        serde_json::json!([0, 0, 50 * 6400])
    );
    for (at, upright) in [(0, 50_i128), (1, 250)] {
        let six = kernel_suite::pieces_six_volume(&kernel_suite::part_mesh(&b.displays[0], at));
        assert_eq!(six, 6 * 24_000_000 * upright, "layer {at}");
    }
    let l = k.build(&kernel_suite::l_roof(&[300, 250, 0])).unwrap();
    let six = kernel_suite::pieces_six_volume(&kernel_suite::part_mesh(&l.displays[0], 1));
    assert_eq!(six, 6 * 48_000_000 * 250);
    let refused = construct_regen::kernel::Kernel::new(Lofts)
        .unwrap()
        .build(&roof)
        .unwrap_err();
    assert!(refused.code.starts_with("Engine"), "{refused:?}");
}

/// A roof on an engine that does not loft says so, in the engine's words, and builds nothing.
#[test]
fn a_sloped_roof_on_an_engine_that_cannot_loft_is_refused() {
    use construct_model::geometry::Geometry;
    let k = construct_regen::kernel::Kernel::new(Cuboids).unwrap();
    let refused = k
        .build(&kernel_suite::roof_of("gable", &[206, 0]))
        .unwrap_err();
    assert_eq!(refused.code, "EngineUnsupported");
    assert!(refused.message.starts_with("loft"), "{}", refused.message);
    assert!(k.build(&kernel_suite::roof_of("flat", &[200, 0])).is_ok());
}

/// A test engine for placed families and solved roofs: it extrudes any flat polygon with holes
/// along any vector, unites solids that only share sides by keeping their pieces together,
/// keeping it as a piece, and takes an upright box from a solid whose every piece is an upright
/// box, keeping the box each piece shares with it as a piece taken away (so a wall's layers less
/// their openings are signed boxes). Its mesh draws every piece, a piece taken away turned inside
/// out, so the signed volume of a mesh is exactly the solid's. It is judged by the suite like any
/// engine; it is not a geometry engine.
struct Pieces;

/// One piece: a profile's outer loop and holes, the vector it is extruded along, and whether it
/// is added (`1`) or taken away (`-1`).
#[derive(Clone, Debug)]
struct Piece {
    outer: Vec<[i64; 3]>,
    holes: Vec<Vec<[i64; 3]>>,
    by: [i64; 3],
    sign: i8,
}

#[derive(Clone, Debug)]
struct Signed {
    pieces: Vec<Piece>,
}

fn plus(a: [i64; 3], b: [i64; 3]) -> [i64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// A loop's normal by Newell's method, unscaled.
fn newell(points: &[[i64; 3]]) -> [i128; 3] {
    let mut n = [0i128; 3];
    for (i, p) in points.iter().enumerate() {
        let q = points[(i + 1) % points.len()];
        let (p, q) = (p.map(i128::from), q.map(i128::from));
        n[0] += (p[1] - q[1]) * (p[2] + q[2]);
        n[1] += (p[2] - q[2]) * (p[0] + q[0]);
        n[2] += (p[0] - q[0]) * (p[1] + q[1]);
    }
    n
}

fn dot_steps(a: [i128; 3], b: [i64; 3]) -> i128 {
    a[0] * i128::from(b[0]) + a[1] * i128::from(b[1]) + a[2] * i128::from(b[2])
}

/// The loop turned so that its normal runs along `by` (`along`) or against it.
fn facing(points: &[[i64; 3]], by: [i64; 3], along: bool) -> Vec<[i64; 3]> {
    let mut points = points.to_vec();
    if (dot_steps(newell(&points), by) > 0) != along {
        points.reverse();
    }
    points
}

/// The upright box a piece is, if it is one: four corners whose box, swept along `by` (which runs
/// along an axis), holds exactly the piece's volume.
fn upright(piece: &Piece) -> Option<([i64; 3], [i64; 3])> {
    if piece.outer.len() != 4 || !piece.holes.is_empty() {
        return None;
    }
    if piece.by.iter().filter(|&&c| c != 0).count() != 1 {
        return None;
    }
    let mut lo = [i64::MAX; 3];
    let mut hi = [i64::MIN; 3];
    for p in &piece.outer {
        for q in [*p, plus(*p, piece.by)] {
            for a in 0..3 {
                lo[a] = lo[a].min(q[a]);
                hi[a] = hi[a].max(q[a]);
            }
        }
    }
    // Twice the profile's area times the sweep, against twice the box's volume.
    let twice = dot_steps(newell(&piece.outer), piece.by).abs();
    let size = [0, 1, 2].map(|a| i128::from(hi[a] - lo[a]));
    (twice == 2 * size[0] * size[1] * size[2]).then_some((lo, hi))
}

/// The upright box from `lo` to `hi` as a piece of `sign`.
fn box_piece(lo: [i64; 3], hi: [i64; 3], sign: i8) -> Piece {
    Piece {
        outer: vec![
            [lo[0], lo[1], lo[2]],
            [hi[0], lo[1], lo[2]],
            [hi[0], hi[1], lo[2]],
            [lo[0], hi[1], lo[2]],
        ],
        holes: Vec::new(),
        by: [0, 0, hi[2] - lo[2]],
        sign,
    }
}

/// Adds one triangle, turned over for a piece taken away.
fn face(mesh: &mut DisplayMesh, t: [[i64; 3]; 3], sign: i8) {
    let t = if sign > 0 { t } else { [t[0], t[2], t[1]] };
    let base = mesh.positions.len() as u32;
    for p in t {
        mesh.positions.push(p.map(|c| c as f64));
        mesh.normals.push([0.0, 0.0, 0.0]);
    }
    mesh.triangles.push([base, base + 1, base + 2]);
}

impl Engine for Pieces {
    type Solid = Signed;

    fn edition(&self) -> Edition {
        Edition {
            engine: "test.pieces".to_owned(),
            ..Cuboids.edition()
        }
    }

    fn extrude(&mut self, profile: &Profile, by: [i64; 3]) -> Result<Signed, EngineError> {
        let steps = |p: &engine_geometry::value::grid::P3| [p.x.get(), p.y.get(), p.z.get()];
        let outer: Vec<[i64; 3]> = profile.outer.iter().map(steps).collect();
        if outer.len() < 3 || dot_steps(newell(&outer), by) == 0 {
            return Err(EngineError::Invalid {
                operation: "extrude",
                what: "a flat loop of three or more corners, swept off its plane".to_owned(),
            });
        }
        Ok(Signed {
            pieces: vec![Piece {
                outer,
                holes: profile
                    .holes
                    .iter()
                    .map(|h| h.iter().map(steps).collect())
                    .collect(),
                by,
                sign: 1,
            }],
        })
    }

    fn subtract(&mut self, target: &Signed, tool: &Signed) -> Result<Signed, EngineError> {
        let refused = || EngineError::Unsupported {
            operation: "subtract",
            what: "anything but an upright box from upright boxes".to_owned(),
        };
        let [tool] = tool.pieces.as_slice() else {
            return Err(refused());
        };
        let (tlo, thi) = upright(tool).ok_or_else(refused)?;
        let mut pieces = target.pieces.clone();
        for piece in &target.pieces {
            let (lo, hi) = upright(piece).ok_or_else(refused)?;
            let (lo, hi) = (
                [0, 1, 2].map(|a| lo[a].max(tlo[a])),
                [0, 1, 2].map(|a| hi[a].min(thi[a])),
            );
            if (0..3).all(|a| lo[a] < hi[a]) {
                pieces.push(box_piece(lo, hi, -piece.sign));
            }
        }
        Ok(Signed { pieces })
    }

    fn subtract_pieces(&mut self, _: &Signed, _: &Signed) -> Result<Vec<Signed>, EngineError> {
        Err(unsupported("subtract_pieces"))
    }

    fn loft(&mut self, sections: &[Section]) -> Result<Signed, EngineError> {
        // A section lofted to the same section moved by one vector: that section extruded.
        let point = |p: &engine_geometry::value::grid::P3| [p.x.get(), p.y.get(), p.z.get()];
        let [first, second] = sections else {
            return Err(unsupported("loft"));
        };
        let (a, b): (Vec<[i64; 3]>, Vec<[i64; 3]>) = (
            first.corners.iter().map(point).collect(),
            second.corners.iter().map(point).collect(),
        );
        if a.len() < 3 || a.len() != b.len() {
            return Err(unsupported("loft"));
        }
        let by = [0, 1, 2].map(|k| b[0][k] - a[0][k]);
        if !a
            .iter()
            .zip(&b)
            .all(|(p, q)| [0, 1, 2].map(|k| q[k] - p[k]) == by)
        {
            return Err(unsupported("loft"));
        }
        let profile = Profile {
            outer: first.corners.clone(),
            holes: Vec::new(),
        };
        self.extrude(&profile, by)
    }

    fn unite(&mut self, a: &Signed, b: &Signed) -> Result<Signed, EngineError> {
        // Pieces that only share sides: kept side by side, as a solved roof's are.
        Ok(Signed {
            pieces: a.pieces.iter().chain(&b.pieces).cloned().collect(),
        })
    }

    fn volume(&mut self, _: &Signed) -> Result<Volume, EngineError> {
        Err(unsupported("volume"))
    }

    fn mesh(&mut self, solid: &Signed) -> Result<DisplayMesh, EngineError> {
        let mut mesh = DisplayMesh::default();
        for piece in &solid.pieces {
            let mut loops = vec![facing(&piece.outer, piece.by, true)];
            loops.extend(piece.holes.iter().map(|h| facing(h, piece.by, false)));
            for l in &loops {
                for i in 1..l.len() - 1 {
                    face(&mut mesh, [l[0], l[i + 1], l[i]], piece.sign);
                    let top = [l[0], l[i], l[i + 1]].map(|p| plus(p, piece.by));
                    face(&mut mesh, top, piece.sign);
                }
                for i in 0..l.len() {
                    let (a, b) = (l[i], l[(i + 1) % l.len()]);
                    let (a2, b2) = (plus(a, piece.by), plus(b, piece.by));
                    face(&mut mesh, [a, b, b2], piece.sign);
                    face(&mut mesh, [a, b2, a2], piece.sign);
                }
            }
        }
        Ok(mesh)
    }

    fn section(&mut self, _: &Signed, _: &GridPlane) -> Result<Cut, EngineError> {
        Err(unsupported("section"))
    }
}

#[test]
fn a_family_cuts_every_layer_and_stands_in_its_opening() {
    kernel_suite::a_family_cuts_every_layer_and_stands_in_its_opening(|| Pieces);
}

#[test]
fn the_starter_content_builds_and_hangs_without_cutting() {
    kernel_suite::the_starter_content_builds_and_hangs_without_cutting(|| Pieces);
}

#[test]
fn a_wall_moved_or_a_type_changed_moves_or_resizes_the_opening() {
    kernel_suite::a_wall_moved_or_a_type_changed_moves_or_resizes_the_opening(|| Pieces);
}

#[test]
fn a_family_that_does_not_fit_is_refused_with_a_reason() {
    kernel_suite::a_family_that_does_not_fit_is_refused_with_a_reason(|| Pieces);
}

#[test]
fn a_curtain_wall_is_its_panels_and_mullions() {
    kernel_suite::a_curtain_wall_is_its_panels_and_mullions(|| Cuboids);
}

#[test]
fn a_door_panel_is_its_familys_solids() {
    kernel_suite::a_door_panel_is_its_familys_solids(|| Pieces);
}

/// What one built shape is, for a pin: its body's content id, its bounds, its names and every
/// display.
fn shape_of(built: &construct_model::geometry::Built) -> serde_json::Value {
    serde_json::json!({
        "body": built.body_id(),
        "bounds": built.bounds,
        "names": built.names,
        "displays": built.displays,
    })
}

/// Walls along x and y host as they did before hosting at any angle came (piece A83): on each of
/// the four ways a partition runs (its faces off whole millimetres, 12.5 mm layers), a box door,
/// a wall opening, a placed door and a placed window, each on either side, are seated where they
/// were with the recipe they were handed, the wall built with each, each built alone on the
/// kernel's builder and on the stand-in, all exactly the bytes they were on main before (the pin
/// was taken there).
#[test]
fn walls_along_the_axes_host_byte_for_byte_as_before() {
    use construct_families::Turn;
    use construct_model::element::{Element, Placement};
    use construct_model::geometry::{Geometry, Hosted as HostedAt};
    use construct_params::Length;
    use construct_regen::hosting;
    use construct_regen::stand_in::Boxes;
    use std::collections::BTreeMap;
    let k = construct_regen::kernel::Kernel::new(Pieces).unwrap();
    let edition = k.edition();
    let id = |text: &str| kit_core::Id::parse(text).unwrap();
    let element = |key: &str, recipe: &construct_model::geometry::Recipe| Element {
        id: id(key),
        document: id("doc"),
        kind: "door".to_owned(),
        name: key.to_owned(),
        level: 0,
        placement: Placement {
            x: 1000,
            y: 2000,
            z: 0,
        },
        recipe: recipe.clone(),
        props: BTreeMap::new(),
        references: Vec::new(),
    };
    let mut all = Vec::new();
    for (start, end) in [
        ([0, 0], [4000, 0]),
        ([4000, 0], [0, 0]),
        ([0, 0], [0, 4000]),
        ([0, 4000], [0, 0]),
    ] {
        let wall = kernel_suite::layered_wall(start, end, 2700, &kernel_suite::PARTITION);
        let axis = if start[0] == end[0] { "y" } else { "x" };
        let host = element("W-1", &wall);
        let door = kernel_suite::recipe(
            "door.single",
            serde_json::json!({ "width": 900, "thickness": 100, "height": 2100, "axis": axis }),
        );
        let kinds = [
            (door, 0),
            (kernel_suite::plain_opening(900, 1200, axis), 900),
            (
                kernel_suite::placed(kernel_suite::DOOR_FAMILY, "d900x2100", Turn::None),
                0,
            ),
            (
                kernel_suite::placed(kernel_suite::WINDOW_FAMILY, "w1200x1200", Turn::None),
                0,
            ),
        ];
        for role in ["exterior", "interior"] {
            let face = hosting::side_by_role(&edition, &wall, role).unwrap();
            for (recipe, up) in &kinds {
                let mut hosted = element("D-1", recipe);
                hosting::write(
                    &mut hosted,
                    &host.id,
                    &face,
                    Length::from_mm(1500),
                    Length::from_mm(*up),
                );
                let how = hosting::read(&hosted).unwrap().unwrap();
                let (at, seated) =
                    hosting::seat(&edition, &host, &wall, &hosted, &hosted.recipe, &how).unwrap();
                let list = [HostedAt {
                    key: "D-1",
                    recipe: &seated,
                    at,
                }];
                construct_regen::kernel::fits(&edition, &wall, &list).unwrap();
                let walled = Geometry::build_hosting(&k, &wall, &list).unwrap();
                all.push(serde_json::json!({
                    "at": at,
                    "seated": seated,
                    "walled": shape_of(&walled),
                    "kernel": shape_of(&k.build(&seated).unwrap()),
                    "stand_in": shape_of(&Boxes.build(&seated).unwrap()),
                }));
            }
        }
    }
    let digest = kit_ops::sha256_hex(serde_json::to_string(&all).unwrap().as_bytes());
    // Taken on main at 93c7e8510, before this piece changed anything.
    assert_eq!(
        digest,
        "ca7fc24fa7e394c05886ebea744ef0833f227d4771fcc42039b4807852e2c234"
    );
}

/// A test engine that extrudes as [`Prisms`] does and answers a subtraction or a union with its
/// first solid unchanged: it judges what the builder asks of a wall at an angle and of what is
/// seated in it (each layer's cells, each element's own), not the shape the engine gives.
struct Seated;

impl Engine for Seated {
    type Solid = Prism;

    fn edition(&self) -> Edition {
        Edition {
            engine: "test.seated".to_owned(),
            ..Cuboids.edition()
        }
    }

    fn extrude(&mut self, profile: &Profile, by: [i64; 3]) -> Result<Prism, EngineError> {
        Prisms.extrude(profile, by)
    }

    fn subtract(&mut self, target: &Prism, _: &Prism) -> Result<Prism, EngineError> {
        Ok(target.clone())
    }

    fn subtract_pieces(&mut self, _: &Prism, _: &Prism) -> Result<Vec<Prism>, EngineError> {
        Err(unsupported("subtract_pieces"))
    }

    fn loft(&mut self, _: &[Section]) -> Result<Prism, EngineError> {
        Err(unsupported("loft"))
    }

    fn unite(&mut self, a: &Prism, _: &Prism) -> Result<Prism, EngineError> {
        Ok(a.clone())
    }

    fn volume(&mut self, _: &Prism) -> Result<Volume, EngineError> {
        Err(unsupported("volume"))
    }

    fn mesh(&mut self, s: &Prism) -> Result<DisplayMesh, EngineError> {
        Prisms.mesh(s)
    }

    fn section(&mut self, _: &Prism, _: &GridPlane) -> Result<Cut, EngineError> {
        Err(unsupported("section"))
    }
}

#[test]
fn elements_at_any_angle_cut_their_wall_and_stand_in_it() {
    kernel_suite::elements_at_any_angle_cut_their_wall_and_stand_in_it(|| Seated);
}
