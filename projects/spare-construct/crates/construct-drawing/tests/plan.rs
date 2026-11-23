//! The plan of two walls meeting at a corner, made by the kernel builder over a test engine of this
//! file's own: its segments exactly, its regions, its bytes twice, its stored form, its printed
//! widths with the default styles, and its SVG at 1:100, pinned.

// The crate denies native arithmetic, lossy casts and unchecked indexing outside tests
// (Cargo.toml); a test's helpers may use them, and a broken helper must stop the test loudly.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::many_single_char_names,
    reason = "tests; the rules are for the crate's own code"
)]

use construct_categories::{CategoryTable, WALLS};
use construct_drawing::drawing::{
    Drawing, Entity, Line, LineKind, Region, Shape, Source, ViewPoint,
};
use construct_drawing::pens::{Styles, widths};
use construct_drawing::plan::{PlanElement, plan};
use construct_drawing::svg::{PER_MM, to_svg};
use construct_drawing::view::View;
use construct_materials::MaterialId;
use construct_model::element::Placement;
use construct_model::geometry::Recipe;
use construct_params::Length;
use construct_regen::kernel::{Kernel, KernelSolid};
use construct_styles::{LineStyles, ObjectStyles, Scale};
use engine_geometry::value::engine::{
    Answer, Cut, CutEdge, CutLoop, CutPoint, DisplayMesh, Edition, Engine, EngineError, GridPlane,
    Profile, Section, Tolerance, Volume,
};
use engine_geometry::value::grid::GridStep;
use kit_core::Id;
use serde_json::json;

/// A test engine: it makes an upright rectangle swept straight up, keeps it as the box it is, and
/// cuts a box by a plane square to a grid axis. Nothing else. It is not a geometry engine; the
/// engines of the seam are judged by geom-solidcore's suite.
struct Boxes;

/// A box's lowest and highest corners, in grid steps.
#[derive(Clone, Debug)]
struct Cuboid {
    lo: [i64; 3],
    hi: [i64; 3],
}

fn unsupported(operation: &'static str) -> EngineError {
    EngineError::Unsupported {
        operation,
        what: "the test engine makes and cuts boxes only".to_owned(),
    }
}

fn unit(axis: usize, sign: i128) -> [i128; 3] {
    let mut v = [0; 3];
    v[axis] = sign;
    v
}

fn dot(a: [i128; 3], b: [i128; 3]) -> i128 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

impl Engine for Boxes {
    type Solid = Cuboid;

    fn edition(&self) -> Edition {
        Edition {
            engine: "test.boxes".to_owned(),
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

    fn extrude(&mut self, profile: &Profile, by: [i64; 3]) -> Result<Cuboid, EngineError> {
        let [0, 0, h] = by else {
            return Err(unsupported("extrude"));
        };
        let xs: Vec<i64> = profile.outer.iter().map(|p| p.x.get()).collect();
        let ys: Vec<i64> = profile.outer.iter().map(|p| p.y.get()).collect();
        let z = profile.outer[0].z.get();
        Ok(Cuboid {
            lo: [*xs.iter().min().unwrap(), *ys.iter().min().unwrap(), z],
            hi: [*xs.iter().max().unwrap(), *ys.iter().max().unwrap(), z + h],
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

    /// The rectangle where a plane square to an axis crosses the box strictly inside, as the
    /// seam's contract gives it: counter-clockwise seen from the plane's normal, from its least
    /// corner, each edge on its side's plane, the normal out of the box.
    fn section(&mut self, s: &Cuboid, plane: &GridPlane) -> Result<Cut, EngineError> {
        let axis = (0..3)
            .find(|&k| plane.normal == unit(k, 1) || plane.normal == unit(k, -1))
            .ok_or_else(|| unsupported("section"))?;
        let sign = plane.normal[axis];
        let at = plane.offset * sign;
        let (lo, hi) = (s.lo.map(i128::from), s.hi.map(i128::from));
        if at <= lo[axis] || at >= hi[axis] {
            return Ok(Cut {
                loops: vec![],
                exactness: engine_geometry::value::Exactness::Exact,
            });
        }
        let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
        let (u, v) = if sign > 0 {
            (unit(a, 1), unit(b, 1))
        } else {
            (unit(b, 1), unit(a, 1))
        };
        let span = |w: [i128; 3]| {
            let (p, q) = (dot(w, lo), dot(w, hi));
            (p.min(q), p.max(q))
        };
        let ((u0, u1), (v0, v1)) = (span(u), span(v));
        let point = |pu: i128, pv: i128| {
            let mut num = [0; 3];
            for k in 0..3 {
                num[k] = pu * u[k] + pv * v[k] + at * unit(axis, 1)[k];
            }
            CutPoint { num, den: 1 }
        };
        let neg = |w: [i128; 3]| w.map(|c| -c);
        let corners = [point(u0, v0), point(u1, v0), point(u1, v1), point(u0, v1)];
        let faces = [
            GridPlane {
                normal: neg(v),
                offset: -v0,
            },
            GridPlane {
                normal: u,
                offset: u1,
            },
            GridPlane {
                normal: v,
                offset: v1,
            },
            GridPlane {
                normal: neg(u),
                offset: -u0,
            },
        ];
        let mut edges: Vec<CutEdge> = (0..4)
            .map(|i| CutEdge {
                start: corners[i],
                end: corners[(i + 1) % 4],
                face: faces[i],
            })
            .collect();
        let least = (0..4).min_by_key(|&i| edges[i].start.num).unwrap();
        edges.rotate_left(least);
        Ok(Cut {
            loops: vec![CutLoop { edges }],
            exactness: engine_geometry::value::Exactness::Exact,
        })
    }
}

fn recipe(op: &str, args: serde_json::Value) -> Recipe {
    Recipe::construct(op, serde_json::from_value(args).unwrap())
}

/// The two walls: one 4000 mm along x from the origin, and one 3000 mm along y standing against
/// its interior face at its far end, at (3800, 200).
fn walls() -> [(Id, Recipe, Placement); 2] {
    [
        (
            Id::parse("wall-a").unwrap(),
            recipe(
                "wall.straight",
                json!({ "length": 4000, "thickness": 200, "height": 2700 }),
            ),
            Placement { x: 0, y: 0, z: 0 },
        ),
        (
            Id::parse("wall-b").unwrap(),
            recipe(
                "wall.straight",
                json!({ "length": 3000, "thickness": 200, "height": 2700, "axis": "y" }),
            ),
            Placement {
                x: 3800,
                y: 200,
                z: 0,
            },
        ),
    ]
}

/// The plan of the two walls cut 1200 mm up, and each wall's named faces.
fn two_walls() -> (Drawing, Vec<KernelSolid<Cuboid>>) {
    let kernel = Kernel::new(Boxes).unwrap();
    let walls = walls();
    let shapes: Vec<KernelSolid<Cuboid>> = walls
        .iter()
        .map(|(_, r, _)| kernel.solid(r).unwrap())
        .collect();
    let concrete = MaterialId::parse("concrete-regular").unwrap();
    let elements: Vec<PlanElement<'_, Cuboid>> = walls
        .iter()
        .zip(&shapes)
        .map(|((id, _, placement), shape)| PlanElement {
            id: id.clone(),
            category: WALLS,
            material: Some(concrete.clone()),
            placement: *placement,
            shape,
        })
        .collect();
    let drawing = plan(&kernel, View::plan(Length::from_mm(1200)), &elements).unwrap();
    (drawing, shapes)
}

fn p(x: i128, y: i128) -> ViewPoint {
    ViewPoint::grid(x * PER_MM, y * PER_MM)
}

#[test]
fn a_millimetre_is_the_drawings_steps() {
    assert_eq!(PER_MM, i128::from(Length::STEPS_PER_MM));
}

/// Two walls meeting at a corner give each wall's rectangle as cut lines, each on its named face,
/// exactly, and one region per wall with its material.
#[test]
fn two_walls_at_a_corner_give_their_cut_outlines_exactly() {
    let (drawing, shapes) = two_walls();
    let ids = walls().map(|(id, _, _)| id);
    let named = |wall: usize, role: &str| {
        let face = shapes[wall]
            .faces
            .iter()
            .find(|f| f.name.role == role)
            .unwrap();
        Source {
            element: ids[wall].clone(),
            entity: Entity::Face,
            name: face.name.name.clone(),
            role: role.to_owned(),
        }
    };
    let cut = |wall: usize, role: &str, a: ViewPoint, b: ViewPoint| Line {
        kind: LineKind::Cut,
        source: named(wall, role),
        category: WALLS,
        shape: Shape::Segment { start: a, end: b },
    };
    let concrete = MaterialId::parse("concrete-regular").unwrap();
    let expected = Drawing::new(
        View::plan(Length::from_mm(1200)),
        vec![
            cut(0, "exterior", p(0, 0), p(4000, 0)),
            cut(0, "end1", p(4000, 0), p(4000, 200)),
            cut(0, "interior", p(4000, 200), p(0, 200)),
            cut(0, "end0", p(0, 200), p(0, 0)),
            cut(1, "end0", p(3800, 200), p(4000, 200)),
            cut(1, "exterior", p(4000, 200), p(4000, 3200)),
            cut(1, "end1", p(4000, 3200), p(3800, 3200)),
            cut(1, "interior", p(3800, 3200), p(3800, 200)),
        ],
        vec![
            Region {
                element: ids[0].clone(),
                category: WALLS,
                material: Some(concrete.clone()),
                loops: vec![vec![p(0, 0), p(4000, 0), p(4000, 200), p(0, 200)]],
            },
            Region {
                element: ids[1].clone(),
                category: WALLS,
                material: Some(concrete),
                loops: vec![vec![
                    p(3800, 200),
                    p(4000, 200),
                    p(4000, 3200),
                    p(3800, 3200),
                ]],
            },
        ],
    )
    .unwrap();
    assert_eq!(drawing, expected);
    assert_eq!(drawing.lines().len(), 8);
}

/// The same model and view give the same bytes, and the bytes read back to the same drawing.
#[test]
fn the_same_plan_twice_gives_the_same_bytes() {
    let (a, _) = two_walls();
    let (b, _) = two_walls();
    let bytes = a.to_bytes().unwrap();
    assert_eq!(bytes, b.to_bytes().unwrap());
    assert_eq!(Drawing::from_bytes(&bytes).unwrap(), a);
}

/// Where a pinned file differs, the bytes found are written beside the test's target so they can
/// be read, and the test fails.
fn pinned(name: &str, found: &[u8], expected: &[u8]) {
    if found != expected {
        let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
        std::fs::write(&path, found).unwrap();
        panic!(
            "{name} differs from the pinned file; found written to {}",
            path.display()
        );
    }
}

/// The plan's stored form, pinned.
#[test]
fn the_two_wall_plan_is_stored_as_pinned() {
    let (drawing, _) = two_walls();
    pinned(
        "two_walls_plan.json",
        &drawing.to_bytes().unwrap(),
        include_bytes!("two_walls_plan.json"),
    );
}

fn styles<'a>(
    categories: &'a CategoryTable,
    lines: &'a LineStyles,
    objects: &'a ObjectStyles,
) -> Styles<'a> {
    Styles {
        categories,
        lines,
        objects,
    }
}

/// With the default styles, every cut wall line prints 0.5 mm at 1:100 and 0.7 mm at 1:20, and
/// the same wall seen in projection 0.25 mm at 1:100.
#[test]
fn the_cut_walls_print_with_the_default_pens() {
    let (drawing, _) = two_walls();
    let (categories, lines, objects) = (
        CategoryTable::new(),
        LineStyles::default_metric(),
        ObjectStyles::default_metric(),
    );
    let s = styles(&categories, &lines, &objects);
    let at = |n: u32| -> Vec<u32> {
        widths(&drawing, s, Scale::new(n).unwrap())
            .unwrap()
            .into_iter()
            .map(construct_styles::Micrometres::get)
            .collect()
    };
    assert_eq!(at(100), vec![500; 8]);
    assert_eq!(at(20), vec![700; 8]);
    let mut seen = drawing.lines()[0].clone();
    seen.kind = LineKind::Projected;
    let projected = construct_drawing::pens::style(s, &seen, Scale::new(100).unwrap()).unwrap();
    assert_eq!(projected.width.get(), 250);
}

/// The two-wall plan as SVG at 1:100, pinned: a person can open `tests/two_walls_plan.svg`.
#[test]
fn the_two_wall_plan_as_svg_is_pinned() {
    let (drawing, _) = two_walls();
    let (categories, lines, objects) = (
        CategoryTable::new(),
        LineStyles::default_metric(),
        ObjectStyles::default_metric(),
    );
    let svg = to_svg(
        &drawing,
        styles(&categories, &lines, &objects),
        Scale::new(100).unwrap(),
    )
    .unwrap();
    pinned(
        "two_walls_plan.svg",
        svg.as_bytes(),
        include_bytes!("two_walls_plan.svg"),
    );
}
