//! The plan of two walls meeting at a corner, made by the kernel builder over a test engine of this
//! file's own: its segments exactly, its regions, its bytes twice, its stored form, its printed
//! widths with the default styles, and its SVG at 1:100, pinned. Then printed: the two concrete
//! walls merged into one hatched region outlined at the walls' cut weight for the scale, and a
//! third wall of plaster beside them keeping its line, drawn once; that SVG pinned too.

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
use construct_drawing::hatch::FillPatterns;
use construct_drawing::pens::{Styles, widths};
use construct_drawing::picture::{Output, picture};
use construct_drawing::plan::{PlanElement, plan};
use construct_drawing::svg::{PER_MM, Svg, to_svg};
use construct_drawing::view::View;
use construct_materials::{MaterialId, Materials};
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

/// The plan of `walls`, each with its material, cut 1200 mm up, and each wall's named faces.
fn plan_of(walls: &[(Id, Recipe, Placement, &str)]) -> (Drawing, Vec<KernelSolid<Cuboid>>) {
    let kernel = Kernel::new(Boxes).unwrap();
    let shapes: Vec<KernelSolid<Cuboid>> = walls
        .iter()
        .map(|(_, r, _, _)| kernel.solid(r).unwrap())
        .collect();
    let elements: Vec<PlanElement<'_, Cuboid>> = walls
        .iter()
        .zip(&shapes)
        .map(|((id, _, placement, material), shape)| PlanElement {
            id: id.clone(),
            category: WALLS,
            material: Some(MaterialId::parse(material).unwrap()),
            placement: *placement,
            shape,
        })
        .collect();
    let drawing = plan(&kernel, View::plan(Length::from_mm(1200)), &elements).unwrap();
    (drawing, shapes)
}

/// The plan of the two concrete walls cut 1200 mm up, and each wall's named faces.
fn two_walls() -> (Drawing, Vec<KernelSolid<Cuboid>>) {
    let walls: Vec<_> = walls()
        .into_iter()
        .map(|(id, r, at)| (id, r, at, "concrete-regular"))
        .collect();
    plan_of(&walls)
}

/// The two concrete walls and a third of plaster, 1000 mm long, carrying the first wall on west
/// from its end at the origin.
fn two_walls_and_one() -> Drawing {
    let mut walls: Vec<_> = walls()
        .into_iter()
        .map(|(id, r, at)| (id, r, at, "concrete-regular"))
        .collect();
    walls.push((
        Id::parse("wall-c").unwrap(),
        recipe(
            "wall.straight",
            json!({ "length": 1000, "thickness": 200, "height": 2700 }),
        ),
        Placement {
            x: -1000,
            y: 0,
            z: 0,
        },
        "plaster-gypsum",
    ));
    plan_of(&walls).0
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

/// The default styles, materials and fill patterns, kept for a test to borrow.
struct Defaults {
    categories: CategoryTable,
    lines: LineStyles,
    objects: ObjectStyles,
    materials: Materials,
    fills: FillPatterns,
}

impl Defaults {
    fn new() -> Self {
        Self {
            categories: CategoryTable::new(),
            lines: LineStyles::default_metric(),
            objects: ObjectStyles::default_metric(),
            materials: Materials::default_metric(),
            fills: FillPatterns::default_metric(),
        }
    }

    fn styles(&self) -> Styles<'_> {
        Styles {
            categories: &self.categories,
            lines: &self.lines,
            objects: &self.objects,
            materials: &self.materials,
            fills: &self.fills,
        }
    }
}

/// With the default styles, every cut wall line prints 0.5 mm at 1:100 and 0.7 mm at 1:20, and
/// the same wall seen in projection 0.25 mm at 1:100.
#[test]
fn the_cut_walls_print_with_the_default_pens() {
    let (drawing, _) = two_walls();
    let defaults = Defaults::new();
    let s = defaults.styles();
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

/// The two-wall plan as SVG at 1:100, pinned: a person can open `tests/two_walls_plan.svg`. The
/// two concrete walls draw as one region, hatched, with no line where they meet.
#[test]
fn the_two_wall_plan_as_svg_is_pinned() {
    let (drawing, _) = two_walls();
    let defaults = Defaults::new();
    let svg = to_svg(&drawing, defaults.styles(), Scale::new(100).unwrap()).unwrap();
    pinned(
        "two_walls_plan.svg",
        svg.as_bytes(),
        include_bytes!("two_walls_plan.svg"),
    );
}

fn segment(start: ViewPoint, end: ViewPoint) -> Shape {
    Shape::Segment { start, end }
}

/// Printed at 1:100, the two concrete walls are one hatched region: one hatch of the concrete
/// pattern, and their outline is seven strokes round both, none where they meet; the plaster wall
/// is its own hatched region, and the line it shares with the first wall is one stroke.
#[test]
fn walls_of_one_material_print_as_one_hatched_region() {
    let drawing = two_walls_and_one();
    let defaults = Defaults::new();
    let pic = picture(&drawing, defaults.styles(), Scale::new(100).unwrap()).unwrap();
    let materials: Vec<&str> = pic.hatches.iter().map(|h| h.material.as_str()).collect();
    assert_eq!(materials, ["concrete-regular", "plaster-gypsum"]);
    assert!(pic.hatches.iter().all(|h| !h.lines.is_empty()));
    let on = |shape: Shape| {
        pic.strokes
            .iter()
            .filter(|s| {
                let Shape::Segment { start, end } = s.shape else {
                    return false;
                };
                shape == segment(start, end) || shape == segment(end, start)
            })
            .count()
    };
    // Where the walls of one material meet: nothing.
    assert_eq!(on(segment(p(3800, 200), p(4000, 200))), 0);
    // Between concrete and plaster: one line.
    assert_eq!(on(segment(p(0, 0), p(0, 200))), 1);
    // The concrete outline's seven edges and the plaster's three others: the shared one is the
    // concrete wall's.
    assert_eq!(pic.strokes.len(), 7 + 3);
    let concrete = pic
        .strokes
        .iter()
        .filter(|s| s.element.as_str() != "wall-c")
        .count();
    assert_eq!(concrete, 7);
}

/// The merged outline prints at the walls' cut pen for the view's scale: 0.5 mm at 1:100 and
/// 0.7 mm at 1:20 with the default pens; the hatch lines at pen 1, 0.13 mm at both, in the
/// material's cut colour.
#[test]
fn the_outline_weight_is_chosen_by_scale() {
    let drawing = two_walls_and_one();
    let defaults = Defaults::new();
    let at = |n: u32| picture(&drawing, defaults.styles(), Scale::new(n).unwrap()).unwrap();
    for (n, cut) in [(100, 500), (20, 700)] {
        let pic = at(n);
        assert!(pic.strokes.iter().all(|s| s.width.get() == cut), "1:{n}");
        assert!(pic.hatches.iter().all(|h| h.width.get() == 130), "1:{n}");
    }
    let concrete = defaults
        .materials
        .get(&MaterialId::parse("concrete-regular").unwrap())
        .unwrap();
    assert_eq!(at(100).hatches[0].colour, concrete.drawing.cut.colour);
}

/// Two concrete walls at a corner and a plaster wall beside them, as SVG at 1:100, pinned: a
/// person can open `tests/two_walls_and_one.svg`. The same drawing gives the same bytes twice,
/// and the SVG output written through its trait gives the same bytes.
#[test]
fn two_walls_and_one_as_svg_is_pinned() {
    let defaults = Defaults::new();
    let scale = Scale::new(100).unwrap();
    let svg = to_svg(&two_walls_and_one(), defaults.styles(), scale).unwrap();
    assert_eq!(
        svg,
        to_svg(&two_walls_and_one(), defaults.styles(), scale).unwrap()
    );
    let pic = picture(&two_walls_and_one(), defaults.styles(), scale).unwrap();
    assert_eq!(Svg.write(&pic).unwrap(), svg.as_bytes());
    pinned(
        "two_walls_and_one.svg",
        svg.as_bytes(),
        include_bytes!("two_walls_and_one.svg"),
    );
}
