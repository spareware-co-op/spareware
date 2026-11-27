//! Sections and elevations worked by hand, over a test engine of this file's own that cuts prisms
//! exactly: a section through a layered wall with a door (each layer's cut region above the door's
//! head, the door's own region from its sill to its head, and the jamb seen beyond, less what the
//! cut hides), one through a window (its sill and head), the two elevations of a gable roof (its
//! outline) and one clipped at its depth, a wall wholly in front hiding the one behind it, an
//! elevation that cuts nothing, the map from a view's own frame to the drawing, the refusals, the
//! same drawing in any order, and the levels and grids a section shows with their names.

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
    clippy::too_many_lines,
    reason = "tests; the rules are for the crate's own code"
)]

use std::cmp::Ordering;
use std::collections::BTreeSet;

use construct_categories::{CategoryTable, DOORS, GRIDS, LEVELS, ROOFS, WALLS, WINDOWS};
use construct_drawing::DrawingError;
use construct_drawing::datum::{Anchor, Datum, DatumAt, LABEL_HEIGHT, frame, marks};
use construct_drawing::drawing::{Drawing, Entity, LineKind, Region, Shape, Turn, ViewPoint};
use construct_drawing::hatch::FillPatterns;
use construct_drawing::pens::Styles;
use construct_drawing::picture::picture;
use construct_drawing::plan::PlanElement;
use construct_drawing::section::{Look, SectionView, draw};
use construct_drawing::svg::{PER_MM, Svg};
use construct_drawing::view::Looking;
use construct_materials::{MaterialId, Materials};
use construct_model::element::Placement;
use construct_model::geometry::FaceName;
use construct_params::Length;
use construct_regen::kernel::{Kernel, KernelSolid, NamedFace};
use construct_styles::{Edge, LineStyles, ObjectStyles, Scale};
use engine_geometry::value::engine::{
    Answer, Cut, CutEdge, CutLoop, CutPoint, DisplayMesh, Edition, Engine, EngineError, GridPlane,
    Profile, Section, Tolerance, Volume,
};
use engine_geometry::value::grid::GridStep;
use kit_core::Id;

// ----- exact fractions for the test engine -----

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// `n / d` in lowest terms, `d` positive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct F(i128, i128);

impl F {
    fn new(n: i128, d: i128) -> Self {
        assert_ne!(d, 0);
        let g = gcd(n, d).max(1) * d.signum();
        Self(n / g, d / g)
    }
    fn whole(n: i128) -> Self {
        Self(n, 1)
    }
}

impl Ord for F {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.0 * other.1).cmp(&(other.0 * self.1))
    }
}

impl PartialOrd for F {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A point of space with each coordinate a fraction, as the seam gives it.
fn cut_point(p: [F; 3]) -> CutPoint {
    let den = p.iter().fold(1, |l, f| l / gcd(l, f.1) * f.1);
    let num = p.map(|f| f.0 * (den / f.1));
    let g = gcd(gcd(gcd(num[0], num[1]), num[2]), den).max(1);
    CutPoint {
        num: num.map(|n| n / g),
        den: den / g,
    }
}

fn point_order(a: &CutPoint, b: &CutPoint) -> Ordering {
    (0..3)
        .map(|k| (a.num[k] * b.den).cmp(&(b.num[k] * a.den)))
        .find(|o| *o != Ordering::Equal)
        .unwrap_or(Ordering::Equal)
}

/// The plane `normal · p = offset` with the common factor of all four taken out, as the kernel
/// builder names a face's carrier.
fn carrier(normal: [i128; 3], offset: i128) -> GridPlane {
    let g = normal.iter().fold(offset.abs(), |g, c| gcd(g, *c)).max(1);
    GridPlane {
        normal: normal.map(|c| c / g),
        offset: offset / g,
    }
}

// ----- the test engine -----

/// A prism: a flat polygon (`outer`, counter-clockwise, and `holes`, clockwise, in the two
/// coordinates `(u, w)` that follow `axis` in turn) square to the grid axis `axis`, swept along it
/// from `at` to `at + len` steps.
#[derive(Clone, Debug)]
struct Prism {
    axis: usize,
    at: i128,
    len: i128,
    outer: Vec<[i128; 2]>,
    holes: Vec<Vec<[i128; 2]>>,
}

fn uw(axis: usize) -> (usize, usize) {
    ((axis + 1) % 3, (axis + 2) % 3)
}

impl Prism {
    /// The plane of the side swept by the polygon's edge from `a` to `b`, its normal out of the
    /// material (on the edge's right, as every loop has the material on its left).
    fn side(&self, a: [i128; 2], b: [i128; 2]) -> GridPlane {
        let (u, w) = uw(self.axis);
        let mut normal = [0; 3];
        normal[u] = b[1] - a[1];
        normal[w] = -(b[0] - a[0]);
        carrier(normal, normal[u] * a[0] + normal[w] * a[1])
    }

    fn caps(&self) -> [GridPlane; 2] {
        let mut lo = [0; 3];
        lo[self.axis] = -1;
        let mut hi = [0; 3];
        hi[self.axis] = 1;
        [carrier(lo, -self.at), carrier(hi, self.at + self.len)]
    }

    fn rings(&self) -> Vec<&Vec<[i128; 2]>> {
        std::iter::once(&self.outer).chain(&self.holes).collect()
    }

    fn at3(&self, along: F, u_: F, w_: F) -> CutPoint {
        let (u, w) = uw(self.axis);
        let mut p = [F::whole(0); 3];
        p[self.axis] = along;
        p[u] = u_;
        p[w] = w_;
        cut_point(p)
    }
}

/// The test engine: it cuts a prism by a plane square to a grid axis at any depth, exactly, as
/// the seam's contract says; it makes nothing (the tests make the prisms). It is not a geometry
/// engine; the engines of the seam are judged by geom-solidcore's suite.
struct Prisms;

fn unsupported(operation: &'static str, what: &str) -> EngineError {
    EngineError::Unsupported {
        operation,
        what: what.to_owned(),
    }
}

fn exact(loops: Vec<CutLoop>) -> Cut {
    Cut {
        loops,
        exactness: engine_geometry::value::Exactness::Exact,
    }
}

/// A loop of corners and the face each edge leaving a corner lies in, turned to start at its
/// least corner.
fn closed(corners: &[(CutPoint, GridPlane)]) -> CutLoop {
    let n = corners.len();
    let mut edges: Vec<CutEdge> = (0..n)
        .map(|i| CutEdge {
            start: corners[i].0,
            end: corners[(i + 1) % n].0,
            face: corners[i].1,
        })
        .collect();
    let least = (0..n)
        .min_by(|&a, &b| point_order(&edges[a].start, &edges[b].start))
        .unwrap();
    edges.rotate_left(least);
    CutLoop { edges }
}

impl Engine for Prisms {
    type Solid = Prism;

    fn edition(&self) -> Edition {
        Edition {
            engine: "test.prisms".to_owned(),
            version: "this test file".to_owned(),
            step: GridStep::Q0,
            tolerances: vec![Tolerance {
                name: "none".to_owned(),
                value: 0.0,
                unit: "m".to_owned(),
            }],
            answers: vec![Answer {
                operation: "section".to_owned(),
                case: "a prism cut square to a grid axis".to_owned(),
                by: "the test engine".to_owned(),
            }],
        }
    }

    fn extrude(&mut self, _: &Profile, _: [i64; 3]) -> Result<Prism, EngineError> {
        Err(unsupported("extrude", "the test engine makes nothing"))
    }

    fn subtract(&mut self, _: &Prism, _: &Prism) -> Result<Prism, EngineError> {
        Err(unsupported("subtract", "the test engine makes nothing"))
    }

    fn subtract_pieces(&mut self, _: &Prism, _: &Prism) -> Result<Vec<Prism>, EngineError> {
        Err(unsupported(
            "subtract_pieces",
            "the test engine makes nothing",
        ))
    }

    fn loft(&mut self, _: &[Section]) -> Result<Prism, EngineError> {
        Err(unsupported("loft", "the test engine makes nothing"))
    }

    fn unite(&mut self, _: &Prism, _: &Prism) -> Result<Prism, EngineError> {
        Err(unsupported("unite", "the test engine makes nothing"))
    }

    fn volume(&mut self, _: &Prism) -> Result<Volume, EngineError> {
        Err(unsupported("volume", "the test engine measures nothing"))
    }

    fn mesh(&mut self, _: &Prism) -> Result<DisplayMesh, EngineError> {
        Err(unsupported("mesh", "the test engine meshes nothing"))
    }

    fn section(&mut self, s: &Prism, plane: &GridPlane) -> Result<Cut, EngineError> {
        let k = (0..3)
            .find(|&k| plane.normal[k] != 0 && (0..3).all(|j| j == k || plane.normal[j] == 0))
            .ok_or_else(|| unsupported("section", "a plane not square to a grid axis"))?;
        let up = plane.normal[k] > 0;
        // The plane is p[k] = c.
        let c = F::new(plane.offset, plane.normal[k]);
        let (u, _) = uw(s.axis);
        let [cap_lo, cap_hi] = s.caps();
        let mut loops = Vec::new();
        if k == s.axis {
            if c <= F::whole(s.at) || c >= F::whole(s.at + s.len) {
                return Ok(exact(vec![]));
            }
            for ring in s.rings() {
                let n = ring.len();
                let mut corners: Vec<(CutPoint, GridPlane)> = (0..n)
                    .map(|i| {
                        let (a, b) = (ring[i], ring[(i + 1) % n]);
                        (s.at3(c, F::whole(a[0]), F::whole(a[1])), s.side(a, b))
                    })
                    .collect();
                if !up {
                    // Seen from the other side: the same corners the other way, each edge still in
                    // its face.
                    let faces: Vec<GridPlane> = corners.iter().map(|x| x.1).collect();
                    let pts: Vec<CutPoint> = corners.iter().map(|x| x.0).collect();
                    corners = (0..n)
                        .map(|i| {
                            let from = (n - i) % n;
                            let edge = (2 * n - i - 1) % n;
                            (pts[from], faces[edge])
                        })
                        .collect();
                }
                loops.push(closed(&corners));
            }
        } else {
            // A plane along the sweep: the polygon's line `q = c` (q is u or w), crossed edge by
            // edge, the pieces inside swept from one cap to the other.
            let (q, o) = if k == u { (0, 1) } else { (1, 0) };
            let all: Vec<[i128; 2]> = s.rings().into_iter().flatten().copied().collect();
            let side = |p: &[i128; 2]| F::whole(p[q]).cmp(&c);
            if !all.iter().any(|p| side(p) == Ordering::Less)
                || !all.iter().any(|p| side(p) == Ordering::Greater)
            {
                return Ok(exact(vec![]));
            }
            if all.iter().any(|p| side(p) == Ordering::Equal) {
                return Err(unsupported("section", "a corner of the solid on the plane"));
            }
            let mut crossings: Vec<(F, GridPlane)> = Vec::new();
            for ring in s.rings() {
                let n = ring.len();
                for i in 0..n {
                    let (a, b) = (ring[i], ring[(i + 1) % n]);
                    if side(&a) == side(&b) {
                        continue;
                    }
                    // a[o] + (c - a[q]) (b[o] - a[o]) / (b[q] - a[q])
                    let num = (c.0 - a[q] * c.1) * (b[o] - a[o]) + a[o] * c.1 * (b[q] - a[q]);
                    crossings.push((F::new(num, c.1 * (b[q] - a[q])), s.side(a, b)));
                }
            }
            crossings.sort_by_key(|x| x.0);
            for pair in crossings.chunks(2) {
                let [(o0, f0), (o1, f1)] = [pair[0], pair[1]];
                let at = |along: i128, other: F| {
                    let mut uwv = [F::whole(0); 2];
                    uwv[q] = c;
                    uwv[o] = other;
                    s.at3(F::whole(along), uwv[0], uwv[1])
                };
                let (a0, a1) = (s.at, s.at + s.len);
                let mut corners = vec![
                    (at(a0, o0), f0),
                    (at(a1, o0), cap_hi),
                    (at(a1, o1), f1),
                    (at(a0, o1), cap_lo),
                ];
                // The loop runs up the sweep, then along the other axis: it turns about
                // e_axis × e_other, which is +e_k when (axis, other, k) run in the axes' own
                // cyclic order. Counter-clockwise seen from the plane's normal, or turned round.
                let other = if o == 0 { u } else { uw(s.axis).1 };
                let cyclic = (s.axis + 1) % 3 == other;
                if cyclic != up {
                    let faces: Vec<GridPlane> = corners.iter().map(|x| x.1).collect();
                    let pts: Vec<CutPoint> = corners.iter().map(|x| x.0).collect();
                    corners = (0..4)
                        .map(|i| (pts[(4 - i) % 4], faces[(7 - i) % 4]))
                        .collect();
                }
                loops.push(closed(&corners));
            }
        }
        loops.sort_by(|a, b| point_order(&a.edges[0].start, &b.edges[0].start));
        Ok(exact(loops))
    }
}

// ----- building the test's solids -----

const MM: i128 = PER_MM;

/// A prism along `axis` from `from` to `to` millimetres, its polygon's corners in millimetres in
/// the order `(u, w)`, each with the role of the side leaving it, and its holes; its caps named
/// `caps`; every face named `<op>/<role>`.
fn prism(
    op: &str,
    axis: usize,
    [from, to]: [i128; 2],
    caps: [&str; 2],
    outer: &[([i128; 2], &str)],
    holes: &[&[([i128; 2], &str)]],
) -> KernelSolid<Prism> {
    // Each loop turned the way the prism holds it: outlines counter-clockwise, holes clockwise.
    let turned = |ring: &[([i128; 2], &str)], ccw: bool| -> Vec<([i128; 2], String)> {
        let pts: Vec<[i128; 2]> = ring.iter().map(|(p, _)| [p[0] * MM, p[1] * MM]).collect();
        let area: i128 = (0..pts.len())
            .map(|i| {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                a[0] * b[1] - a[1] * b[0]
            })
            .sum();
        let edges: Vec<([i128; 2], String)> = pts
            .iter()
            .zip(ring)
            .map(|(p, (_, r))| (*p, (*r).to_owned()))
            .collect();
        if (area > 0) == ccw {
            edges
        } else {
            // The same edges the other way: the edge from corner i + 1 back to i keeps its role.
            let n = edges.len();
            (0..n)
                .map(|i| {
                    let p = edges[(n - i) % n].0;
                    let role = edges[(2 * n - i - 1) % n].1.clone();
                    (p, role)
                })
                .collect()
        }
    };
    let outer = turned(outer, true);
    let holes: Vec<Vec<([i128; 2], String)>> = holes.iter().map(|h| turned(h, false)).collect();
    let solid = Prism {
        axis,
        at: from * MM,
        len: (to - from) * MM,
        outer: outer.iter().map(|(p, _)| *p).collect(),
        holes: holes
            .iter()
            .map(|h| h.iter().map(|(p, _)| *p).collect())
            .collect(),
    };
    let named = |plane: GridPlane, role: &str| NamedFace {
        plane,
        name: FaceName {
            name: format!("{op}/{role}"),
            role: role.to_owned(),
        },
    };
    let [lo, hi] = solid.caps();
    let mut faces = vec![named(lo, caps[0]), named(hi, caps[1])];
    for ring in std::iter::once(&outer).chain(&holes) {
        for i in 0..ring.len() {
            let (a, role) = &ring[i];
            let b = ring[(i + 1) % ring.len()].0;
            let plane = solid.side(*a, b);
            if !faces.iter().any(|f| f.plane == plane) {
                faces.push(named(plane, role));
            }
        }
    }
    KernelSolid { solid, faces }
}

/// A loop's corners with their roles borrowed.
fn borrow(ring: &[([i128; 2], String)]) -> Vec<([i128; 2], &str)> {
    ring.iter().map(|(p, r)| (*p, r.as_str())).collect()
}

/// A prism along y from `y0` to `y1` of the polygon given as (x, z) corners.
fn along_y(
    op: &str,
    y: [i128; 2],
    caps: [&str; 2],
    outline: &[([i128; 2], &str)],
    holes: &[&[([i128; 2], &str)]],
) -> KernelSolid<Prism> {
    // Along y the polygon's coordinates are (z, x).
    let swap = |ring: &[([i128; 2], &str)]| -> Vec<([i128; 2], String)> {
        ring.iter()
            .map(|([x, z], r)| ([*z, *x], (*r).to_owned()))
            .collect()
    };
    let outline = swap(outline);
    let holes: Vec<Vec<([i128; 2], String)>> = holes.iter().map(|h| swap(h)).collect();
    let outline = borrow(&outline);
    let holes: Vec<Vec<([i128; 2], &str)>> = holes.iter().map(|h| borrow(h)).collect();
    let hole_refs: Vec<&[([i128; 2], &str)]> = holes.iter().map(Vec::as_slice).collect();
    prism(op, 1, y, caps, &outline, &hole_refs)
}

/// A box from `lo` to `hi` millimetres, swept up z: its sides named by the way they face.
fn cuboid(op: &str, lo: [i128; 3], hi: [i128; 3]) -> KernelSolid<Prism> {
    prism(
        op,
        2,
        [lo[2], hi[2]],
        ["base", "top"],
        &[
            ([lo[0], lo[1]], "south"),
            ([hi[0], lo[1]], "east"),
            ([hi[0], hi[1]], "north"),
            ([lo[0], hi[1]], "west"),
        ],
        &[],
    )
}

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(v: i32) -> Length {
    Length::from_mm(v)
}

fn p(x: i128, y: i128) -> ViewPoint {
    ViewPoint::grid(x * MM, y * MM)
}

fn kernel() -> Kernel<Prisms> {
    Kernel::new(Prisms).unwrap()
}

fn element<'a>(
    name: &str,
    category: construct_categories::CategoryId,
    material: Option<&str>,
    shape: &'a KernelSolid<Prism>,
) -> PlanElement<'a, Prism> {
    PlanElement {
        id: id(name),
        category,
        material: material.map(|m| MaterialId::parse(m).unwrap()),
        placement: Placement { x: 0, y: 0, z: 0 },
        shape,
    }
}

/// Each segment of `drawing` of `kind`, from its start to its end, with its element and role.
fn segments(drawing: &Drawing, kind: LineKind) -> BTreeSet<(String, String, ViewPoint, ViewPoint)> {
    drawing
        .lines()
        .iter()
        .filter(|l| l.kind == kind)
        .map(|l| match l.shape {
            Shape::Segment { start, end } => (
                l.source.element.to_string(),
                l.source.role.clone(),
                start,
                end,
            ),
            Shape::Arc { .. } => panic!("no arcs here"),
        })
        .collect()
}

fn seg(
    element: &str,
    role: &str,
    a: ViewPoint,
    b: ViewPoint,
) -> (String, String, ViewPoint, ViewPoint) {
    (element.to_owned(), role.to_owned(), a, b)
}

/// A rectangle's loop from its least corner, counter-clockwise: as a drawing holds a region's.
fn rect(x0: i128, y0: i128, x1: i128, y1: i128) -> Vec<ViewPoint> {
    vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)]
}

fn region(
    element: &str,
    category: construct_categories::CategoryId,
    material: Option<&str>,
    loops: Vec<Vec<ViewPoint>>,
) -> Region {
    Region {
        element: id(element),
        category,
        material: material.map(|m| MaterialId::parse(m).unwrap()),
        loops,
    }
}

// ----- the wall and its openings -----

const LAYERS: [(&str, [i128; 2], &str, [&str; 2]); 3] = [
    ("wall-a", [0, 20], "plaster-gypsum", ["exterior", "face.1"]),
    (
        "wall-a",
        [20, 220],
        "concrete-regular",
        ["face.1", "face.2"],
    ),
    (
        "wall-a",
        [220, 240],
        "plaster-gypsum",
        ["face.2", "interior"],
    ),
];

/// The three layers of a wall 3000 mm along x and 2700 mm high, with a door's opening from
/// x = 1000 to 1900 and up to 2100: each layer's elevation is a U, swept through its thickness.
fn door_wall() -> Vec<KernelSolid<Prism>> {
    let outline: &[([i128; 2], &str)] = &[
        ([0, 0], "base"),
        ([1000, 0], "jamb.0"),
        ([1000, 2100], "head"),
        ([1900, 2100], "jamb.1"),
        ([1900, 0], "base"),
        ([3000, 0], "end1"),
        ([3000, 2700], "top"),
        ([0, 2700], "end0"),
    ];
    LAYERS
        .iter()
        .map(|(op, y, _, caps)| along_y(op, *y, *caps, outline, &[]))
        .collect()
}

/// The section through the door, from (1450, -1000) to (1450, 2000), looking right (east) 2000 mm.
fn through_the_door() -> SectionView {
    SectionView::on_line(
        [mm(1450), mm(-1000)],
        [mm(1450), mm(2000)],
        Look::Right,
        mm(2000),
        true,
    )
    .unwrap()
}

fn door_drawing(reversed: bool) -> Drawing {
    let k = kernel();
    let layers = door_wall();
    let door = cuboid("door-a", [1000, 70, 0], [1900, 170, 2100]);
    let mut elements: Vec<PlanElement<'_, Prism>> = layers
        .iter()
        .zip(LAYERS)
        .map(|(shape, (_, _, material, _))| element("wall-a", WALLS, Some(material), shape))
        .collect();
    elements.push(element("door-a", DOORS, Some("timber-white-oak"), &door));
    if reversed {
        elements.reverse();
    }
    draw(&k, &through_the_door(), &elements).unwrap()
}

/// The section through the door: each layer's cut region is above the door's head (2100 to 2700,
/// across the layer's thickness, the view's x being -y looking east), the door's region runs from
/// its sill (0) to its head; and beyond, the jamb face at x = 1900 shows below the head, its
/// bottom edge broken where the door's cut region stands in front of it.
#[test]
fn a_section_through_a_layered_wall_with_a_door() {
    let drawing = door_drawing(false);
    assert_eq!(drawing.view().looking, Looking::East);
    assert_eq!(drawing.view().cut, mm(1450));
    let expected = vec![
        region(
            "door-a",
            DOORS,
            Some("timber-white-oak"),
            vec![rect(-170, 0, -70, 2100)],
        ),
        region(
            "wall-a",
            WALLS,
            Some("concrete-regular"),
            vec![rect(-220, 2100, -20, 2700)],
        ),
        region(
            "wall-a",
            WALLS,
            Some("plaster-gypsum"),
            vec![rect(-240, 2100, -220, 2700)],
        ),
        region(
            "wall-a",
            WALLS,
            Some("plaster-gypsum"),
            vec![rect(-20, 2100, 0, 2700)],
        ),
    ];
    let mut expected = expected;
    expected.sort();
    assert_eq!(drawing.regions(), expected.as_slice());
    // The head is the bottom of every layer's region; nothing of the wall is cut below it.
    let cut = segments(&drawing, LineKind::Cut);
    for (element, role, a, b) in &cut {
        if element == "wall-a" {
            assert!(a.y >= 2100 * MM && b.y >= 2100 * MM, "{role}: {a:?} {b:?}");
        }
    }
    assert!(cut.contains(&seg("wall-a", "head", p(-20, 2100), p(0, 2100))));
    assert!(cut.contains(&seg("wall-a", "head", p(-220, 2100), p(-20, 2100))));
    assert!(cut.contains(&seg("wall-a", "top", p(0, 2700), p(-20, 2700))));
    assert!(cut.contains(&seg("wall-a", "exterior", p(0, 2100), p(0, 2700))));
    assert_eq!(cut.iter().filter(|s| s.0 == "wall-a").count(), 12);
    assert_eq!(cut.iter().filter(|s| s.0 == "door-a").count(), 4);
    // Seen beyond: the jamb's outline below the head, less what the cut hides.
    assert_eq!(
        segments(&drawing, LineKind::Projected),
        BTreeSet::from([
            seg("wall-a", "base", p(-240, 0), p(-170, 0)),
            seg("wall-a", "base", p(-70, 0), p(0, 0)),
            seg("wall-a", "exterior", p(0, 0), p(0, 2100)),
            seg("wall-a", "interior", p(-240, 2100), p(-240, 0)),
        ])
    );
    for line in drawing
        .lines()
        .iter()
        .filter(|l| l.kind == LineKind::Projected)
    {
        assert_eq!(line.source.entity, Entity::Face);
        assert_eq!(line.category, WALLS);
    }
}

/// The same elements in another order give the same drawing, and the same bytes every time.
#[test]
fn the_same_section_in_any_order_is_the_same_bytes() {
    let a = door_drawing(false);
    let b = door_drawing(true);
    assert_eq!(a, b);
    assert_eq!(a.to_bytes().unwrap(), b.to_bytes().unwrap());
    assert_eq!(Drawing::from_bytes(&a.to_bytes().unwrap()).unwrap(), a);
}

/// A section through a window: each layer is cut below the sill (0 to 900) and above the head
/// (2100 to 2700), the window between; beyond, the opening's jamb shows between sill and head,
/// looking west (the view's x is y).
#[test]
fn a_section_through_a_window_shows_its_sill_and_head() {
    let k = kernel();
    let outline: &[([i128; 2], &str)] = &[
        ([0, 0], "base"),
        ([3000, 0], "end1"),
        ([3000, 2700], "top"),
        ([0, 2700], "end0"),
    ];
    let opening: &[([i128; 2], &str)] = &[
        ([2200, 900], "sill"),
        ([2800, 900], "jamb.1"),
        ([2800, 2100], "head"),
        ([2200, 2100], "jamb.0"),
    ];
    let layers: Vec<KernelSolid<Prism>> = LAYERS
        .iter()
        .map(|(op, y, _, caps)| along_y(op, *y, *caps, outline, &[opening]))
        .collect();
    let window = cuboid("window-a", [2200, 100, 900], [2800, 140, 2100]);
    let mut elements: Vec<PlanElement<'_, Prism>> = layers
        .iter()
        .zip(LAYERS)
        .map(|(shape, (_, _, material, _))| element("wall-a", WALLS, Some(material), shape))
        .collect();
    elements.push(element("window-a", WINDOWS, Some("glass-plate"), &window));
    let view = SectionView::on_line(
        [mm(2500), mm(-1000)],
        [mm(2500), mm(2000)],
        Look::Left,
        mm(1000),
        true,
    )
    .unwrap();
    assert_eq!(view.view().looking, Looking::West);
    let drawing = draw(&k, &view, &elements).unwrap();
    let mut expected = vec![
        region(
            "window-a",
            WINDOWS,
            Some("glass-plate"),
            vec![rect(100, 900, 140, 2100)],
        ),
        region(
            "wall-a",
            WALLS,
            Some("plaster-gypsum"),
            vec![rect(0, 0, 20, 900), rect(0, 2100, 20, 2700)],
        ),
        region(
            "wall-a",
            WALLS,
            Some("concrete-regular"),
            vec![rect(20, 0, 220, 900), rect(20, 2100, 220, 2700)],
        ),
        region(
            "wall-a",
            WALLS,
            Some("plaster-gypsum"),
            vec![rect(220, 0, 240, 900), rect(220, 2100, 240, 2700)],
        ),
    ];
    expected.sort();
    assert_eq!(drawing.regions(), expected.as_slice());
    let cut = segments(&drawing, LineKind::Cut);
    assert!(cut.contains(&seg("wall-a", "sill", p(20, 900), p(0, 900))));
    assert!(cut.contains(&seg("wall-a", "head", p(0, 2100), p(20, 2100))));
    assert_eq!(
        segments(&drawing, LineKind::Projected),
        BTreeSet::from([
            seg("wall-a", "exterior", p(0, 2100), p(0, 900)),
            seg("wall-a", "interior", p(240, 900), p(240, 2100)),
        ])
    );
}

// ----- the gable roof -----

/// A gable roof 8000 mm along x, its eaves at y = 0 and 6000 and its ridge at y = 3000: the slab
/// 200 mm deep (measured upright) rises 2000 mm from its eaves (top 2400, underside 2200) to the
/// ridge (top 4400, underside 4200).
fn gable() -> KernelSolid<Prism> {
    prism(
        "roof-a",
        0,
        [0, 8000],
        ["verge.0", "verge.1"],
        &[
            ([0, 2200], "soffit.south"),
            ([3000, 4200], "soffit.north"),
            ([6000, 2200], "fascia.north"),
            ([6000, 2400], "slope.north"),
            ([3000, 4400], "slope.south"),
            ([0, 2400], "fascia.south"),
        ],
        &[],
    )
}

fn elevation(from: [i32; 2], to: [i32; 2], look: Look, depth: i32) -> SectionView {
    SectionView::on_line(
        [mm(from[0]), mm(from[1])],
        [mm(to[0]), mm(to[1])],
        look,
        mm(depth),
        false,
    )
    .unwrap()
}

/// The gable roof seen from its end, looking east from x = -1000: its outline is the gable's own
/// six-sided shape (the view's x is -y), each side named by the face seen edge on.
#[test]
fn an_elevation_of_a_gable_roof_is_its_outline() {
    let k = kernel();
    let roof = gable();
    let view = elevation([-1000, 7000], [-1000, -1000], Look::Left, 20000);
    assert_eq!(view.view().looking, Looking::East);
    let drawing = draw(&k, &view, &[element("roof-a", ROOFS, None, &roof)]).unwrap();
    assert_eq!(drawing.regions(), &[] as &[Region]);
    assert_eq!(segments(&drawing, LineKind::Cut), BTreeSet::new());
    assert_eq!(
        segments(&drawing, LineKind::Projected),
        BTreeSet::from([
            seg("roof-a", "soffit.south", p(0, 2200), p(-3000, 4200)),
            seg("roof-a", "soffit.north", p(-3000, 4200), p(-6000, 2200)),
            seg("roof-a", "fascia.north", p(-6000, 2200), p(-6000, 2400)),
            seg("roof-a", "slope.north", p(-6000, 2400), p(-3000, 4400)),
            seg("roof-a", "slope.south", p(-3000, 4400), p(0, 2400)),
            seg("roof-a", "fascia.south", p(0, 2400), p(0, 2200)),
        ])
        .into_iter()
        .map(|(e, r, a, b)| {
            // The outline turns counter-clockwise in the drawing: each side the other way round.
            (e, r, b, a)
        })
        .collect()
    );
}

/// The gable roof seen from the south, looking north: a rectangle from the eaves' underside to the
/// ridge, its top the ridge (where the two slopes meet) and its foot the soffit's edge at the
/// fascia; and seen only 1500 mm deep, the far clip stops it where the clip's plane crosses the
/// southern slope, 3400 mm up.
#[test]
fn a_gable_roof_from_its_side_and_cut_short_by_the_depth() {
    let k = kernel();
    let roof = gable();
    let whole = elevation([-1000, -1000], [9000, -1000], Look::Left, 20000);
    assert_eq!(whole.view().looking, Looking::North);
    let drawing = draw(&k, &whole, &[element("roof-a", ROOFS, None, &roof)]).unwrap();
    let found = segments(&drawing, LineKind::Projected);
    let ends: BTreeSet<(ViewPoint, ViewPoint)> = found.iter().map(|s| (s.2, s.3)).collect();
    assert_eq!(
        ends,
        BTreeSet::from([
            (p(0, 2200), p(8000, 2200)),
            (p(8000, 2200), p(8000, 4400)),
            (p(8000, 4400), p(0, 4400)),
            (p(0, 4400), p(0, 2200)),
        ])
    );
    // The ridge is where the two slopes meet; of the faces it lies in, the least by name names it.
    let top = found.iter().find(|s| s.2 == p(8000, 4400)).unwrap();
    assert_eq!(top.1, "slope.north");
    // A side is the verge seen edge on: the face names it, before the edges it holds.
    let side = found.iter().find(|s| s.2 == p(8000, 2200)).unwrap();
    assert_eq!(side.1, "verge.1");
    let short = elevation([-1000, -1000], [9000, -1000], Look::Left, 2500);
    let drawing = draw(&k, &short, &[element("roof-a", ROOFS, None, &roof)]).unwrap();
    let ends: BTreeSet<(ViewPoint, ViewPoint)> = segments(&drawing, LineKind::Projected)
        .into_iter()
        .map(|s| (s.2, s.3))
        .collect();
    assert_eq!(
        ends,
        BTreeSet::from([
            (p(0, 2200), p(8000, 2200)),
            (p(8000, 2200), p(8000, 3400)),
            (p(8000, 3400), p(0, 3400)),
            (p(0, 3400), p(0, 2200)),
        ])
    );
}

// ----- what hides what -----

/// An elevation looking north at a wall wholly in front of a taller, longer one: the one behind
/// shows only where the one in front does not stand.
#[test]
fn a_wall_wholly_in_front_hides_the_one_behind() {
    let k = kernel();
    let front = cuboid("wall-front", [0, 0, 0], [4000, 200, 2700]);
    let back = cuboid("wall-back", [1000, 3000, 0], [5000, 3200, 3000]);
    let view = elevation([-1000, -1000], [9000, -1000], Look::Left, 10000);
    let drawing = draw(
        &k,
        &view,
        &[
            element("wall-front", WALLS, None, &front),
            element("wall-back", WALLS, None, &back),
        ],
    )
    .unwrap();
    let ends: BTreeSet<(String, ViewPoint, ViewPoint)> = segments(&drawing, LineKind::Projected)
        .into_iter()
        .map(|s| (s.0, s.2, s.3))
        .collect();
    let at = |e: &str, a: ViewPoint, b: ViewPoint| (e.to_owned(), a, b);
    assert_eq!(
        ends,
        BTreeSet::from([
            at("wall-front", p(0, 0), p(4000, 0)),
            at("wall-front", p(4000, 0), p(4000, 2700)),
            at("wall-front", p(4000, 2700), p(0, 2700)),
            at("wall-front", p(0, 2700), p(0, 0)),
            at("wall-back", p(4000, 0), p(5000, 0)),
            at("wall-back", p(5000, 0), p(5000, 3000)),
            at("wall-back", p(5000, 3000), p(1000, 3000)),
            at("wall-back", p(1000, 3000), p(1000, 2700)),
        ])
    );
}

/// An elevation cuts nothing: a wall across its plane gives no region, and the part of it beyond
/// the plane shows as its outline.
#[test]
fn an_elevation_cuts_nothing() {
    let k = kernel();
    let wall = cuboid("wall-a", [0, -500, 0], [200, 500, 2700]);
    let view = elevation([-1000, 0], [9000, 0], Look::Left, 10000);
    let drawing = draw(&k, &view, &[element("wall-a", WALLS, None, &wall)]).unwrap();
    assert_eq!(drawing.regions(), &[] as &[Region]);
    let ends: BTreeSet<(ViewPoint, ViewPoint)> = segments(&drawing, LineKind::Projected)
        .into_iter()
        .map(|s| (s.2, s.3))
        .collect();
    assert_eq!(
        ends,
        BTreeSet::from([
            (p(0, 0), p(200, 0)),
            (p(200, 0), p(200, 2700)),
            (p(200, 2700), p(0, 2700)),
            (p(0, 2700), p(0, 0)),
        ])
    );
    // A section on the same line cuts it, and its own region hides its outline.
    let section = SectionView::on_line(
        [mm(-1000), mm(0)],
        [mm(9000), mm(0)],
        Look::Left,
        mm(10000),
        true,
    )
    .unwrap();
    let drawing = draw(&k, &section, &[element("wall-a", WALLS, None, &wall)]).unwrap();
    assert_eq!(
        drawing.regions(),
        &[region("wall-a", WALLS, None, vec![rect(0, 0, 200, 2700)])]
    );
    assert_eq!(segments(&drawing, LineKind::Projected), BTreeSet::new());
}

// ----- the view itself -----

/// The view's own frame (along the line from its first end, and up) maps to the drawing: looking
/// left the distance runs to the right on the sheet, looking right it runs to the left.
#[test]
fn the_views_own_frame_maps_to_the_drawing() {
    let left = SectionView::on_line(
        [mm(1000), mm(500)],
        [mm(5000), mm(500)],
        Look::Left,
        mm(100),
        true,
    )
    .unwrap();
    assert_eq!(left.view().looking, Looking::North);
    assert_eq!(left.view().cut, mm(500));
    assert_eq!(left.point(mm(0), mm(0)).unwrap(), p(1000, 0));
    assert_eq!(left.point(mm(4000), mm(3000)).unwrap(), p(5000, 3000));
    assert_eq!(left.ends().unwrap(), [1000 * MM, 5000 * MM]);
    let right = SectionView::on_line(
        [mm(1000), mm(500)],
        [mm(5000), mm(500)],
        Look::Right,
        mm(100),
        true,
    )
    .unwrap();
    assert_eq!(right.view().looking, Looking::South);
    assert_eq!(right.point(mm(0), mm(0)).unwrap(), p(-1000, 0));
    assert_eq!(right.point(mm(4000), mm(0)).unwrap(), p(-5000, 0));
    assert_eq!(right.ends().unwrap(), [-5000 * MM, -1000 * MM]);
    let west =
        SectionView::on_line([mm(0), mm(0)], [mm(0), mm(-10)], Look::Right, mm(1), false).unwrap();
    assert_eq!(west.view().looking, Looking::West);
    assert!(!west.cuts());
}

/// A line not along a grid axis, a line of no length and a view of no depth are refused in words.
#[test]
fn a_view_the_drawing_cannot_make_is_refused() {
    let at = |x: i32, y: i32| [mm(x), mm(y)];
    let slanted = SectionView::on_line(at(0, 0), at(1000, 1000), Look::Left, mm(100), true);
    assert!(
        matches!(&slanted, Err(DrawingError::View(why)) if why.contains("grid axis")),
        "{slanted:?}"
    );
    let point = SectionView::on_line(at(5, 5), at(5, 5), Look::Left, mm(100), true);
    assert!(matches!(point, Err(DrawingError::View(_))));
    let flat = SectionView::on_line(at(0, 0), at(1000, 0), Look::Left, mm(0), true);
    assert!(matches!(flat, Err(DrawingError::View(_))));
}

// ----- levels and grids -----

fn styles<'a>(
    categories: &'a CategoryTable,
    lines: &'a LineStyles,
    objects: &'a ObjectStyles,
    materials: &'a Materials,
    fills: &'a FillPatterns,
) -> Styles<'a> {
    Styles {
        categories,
        lines,
        objects,
        materials,
        fills,
        looks: None,
    }
}

/// The section through the door shows its two levels as lines across its width with their names
/// past the right end, and the grid that crosses its plane as an upright line with its bubble and
/// name; a grid parallel to the plane, one that never reaches the part of the plan it sees, and an
/// arc grid do not show, the arc grid named with its reason.
#[test]
fn levels_and_grids_show_where_they_cross_a_section() {
    let drawing = door_drawing(false);
    let view = through_the_door();
    let datums = vec![
        Datum {
            id: id("level-1"),
            category: LEVELS,
            name: "Level 1".to_owned(),
            at: DatumAt::Level { height: mm(0) },
        },
        Datum {
            id: id("level-2"),
            category: LEVELS,
            name: "Level 2".to_owned(),
            at: DatumAt::Level { height: mm(3000) },
        },
        Datum {
            id: id("grid-1"),
            category: GRIDS,
            name: "1".to_owned(),
            at: DatumAt::Grid {
                from: [mm(-500), mm(120)],
                to: [mm(4000), mm(120)],
            },
        },
        Datum {
            id: id("grid-b"),
            category: GRIDS,
            name: "B".to_owned(),
            at: DatumAt::Grid {
                from: [mm(2000), mm(-500)],
                to: [mm(2000), mm(4000)],
            },
        },
        Datum {
            id: id("grid-9"),
            category: GRIDS,
            name: "9".to_owned(),
            at: DatumAt::Grid {
                from: [mm(-500), mm(9000)],
                to: [mm(4000), mm(9000)],
            },
        },
        Datum {
            id: id("grid-arc"),
            category: GRIDS,
            name: "R".to_owned(),
            at: DatumAt::ArcGrid,
        },
    ];
    let box_ = frame(&view, &drawing, &datums).unwrap().unwrap();
    assert_eq!(box_, [p(-2000, 0), p(1000, 3000)]);
    let (categories, lines, objects, materials, fills) = (
        CategoryTable::new(),
        LineStyles::default_metric(),
        ObjectStyles::default_metric(),
        Materials::default_metric(),
        FillPatterns::default_metric(),
    );
    let scale = Scale::new(50).unwrap();
    let marked = marks(
        &view,
        &datums,
        box_,
        styles(&categories, &lines, &objects, &materials, &fills),
        scale,
    )
    .unwrap();
    let shapes: Vec<(String, Shape)> = marked
        .strokes
        .iter()
        .map(|s| (s.element.to_string(), s.shape))
        .collect();
    // At 1:50 a bubble's 4 mm is 200 mm of the model, and a name stands 100 mm past its line.
    assert_eq!(
        shapes,
        vec![
            (
                "level-1".to_owned(),
                Shape::Segment {
                    start: p(-2000, 0),
                    end: p(1000, 0)
                }
            ),
            (
                "level-2".to_owned(),
                Shape::Segment {
                    start: p(-2000, 3000),
                    end: p(1000, 3000)
                }
            ),
            (
                "grid-1".to_owned(),
                Shape::Segment {
                    start: p(-120, 0),
                    end: p(-120, 3000)
                }
            ),
            (
                "grid-1".to_owned(),
                Shape::Arc {
                    centre: p(-120, 3200),
                    start: p(80, 3200),
                    end: p(-320, 3200),
                    turn: Turn::Counterclockwise
                }
            ),
            (
                "grid-1".to_owned(),
                Shape::Arc {
                    centre: p(-120, 3200),
                    start: p(-320, 3200),
                    end: p(80, 3200),
                    turn: Turn::Counterclockwise
                }
            ),
        ]
    );
    let labels: Vec<(String, ViewPoint, Anchor)> = marked
        .labels
        .iter()
        .map(|l| (l.text.clone(), l.at, l.anchor))
        .collect();
    assert_eq!(
        labels,
        vec![
            ("Level 1".to_owned(), p(1100, 0), Anchor::Start),
            ("Level 2".to_owned(), p(1100, 3000), Anchor::Start),
            ("1".to_owned(), p(-120, 3200), Anchor::Middle),
        ]
    );
    assert!(marked.labels.iter().all(|l| l.height.get() == LABEL_HEIGHT));
    assert_eq!(marked.skipped.len(), 1);
    assert_eq!(marked.skipped[0].0, id("grid-arc"));
    // Each line is penned as its category's projected edge.
    let level = objects
        .lookup(&categories, &lines, LEVELS, Edge::Projected, scale)
        .unwrap();
    let grid = objects
        .lookup(&categories, &lines, GRIDS, Edge::Projected, scale)
        .unwrap();
    assert_eq!(
        (marked.strokes[0].width, marked.strokes[0].pattern),
        (level.width, level.pattern)
    );
    assert_eq!(
        (marked.strokes[2].width, marked.strokes[2].pattern),
        (grid.width, grid.pattern)
    );
    assert_eq!(marked.labels[2].colour, grid.colour);
}

/// Names are written as SVG text after the drawing, escaped; with none the document is exactly
/// the picture's own.
#[test]
fn names_are_written_into_the_svg() {
    let drawing = door_drawing(false);
    let (categories, lines, objects, materials, fills) = (
        CategoryTable::new(),
        LineStyles::default_metric(),
        ObjectStyles::default_metric(),
        Materials::default_metric(),
        FillPatterns::default_metric(),
    );
    let scale = Scale::new(50).unwrap();
    let st = styles(&categories, &lines, &objects, &materials, &fills);
    let pic = picture(&drawing, st, scale).unwrap();
    assert_eq!(
        Svg.labelled(&pic, &[]).unwrap(),
        Svg.document(&pic).unwrap()
    );
    let datums = vec![Datum {
        id: id("level-1"),
        category: LEVELS,
        name: "Ground & <first>".to_owned(),
        at: DatumAt::Level { height: mm(0) },
    }];
    let view = through_the_door();
    let box_ = frame(&view, &drawing, &datums).unwrap().unwrap();
    let marked = marks(&view, &datums, box_, st, scale).unwrap();
    let svg = Svg.labelled(&pic, &marked.labels).unwrap();
    assert!(
        svg.contains("font-family=\"sans-serif\" font-size=\"125\" text-anchor=\"start\""),
        "{svg}"
    );
    assert!(svg.contains(">Ground &amp; &lt;first&gt;</text>"), "{svg}");
    assert!(
        svg.contains("data-element=\"level-1\" data-role=\"level\""),
        "{svg}"
    );
    assert!(svg.ends_with("</text>\n</svg>\n"));
}
