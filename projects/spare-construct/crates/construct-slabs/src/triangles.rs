//! An exact constrained Delaunay triangulation of grid points in plan (job 14.15): what divides a
//! shape-edited floor's top face into the triangles its points and split lines define
//! ([`crate::top`]).
//!
//! What it does: [`triangulate`] takes grid points, the segments between them that must be edges
//! (the floor's own edges and its split lines, each already cut at every point that lies on it,
//! none crossing another), and a test of which side of those edges is the floor; it returns the
//! triangles over the floor, each anticlockwise seen from above, whose edges include every segment
//! and which are otherwise as near equal-sided as the segments allow (no triangle's circle holds a
//! point it can see across an edge that is not a segment).
//!
//! Promises: exact, every decision a sign of a whole-number determinant: the orientation of three
//! points in 128 bits (coordinates within the grid's 2^40 steps, a big enclosing triangle within
//! 2^43) and the circle test in num-bigint's integers, nothing compared with a tolerance and no
//! point moved or made. The same points and segments in the same order give the same triangles in
//! the same order on every machine and every run; four points on one circle (a rectangle's
//! corners) are divided as the order of insertion leaves them, which is fixed by the input's order.
//! Nothing panics: a triangulation that cannot finish within its work budget is `None`, which the
//! caller refuses in words.
//!
//! Method: points inserted one by one into a triangle that encloses the grid, each splitting the
//! triangle or edge it falls in; each segment then made an edge by flipping the edges it crosses
//! (Sloan, "A fast algorithm for generating constrained Delaunay triangulations", Computers &
//! Structures 47(3), 1993); the triangles outside the floor taken away (each judged by its
//! centroid, which no segment passes through); and every edge that is not a segment flipped while
//! the point across it lies inside its triangle's circle (Lawson's flips, which end at the
//! constrained Delaunay triangulation). Small inputs only (a floor's few hundred points at most,
//! [`crate::top`]'s limit), so every search is a plain scan.
//!
//! Interim: the kernel has its own exact constrained Delaunay triangulation (geom-mesh's
//! `triangulate`, piece G153), internal to the kernel and not on the facade (engine-geometry) that
//! programs may use; this one is our own, with the same exact rules, until the facade offers it.
//! It is behind [`crate::top::Top`], so replacing it changes this file and no caller (the interim
//! register's row "construct-slabs triangles").

#![allow(
    clippy::many_single_char_names,
    reason = "points and triangles' corners named as the papers name them: a, b, c, d, p"
)]

use construct_solver::Point;
use num_bigint::BigInt;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Half the side of the square the grid's points lie in: 2^40 steps.
const HALF: i64 = 1 << 40;

/// What a flip of an edge did.
enum Flip {
    /// The edge became the other diagonal, `c`-`d`.
    Done(usize, usize),
    /// The four corners are not a strictly convex quadrilateral, so the edge stays.
    Stays,
}

/// The work a triangulation may take: point insertions, flips and segment steps, together.
const WORK: u64 = 50_000_000;

/// The sign of the orientation of `a`, `b`, `c`: above 0 when they turn anticlockwise.
fn orient(a: Point, b: Point, c: Point) -> i128 {
    let (ax, ay) = (i128::from(a.x), i128::from(a.y));
    let ux = i128::from(b.x).saturating_sub(ax);
    let uy = i128::from(b.y).saturating_sub(ay);
    let vx = i128::from(c.x).saturating_sub(ax);
    let vy = i128::from(c.y).saturating_sub(ay);
    // Within 2^44 each, so each product is under 2^88 and the difference fits.
    ux.saturating_mul(vy)
        .saturating_sub(uy.saturating_mul(vx))
        .signum()
}

/// Whether `d` lies strictly inside the circle through `a`, `b`, `c` (anticlockwise).
#[allow(
    clippy::arithmetic_side_effects,
    reason = "arbitrary-precision integers cannot overflow"
)]
fn in_circle(a: Point, b: Point, c: Point, d: Point) -> bool {
    let rel = |p: Point| {
        let x = BigInt::from(p.x) - BigInt::from(d.x);
        let y = BigInt::from(p.y) - BigInt::from(d.y);
        let lift = &x * &x + &y * &y;
        (x, y, lift)
    };
    let (ax, ay, al) = rel(a);
    let (bx, by, bl) = rel(b);
    let (cx, cy, cl) = rel(c);
    let det =
        al * (&bx * &cy - &cx * &by) + bl * (&cx * &ay - &ax * &cy) + cl * (&ax * &by - &bx * &ay);
    det > BigInt::from(0)
}

/// Whether the segments `a`-`b` and `c`-`d` cross at one point inside both.
fn cross_properly(a: Point, b: Point, c: Point, d: Point) -> bool {
    let (d1, d2) = (orient(a, b, c), orient(a, b, d));
    let (d3, d4) = (orient(c, d, a), orient(c, d, b));
    d1.saturating_mul(d2) < 0 && d3.saturating_mul(d4) < 0
}

/// An undirected edge, its lower index first.
fn key(a: usize, b: usize) -> (usize, usize) {
    if a <= b { (a, b) } else { (b, a) }
}

/// The triangulation being built: the points (the caller's, then the enclosing triangle's three),
/// the triangles (anticlockwise, `None` once taken away), and which triangle has each directed
/// edge.
struct Mesh {
    points: Vec<Point>,
    triangles: Vec<Option<[usize; 3]>>,
    edges: BTreeMap<(usize, usize), usize>,
    work: u64,
}

impl Mesh {
    fn point(&self, i: usize) -> Option<Point> {
        self.points.get(i).copied()
    }

    fn spend(&mut self) -> Option<()> {
        self.work = self.work.checked_add(1)?;
        (self.work <= WORK).then_some(())
    }

    fn add(&mut self, t: [usize; 3]) -> usize {
        let id = self.triangles.len();
        let [a, b, c] = t;
        for edge in [(a, b), (b, c), (c, a)] {
            self.edges.insert(edge, id);
        }
        self.triangles.push(Some(t));
        id
    }

    fn take(&mut self, id: usize) -> Option<[usize; 3]> {
        let t = self.triangles.get_mut(id)?.take()?;
        let [a, b, c] = t;
        for edge in [(a, b), (b, c), (c, a)] {
            if self.edges.get(&edge) == Some(&id) {
                self.edges.remove(&edge);
            }
        }
        Some(t)
    }

    /// The triangle with the directed edge `a`-`b`, and its third corner.
    fn across(&self, a: usize, b: usize) -> Option<(usize, usize)> {
        let id = *self.edges.get(&(a, b))?;
        let [p, q, r] = (*self.triangles.get(id)?)?;
        let third = if (p, q) == (a, b) {
            r
        } else if (q, r) == (a, b) {
            p
        } else {
            q
        };
        Some((id, third))
    }

    fn orient_of(&self, a: usize, b: usize, c: usize) -> Option<i128> {
        Some(orient(self.point(a)?, self.point(b)?, self.point(c)?))
    }

    /// Puts point `p` in: the triangle it is inside becomes three, or the two either side of the
    /// edge it is on become four.
    fn insert(&mut self, p: usize) -> Option<()> {
        self.spend()?;
        let at = self.point(p)?;
        let mut found = None;
        for (id, t) in self.triangles.iter().enumerate() {
            let Some([a, b, c]) = *t else { continue };
            let (Some(pa), Some(pb), Some(pc)) = (self.point(a), self.point(b), self.point(c))
            else {
                return None;
            };
            let sides = [orient(pa, pb, at), orient(pb, pc, at), orient(pc, pa, at)];
            if sides.iter().all(|s| *s >= 0) {
                found = Some((id, [a, b, c], sides));
                break;
            }
        }
        let (id, [a, b, c], sides) = found?;
        // On an edge: the corners rotated so the edge is the first two.
        let on = sides.iter().position(|s| *s == 0);
        self.take(id)?;
        match on {
            None => {
                self.add([a, b, p]);
                self.add([b, c, p]);
                self.add([c, a, p]);
            }
            Some(k) => {
                let (u, v, w) = match k {
                    0 => (a, b, c),
                    1 => (b, c, a),
                    _ => (c, a, b),
                };
                self.add([u, p, w]);
                self.add([p, v, w]);
                if let Some((other, x)) = self.across(v, u) {
                    self.take(other)?;
                    self.add([v, p, x]);
                    self.add([p, u, x]);
                }
            }
        }
        Some(())
    }

    /// Flips the edge `a`-`b` between its two triangles, when the four corners make a strictly
    /// convex quadrilateral; `None` when the edge has not two triangles or the budget is spent.
    fn flip(&mut self, a: usize, b: usize) -> Option<Flip> {
        let (t1, c) = self.across(a, b)?;
        let (t2, d) = self.across(b, a)?;
        if self.orient_of(c, a, d)? <= 0 || self.orient_of(d, b, c)? <= 0 {
            return Some(Flip::Stays);
        }
        self.spend()?;
        self.take(t1)?;
        self.take(t2)?;
        self.add([c, a, d]);
        self.add([d, b, c]);
        Some(Flip::Done(c, d))
    }

    /// Makes `a`-`b` an edge by flipping every edge that crosses it.
    fn enforce(&mut self, a: usize, b: usize) -> Option<()> {
        if self.edges.contains_key(&(a, b)) || self.edges.contains_key(&(b, a)) {
            return Some(());
        }
        let (pa, pb) = (self.point(a)?, self.point(b)?);
        let undirected: BTreeSet<(usize, usize)> =
            self.edges.keys().map(|&(u, v)| key(u, v)).collect();
        let mut crossing = VecDeque::new();
        for (u, v) in undirected {
            let (pu, pv) = (self.point(u)?, self.point(v)?);
            if cross_properly(pa, pb, pu, pv) {
                crossing.push_back((u, v));
            }
        }
        while let Some((u, v)) = crossing.pop_front() {
            self.spend()?;
            match self.flip(u, v)? {
                Flip::Done(c, d) => {
                    let (pc, pd) = (self.point(c)?, self.point(d)?);
                    if cross_properly(pa, pb, pc, pd) {
                        crossing.push_back((c, d));
                    }
                }
                Flip::Stays => crossing.push_back((u, v)),
            }
        }
        (self.edges.contains_key(&(a, b)) || self.edges.contains_key(&(b, a))).then_some(())
    }

    /// Flips every edge that is not a segment while the point across it lies inside its
    /// triangle's circle, until none does.
    fn settle(&mut self, fixed: &BTreeSet<(usize, usize)>) -> Option<()> {
        loop {
            let mut flipped = false;
            let inner: Vec<(usize, usize)> = self
                .edges
                .keys()
                .copied()
                .filter(|&(u, v)| u < v && !fixed.contains(&(u, v)))
                .collect();
            for (u, v) in inner {
                let (Some((_, c)), Some((_, d))) = (self.across(u, v), self.across(v, u)) else {
                    continue;
                };
                let (pu, pv, pc, pd) = (
                    self.point(u)?,
                    self.point(v)?,
                    self.point(c)?,
                    self.point(d)?,
                );
                if in_circle(pu, pv, pc, pd) && matches!(self.flip(u, v)?, Flip::Done(..)) {
                    flipped = true;
                }
            }
            if !flipped {
                return Some(());
            }
        }
    }
}

/// The triangles over a floor, as the module header says: `points` (each within the grid, no two
/// alike), `segments` (pairs of their indices; none with a point inside it, none crossing
/// another), and `inside`, which says whether a point given at three times its coordinates (a
/// triangle's corners summed) is on the floor. Each triangle is three indices of `points`,
/// anticlockwise, its least index first, and the triangles are in order. `None` when the budget
/// runs out or a segment cannot be made an edge (inputs that break the rules above).
pub(crate) fn triangulate(
    points: &[Point],
    segments: &[(usize, usize)],
    inside: impl Fn(Point) -> bool,
) -> Option<Vec<[usize; 3]>> {
    let n = points.len();
    let near = HALF.checked_mul(2)?.checked_neg()?;
    let far = HALF.checked_mul(6)?;
    let mut all = points.to_vec();
    all.push(Point { x: near, y: near });
    all.push(Point { x: far, y: near });
    all.push(Point { x: near, y: far });
    let mut mesh = Mesh {
        points: all,
        triangles: Vec::new(),
        edges: BTreeMap::new(),
        work: 0,
    };
    mesh.add([n, n.checked_add(1)?, n.checked_add(2)?]);
    for p in 0..n {
        mesh.insert(p)?;
    }
    let mut fixed = BTreeSet::new();
    for &(a, b) in segments {
        if a >= n || b >= n || a == b {
            return None;
        }
        mesh.enforce(a, b)?;
        fixed.insert(key(a, b));
    }
    for id in 0..mesh.triangles.len() {
        let Some(Some([a, b, c])) = mesh.triangles.get(id).copied() else {
            continue;
        };
        let keep = a < n && b < n && c < n && {
            let (pa, pb, pc) = (mesh.point(a)?, mesh.point(b)?, mesh.point(c)?);
            inside(Point {
                x: pa.x.checked_add(pb.x)?.checked_add(pc.x)?,
                y: pa.y.checked_add(pb.y)?.checked_add(pc.y)?,
            })
        };
        if !keep {
            mesh.take(id)?;
        }
    }
    mesh.settle(&fixed)?;
    let mut out: Vec<[usize; 3]> = mesh
        .triangles
        .iter()
        .flatten()
        .map(|&[a, b, c]| {
            if a <= b && a <= c {
                [a, b, c]
            } else if b <= a && b <= c {
                [b, c, a]
            } else {
                [c, a, b]
            }
        })
        .collect();
    out.sort_unstable();
    Some(out)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]
mod tests {
    use super::*;

    fn p(x: i64, y: i64) -> Point {
        Point { x, y }
    }

    /// A square with a point in its middle is four triangles round the point.
    #[test]
    fn a_square_with_a_middle_point_is_four_triangles() {
        let points = [p(0, 0), p(10, 0), p(10, 10), p(0, 10), p(5, 5)];
        let segments = [(0, 1), (1, 2), (2, 3), (3, 0)];
        let inside = |c: Point| c.x > 0 && c.x < 30 && c.y > 0 && c.y < 30;
        let t = triangulate(&points, &segments, inside).unwrap();
        assert_eq!(t, [[0, 1, 4], [0, 4, 3], [1, 2, 4], [2, 3, 4]]);
    }

    /// A segment across the square is kept as an edge, whatever the circles say.
    #[test]
    fn a_segment_is_kept() {
        // A long thin quadrilateral whose Delaunay diagonal is 1-3; the segment asks for 0-2.
        let points = [p(0, 0), p(20, -1), p(40, 0), p(20, 1)];
        let segments = [(0, 1), (1, 2), (2, 3), (3, 0), (0, 2)];
        let inside = |_: Point| true;
        let t = triangulate(&points, &segments, inside).unwrap();
        assert_eq!(t, [[0, 1, 2], [0, 2, 3]]);
        let free = triangulate(&points, &segments[..4], inside).unwrap();
        assert_eq!(free, [[0, 1, 3], [1, 2, 3]]);
    }

    /// What lies outside the floor (a notch, a hole) is taken away.
    #[test]
    fn what_is_outside_is_taken_away() {
        // An L: the square (0,0)-(20,20) less its top right quarter.
        let points = [p(0, 0), p(20, 0), p(20, 10), p(10, 10), p(10, 20), p(0, 20)];
        let segments = [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 0)];
        let inside = |c: Point| !(c.x > 30 && c.y > 30);
        let t = triangulate(&points, &segments, inside).unwrap();
        assert_eq!(t.len(), 4);
        assert!(t.iter().all(|tri| !tri.contains(&usize::MAX)));
    }
}
