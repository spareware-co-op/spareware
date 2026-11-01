//! Group 2 of chapter 8.3, geometric consistency: every vertex lies on its faces' surfaces and at
//! its edges' curve ends, and every edge on its faces, within the bound the body is recorded with.
//!
//! What it does: [`check`] walks the faces, fins and edges in row order. A face's surface is a
//! plane, cylinder, cone, sphere or ring torus (`crate::curved::Carrier`); any other kind is
//! reported as [`FaultKind::Unchecked`], never passed (8.3: "certified with bounds, not sampled").
//! For each fin, the vertex it starts at must lie on its face's surface, and its edge's line or
//! circle on that surface; for each edge, its vertices must lie at its line's or circle's ends. An
//! edge on another kind of curve, and a fin's pcurve, are reported unchecked. A vertex where three
//! planes meet must have three that do meet ([`FaultKind::MeetSingular`]).
//!
//! The bound: the body's [`Exactness`] record, in the steps of its coordinates (a vertex is a whole
//! number of grid steps, and the curves and surfaces are doubles in the same units, chapter 6.1):
//! 0 for `Exact`, the proven bound for `Certified` and the measured deviation for `Approximate`.
//!
//! Guarantees:
//!
//! - **Exact on flat faces.** A plane given by integer-valued doubles (its origin and two
//!   spanning vectors within ±(2^40 − 1)) becomes the integer plane through three grid points, and
//!   a vertex passes exactly when (n · p + d)^2 ≤ b^2 · |n|^2, compared in 320-bit integers; for
//!   b = 0 that is the orient test, n · p + d = 0. A line whose ends are grid points passes the
//!   same way, end by end: a segment is never farther from a plane than its farther end, and no
//!   double holds the squared normal of a long edge's plane, so an interval could not say 0. A
//!   plane not given by integers is unchecked in a body recorded Exact, and measured in certified
//!   intervals otherwise.
//! - **Exact on plane-based bodies.** A vertex where three planes meet lies exactly on an integer
//!   plane or it is a fault, in every class: `geom_num::plane::Plane::side_of_tpi`, the sign of a
//!   determinant within 384 bits (6.3, 8.3). A line with such an end is only a shadow of the edge,
//!   which is the two planes it lies on: the vertices decide what the edge lies on, and the line's
//!   end must be within a step of the vertex rounded to the grid, or of the body's bound.
//! - **Certified on lines and curved carriers.** Every other distance is a certified upper bound
//!   (`crate::curved`, and a line's end points enclosed with `geom_curve::tight`, exact where the
//!   doubles are), so a vertex exactly on the bound passes where the data are exact and one a step
//!   beyond fails.
//! - **Fails closed.** Every comparison whose operands cannot be formed (a value out of its range,
//!   an overflow, a cone past its apex) is a fault, never a pass.
//! - **No sampling, nothing allocated.** Each check is an exact predicate or one interval
//!   evaluation of a closed form.
//!
//! Method: chapter 8.3's group 2 and its certified-bounds rule
//! (`projects/engines/geometry/architecture/08-topology.md`), on the exact integer plane of 6.3
//! (`geom_num::plane`, after Campen and Kobbelt 2010, <https://doi.org/10.1111/j.1467-8659.2009.01609.x>)
//! and interval enclosures that stay exact where the doubles are (R. E. Moore, R. B. Kearfott and
//! M. J. Cloud, *Introduction to Interval Analysis*, SIAM 2009,
//! <https://doi.org/10.1137/1.9780898717716>).
//!
//! Cost: linear in faces, fins and edges, one meter unit each; constant work per row; nothing
//! allocated. Recursion: none.

use alloc::vec::Vec;

use geom_curve::tight;
use geom_curve::{Circle, Line, ParamCurve};
use geom_num::interval::Interval;
use geom_num::plane::{Plane as GridPlane, Tpi};
use geom_num::wide::I320;
use geom_num::{Coord, Exactness, P3, Sign};
use geom_op::run::reserve;
use geom_run::{LogicalSize, Stopped};
use geom_topo::entity::{Edge, Face, Fin, PcurveIx, PointKind, Vertex};
use geom_topo::ix::Arena;
use geom_topo::mass::Corner;
use geom_topo::{Arenas, Body, CurveKind, Ix, SurfaceKind};

use crate::combinatorial::{edge_fins, fin_edge, fin_ends, fin_face, rows};
use crate::curved::{self, Carrier, Pair};
use crate::general::{self, Verdict};
use crate::report::{Entity, Fault, FaultKind};
use crate::sink::Sink;

/// The largest grid coordinate, 2^40 − 1 steps (chapter 6.1).
const COORD_MAX: f64 = 1_099_511_627_775.0;

/// Group 2 on `body`, recorded `exactness`.
pub(crate) fn check(
    body: &Body,
    exactness: Exactness,
    sink: &mut Sink<'_, '_>,
) -> Result<(), Stopped> {
    let a = body.arenas();
    let b = bound(exactness);
    #[allow(
        clippy::cast_precision_loss,
        reason = "a bound past 2^53 steps exceeds every distance between points of the grid's range (below 2^42), so rounding it changes no answer"
    )]
    let limit = b as f64;
    let exact = matches!(exactness, Exactness::Exact);
    for v in rows::<Vertex>(a.rows(Arena::Vertex)) {
        sink.charge()?;
        if corner(a, v).is_none() {
            sink.push(Fault::at(FaultKind::MeetSingular, Entity::Vertex(v)));
        }
    }
    for f in rows::<Face>(a.rows(Arena::Face)) {
        sink.charge()?;
        if surface(a, f, exact).is_none() && any_kind(a, f, exact).is_none() {
            let at = a
                .faces
                .surface
                .get(f)
                .copied()
                .map_or(Entity::Face(f), Entity::Surface);
            sink.push(Fault::at(FaultKind::Unchecked, at).with(Entity::Face(f)));
        }
    }
    // Whether each fin's pcurve proved its edge on its face, in row order, for the edges on curves
    // no closed form covers (G177).
    let mut held: Vec<Held> = reserve(sink.meter(), u64::from(a.rows(Arena::Fin)))?;
    for n in rows::<Fin>(a.rows(Arena::Fin)) {
        sink.charge()?;
        // A fin's pcurve against its edge's curve, through its face's surface, on any kind (G177).
        let mapped = match a.fins.pcurve.get(n).copied().flatten() {
            Some(p) => {
                let verdict = pcurve_of(a, n, p, limit, sink)?;
                match verdict {
                    Verdict::Fails => sink.push(
                        Fault::at(FaultKind::PcurveOffCurve, Entity::Pcurve(p))
                            .with(Entity::Fin(n)),
                    ),
                    Verdict::Unknown => sink.push(
                        Fault::at(FaultKind::Unchecked, Entity::Pcurve(p)).with(Entity::Fin(n)),
                    ),
                    Verdict::Holds => {}
                }
                verdict
            }
            None => Verdict::Unknown,
        };
        held.push(Held(mapped == Verdict::Holds));
        let Some(face) = fin_face(a, n) else {
            continue;
        };
        let Some(on) = surface(a, face, exact) else {
            general_fin(a, (n, face), (limit, exact), mapped, sink)?;
            continue;
        };
        if let Some((v, _)) = fin_ends(a, n)
            && !on.holds_vertex(a, v, b, limit)
        {
            sink.push(
                Fault::at(FaultKind::VertexOffSurface, Entity::Vertex(v)).with(Entity::Face(face)),
            );
        }
        // Every fin of a published body names its edge: the filter decides, not the lookup.
        fin_edge(a, n)
            .filter(|&e| !edge_on(a, e, &on, b, limit))
            .into_iter()
            .for_each(|e| {
                sink.push(
                    Fault::at(FaultKind::EdgeOffSurface, Entity::Edge(e)).with(Entity::Face(face)),
                );
            });
    }
    for e in rows::<Edge>(a.rows(Arena::Edge)) {
        sink.charge()?;
        let c = a.edges.curve.get(e).copied();
        let ends = [
            a.edges.start.get(e).copied().flatten(),
            a.edges.end.get(e).copied().flatten(),
        ];
        let Some(curve) = Ends::of(c.and_then(|c| a.curves.get(c))) else {
            general_edge(a, e, (limit, &held), sink)?;
            continue;
        };
        let (lo, hi) = curve.domain();
        for (vertex, t) in ends.into_iter().zip([lo, hi]) {
            if let Some(v) = vertex
                && !curve.holds_vertex(a, v, t, b, limit)
            {
                sink.push(
                    Fault::at(FaultKind::VertexOffCurve, Entity::Vertex(v)).with(Entity::Edge(e)),
                );
            }
        }
    }
    Ok(())
}

/// Whether a fin's pcurve proved its edge on its face (G177).
#[derive(Clone, Copy)]
struct Held(bool);

impl LogicalSize for Held {
    /// One byte.
    const LOGICAL_BYTES: u64 = 1;
}

/// Face `f`'s surface when no closed form covers it, for the general checks (G177): any kind but a
/// plane not given by integers in a body recorded Exact, which stays unchecked (G134).
fn any_kind(a: &Arenas, f: Ix<Face>, exact: bool) -> Option<&SurfaceKind> {
    let kind = a.faces.surface.get(f).and_then(|s| a.surfaces.get(*s))?;
    (!(exact && matches!(kind, SurfaceKind::Plane(_)))).then_some(kind)
}

/// Where vertex `v` lies, exactly, and the step more a vertex where three planes meet takes when
/// it is rounded to the grid; `None` for three planes that do not meet.
fn placed(a: &Arenas, v: Ix<Vertex>) -> Option<([Interval; 3], f64)> {
    match corner(a, v)? {
        Corner::Grid(p) => curved::grid(p).map(|x| (x, 0.0)),
        Corner::Tpi(meeting) => {
            let rounded = meeting.to_grid().ok()?;
            curved::grid(rounded.value()).map(|x| (x, 1.0))
        }
        Corner::Lpi(_) => None,
    }
}

/// Fin `n`'s pcurve against its edge's curve through its face's surface, within `limit`.
fn pcurve_of(
    a: &Arenas,
    n: Ix<Fin>,
    pcurve: PcurveIx,
    limit: f64,
    sink: &mut Sink<'_, '_>,
) -> Result<Verdict, Stopped> {
    let surface = fin_face(a, n)
        .and_then(|f| a.faces.surface.get(f))
        .and_then(|s| a.surfaces.get(*s));
    let curve = fin_edge(a, n)
        .and_then(|e| a.edges.curve.get(e))
        .and_then(|c| a.curves.get(*c));
    match (surface, a.pcurves.get(pcurve), curve) {
        (Some(s), Some(pc), Some(c)) => general::pcurve_on(s, pc, c, limit, sink.meter()),
        _ => Ok(Verdict::Unknown),
    }
}

/// Group 2 for a fin on a face no closed form covers (G177): its start on the face's surface, by
/// the surface's nearest points, and its edge on the face, which its pcurve proves when `mapped`
/// holds and which is unchecked otherwise.
fn general_fin(
    a: &Arenas,
    (n, face): (Ix<Fin>, Ix<Face>),
    (limit, exact): (f64, bool),
    mapped: Verdict,
    sink: &mut Sink<'_, '_>,
) -> Result<(), Stopped> {
    let Some(kind) = any_kind(a, face, exact) else {
        return Ok(());
    };
    if let Some((v, _)) = fin_ends(a, n) {
        let verdict = match placed(a, v) {
            Some((place, more)) => {
                general::vertex_on_surface(kind, &place, limit + more, sink.meter())?
            }
            None => Verdict::Unknown,
        };
        match verdict {
            Verdict::Fails => sink.push(
                Fault::at(FaultKind::VertexOffSurface, Entity::Vertex(v)).with(Entity::Face(face)),
            ),
            Verdict::Unknown => sink
                .push(Fault::at(FaultKind::Unchecked, Entity::Vertex(v)).with(Entity::Face(face))),
            Verdict::Holds => {}
        }
    }
    // Once per edge and face: an edge a face uses twice, as a seam, is one edge on it.
    if mapped != Verdict::Holds
        && let Some(edge) = fin_edge(a, n)
        && !edge_fins(a, edge).any(|m| m.to_usize() < n.to_usize() && fin_face(a, m) == Some(face))
    {
        let at = a
            .faces
            .surface
            .get(face)
            .copied()
            .map_or(Entity::Face(face), Entity::Surface);
        sink.push(Fault::at(FaultKind::Unchecked, at).with(Entity::Edge(edge)));
    }
    Ok(())
}

/// Group 2 for an edge on a curve no closed form covers (G177): each vertex at its end of the
/// curve, by the curve's enclosure there, and the curve on its faces, which every fin's pcurve
/// proves when each held (`held`, by fin row) and which is reported unchecked, once, otherwise.
fn general_edge(
    a: &Arenas,
    e: Ix<Edge>,
    (limit, held): (f64, &[Held]),
    sink: &mut Sink<'_, '_>,
) -> Result<(), Stopped> {
    let ix = a.edges.curve.get(e).copied();
    let Some(curve) = ix.and_then(|c| a.curves.get(c)) else {
        sink.push(Fault::at(FaultKind::Unchecked, Entity::Edge(e)).with(Entity::Edge(e)));
        return Ok(());
    };
    let (lo, hi) = curve.domain();
    let ends = [
        a.edges.start.get(e).copied().flatten(),
        a.edges.end.get(e).copied().flatten(),
    ];
    let mut unchecked = edge_fins(a, e).any(|n| !held.get(n.to_usize()).is_some_and(|h| h.0));
    for (vertex, t) in ends.into_iter().zip([lo, hi]) {
        let Some(v) = vertex else {
            continue;
        };
        let verdict = match placed(a, v) {
            Some((place, more)) => {
                general::vertex_at(curve, t, &place, limit + more, sink.meter())?
            }
            None => Verdict::Unknown,
        };
        match verdict {
            Verdict::Fails => sink.push(
                Fault::at(FaultKind::VertexOffCurve, Entity::Vertex(v)).with(Entity::Edge(e)),
            ),
            Verdict::Unknown => unchecked = true,
            Verdict::Holds => {}
        }
    }
    if unchecked {
        let at = ix.map_or(Entity::Edge(e), Entity::Curve);
        sink.push(Fault::at(FaultKind::Unchecked, at).with(Entity::Edge(e)));
    }
    Ok(())
}

/// An edge's curve, of a kind whose ends group 2 can certify.
pub(crate) enum Ends<'c> {
    Line(&'c Line<3>),
    Circle(&'c Circle<3>),
}

impl<'c> Ends<'c> {
    /// The curve `curve` names, if group 2 can certify its kind.
    pub(crate) fn of(curve: Option<&'c CurveKind<3>>) -> Option<Ends<'c>> {
        match curve? {
            CurveKind::Line(line) => Some(Ends::Line(line)),
            CurveKind::Circle(circle) => Some(Ends::Circle(circle)),
            _ => None,
        }
    }

    /// The parameters of the curve's two ends.
    fn domain(&self) -> (f64, f64) {
        match self {
            Ends::Line(line) => line.domain(),
            Ends::Circle(circle) => circle.domain(),
        }
    }

    /// Whether `p` lies within `b` steps (`limit` as a double) of the curve's point at `t`.
    fn holds(&self, t: f64, p: P3, b: u64, limit: f64) -> bool {
        match self {
            Ends::Line(line) => on_line(line.origin(), line.direction(), t, p, b),
            Ends::Circle(circle) => curved::at_circle_end(circle, t, p, limit),
        }
    }

    /// Whether vertex `v` lies at the curve's point at `t`, within `b` steps (`limit` as a double);
    /// a step more for a vertex where three planes meet, which the curve only shadows.
    fn holds_vertex(&self, a: &Arenas, v: Ix<Vertex>, t: f64, b: u64, limit: f64) -> bool {
        match corner(a, v) {
            Some(Corner::Grid(place)) => self.holds(t, place, b, limit),
            Some(Corner::Tpi(meeting)) => meeting
                .to_grid()
                .is_ok_and(|rounded| self.holds(t, rounded.value(), b.saturating_add(1), limit)),
            _ => false,
        }
    }
}

/// What group 2 measures a face's vertices and edges against: its surface's closed form, and the
/// exact integer plane when the surface is a plane given by integers.
pub(crate) struct Surface {
    pub(crate) grid: Option<GridPlane>,
    pub(crate) carrier: Carrier,
}

impl Surface {
    /// Whether `p` lies within `b` steps (`limit` as a double) of the surface: exactly on an
    /// integer plane, by a certified bound otherwise.
    fn holds_point(&self, p: P3, b: u64, limit: f64) -> bool {
        match &self.grid {
            Some(plane) => on_plane(plane, p, b),
            None => self.carrier.holds_point(p, limit),
        }
    }

    /// Whether vertex `v` lies within `b` steps (`limit` as a double) of the surface: a vertex
    /// where three planes meet exactly on an integer plane, in any class, and on no other surface.
    fn holds_vertex(&self, a: &Arenas, v: Ix<Vertex>, b: u64, limit: f64) -> bool {
        match corner(a, v) {
            Some(Corner::Grid(p)) => self.holds_point(p, b, limit),
            Some(Corner::Tpi(meeting)) => self
                .grid
                .as_ref()
                .is_some_and(|plane| plane.side_of_tpi(&meeting) == Ok(Sign::Zero)),
            _ => false,
        }
    }

    /// Whether every point of `line`'s segment lies within `b` steps (`limit` as a double) of the
    /// surface: exactly, by its ends, on an integer plane when they are grid points; by a certified
    /// bound otherwise.
    fn holds_line(&self, line: &Line<3>, b: u64, limit: f64) -> bool {
        let (lo, hi) = line.domain();
        match (&self.grid, line_end(line, lo).zip(line_end(line, hi))) {
            (Some(plane), Some((start, end))) => {
                on_plane(plane, start, b) && on_plane(plane, end, b)
            }
            _ => self.carrier.holds_line(line, limit),
        }
    }

    /// How this surface and `other` lie, as far as group 3 can certify.
    pub(crate) fn lies_with(&self, other: &Surface) -> Pair {
        curved::pair(&self.carrier, &other.carrier)
    }
}

/// How two integer planes lie, exactly: crossing when their normals' cross product is not zero;
/// parallel otherwise, and apart unless each is the other, which is left unknown. Every normal
/// coordinate and offset is an `i128`, so each product and difference fits 320 bits.
pub(crate) fn planes_lie(p: &GridPlane, q: &GridPlane) -> Pair {
    let [px, py, pz] = p.n().map(I320::from_i128);
    let [qx, qy, qz] = q.n().map(I320::from_i128);
    let (pd, qd) = (I320::from_i128(p.d()), I320::from_i128(q.d()));
    let zero = I320::from_i128(0);
    let minus = |first: &I320, by: &I320, second: &I320, and: &I320| -> Option<I320> {
        first
            .checked_mul(by)?
            .checked_sub(&second.checked_mul(and)?)
    };
    let turn = [
        minus(&py, &qz, &pz, &qy),
        minus(&pz, &qx, &px, &qz),
        minus(&px, &qy, &py, &qx),
    ];
    let same = [(&px, &qx), (&py, &qy), (&pz, &qz)]
        .into_iter()
        .all(|(pn, qn)| minus(&pd, qn, &qd, pn) == Some(zero));
    if turn.iter().any(|t| t.as_ref() != Some(&zero)) {
        Pair::Crossing
    } else if same {
        Pair::Unknown
    } else {
        Pair::Apart
    }
}

/// `line`'s point at parameter `t`, when it is exactly a grid point.
fn line_end(line: &Line<3>, t: f64) -> Option<P3> {
    let [ox, oy, oz] = line.origin();
    let [dx, dy, dz] = line.direction();
    let at = |start: f64, step: f64| -> Option<i64> {
        let along = tight::mul(tight::point(t).ok()?, tight::point(step).ok()?).ok()?;
        let end = tight::add(tight::point(start).ok()?, along).ok()?;
        (end.lo() == end.hi()).then(|| grid(end.lo())).flatten()
    };
    P3::new(at(ox, dx)?, at(oy, dy)?, at(oz, dz)?).ok()
}

/// The surface of `face`, if group 2 has a certified check for it: not for a kind without a closed
/// form here, nor, in a body recorded Exact (`exact`), for a plane not given by integers.
pub(crate) fn surface(a: &Arenas, face: Ix<Face>, exact: bool) -> Option<Surface> {
    let kind = a.faces.surface.get(face).and_then(|s| a.surfaces.get(*s))?;
    let grid = plane(kind);
    if exact && grid.is_none() && matches!(kind, SurfaceKind::Plane(_)) {
        return None;
    }
    Some(Surface {
        grid,
        carrier: curved::carrier(kind)?,
    })
}

/// Whether edge `e`'s curve lies within `b` steps (`limit` as a double) of `on`: a line as
/// [`Surface::holds_line`] says, a circle by its certified bound; any other curve is the edge rows'
/// to report unchecked, so it is not reported again here. A straight edge with an end where three
/// planes meet lies on a plane exactly when its ends do, and the fins' vertices are tested already.
fn edge_on(a: &Arenas, e: Ix<Edge>, on: &Surface, b: u64, limit: f64) -> bool {
    match Ends::of(a.edges.curve.get(e).and_then(|c| a.curves.get(*c))) {
        Some(Ends::Line(_)) if implicit_end(a, e) => true,
        Some(Ends::Line(line)) => on.holds_line(line, b, limit),
        Some(Ends::Circle(circle)) => on.carrier.holds_circle(circle, limit),
        None => true,
    }
}

/// The bound a vertex may lie from its carriers, in steps: 0 for an Exact body.
pub(crate) const fn bound(exactness: Exactness) -> u64 {
    match exactness {
        Exactness::Exact => 0,
        Exactness::Certified { bound, .. } => bound.get(),
        Exactness::Approximate { deviation, .. } => deviation.get(),
    }
}

/// Where vertex `v` lies: `None` for three planes that do not meet in one point.
pub(crate) fn corner(a: &Arenas, v: Ix<Vertex>) -> Option<Corner> {
    match a.vertices.point.get(v).and_then(|p| a.points.get(*p))? {
        PointKind::Grid(p) => Some(Corner::Grid(*p)),
        PointKind::Meet(planes) => {
            let [first, second, third] = planes.map(|ix| a.planes.get(ix).copied());
            Tpi::new(first?, second?, third?).ok().map(Corner::Tpi)
        }
    }
}

/// Whether edge `e` has an end where three planes meet.
fn implicit_end(a: &Arenas, e: Ix<Edge>) -> bool {
    [a.edges.start.get(e), a.edges.end.get(e)]
        .into_iter()
        .flatten()
        .flatten()
        .any(|v| !matches!(corner(a, *v), Some(Corner::Grid(_))))
}

/// The integer plane of a surface that is a plane given by integers: its origin and two spanning
/// vectors, every coordinate a whole number of steps within the grid's range.
pub(crate) fn plane(surface: &SurfaceKind) -> Option<GridPlane> {
    let SurfaceKind::Plane(flat) = surface else {
        return None;
    };
    let (origin, along, across) = flat.vectors();
    let on_grid = |w: [f64; 3]| -> Option<P3> {
        let [px, py, pz] = w.map(grid);
        P3::new(px?, py?, pz?).ok()
    };
    let offset = |w: [f64; 3]| -> [f64; 3] {
        let [ox, oy, oz] = origin;
        let [wx, wy, wz] = w;
        [ox + wx, oy + wy, oz + wz]
    };
    GridPlane::through(
        on_grid(origin)?,
        on_grid(offset(along))?,
        on_grid(offset(across))?,
    )
    .ok()
}

/// A double that is a whole number of steps within the grid's range, as that number.
fn grid(x: f64) -> Option<i64> {
    if !(-COORD_MAX..=COORD_MAX).contains(&x) {
        return None;
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "|x| <= 2^40 - 1, so the integer part fits; the comparison below keeps x only if it is whole"
    )]
    let i = x as i64;
    #[allow(
        clippy::cast_precision_loss,
        reason = "|i| < 2^53, so the double holds it exactly"
    )]
    let back = i as f64;
    (back == x).then_some(i)
}

/// Whether `p` lies within `b` steps of the plane: (n · p + d)^2 ≤ b^2 · |n|^2, exactly.
fn on_plane(plane: &GridPlane, p: P3, b: u64) -> bool {
    let n = plane.n();
    let s = [(n[0], p.x), (n[1], p.y), (n[2], p.z)]
        .into_iter()
        .try_fold(plane.d(), |acc, (ni, c)| {
            ni.checked_mul(i128::from(c.get()))
                .and_then(|t| acc.checked_add(t))
        });
    let wide = |x: i128| I320::from_i128(x);
    let square = |x: &I320| x.checked_mul(x);
    let lhs = s.map(wide).as_ref().and_then(square);
    let norm = n.iter().try_fold(I320::from_i128(0), |acc, &ni| {
        square(&wide(ni)).and_then(|t| acc.checked_add(&t))
    });
    let rhs = square(&wide(i128::from(b)))
        .zip(norm)
        .and_then(|(b2, n2)| b2.checked_mul(&n2));
    lhs.zip(rhs).is_some_and(|(l, r)| l <= r)
}

/// Whether `vertex` lies within `limit_steps` of the line's point at parameter `param`,
/// origin + param · direction: the certified upper bound of the squared distance is at most the
/// certified lower bound of the squared limit. Nothing allocates.
fn on_line(
    origin: [f64; 3],
    direction: [f64; 3],
    param: f64,
    vertex: P3,
    limit_steps: u64,
) -> bool {
    let gap = |start: f64, step: f64, at: Coord| -> Option<Interval> {
        let along = tight::mul(tight::point(param).ok()?, tight::point(step).ok()?).ok()?;
        let on_line = tight::add(tight::point(start).ok()?, along).ok()?;
        #[allow(
            clippy::cast_precision_loss,
            reason = "a grid coordinate is below 2^40, so the double holds it exactly"
        )]
        let target = at.get() as f64;
        tight::square(tight::sub(on_line, tight::point(target).ok()?).ok()?).ok()
    };
    #[allow(
        clippy::cast_precision_loss,
        reason = "a bound past 2^53 steps exceeds every distance between points of the grid's range (below 2^42), so rounding it changes no answer"
    )]
    let limit = limit_steps as f64;
    let [start_x, start_y, start_z] = origin;
    let [step_x, step_y, step_z] = direction;
    let squared = gap(start_x, step_x, vertex.x)
        .zip(gap(start_y, step_y, vertex.y))
        .zip(gap(start_z, step_z, vertex.z))
        .and_then(|((gx, gy), gz)| tight::add(tight::add(gx, gy).ok()?, gz).ok());
    let allowed = tight::point(limit).and_then(tight::square).ok();
    squared
        .zip(allowed)
        .is_some_and(|(have, room)| have.hi() <= room.lo())
}
