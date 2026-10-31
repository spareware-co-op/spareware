//! The kernel's one exact constrained Delaunay triangulation (piece G153; chapters 17.3 and 19.3).
//!
//! What it does: [`triangulate`] takes grid points in the plane, each with a key from the caller,
//! and constraints between them, and returns the triangles that keep every constraint as edges
//! and are otherwise as close to equal-sided as the points allow. Constraints that cross are cut
//! where they cross, each crossing constructed exactly from the two constraints' ends and committed
//! to the grid before any circle test, so every circle test runs on grid points. With a
//! [`Quality`] it then adds points until every triangle meets it.
//!
//! Guarantees:
//!
//! - **One answer.** Every tie breaks by the caller's keys: four points exactly on one circle are
//!   separated by Simulation of Simplicity in key order, never by insertion order or address. So
//!   the triangles depend only on the points, keys and constraints: shuffling the input changes
//!   no byte of the result, which is in canonical order ([`Cdt`]). Cocircular grids are the hard
//!   case: tests hold their results to a golden digest, the same on every target.
//! - **Exact.** Every decision is `geom-num`'s exact `orient2d` or `incircle`. Nothing is
//!   compared against a tolerance, and no point moves but a crossing, by the one rounding of
//!   [`Ssi::to_grid`].
//! - **Constraints kept.** Every constraint is a chain of edges of the result. A point exactly on a
//!   constraint splits it there; collinear constraints that overlap share their pieces.
//! - **Crossings gone before the first circle test.** After [`commit_crossings`] no two constraints
//!   cross in their interiors. A rounded crossing bends its two constraints a little, which can
//!   make a new crossing with a third; that one is committed in the next round, and rounds go on
//!   until none is left. Each round works on the whole set in rank order, so the result does not
//!   depend on the input's order.
//! - **Metered.** One work unit for each point, each flip and each constraint piece, and each
//!   round of crossings charges one for each constraint. A stopped meter stops the triangulation.
//!
//! Method: Bowyer and Watson's incremental insertion in Lawson's flip form (Lawson, "Software for
//! C¹ surface interpolation", 1977); constraints by flips, Sloan's method ("A fast algorithm for
//! generating constrained Delaunay triangulations", Computers & Structures 47(3), 441-450, 1993,
//! <https://doi.org/10.1016/0045-7949(93)90239-A>), as Geogram's `CDT_2d` does. A crossing from the
//! two original constraint ends, as its `create_intersection` does. Unlike it, a tie is broken by Simulation of
//! Simplicity (Edelsbrunner and Mücke, 1990, <https://doi.org/10.1145/77635.77639>, in the form
//! Devillers and Teillaud give for Delaunay, 2011, <https://doi.org/10.1016/j.comgeo.2010.09.010>):
//! each lifted height is raised by an infinitesimal that is larger the lower the vertex's rank, so
//! the sign of a tie is the sign of the cofactor of the lowest-ranked vertex. Three distinct
//! points on a circle are never collinear, so that cofactor is never zero, and the triangulation of
//! generic lifted points is unique, which is why the insertion order cannot show. Refinement is
//! Ruppert's ("A Delaunay refinement algorithm for quality 2-dimensional mesh generation", 1995,
//! <https://doi.org/10.1006/jagm.1995.1021>) with Shewchuk's priorities (2002,
//! <https://doi.org/10.1016/S0925-7721(01)00047-5>): encroached segments first, then bad
//! triangles, in canonical order. Geometry beyond the hull is handled with one ghost vertex, as in
//! CGAL and Shewchuk's Triangle: every hull edge has a ghost triangle, so neighbours always exist.
//!
//! Consulted: Geogram `CDT_2d`, BSD-3-Clause,
//!  <https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/delaunay/CDT_2d.cpp,>
//!  for Sloan's flip insertion of constraints and the exact construction of a crossing from the
//!  two original constraint ends; its method, not its code.
//!
//! Cost: Points are inserted along a Morton curve, each walked to from the last, O(n log n)
//! expected on points that are not adversarial (sorting, then a short walk and a few flips each).
//! Each constraint costs its length in triangles. Crossings cost a box tree of the constraints per
//! round. Nothing recurses.

#![allow(
    clippy::many_single_char_names,
    reason = "vertices, triangles and slots, named as the papers name them"
)]

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use core::fmt;

use geom_bvh::{Aabb, BoxTree, BvhError, Entry};
use geom_num::crossing::Ssi;
use geom_num::grid::P2;
use geom_num::predicate::{incircle, orient2d};
use geom_num::{NumError, Sign};
use geom_run::{Exec, Stopped, WorkMeter};

/// The ghost vertex: the point at infinity every hull edge's ghost triangle reaches.
const GHOST: u32 = u32::MAX;

/// How many rounds [`commit_crossings`] takes before it stops with [`CdtError::Crossings`]. Each
/// round's bends are within a grid step, so a few rounds settle any input the tests have met.
const ROUNDS: u32 = 64;

/// One input point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Site<K> {
    /// Where it is.
    pub at: P2,
    /// Its key, distinct among the sites: the order ties break in.
    pub key: K,
}

/// What a refinement aims for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quality {
    /// The largest ratio of circumradius to shortest edge, squared: 2 gives angles above 20.7°.
    pub ratio_squared: f64,
    /// The longest edge allowed, squared, in grid steps.
    pub max_edge_squared: Option<u128>,
    /// A triangle whose shortest edge, squared, is at most this is left as it is, so refinement
    /// ends beside small input angles and on the grid's own resolution.
    pub min_edge_squared: u128,
}

/// Why no triangulation came out.
#[derive(Clone, Debug, PartialEq)]
pub enum CdtError {
    /// Two sites carry one key.
    DuplicateKey {
        /// The first site, in input order.
        first: u32,
        /// The second.
        second: u32,
    },
    /// Two sites are at one point.
    DuplicatePoint {
        /// The first site, in input order.
        first: u32,
        /// The second.
        second: u32,
    },
    /// A constraint names a site that does not exist, or the same site twice.
    BadConstraint {
        /// Its index.
        index: u32,
    },
    /// Crossings did not settle within [`ROUNDS`] rounds.
    Crossings,
    /// More vertices or triangles than a 32-bit index reaches.
    Full,
    /// The numbers layer refused: a crossing outside the frame.
    Num(NumError),
    /// The meter stopped the run.
    Stopped(Stopped),
    /// The box tree refused.
    Bvh(BvhError),
    /// A broken invariant of this module.
    Internal,
}

impl fmt::Display for CdtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CdtError::DuplicateKey { first, second } => {
                write!(f, "sites {first} and {second} carry one key")
            }
            CdtError::DuplicatePoint { first, second } => {
                write!(f, "sites {first} and {second} are at one point")
            }
            CdtError::BadConstraint { index } => write!(f, "constraint {index} is not two sites"),
            CdtError::Crossings => f.write_str("the crossings did not settle"),
            CdtError::Full => f.write_str("too many vertices or triangles"),
            CdtError::Num(e) => write!(f, "{e}"),
            CdtError::Stopped(e) => write!(f, "{e}"),
            CdtError::Bvh(e) => write!(f, "{e}"),
            CdtError::Internal => f.write_str("a broken invariant of the triangulation"),
        }
    }
}

impl core::error::Error for CdtError {}

impl From<NumError> for CdtError {
    fn from(e: NumError) -> Self {
        CdtError::Num(e)
    }
}

impl From<Stopped> for CdtError {
    fn from(e: Stopped) -> Self {
        CdtError::Stopped(e)
    }
}

impl From<BvhError> for CdtError {
    fn from(e: BvhError) -> Self {
        CdtError::Bvh(e)
    }
}

/// A triangulation, in canonical order: it depends only on the sites, keys, constraints and
/// quality, never on the order they came in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cdt<K> {
    /// Every vertex: the sites in key order, then crossings and refinement points, each with
    /// `key` `None`, in the order they were committed.
    pub vertices: Vec<Vertex<K>>,
    /// The triangles, counter-clockwise, each starting at its lowest vertex, sorted.
    pub triangles: Vec<[u32; 3]>,
    /// The constrained edges, low vertex first, sorted.
    pub constraints: Vec<[u32; 2]>,
}

/// One vertex of a [`Cdt`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Vertex<K> {
    /// Where it is.
    pub at: P2,
    /// Its site's key; `None` for a vertex the triangulation made.
    pub key: Option<K>,
}

/// One triangle: vertices counter-clockwise, and for each vertex the neighbour across the edge
/// opposite it and whether that edge is constrained.
#[derive(Clone, Copy, Debug)]
struct Tri {
    v: [u32; 3],
    n: [u32; 3],
    fixed: [bool; 3],
}

/// Where a point is.
#[derive(Clone, Copy)]
enum Place {
    /// Strictly inside a triangle, or beyond the hull edge of a ghost triangle.
    In(u32),
    /// On the edge of `.0` opposite slot `.1`.
    On(u32, usize),
}

/// The slot after `i`.
const fn next(i: usize) -> usize {
    match i {
        0 => 1,
        1 => 2,
        _ => 0,
    }
}

/// The slot before `i`.
const fn prev(i: usize) -> usize {
    match i {
        0 => 2,
        1 => 0,
        _ => 1,
    }
}

/// Item `i` of a triple.
fn pick<T: Copy>(a: &[T; 3], i: usize) -> T {
    let [x, y, z] = *a;
    match i {
        0 => x,
        1 => y,
        _ => z,
    }
}

/// Sets item `i` of a triple.
fn put<T>(a: &mut [T; 3], i: usize, value: T) {
    let [x, y, z] = a;
    match i {
        0 => *x = value,
        1 => *y = value,
        _ => *z = value,
    }
}

/// Whether `q` is strictly between `a` and `b`, three collinear points: their order by (x, y) is
/// their order along the line.
fn between(a: P2, q: P2, b: P2) -> bool {
    (a < q && q < b) || (b < q && q < a)
}

/// The Morton code of a point: its coordinates' bits interleaved, for a curve that keeps near
/// points near.
fn morton(p: P2) -> u128 {
    fn spread(v: u64) -> u128 {
        let mut x = u128::from(v);
        x = (x | (x << 32)) & 0x0000_0000_ffff_ffff_0000_0000_ffff_ffff;
        x = (x | (x << 16)) & 0x0000_ffff_0000_ffff_0000_ffff_0000_ffff;
        x = (x | (x << 8)) & 0x00ff_00ff_00ff_00ff_00ff_00ff_00ff_00ff;
        x = (x | (x << 4)) & 0x0f0f_0f0f_0f0f_0f0f_0f0f_0f0f_0f0f_0f0f;
        x = (x | (x << 2)) & 0x3333_3333_3333_3333_3333_3333_3333_3333;
        (x | (x << 1)) & 0x5555_5555_5555_5555_5555_5555_5555_5555
    }
    let shift = |c: i64| c.wrapping_add(1 << 41).cast_unsigned();
    spread(shift(p.x.get())) | (spread(shift(p.y.get())) << 1)
}

/// A count as a 32-bit index.
fn index(n: usize) -> Result<u32, CdtError> {
    u32::try_from(n).map_err(|_| CdtError::Full)
}

/// A slice's item at a 32-bit index.
fn at<T>(items: &[T], i: u32) -> Result<&T, CdtError> {
    items.get(i as usize).ok_or(CdtError::Internal)
}

/// The triangles under construction.
struct Mesh {
    /// Every vertex, by rank.
    points: Vec<P2>,
    /// Every triangle, ghosts included. None is ever removed.
    tris: Vec<Tri>,
    /// One triangle at each vertex.
    around: Vec<u32>,
    /// Where the last walk ended.
    hint: u32,
    /// Turns the slot each walk step starts from, so a walk cannot circle.
    spin: usize,
    /// The triangles an insertion still has to test, kept so each insertion need not allocate.
    stack: Vec<u32>,
}

impl Mesh {
    fn p(&self, v: u32) -> Result<P2, CdtError> {
        at(&self.points, v).copied()
    }

    fn t(&self, t: u32) -> Result<Tri, CdtError> {
        at(&self.tris, t).copied()
    }

    fn set(&mut self, t: u32, tri: Tri) -> Result<(), CdtError> {
        *self.tris.get_mut(t as usize).ok_or(CdtError::Internal)? = tri;
        for v in tri.v {
            if v != GHOST {
                *self.around.get_mut(v as usize).ok_or(CdtError::Internal)? = t;
            }
        }
        Ok(())
    }

    /// Room for a new triangle, whose rows `set` then writes.
    fn fresh(&mut self) -> Result<u32, CdtError> {
        let t = index(self.tris.len())?;
        self.tris.push(Tri {
            v: [GHOST; 3],
            n: [GHOST; 3],
            fixed: [false; 3],
        });
        Ok(t)
    }

    /// `t`'s neighbour that was `from` now is `to`.
    fn relink(&mut self, t: u32, from: u32, to: u32) -> Result<(), CdtError> {
        let tri = self.tris.get_mut(t as usize).ok_or(CdtError::Internal)?;
        let i = tri.n.iter().position(|&n| n == from);
        put(&mut tri.n, i.ok_or(CdtError::Internal)?, to);
        Ok(())
    }

    fn orient(&self, a: u32, b: u32, c: u32) -> Result<Sign, CdtError> {
        Ok(orient2d(self.p(a)?, self.p(b)?, self.p(c)?)?)
    }

    /// Whether `d` is inside the circle of `a`, `b`, `c`, counter-clockwise, with ties broken by
    /// rank: see the module's method. The cofactors follow from raising one row's lifted height in
    /// the 4 × 4 form of `incircle`.
    fn in_disk(&self, a: u32, b: u32, c: u32, d: u32) -> Result<bool, CdtError> {
        match incircle(self.p(a)?, self.p(b)?, self.p(c)?, self.p(d)?)? {
            Sign::Positive => Ok(true),
            Sign::Negative => Ok(false),
            Sign::Zero => {
                let low = a.min(b).min(c).min(d);
                if low == d {
                    return Ok(false);
                }
                let sign = if low == a {
                    self.orient(b, c, d)?
                } else if low == b {
                    self.orient(a, d, c)?
                } else {
                    self.orient(a, b, d)?
                };
                Ok(sign == Sign::Positive)
            }
        }
    }

    /// Whether the edge of `t` opposite slot `i` should flip.
    fn should_flip(&self, t: u32, i: usize) -> Result<bool, CdtError> {
        let tri = self.t(t)?;
        if pick(&tri.fixed, i) {
            return Ok(false);
        }
        let (a, x, y) = (
            pick(&tri.v, i),
            pick(&tri.v, next(i)),
            pick(&tri.v, prev(i)),
        );
        let o = pick(&tri.n, i);
        let other = self.t(o)?;
        let j = other
            .n
            .iter()
            .position(|&n| n == t)
            .ok_or(CdtError::Internal)?;
        let d = pick(&other.v, j);
        if a == GHOST || d == GHOST {
            return Ok(false);
        }
        if x == GHOST {
            return Ok(self.orient(y, a, d)? == Sign::Positive);
        }
        if y == GHOST {
            return Ok(self.orient(a, x, d)? == Sign::Positive);
        }
        self.in_disk(x, y, a, d)
    }

    /// Flips the edge of `t` opposite slot `i`, returning the two triangles, each holding `t`'s
    /// apex.
    fn flip(&mut self, t: u32, i: usize) -> Result<(u32, u32), CdtError> {
        let tri = self.t(t)?;
        let o = pick(&tri.n, i);
        let other = self.t(o)?;
        let j = other
            .n
            .iter()
            .position(|&n| n == t)
            .ok_or(CdtError::Internal)?;
        let (a, x, y) = (
            pick(&tri.v, i),
            pick(&tri.v, next(i)),
            pick(&tri.v, prev(i)),
        );
        let d = pick(&other.v, j);
        let (na, nb) = (pick(&tri.n, next(i)), pick(&tri.n, prev(i)));
        let (nc, nd) = (pick(&other.n, next(j)), pick(&other.n, prev(j)));
        let (fa, fb) = (pick(&tri.fixed, next(i)), pick(&tri.fixed, prev(i)));
        let (fc, fd) = (pick(&other.fixed, next(j)), pick(&other.fixed, prev(j)));
        self.set(
            t,
            Tri {
                v: [a, x, d],
                n: [nc, o, nb],
                fixed: [fc, false, fb],
            },
        )?;
        self.set(
            o,
            Tri {
                v: [a, d, y],
                n: [nd, na, t],
                fixed: [fd, fa, false],
            },
        )?;
        self.relink(nc, o, t)?;
        self.relink(na, t, o)?;
        Ok((t, o))
    }

    /// Where `q` is, walking from the last place.
    fn locate(&mut self, q: u32) -> Result<Place, CdtError> {
        let p = self.p(q)?;
        let mut t = self.hint;
        let limit = self.tris.len().saturating_mul(4).saturating_add(16);
        for _ in 0..limit {
            let tri = self.t(t)?;
            self.spin = self.spin.wrapping_add(1);
            if let Some(g) = tri.v.iter().position(|&v| v == GHOST) {
                let (u, v) = (pick(&tri.v, next(g)), pick(&tri.v, prev(g)));
                let (pu, pv) = (self.p(u)?, self.p(v)?);
                match orient2d(pu, pv, p)? {
                    Sign::Positive => {
                        self.hint = t;
                        return Ok(Place::In(t));
                    }
                    Sign::Negative => t = pick(&tri.n, g),
                    Sign::Zero if between(pu, p, pv) => {
                        self.hint = t;
                        return Ok(Place::On(t, g));
                    }
                    // Past one end of the hull edge, along it: on to the ghost at that end.
                    Sign::Zero if between(pu, pv, p) => t = pick(&tri.n, next(g)),
                    Sign::Zero => t = pick(&tri.n, prev(g)),
                }
                continue;
            }
            let first = self.spin % 3;
            let mut slots = [first, next(first), prev(first)].into_iter();
            let mut on = None;
            let moved = loop {
                let Some(i) = slots.next() else {
                    break false;
                };
                let (a, b) = (pick(&tri.v, next(i)), pick(&tri.v, prev(i)));
                match orient2d(self.p(a)?, self.p(b)?, p)? {
                    Sign::Negative => {
                        t = pick(&tri.n, i);
                        break true;
                    }
                    Sign::Zero => on = Some(i),
                    Sign::Positive => {}
                }
            };
            if !moved {
                self.hint = t;
                return Ok(on.map_or(Place::In(t), |i| Place::On(t, i)));
            }
        }
        Err(CdtError::Internal)
    }

    /// Splits `t` around vertex `p`, which is inside it or, for a ghost, beyond its hull edge.
    fn split(&mut self, t: u32, p: u32) -> Result<[u32; 3], CdtError> {
        let old = self.t(t)?;
        let ([a, b, c], [na, nb, nc], [fa, fb, fc]) = (old.v, old.n, old.fixed);
        let (t1, t2) = (self.fresh()?, self.fresh()?);
        self.set(
            t,
            Tri {
                v: [a, b, p],
                n: [t1, t2, nc],
                fixed: [false, false, fc],
            },
        )?;
        self.set(
            t1,
            Tri {
                v: [b, c, p],
                n: [t2, t, na],
                fixed: [false, false, fa],
            },
        )?;
        self.set(
            t2,
            Tri {
                v: [c, a, p],
                n: [t, t1, nb],
                fixed: [false, false, fb],
            },
        )?;
        self.relink(na, t, t1)?;
        self.relink(nb, t, t2)?;
        Ok([t, t1, t2])
    }

    /// Splits the edge of `t` opposite slot `i` at vertex `p`, on it.
    fn split_edge(&mut self, t: u32, i: usize, p: u32) -> Result<[u32; 4], CdtError> {
        let tri = self.t(t)?;
        let o = pick(&tri.n, i);
        let other = self.t(o)?;
        let j = other
            .n
            .iter()
            .position(|&n| n == t)
            .ok_or(CdtError::Internal)?;
        let (c, a, b) = (
            pick(&tri.v, i),
            pick(&tri.v, next(i)),
            pick(&tri.v, prev(i)),
        );
        let d = pick(&other.v, j);
        let (nbc, nca) = (pick(&tri.n, next(i)), pick(&tri.n, prev(i)));
        let (nad, ndb) = (pick(&other.n, next(j)), pick(&other.n, prev(j)));
        let (fbc, fca) = (pick(&tri.fixed, next(i)), pick(&tri.fixed, prev(i)));
        let (fad, fdb) = (pick(&other.fixed, next(j)), pick(&other.fixed, prev(j)));
        let f = pick(&tri.fixed, i);
        let (t1, o1) = (self.fresh()?, self.fresh()?);
        self.set(
            t,
            Tri {
                v: [c, a, p],
                n: [o, t1, nca],
                fixed: [f, false, fca],
            },
        )?;
        self.set(
            t1,
            Tri {
                v: [b, c, p],
                n: [t, o1, nbc],
                fixed: [false, f, fbc],
            },
        )?;
        self.set(
            o,
            Tri {
                v: [a, d, p],
                n: [o1, t, nad],
                fixed: [false, f, fad],
            },
        )?;
        self.set(
            o1,
            Tri {
                v: [d, b, p],
                n: [t1, o, ndb],
                fixed: [f, false, fdb],
            },
        )?;
        self.relink(nbc, t, t1)?;
        self.relink(ndb, o, o1)?;
        Ok([t, t1, o, o1])
    }

    /// Puts vertex `p` in, and flips until every edge around it is locally Delaunay.
    fn insert(&mut self, p: u32, meter: &mut WorkMeter<'_>) -> Result<(), CdtError> {
        meter.charge(1)?;
        let mut stack = core::mem::take(&mut self.stack);
        match self.locate(p)? {
            Place::In(t) => stack.extend(self.split(t, p)?),
            Place::On(t, i) => stack.extend(self.split_edge(t, i, p)?),
        }
        while let Some(t) = stack.pop() {
            let tri = self.t(t)?;
            let Some(i) = tri.v.iter().position(|&v| v == p) else {
                continue;
            };
            if self.should_flip(t, i)? {
                meter.charge(1)?;
                let (x, y) = self.flip(t, i)?;
                stack.push(x);
                stack.push(y);
            }
        }
        self.stack = stack;
        self.hint = *at(&self.around, p)?;
        Ok(())
    }

    /// The triangle and slot of the edge from `a` to `b`: slot `i` of the triangle holds the vertex
    /// opposite the edge.
    fn find_edge(&self, a: u32, b: u32) -> Result<Option<(u32, usize)>, CdtError> {
        let (a, b) = if a == GHOST { (b, a) } else { (a, b) };
        let start = *at(&self.around, a)?;
        let mut t = start;
        for _ in 0..self.tris.len() {
            let tri = self.t(t)?;
            let i = tri
                .v
                .iter()
                .position(|&v| v == a)
                .ok_or(CdtError::Internal)?;
            if pick(&tri.v, next(i)) == b {
                return Ok(Some((t, prev(i))));
            }
            if pick(&tri.v, prev(i)) == b {
                return Ok(Some((t, next(i))));
            }
            t = pick(&tri.n, next(i));
            if t == start {
                return Ok(None);
            }
        }
        Err(CdtError::Internal)
    }

    /// From `u` towards `b`: the next vertex on the line and the edges crossed on the way there.
    fn trace(&self, u: u32, b: u32) -> Result<(u32, Vec<[u32; 2]>), CdtError> {
        let (pu, pb) = (self.p(u)?, self.p(b)?);
        let start = *at(&self.around, u)?;
        let mut t = start;
        let mut fan = None;
        for _ in 0..self.tris.len() {
            let tri = self.t(t)?;
            let i = tri
                .v
                .iter()
                .position(|&v| v == u)
                .ok_or(CdtError::Internal)?;
            let (x, y) = (pick(&tri.v, next(i)), pick(&tri.v, prev(i)));
            if x == b || y == b {
                return Ok((b, Vec::new()));
            }
            if x != GHOST && y != GHOST {
                let (px, py) = (self.p(x)?, self.p(y)?);
                let (ox, oy) = (orient2d(pu, pb, px)?, orient2d(pu, pb, py)?);
                if ox == Sign::Zero && (pu < px) == (pu < pb) {
                    return Ok((x, Vec::new()));
                }
                if oy == Sign::Zero && (pu < py) == (pu < pb) {
                    return Ok((y, Vec::new()));
                }
                if ox == Sign::Negative && oy == Sign::Positive {
                    fan = Some((t, i));
                    break;
                }
            }
            t = pick(&tri.n, next(i));
            if t == start {
                break;
            }
        }
        let (mut t, i) = fan.ok_or(CdtError::Internal)?;
        let mut i = i;
        let mut crossed = Vec::new();
        loop {
            let tri = self.t(t)?;
            let (x, y) = (pick(&tri.v, next(i)), pick(&tri.v, prev(i)));
            if pick(&tri.fixed, i) {
                return Err(CdtError::Internal);
            }
            crossed.push([x, y]);
            let o = pick(&tri.n, i);
            let other = self.t(o)?;
            let j = other
                .n
                .iter()
                .position(|&n| n == t)
                .ok_or(CdtError::Internal)?;
            let d = pick(&other.v, j);
            if d == GHOST {
                return Err(CdtError::Internal);
            }
            match orient2d(pu, pb, self.p(d)?)? {
                Sign::Zero => return Ok((d, crossed)),
                // `d` on `x`'s side: the line goes on through the edge from `d` to `y`.
                Sign::Negative => {
                    t = o;
                    i = next(j);
                    if pick(&other.v, i) == y {
                        i = prev(j);
                    }
                }
                Sign::Positive => {
                    t = o;
                    i = next(j);
                    if pick(&other.v, i) == x {
                        i = prev(j);
                    }
                }
            }
        }
    }

    /// Flips away every edge of `crossed` that crosses from `u` to `w`, Sloan's way: an edge
    /// whose quadrilateral is not convex waits its turn. Returns the edges to check afterwards.
    fn clear(
        &mut self,
        u: u32,
        w: u32,
        crossed: Vec<[u32; 2]>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<[u32; 2]>, CdtError> {
        let mut now = crossed;
        let mut later = Vec::new();
        let mut check = Vec::new();
        let mut flipped = false;
        loop {
            let Some([a, b]) = now.pop() else {
                if later.is_empty() {
                    return Ok(check);
                }
                if !flipped {
                    return Err(CdtError::Internal);
                }
                flipped = false;
                core::mem::swap(&mut now, &mut later);
                continue;
            };
            let (t, i) = self.find_edge(a, b)?.ok_or(CdtError::Internal)?;
            let tri = self.t(t)?;
            let o = pick(&tri.n, i);
            let other = self.t(o)?;
            let j = other
                .n
                .iter()
                .position(|&n| n == t)
                .ok_or(CdtError::Internal)?;
            let (p, q) = (pick(&tri.v, i), pick(&other.v, j));
            let (x, y) = (pick(&tri.v, next(i)), pick(&tri.v, prev(i)));
            let (ox, oy) = (self.orient(p, q, x)?, self.orient(p, q, y)?);
            let convex = matches!(
                (ox, oy),
                (Sign::Negative, Sign::Positive) | (Sign::Positive, Sign::Negative)
            );
            if !convex {
                later.push([a, b]);
                continue;
            }
            flipped = true;
            meter.charge(1)?;
            self.flip(t, i)?;
            check.extend([[p, x], [x, q], [q, y], [y, p]]);
            let still = p != u && p != w && q != u && q != w && {
                let (op, oq) = (self.orient(u, w, p)?, self.orient(u, w, q)?);
                matches!(
                    (op, oq),
                    (Sign::Negative, Sign::Positive) | (Sign::Positive, Sign::Negative)
                )
            };
            if still {
                later.push([p, q]);
            } else {
                check.push([p, q]);
            }
        }
    }

    /// Flips until every edge of `edges`, and every edge a flip touches, is locally Delaunay.
    fn legalize(
        &mut self,
        mut edges: Vec<[u32; 2]>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(), CdtError> {
        while let Some([a, b]) = edges.pop() {
            let Some((t, i)) = self.find_edge(a, b)? else {
                continue;
            };
            if self.should_flip(t, i)? {
                meter.charge(1)?;
                let tri = self.t(t)?;
                let other = self.t(pick(&tri.n, i))?;
                let j = other
                    .n
                    .iter()
                    .position(|&n| n == t)
                    .ok_or(CdtError::Internal)?;
                let (p, q) = (pick(&tri.v, i), pick(&other.v, j));
                let (x, y) = (pick(&tri.v, next(i)), pick(&tri.v, prev(i)));
                self.flip(t, i)?;
                edges.extend([[p, x], [x, q], [q, y], [y, p]]);
            }
        }
        Ok(())
    }

    /// Marks the edge of `t` opposite slot `i`, on both its triangles.
    fn fix(&mut self, t: u32, i: usize, fixed: bool) -> Result<(), CdtError> {
        let tri = self.t(t)?;
        let o = pick(&tri.n, i);
        let j = self
            .t(o)?
            .n
            .iter()
            .position(|&n| n == t)
            .ok_or(CdtError::Internal)?;
        for (x, slot) in [(t, i), (o, j)] {
            let row = self.tris.get_mut(x as usize).ok_or(CdtError::Internal)?;
            put(&mut row.fixed, slot, fixed);
        }
        Ok(())
    }

    /// Makes the constraint from `a` to `b` a chain of constrained edges.
    fn enforce(&mut self, a: u32, b: u32, meter: &mut WorkMeter<'_>) -> Result<(), CdtError> {
        let mut u = a;
        while u != b {
            meter.charge(1)?;
            let (w, crossed) = self.trace(u, b)?;
            let check = if crossed.is_empty() {
                Vec::new()
            } else {
                self.clear(u, w, crossed, meter)?
            };
            let (t, i) = self.find_edge(u, w)?.ok_or(CdtError::Internal)?;
            self.fix(t, i, true)?;
            self.legalize(check, meter)?;
            u = w;
        }
        Ok(())
    }

    /// The solid triangles, each from its lowest vertex, sorted, and the constrained edges.
    fn finish(&self) -> (Vec<[u32; 3]>, Vec<[u32; 2]>) {
        let mut triangles = Vec::new();
        let mut fixed = BTreeSet::new();
        for tri in &self.tris {
            if tri.v.contains(&GHOST) {
                continue;
            }
            let [a, b, c] = tri.v;
            triangles.push(if a < b && a < c {
                [a, b, c]
            } else if b < c {
                [b, c, a]
            } else {
                [c, a, b]
            });
            for i in 0..3 {
                if pick(&tri.fixed, i) {
                    let (x, y) = (pick(&tri.v, next(i)), pick(&tri.v, prev(i)));
                    fixed.insert([x.min(y), x.max(y)]);
                }
            }
        }
        triangles.sort_unstable();
        (triangles, fixed.into_iter().collect())
    }
}

/// Every pair of constraints that cross in their interiors, each lower first, in order.
fn crossings(
    points: &[P2],
    segments: &[[u32; 2]],
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<(u32, u32)>, CdtError> {
    let mut entries = Vec::with_capacity(segments.len());
    for (key, &[a, b]) in (0_u64..).zip(segments) {
        let (pa, pb) = (*at(points, a)?, *at(points, b)?);
        let (ax, ay, bx, by) = (pa.x.get(), pa.y.get(), pb.x.get(), pb.y.get());
        entries.push(Entry {
            key,
            bbox: Aabb::new([ax.min(bx), ay.min(by)], [ax.max(bx), ay.max(by)])?,
        });
    }
    let tree = BoxTree::build(&entries, &Exec::serial(), meter)?;
    let mut out = Vec::new();
    for (entry, (first, &[a, b])) in entries.iter().zip((0_u32..).zip(segments)) {
        meter.charge(1)?;
        let (pa, pb) = (*at(points, a)?, *at(points, b)?);
        for key in tree.overlapping(&entry.bbox, meter)? {
            let second = u32::try_from(key).map_err(|_| CdtError::Internal)?;
            if second <= first {
                continue;
            }
            let [c, d] = *at(segments, second)?;
            if a == c || a == d || b == c || b == d {
                continue;
            }
            let (pc, pd) = (*at(points, c)?, *at(points, d)?);
            let opposite = |x: Sign, y: Sign| {
                matches!(
                    (x, y),
                    (Sign::Negative, Sign::Positive) | (Sign::Positive, Sign::Negative)
                )
            };
            if opposite(orient2d(pa, pb, pc)?, orient2d(pa, pb, pd)?)
                && opposite(orient2d(pc, pd, pa)?, orient2d(pc, pd, pb)?)
            {
                out.push((first, second));
            }
        }
    }
    Ok(out)
}

/// Cuts every constraint where it crosses another, each crossing constructed from the two
/// constraints' ends and committed to the grid, adding a vertex for it unless one is there.
/// Rounds go on until no two constraints cross (the module's guarantees).
fn commit_crossings(
    points: &mut Vec<P2>,
    by_point: &[(P2, u32)],
    mut segments: Vec<[u32; 2]>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<[u32; 2]>, CdtError> {
    let mut added: BTreeMap<P2, u32> = BTreeMap::new();
    for _ in 0..ROUNDS {
        segments.sort_unstable();
        segments.dedup();
        let pairs = crossings(points, &segments, meter)?;
        if pairs.is_empty() {
            return Ok(segments);
        }
        let mut cuts: BTreeMap<u32, BTreeSet<(P2, u32)>> = BTreeMap::new();
        for (first, second) in pairs {
            let ([a, b], [c, d]) = (*at(&segments, first)?, *at(&segments, second)?);
            let (pa, pb, pc, pd) = (
                *at(points, a)?,
                *at(points, b)?,
                *at(points, c)?,
                *at(points, d)?,
            );
            let at_grid = Ssi::new(pa, pb, pc, pd)?.to_grid()?.value();
            let site = by_point.binary_search_by_key(&at_grid, |&(p, _)| p);
            let vertex = match (site.ok().and_then(|i| by_point.get(i)), added.get(&at_grid)) {
                (Some(&(_, v)), _) | (None, Some(&v)) => v,
                (None, None) => {
                    let v = index(points.len())?;
                    points.push(at_grid);
                    added.insert(at_grid, v);
                    v
                }
            };
            for s in [first, second] {
                cuts.entry(s).or_default().insert((at_grid, vertex));
            }
        }
        let mut cut = Vec::with_capacity(segments.len());
        for (s, &[a, b]) in (0_u32..).zip(&segments) {
            let Some(along) = cuts.get(&s) else {
                cut.push([a, b]);
                continue;
            };
            // Along the constraint from its lower point, the order of the cuts by (x, y).
            let (pa, pb) = (*at(points, a)?, *at(points, b)?);
            let (low, high) = if pa < pb { (a, b) } else { (b, a) };
            let mut last = low;
            for &(_, v) in along {
                if v != last && v != high {
                    cut.push([last.min(v), last.max(v)]);
                    last = v;
                }
            }
            cut.push([last.min(high), last.max(high)]);
        }
        segments = cut;
    }
    Err(CdtError::Crossings)
}

/// What sorting the sites gives.
struct Ranks {
    /// Each site's rank.
    of_site: Vec<u32>,
    /// The sites by point, each with its rank, for finding a vertex at a point.
    by_point: Vec<(P2, u32)>,
}

/// The vertices' ranks: sorted by key, each site's rank, refusing repeated keys and points.
fn ranks<K: Ord + Copy>(sites: &[Site<K>]) -> Result<Ranks, CdtError> {
    let n = index(sites.len())?;
    let mut by_key: Vec<(K, u32)> = sites.iter().map(|s| s.key).zip(0..n).collect();
    by_key.sort_unstable();
    for pair in by_key.windows(2) {
        if let &[(k, first), (l, second)] = pair
            && k == l
        {
            return Err(CdtError::DuplicateKey { first, second });
        }
    }
    let mut by_point: Vec<(P2, u32)> = sites.iter().map(|s| s.at).zip(0..n).collect();
    by_point.sort_unstable();
    for pair in by_point.windows(2) {
        if let &[(p, first), (q, second)] = pair
            && p == q
        {
            return Err(CdtError::DuplicatePoint { first, second });
        }
    }
    let mut rank = alloc::vec![0_u32; sites.len()];
    for (r, &(_, i)) in (0_u32..).zip(&by_key) {
        *rank.get_mut(i as usize).ok_or(CdtError::Internal)? = r;
    }
    for entry in &mut by_point {
        entry.1 = *at(&rank, entry.1)?;
    }
    Ok(Ranks {
        of_site: rank,
        by_point,
    })
}

/// The first triangle: the first point on the curve, the next, and the first off their line,
/// with its three ghosts. `None` if every point is on one line.
fn seed(points: &[P2], order: &[u32]) -> Result<Option<(Mesh, [u32; 3])>, CdtError> {
    let [a, b, rest @ ..] = order else {
        return Ok(None);
    };
    let (a, b) = (*a, *b);
    let (pa, pb) = (*at(points, a)?, *at(points, b)?);
    let mut third = None;
    for &c in rest {
        match orient2d(pa, pb, *at(points, c)?)? {
            Sign::Zero => {}
            sign => {
                third = Some((c, sign));
                break;
            }
        }
    }
    let Some((c, sign)) = third else {
        return Ok(None);
    };
    let (a, b) = if sign == Sign::Positive {
        (a, b)
    } else {
        (b, a)
    };
    let mut mesh = Mesh {
        points: points.to_vec(),
        tris: Vec::new(),
        around: alloc::vec![GHOST; points.len()],
        hint: 0,
        spin: 0,
        stack: Vec::new(),
    };
    for _ in 0..4 {
        mesh.fresh()?;
    }
    // The solid triangle, then the ghost across each of its edges.
    mesh.set(
        0,
        Tri {
            v: [a, b, c],
            n: [2, 3, 1],
            fixed: [false; 3],
        },
    )?;
    mesh.set(
        1,
        Tri {
            v: [b, a, GHOST],
            n: [3, 2, 0],
            fixed: [false; 3],
        },
    )?;
    mesh.set(
        2,
        Tri {
            v: [c, b, GHOST],
            n: [1, 3, 0],
            fixed: [false; 3],
        },
    )?;
    mesh.set(
        3,
        Tri {
            v: [a, c, GHOST],
            n: [2, 1, 0],
            fixed: [false; 3],
        },
    )?;
    Ok(Some((mesh, [a, b, c])))
}

/// One constraint's pieces when every point is on one line: consecutive points along it.
fn on_a_line(points: &[P2], segments: &[[u32; 2]]) -> Result<Vec<[u32; 2]>, CdtError> {
    let mut along: Vec<u32> = (0..index(points.len())?).collect();
    along.sort_unstable_by_key(|&v| points.get(v as usize).copied());
    let mut pieces = BTreeSet::new();
    for &[a, b] in segments {
        let (pa, pb) = (*at(points, a)?, *at(points, b)?);
        let (low, high) = (pa.min(pb), pa.max(pb));
        let mut last = None;
        for &v in &along {
            let p = *at(points, v)?;
            if p >= low && p <= high {
                if let Some(u) = last {
                    pieces.insert([v.min(u), v.max(u)]);
                }
                last = Some(v);
            }
        }
    }
    Ok(pieces.into_iter().collect())
}

/// The constrained Delaunay triangulation of `sites` and `constraints` (pairs of site indices),
/// refined to `quality` if one is given. See the module header.
///
/// # Errors
///
/// [`CdtError::DuplicateKey`], [`CdtError::DuplicatePoint`] and [`CdtError::BadConstraint`] for
/// bad input, before anything is built; [`CdtError::Num`] for a crossing the frame cannot hold;
/// [`CdtError::Stopped`] from the meter; [`CdtError::Crossings`] and [`CdtError::Full`] at the
/// limits; [`CdtError::Bvh`] and [`CdtError::Internal`] for what no input should cause.
pub fn triangulate<K: Ord + Copy>(
    sites: &[Site<K>],
    constraints: &[[u32; 2]],
    quality: Option<&Quality>,
    meter: &mut WorkMeter<'_>,
) -> Result<Cdt<K>, CdtError> {
    let Ranks {
        of_site: rank,
        by_point,
    } = ranks(sites)?;
    let mut segments = Vec::with_capacity(constraints.len());
    for (index, &[a, b]) in (0_u32..).zip(constraints) {
        let bad = CdtError::BadConstraint { index };
        let (ra, rb) = (
            *rank.get(a as usize).ok_or(bad.clone())?,
            *rank.get(b as usize).ok_or(bad.clone())?,
        );
        if ra == rb {
            return Err(bad);
        }
        segments.push([ra.min(rb), ra.max(rb)]);
    }
    let mut points = alloc::vec![P2::new(0, 0)?; sites.len()];
    let mut keys: Vec<Option<K>> = alloc::vec![None; sites.len()];
    for (site, &r) in sites.iter().zip(&rank) {
        *points.get_mut(r as usize).ok_or(CdtError::Internal)? = site.at;
        *keys.get_mut(r as usize).ok_or(CdtError::Internal)? = Some(site.key);
    }
    let segments = commit_crossings(&mut points, &by_point, segments, meter)?;
    let mut order: Vec<u32> = (0..index(points.len())?).collect();
    order.sort_by_cached_key(|&v| points.get(v as usize).copied().map(morton));
    let (triangles, constraints, points) = match seed(&points, &order)? {
        None => {
            let pieces = on_a_line(&points, &segments)?;
            (Vec::new(), pieces, points)
        }
        Some((mut mesh, first)) => {
            for &v in &order {
                if !first.contains(&v) {
                    mesh.insert(v, meter)?;
                }
            }
            for &[a, b] in &segments {
                mesh.enforce(a, b, meter)?;
            }
            if let Some(q) = quality {
                refine(&mut mesh, q, meter)?;
            }
            let (t, c) = mesh.finish();
            (t, c, mesh.points)
        }
    };
    let vertices = points
        .iter()
        .enumerate()
        .map(|(i, &p)| Vertex {
            at: p,
            key: keys.get(i).copied().flatten(),
        })
        .collect();
    Ok(Cdt {
        vertices,
        triangles,
        constraints,
    })
}

/// `b − a` on one axis, in `i128`.
fn gap(a: geom_num::Coord, b: geom_num::Coord) -> i128 {
    i128::from(b.get()).saturating_sub(i128::from(a.get()))
}

/// |ab|², exact: coordinates of the frame give at most 2^84.
fn length_squared(a: P2, b: P2) -> u128 {
    let (dx, dy) = (gap(a.x, b.x), gap(a.y, b.y));
    dx.unsigned_abs()
        .saturating_mul(dx.unsigned_abs())
        .saturating_add(dy.unsigned_abs().saturating_mul(dy.unsigned_abs()))
}

/// (a − c) · (b − c): negative exactly when `c` is inside the circle on `ab` as diameter.
fn dot(a: P2, b: P2, c: P2) -> i128 {
    gap(c.x, a.x)
        .saturating_mul(gap(c.x, b.x))
        .saturating_add(gap(c.y, a.y).saturating_mul(gap(c.y, b.y)))
}

/// The midpoint of `a` and `b`, rounded down on each axis: on the grid.
fn middle(a: P2, b: P2) -> Result<P2, CdtError> {
    let half = |x: i64, y: i64| x.saturating_add(y).div_euclid(2);
    Ok(P2::new(
        half(a.x.get(), b.x.get()),
        half(a.y.get(), b.y.get()),
    )?)
}

/// `n / d` for `d > 0`, to the nearest integer, halves up.
fn nearest(n: i128, d: i128) -> Option<i128> {
    n.checked_mul(2)?
        .checked_add(d)?
        .checked_div_euclid(d.checked_mul(2)?)
}

/// The circumcentre of counter-clockwise `a`, `b`, `c`, rounded to the grid; `None` outside the
/// frame. Within the frame every product here fits `i128`: relative coordinates are at most
/// 2^41, squared lengths at most 2^83, and their products at most 2^124.
fn circumcentre(a: P2, b: P2, c: P2) -> Option<P2> {
    let (bx, by, cx, cy) = (gap(a.x, b.x), gap(a.y, b.y), gap(a.x, c.x), gap(a.y, c.y));
    let d = bx
        .checked_mul(cy)?
        .checked_sub(by.checked_mul(cx)?)?
        .checked_mul(2)?;
    if d <= 0 {
        return None;
    }
    let b2 = bx.checked_mul(bx)?.checked_add(by.checked_mul(by)?)?;
    let c2 = cx.checked_mul(cx)?.checked_add(cy.checked_mul(cy)?)?;
    let ux = nearest(cy.checked_mul(b2)?.checked_sub(by.checked_mul(c2)?)?, d)?;
    let uy = nearest(bx.checked_mul(c2)?.checked_sub(cx.checked_mul(b2)?)?, d)?;
    let x = i64::try_from(ux.checked_add(i128::from(a.x.get()))?).ok()?;
    let y = i64::try_from(uy.checked_add(i128::from(a.y.get()))?).ok()?;
    P2::new(x, y).ok()
}

/// Whether counter-clockwise `a`, `b`, `c` fails `quality`: its circumradius over its shortest
/// edge, squared, is above the bound, or its longest edge too long, and its shortest edge is above
/// the floor. The ratio is a heuristic, not a decision about the shape, so it is computed in
/// doubles, the same on every target.
fn is_bad(a: P2, b: P2, c: P2, quality: &Quality) -> bool {
    let mut lengths = [
        length_squared(a, b),
        length_squared(b, c),
        length_squared(c, a),
    ];
    lengths.sort_unstable();
    let [low, mid, high] = lengths;
    if low <= quality.min_edge_squared {
        return false;
    }
    if quality.max_edge_squared.is_some_and(|limit| high > limit) {
        return true;
    }
    let cross = gap(a.x, b.x)
        .saturating_mul(gap(a.y, c.y))
        .saturating_sub(gap(a.y, b.y).saturating_mul(gap(a.x, c.x)));
    #[allow(
        clippy::cast_precision_loss,
        reason = "a heuristic: the nearest double of each exact value, the same on every target"
    )]
    let (m, h, x) = (mid as f64, high as f64, cross as f64);
    m * h / (4.0 * x * x) > quality.ratio_squared
}

/// What a refinement wants done next: both in canonical order, segments first.
#[derive(Default)]
struct Work {
    segments: BTreeSet<[u32; 2]>,
    triangles: BTreeSet<[u32; 3]>,
}

impl Mesh {
    /// A new vertex at `p`, without a place in the mesh yet.
    fn add_vertex(&mut self, p: P2) -> Result<u32, CdtError> {
        let v = index(self.points.len())?;
        self.points.push(p);
        self.around.push(GHOST);
        Ok(v)
    }

    /// Whether the edge of `t` opposite slot `i` is a segment: constrained, or on the hull.
    fn is_segment(&self, t: u32, i: usize) -> Result<bool, CdtError> {
        let tri = self.t(t)?;
        let other = self.t(pick(&tri.n, i))?;
        Ok(pick(&tri.fixed, i) || other.v.contains(&GHOST) && !tri.v.contains(&GHOST))
    }

    /// Notes what `t` needs: each of its segments that its apex encroaches on, and `t` itself if
    /// it is bad.
    fn survey(&self, t: u32, quality: &Quality, work: &mut Work) -> Result<(), CdtError> {
        let tri = self.t(t)?;
        if tri.v.contains(&GHOST) {
            return Ok(());
        }
        let [a, b, c] = tri.v;
        let (pa, pb, pc) = (self.p(a)?, self.p(b)?, self.p(c)?);
        for (i, apex) in [(0, pa), (1, pb), (2, pc)] {
            let (x, y) = (pick(&tri.v, next(i)), pick(&tri.v, prev(i)));
            if self.is_segment(t, i)? && dot(self.p(x)?, self.p(y)?, apex) < 0 {
                work.segments.insert([x.min(y), x.max(y)]);
            }
        }
        if is_bad(pa, pb, pc, quality) {
            let low = if a < b && a < c {
                [a, b, c]
            } else if b < c {
                [b, c, a]
            } else {
                [c, a, b]
            };
            work.triangles.insert(low);
        }
        Ok(())
    }

    /// Surveys every triangle at `v`.
    fn survey_around(&self, v: u32, quality: &Quality, work: &mut Work) -> Result<(), CdtError> {
        let start = *at(&self.around, v)?;
        let mut t = start;
        for _ in 0..self.tris.len() {
            self.survey(t, quality, work)?;
            let tri = self.t(t)?;
            let i = tri
                .v
                .iter()
                .position(|&x| x == v)
                .ok_or(CdtError::Internal)?;
            t = pick(&tri.n, next(i));
            if t == start {
                return Ok(());
            }
        }
        Err(CdtError::Internal)
    }

    /// Splits the segment `a`-`b` at its midpoint, rounded to the grid. A segment too short for
    /// one, or whose rounded midpoint is taken or lies past its two triangles, stays: `false`.
    fn split_segment(
        &mut self,
        a: u32,
        b: u32,
        known: &mut BTreeMap<P2, u32>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<u32>, CdtError> {
        let Some((t, i)) = self.find_edge(a, b)? else {
            return Ok(None);
        };
        let m = middle(self.p(a)?, self.p(b)?)?;
        if known.contains_key(&m) {
            return Ok(None);
        }
        let fixed = pick(&self.t(t)?.fixed, i);
        let on_line = orient2d(self.p(a)?, self.p(b)?, m)? == Sign::Zero;
        if !on_line {
            // The rounded midpoint must lie in one of the segment's two triangles, or the bent
            // pieces could cross something else.
            let tri = self.t(t)?;
            let other = self.t(pick(&tri.n, i))?;
            let inside = |mesh: &Mesh, v: [u32; 3]| -> Result<bool, CdtError> {
                if v.contains(&GHOST) {
                    return Ok(false);
                }
                let [x, y, z] = v;
                Ok(orient2d(mesh.p(x)?, mesh.p(y)?, m)? == Sign::Positive
                    && orient2d(mesh.p(y)?, mesh.p(z)?, m)? == Sign::Positive
                    && orient2d(mesh.p(z)?, mesh.p(x)?, m)? == Sign::Positive)
            };
            if !inside(self, tri.v)? && !inside(self, other.v)? {
                return Ok(None);
            }
        }
        let v = self.add_vertex(m)?;
        known.insert(m, v);
        if on_line {
            self.insert(v, meter)?;
        } else {
            self.fix(t, i, false)?;
            self.insert(v, meter)?;
            if fixed {
                self.enforce(a, v, meter)?;
                self.enforce(v, b, meter)?;
            }
        }
        Ok(Some(v))
    }

    /// From `t`, walks towards `p`, stopping at the first segment in the way. The next edge is
    /// the least of those `p` is beyond, so the walk depends only on the triangles.
    fn walk(&self, t: u32, p: P2) -> Result<Result<u32, [u32; 2]>, CdtError> {
        let mut t = t;
        for _ in 0..self.tris.len() {
            let tri = self.t(t)?;
            let mut beyond: Option<([u32; 2], usize)> = None;
            for i in 0..3 {
                let (x, y) = (pick(&tri.v, next(i)), pick(&tri.v, prev(i)));
                if orient2d(self.p(x)?, self.p(y)?, p)? == Sign::Negative {
                    let edge = [x.min(y), x.max(y)];
                    if beyond.is_none_or(|(least, _)| edge < least) {
                        beyond = Some((edge, i));
                    }
                }
            }
            let Some((edge, i)) = beyond else {
                return Ok(Ok(t));
            };
            if self.is_segment(t, i)? {
                return Ok(Err(edge));
            }
            t = pick(&tri.n, i);
        }
        Err(CdtError::Internal)
    }
}

/// Refines `mesh` to `quality`, Ruppert's way with Shewchuk's priorities: while a segment is
/// encroached it is split at its midpoint; otherwise the first bad triangle gets its circumcentre,
/// unless that is beyond a segment or encroaches on one, when that segment is split instead. Every
/// choice is in canonical order, so the result depends only on the triangulation it starts from.
fn refine(mesh: &mut Mesh, quality: &Quality, meter: &mut WorkMeter<'_>) -> Result<(), CdtError> {
    let mut known: BTreeMap<P2, u32> = (0_u32..)
        .zip(mesh.points.iter())
        .map(|(v, &p)| (p, v))
        .collect();
    let mut work = Work::default();
    for t in 0..index(mesh.tris.len())? {
        mesh.survey(t, quality, &mut work)?;
    }
    loop {
        meter.charge(1)?;
        if let Some([a, b]) = work.segments.pop_first() {
            if let Some(v) = mesh.split_segment(a, b, &mut known, meter)? {
                mesh.survey_around(v, quality, &mut work)?;
                mesh.survey_around(a, quality, &mut work)?;
                mesh.survey_around(b, quality, &mut work)?;
            }
            continue;
        }
        let Some([a, b, c]) = work.triangles.pop_first() else {
            return Ok(());
        };
        // Still there, and still this triangle? The edge's other triangle may have come first.
        let Some((t, i)) = mesh.find_edge(a, b)? else {
            continue;
        };
        let tri = mesh.t(t)?;
        let t = if pick(&tri.v, i) == c {
            t
        } else {
            let o = pick(&tri.n, i);
            if !mesh.t(o)?.v.contains(&c) {
                continue;
            }
            o
        };
        let (pa, pb, pc) = (mesh.p(a)?, mesh.p(b)?, mesh.p(c)?);
        let Some(centre) = circumcentre(pa, pb, pc) else {
            continue;
        };
        if known.contains_key(&centre) {
            continue;
        }
        let found = mesh.walk(t, centre)?;
        let blocking = match found {
            Err(edge) => Some(edge),
            Ok(there) => {
                let tri = mesh.t(there)?;
                let mut hit = None;
                for i in 0..3 {
                    let (x, y) = (pick(&tri.v, next(i)), pick(&tri.v, prev(i)));
                    if mesh.is_segment(there, i)?
                        && dot(mesh.p(x)?, mesh.p(y)?, centre) < 0
                        && hit.is_none_or(|e| [x.min(y), x.max(y)] < e)
                    {
                        hit = Some([x.min(y), x.max(y)]);
                    }
                }
                hit
            }
        };
        if let Some([x, y]) = blocking {
            if let Some(v) = mesh.split_segment(x, y, &mut known, meter)? {
                mesh.survey_around(v, quality, &mut work)?;
                mesh.survey_around(x, quality, &mut work)?;
                mesh.survey_around(y, quality, &mut work)?;
                work.triangles.insert([a, b, c]);
            }
            continue;
        }
        let v = mesh.add_vertex(centre)?;
        known.insert(centre, v);
        mesh.hint = t;
        mesh.insert(v, meter)?;
        mesh.survey_around(v, quality, &mut work)?;
    }
}
