//! The reference implementation of flat solids (chapter 28.9): booleans, imprint, cells, plane
//! sections, In-On-Out, volumes, centroids, clash and first contact, in exact rationals, by brute
//! force, to check the kernel against.
//!
//! What it does: a [`Solid`] is a closed, oriented set of planar polygons with rational corners.
//! Everything else comes from one object, the arrangement of a set of planes: every plane cuts a
//! large box and every cell already cut, so [`arrangement`] gives convex [`Cell`]s meeting face to
//! face. Each cell lies wholly in or out of each solid whose face planes were among the cuts, and
//! one point of it decides which, by [`Solid::locate`]: a ray from the point, counted against every
//! face.
//!
//! - [`boolean`] keeps the cells a [`Op`] keeps; [`Region::volume`] and [`Region::centroid`] sum
//!   them, and [`Region::boundary`] is the cell faces with one side kept.
//! - [`labelled_cells`] says which solids each cell lies in, for two or more solids.
//! - [`section`] cuts by one more plane and keeps the cell faces on it.
//! - [`imprint`] gives the segments where one solid's faces cross another's, testing every pair of
//!   faces.
//! - [`clash`] sorts a pair of solids; a closed polyhedral intersection is empty, or holds an
//!   arrangement vertex, so testing every cell vertex decides it.
//! - [`first_contact`] finds when a moving solid first touches a still one, from every vertex and
//!   face pair and every edge pair.
//! - [`six_volume_where`] is the volume of any boolean of solids of many faces (Gate 4's corpus:
//!   EMBER's figures and Manifold's models), without an arrangement of space: each face is cut by
//!   the planes of the faces that pass through it, and a piece bounds the result when the winding
//!   numbers just in front of it and just behind it ([`Solid::winding`], signed counts of the faces
//!   a ray crosses) fall on different sides of the boolean, for a surface that crosses itself
//!   too.
//!
//! Guarantees:
//!
//! - **Exact.** Every value is an exact rational; nothing rounds, and every comparison is a
//!   comparison of rationals.
//! - **Independent of the kernel.** Written from the definitions below, without reading any kernel
//!   boolean (none exists as it is written), sharing no code with the kernel and calling none
//!   (chapter 28.9: a different author, a different agent session). A cell method, where a kernel
//!   follows surfaces.
//! - **Refused, never guessed.** A polygon of fewer than three corners, one with no area or not in
//!   a plane, and a solid whose edges do not pair up are [`ReferenceError`]s.
//! - **Regularised.** A boolean's result is the closure of its interior: a shared face or edge
//!   leaves nothing behind.
//!
//! Method: Sutherland and Hodgman's clip of a polygon by a plane, "Reentrant polygon clipping",
//! CACM 17(1), 1974, <https://doi.org/10.1145/360767.360802>; ray parity for point location; Edelsbrunner, O'Rourke and
//! Seidel, "Constructing arrangements of lines and hyperplanes", SIAM J. Computing 15(2), 1986,
//! <https://doi.org/10.1137/0215024>, for the arrangement as a cell complex, whose faces meet
//! exactly; Andrew's monotone chain, 1979, <https://doi.org/10.1016/0020-0190(79)90072-3>, for a cut's
//! outline; the divergence theorem over a fan of tetrahedra for volume and centroid; Canny's
//! vertex-face and edge-edge contact events for polyhedra under translation,
//! <https://doi.org/10.1109/TPAMI.1986.4767788>; boundary evaluation, a boolean's boundary as the
//! pieces of its operands' faces with the result on one side only (Requicha and Voelcker,
//! "Boolean operations in solid modeling: boundary evaluation and merging algorithms", Proceedings
//! of the IEEE 73(1), 1985, <https://doi.org/10.1109/PROC.1985.13108>), with the winding number of
//! a closed surface as a signed count of ray crossings.
//!
//! Cost: `p` planes cut the box into up to O(p³) cells, each cut costing every face of every cell,
//! and rationals grow with each cut. This reference accepts that, because it runs on small cases
//! only. [`six_volume_where`] costs each pair of faces whose boxes meet, found by a sweep, the
//! pieces those cut each face into, and two rays a piece for each solid, each against the faces
//! filed in the ray's bucket of a grid across it. No recursion.

// The lint is for fixed-width arithmetic, which can overflow. Every operator here is on dashu's
// arbitrary-precision rationals, which cannot, and every division is by a value shown nonzero
// beside it.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "arbitrary-precision rationals, which cannot overflow"
)]
#![allow(
    clippy::many_single_char_names,
    reason = "points and vectors, named as the geometry they come from names them"
)]

use core::cmp::Ordering;
use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use dashu_int::{IBig, UBig};
use dashu_ratio::RBig;

/// An exact rational.
pub type Q = RBig;

/// A point or a vector.
pub type Point = [Q; 3];

/// A whole number as a rational.
#[must_use]
pub fn int(n: i64) -> Q {
    Q::from(IBig::from(n))
}

/// `n / d`; `d` is a nonzero count.
#[must_use]
pub fn ratio(n: i64, d: u64) -> Q {
    Q::from_parts(IBig::from(n), UBig::from(d.max(1)))
}

/// The point of three whole numbers.
#[must_use]
pub fn point(x: i64, y: i64, z: i64) -> Point {
    [int(x), int(y), int(z)]
}

fn minus(a: &Point, b: &Point) -> Point {
    let [ax, ay, az] = a;
    let [bx, by, bz] = b;
    [ax - bx, ay - by, az - bz]
}

fn plus(a: &Point, b: &Point) -> Point {
    let [ax, ay, az] = a;
    let [bx, by, bz] = b;
    [ax + bx, ay + by, az + bz]
}

fn times(a: &Point, s: &Q) -> Point {
    let [ax, ay, az] = a;
    [ax * s, ay * s, az * s]
}

fn dot(a: &Point, b: &Point) -> Q {
    let [ax, ay, az] = a;
    let [bx, by, bz] = b;
    ax * bx + ay * by + az * bz
}

fn cross(a: &Point, b: &Point) -> Point {
    let [ax, ay, az] = a;
    let [bx, by, bz] = b;
    [ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx]
}

fn is_zero(a: &Point) -> bool {
    a.iter().all(|x| *x == Q::ZERO)
}

/// Why a shape is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReferenceError {
    /// A polygon of fewer than three corners, or one with no area.
    NoArea,
    /// A polygon whose corners are not in one plane.
    NotPlanar,
    /// A solid with an edge that does not pair with exactly one opposite edge.
    NotClosed,
    /// A motion or plane that says nothing: a zero normal.
    ZeroNormal,
}

impl fmt::Display for ReferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ReferenceError::NoArea => "a polygon has no area",
            ReferenceError::NotPlanar => "a polygon's corners are not in one plane",
            ReferenceError::NotClosed => "a solid's edges do not pair up",
            ReferenceError::ZeroNormal => "a plane has a zero normal",
        })
    }
}

impl core::error::Error for ReferenceError {}

/// Where a point lies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Where {
    /// Strictly inside.
    In,
    /// On the boundary.
    On,
    /// Strictly outside.
    Out,
}

/// The plane `normal · x = offset`. Its positive side is the one `normal` points to.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Plane {
    normal: Point,
    offset: Q,
}

impl Plane {
    /// The plane `normal · x = offset`.
    ///
    /// # Errors
    ///
    /// [`ReferenceError::ZeroNormal`].
    pub fn new(normal: Point, offset: Q) -> Result<Plane, ReferenceError> {
        if is_zero(&normal) {
            Err(ReferenceError::ZeroNormal)
        } else {
            Ok(Plane { normal, offset })
        }
    }

    /// The normal.
    #[must_use]
    pub fn normal(&self) -> &Point {
        &self.normal
    }

    /// Which side `p` is on: `Greater` for the positive side.
    #[must_use]
    pub fn side(&self, p: &Point) -> Ordering {
        dot(&self.normal, p).cmp(&self.offset)
    }

    /// The same plane, scaled so its first nonzero normal coordinate is 1, so two spellings of one
    /// plane compare equal.
    #[must_use]
    pub fn canonical(&self) -> Plane {
        let lead = self
            .normal
            .iter()
            .find(|x| **x != Q::ZERO)
            .cloned()
            .unwrap_or(Q::ONE);
        Plane {
            normal: times(&self.normal, &(Q::ONE / &lead)),
            offset: &self.offset / &lead,
        }
    }
}

/// A planar polygon, its corners in order, counter-clockwise seen from the side it faces.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Polygon {
    corners: Vec<Point>,
}

/// Each corner with the next, the last with the first.
fn edges(corners: &[Point]) -> impl Iterator<Item = (&Point, &Point)> {
    corners.iter().zip(corners.iter().cycle().skip(1))
}

/// Newell's sum: twice the polygon's area vector.
fn area_vector(corners: &[Point]) -> Point {
    let mut sum = [Q::ZERO, Q::ZERO, Q::ZERO];
    for (a, b) in edges(corners) {
        sum = plus(&sum, &cross(a, b));
    }
    sum
}

/// The two coordinates that keep a polygon of normal `n` from collapsing: the axis of a nonzero
/// normal coordinate is dropped.
fn axes(n: &Point) -> (usize, usize) {
    let [x, y, _] = n;
    if *x != Q::ZERO {
        (1, 2)
    } else if *y != Q::ZERO {
        (2, 0)
    } else {
        (0, 1)
    }
}

/// `p`'s coordinate `i`, where `i` is below 3.
fn at(p: &Point, i: usize) -> &Q {
    let [x, y, z] = p;
    match i {
        0 => x,
        1 => y,
        _ => z,
    }
}

impl Polygon {
    /// The polygon on `corners`.
    ///
    /// # Errors
    ///
    /// [`ReferenceError::NoArea`] or [`ReferenceError::NotPlanar`].
    pub fn new(corners: Vec<Point>) -> Result<Polygon, ReferenceError> {
        if corners.len() < 3 {
            return Err(ReferenceError::NoArea);
        }
        let n = area_vector(&corners);
        if is_zero(&n) {
            return Err(ReferenceError::NoArea);
        }
        let first = corners.first().ok_or(ReferenceError::NoArea)?;
        let level = dot(&n, first);
        if corners.iter().any(|c| dot(&n, c) != level) {
            return Err(ReferenceError::NotPlanar);
        }
        Ok(Polygon { corners })
    }

    /// The corners.
    #[must_use]
    pub fn corners(&self) -> &[Point] {
        &self.corners
    }

    /// Twice the area vector: its length is twice the area, and it points the way the polygon
    /// faces.
    #[must_use]
    pub fn area_vector(&self) -> Point {
        area_vector(&self.corners)
    }

    /// The plane the polygon lies in, its normal the way the polygon faces.
    #[must_use]
    pub fn plane(&self) -> Plane {
        let normal = self.area_vector();
        let offset = self
            .corners
            .first()
            .map_or(Q::ZERO, |first| dot(&normal, first));
        Plane { normal, offset }
    }

    /// The same polygon facing the other way.
    #[must_use]
    pub fn flipped(&self) -> Polygon {
        let mut corners = self.corners.clone();
        corners.reverse();
        Polygon { corners }
    }

    /// The corners in sorted order, the same for any start and either direction.
    fn key(&self) -> Vec<Point> {
        let mut key = self.corners.clone();
        key.sort();
        key
    }

    /// Where `p`, known to be in the polygon's plane, lies in it, by crossing parity: `On` for its
    /// outline. The polygon need not be convex.
    fn locate_in_plane(&self, p: &Point) -> Where {
        let (u, v) = axes(&self.area_vector());
        self.locate_across(at(p, u), at(p, v), (u, v))
    }

    /// As [`Polygon::locate_in_plane`], for a point given by its coordinates `(pu, pv)` on the
    /// polygon's two axes `(u, v)` (those [`axes`] gives its normal).
    fn locate_across(&self, pu: &Q, pv: &Q, (u, v): (usize, usize)) -> Where {
        let mut inside = false;
        for (a, b) in edges(&self.corners) {
            let (au, av, bu, bv) = (at(a, u), at(a, v), at(b, u), at(b, v));
            let turn = (bu - au) * (pv - av) - (bv - av) * (pu - au);
            let within =
                pu >= au.min(bu) && pu <= au.max(bu) && pv >= av.min(bv) && pv <= av.max(bv);
            if turn == Q::ZERO && within {
                return Where::On;
            }
            if (av > pv) != (bv > pv) {
                let crossing = au + (bu - au) * (pv - av) / (bv - av);
                if *pu < crossing {
                    inside = !inside;
                }
            }
        }
        if inside { Where::In } else { Where::Out }
    }

    /// Whether `p` is in the closed polygon.
    fn holds(&self, p: &Point) -> bool {
        self.plane().side(p) == Ordering::Equal && self.locate_in_plane(p) != Where::Out
    }
}

/// A closed, oriented solid: planar polygons facing outward, every edge met by exactly one
/// opposite edge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Solid {
    faces: Vec<Polygon>,
}

impl Solid {
    /// The solid bounded by `faces`.
    ///
    /// # Errors
    ///
    /// [`ReferenceError::NotClosed`] unless every directed edge pairs with exactly one opposite
    /// edge.
    pub fn new(faces: Vec<Polygon>) -> Result<Solid, ReferenceError> {
        let mut pairs: BTreeMap<(&Point, &Point), i32> = BTreeMap::new();
        for face in &faces {
            for (a, b) in edges(face.corners()) {
                *pairs.entry((a, b)).or_insert(0) += 1;
            }
        }
        let closed = !faces.is_empty()
            && pairs
                .iter()
                .all(|((a, b), n)| *n == 1 && pairs.get(&(*b, *a)) == Some(&1));
        if closed {
            Ok(Solid { faces })
        } else {
            Err(ReferenceError::NotClosed)
        }
    }

    /// The box with corners `low` and `high`, each coordinate of `low` below `high`'s.
    ///
    /// # Errors
    ///
    /// As [`Polygon::new`], for a box with no extent.
    pub fn cuboid(low: &Point, high: &Point) -> Result<Solid, ReferenceError> {
        Solid::new(cuboid_faces(low, high)?)
    }

    /// The prism over the counter-clockwise polygon `base` from `z0` up to `z1`. `base` need not be
    /// convex.
    ///
    /// # Errors
    ///
    /// As [`Solid::new`].
    pub fn prism(base: &[(Q, Q)], z0: &Q, z1: &Q) -> Result<Solid, ReferenceError> {
        let at_z = |(x, y): &(Q, Q), z: &Q| [x.clone(), y.clone(), z.clone()];
        let bottom: Vec<Point> = base.iter().rev().map(|c| at_z(c, z0)).collect();
        let top: Vec<Point> = base.iter().map(|c| at_z(c, z1)).collect();
        let mut faces = vec![Polygon::new(bottom)?, Polygon::new(top)?];
        for (a, b) in base.iter().zip(base.iter().cycle().skip(1)) {
            faces.push(Polygon::new(vec![
                at_z(a, z0),
                at_z(b, z0),
                at_z(b, z1),
                at_z(a, z1),
            ])?);
        }
        Solid::new(faces)
    }

    /// The tetrahedron on four points in general position.
    ///
    /// # Errors
    ///
    /// [`ReferenceError::NoArea`] for four points in one plane.
    pub fn tetrahedron(corners: [Point; 4]) -> Result<Solid, ReferenceError> {
        let [a, b, c, d] = corners;
        let sign = dot(&cross(&minus(&b, &a), &minus(&c, &a)), &minus(&d, &a));
        if sign == Q::ZERO {
            return Err(ReferenceError::NoArea);
        }
        let (b, c) = if sign > Q::ZERO { (c, b) } else { (b, c) };
        // With d below the face a, b, c as written, each face below faces out.
        Solid::new(vec![
            Polygon::new(vec![a.clone(), b.clone(), c.clone()])?,
            Polygon::new(vec![a.clone(), d.clone(), b.clone()])?,
            Polygon::new(vec![b.clone(), d.clone(), c.clone()])?,
            Polygon::new(vec![c, d, a])?,
        ])
    }

    /// The faces.
    #[must_use]
    pub fn faces(&self) -> &[Polygon] {
        &self.faces
    }

    /// The same solid moved by `by`.
    #[must_use]
    pub fn moved(&self, by: &Point) -> Solid {
        let faces = self
            .faces
            .iter()
            .map(|face| Polygon {
                corners: face.corners.iter().map(|c| plus(c, by)).collect(),
            })
            .collect();
        Solid { faces }
    }

    /// Every distinct corner.
    #[must_use]
    pub fn corners(&self) -> Vec<Point> {
        let set: BTreeSet<&Point> = self.faces.iter().flat_map(Polygon::corners).collect();
        set.into_iter().cloned().collect()
    }

    /// Each edge once, as its two corners in sorted order.
    fn edges(&self) -> Vec<(Point, Point)> {
        let mut set = BTreeSet::new();
        for face in &self.faces {
            for (a, b) in edges(face.corners()) {
                let pair = if a < b { (a, b) } else { (b, a) };
                set.insert(pair);
            }
        }
        set.into_iter()
            .map(|(a, b)| (a.clone(), b.clone()))
            .collect()
    }

    /// Where `p` lies: on a face, or by the parity of a ray's crossings.
    #[must_use]
    pub fn locate(&self, p: &Point) -> Where {
        if self.faces.iter().any(|face| face.holds(p)) {
            return Where::On;
        }
        // A ray that meets no outline and lies in no face plane: each direction misses a finite
        // set of bad slopes, so a later one always serves.
        for k in 1..u64::MAX {
            let direction = [Q::ONE, ratio(1, 1009 + k), ratio(1, 2003 + 7 * k)];
            if let Some(inside) = self.ray_parity(p, &direction) {
                return if inside { Where::In } else { Where::Out };
            }
        }
        Where::Out
    }

    /// Whether the ray from `p` along `direction` crosses an odd number of faces; `None` if it
    /// meets an outline or lies in a face's plane.
    fn ray_parity(&self, p: &Point, direction: &Point) -> Option<bool> {
        let mut inside = false;
        for face in &self.faces {
            let plane = face.plane();
            let slope = dot(plane.normal(), direction);
            if slope == Q::ZERO {
                if plane.side(p) == Ordering::Equal {
                    return None;
                }
                continue;
            }
            let t = (&plane.offset - dot(plane.normal(), p)) / &slope;
            if t <= Q::ZERO {
                continue;
            }
            match face.locate_in_plane(&plus(p, &times(direction, &t))) {
                Where::On => return None,
                Where::In => inside = !inside,
                Where::Out => {}
            }
        }
        Some(inside)
    }

    /// The canonical planes of its faces.
    fn planes(&self) -> BTreeSet<Plane> {
        self.faces.iter().map(|f| f.plane().canonical()).collect()
    }
}

fn cuboid_faces(low: &Point, high: &Point) -> Result<Vec<Polygon>, ReferenceError> {
    let [x0, y0, z0] = low;
    let [x1, y1, z1] = high;
    let p = |x: &Q, y: &Q, z: &Q| [x.clone(), y.clone(), z.clone()];
    Ok(vec![
        Polygon::new(vec![
            p(x0, y0, z0),
            p(x0, y1, z0),
            p(x1, y1, z0),
            p(x1, y0, z0),
        ])?,
        Polygon::new(vec![
            p(x0, y0, z1),
            p(x1, y0, z1),
            p(x1, y1, z1),
            p(x0, y1, z1),
        ])?,
        Polygon::new(vec![
            p(x0, y0, z0),
            p(x1, y0, z0),
            p(x1, y0, z1),
            p(x0, y0, z1),
        ])?,
        Polygon::new(vec![
            p(x0, y1, z0),
            p(x0, y1, z1),
            p(x1, y1, z1),
            p(x1, y1, z0),
        ])?,
        Polygon::new(vec![
            p(x0, y0, z0),
            p(x0, y0, z1),
            p(x0, y1, z1),
            p(x0, y1, z0),
        ])?,
        Polygon::new(vec![
            p(x1, y0, z0),
            p(x1, y1, z0),
            p(x1, y1, z1),
            p(x1, y0, z1),
        ])?,
    ])
}

/// A convex polytope: its faces, facing outward.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    faces: Vec<Polygon>,
}

/// The strict corners of the convex hull of `points`, all in one plane of normal `n`, in
/// counter-clockwise order seen from the side `n` points to; Andrew's monotone chain.
fn outline(points: &[Point], n: &Point) -> Vec<Point> {
    let (u, v) = axes(n);
    let mut sorted: Vec<&Point> = points.iter().collect();
    sorted.sort_by(|a, b| (at(a, u), at(a, v)).cmp(&(at(b, u), at(b, v))));
    sorted.dedup();
    let turn = |o: &Point, a: &Point, b: &Point| {
        (at(a, u) - at(o, u)) * (at(b, v) - at(o, v))
            - (at(a, v) - at(o, v)) * (at(b, u) - at(o, u))
    };
    let chain = |points: &mut dyn Iterator<Item = &&Point>| {
        let mut hull: Vec<Point> = Vec::new();
        for p in points {
            while let [.., a, b] = hull.as_slice() {
                if turn(a, b, p) > Q::ZERO {
                    break;
                }
                hull.pop();
            }
            hull.push((*p).clone());
        }
        hull.pop();
        hull
    };
    let mut hull = chain(&mut sorted.iter());
    hull.extend(chain(&mut sorted.iter().rev()));
    // (u, v, dropped) is a right-handed frame only when the dropped coordinate is positive.
    let flip = dot(&area_vector(&hull), n) < Q::ZERO;
    if flip {
        hull.reverse();
    }
    hull
}

impl Cell {
    /// The faces.
    #[must_use]
    pub fn faces(&self) -> &[Polygon] {
        &self.faces
    }

    /// Every distinct corner.
    #[must_use]
    pub fn corners(&self) -> Vec<Point> {
        let set: BTreeSet<&Point> = self.faces.iter().flat_map(Polygon::corners).collect();
        set.into_iter().cloned().collect()
    }

    /// A point strictly inside: the mean of its corners, in a convex cell with volume.
    #[must_use]
    pub fn inner(&self) -> Point {
        let corners = self.corners();
        let mut sum = [Q::ZERO, Q::ZERO, Q::ZERO];
        for c in &corners {
            sum = plus(&sum, c);
        }
        let count = u64::try_from(corners.len()).unwrap_or(1);
        times(&sum, &ratio(1, count))
    }

    /// The volume and first moment, by the divergence theorem over a fan of tetrahedra from the
    /// origin: each tetrahedron's signed volume, and that volume times its centroid.
    fn mass(&self) -> (Q, Point) {
        let mut volume = Q::ZERO;
        let mut moment = [Q::ZERO, Q::ZERO, Q::ZERO];
        for face in &self.faces {
            let Some((first, rest)) = face.corners.split_first() else {
                continue;
            };
            for pair in rest.windows(2) {
                let [b, c] = pair else { continue };
                let v = dot(first, &cross(b, c)) * ratio(1, 6);
                let sum = plus(&plus(first, b), c);
                moment = plus(&moment, &times(&sum, &(&v * ratio(1, 4))));
                volume += v;
            }
        }
        (volume, moment)
    }

    /// The volume.
    #[must_use]
    pub fn volume(&self) -> Q {
        self.mass().0
    }

    /// The parts of the cell on each side of `plane`: `(negative, positive)`.
    fn split(&self, plane: &Plane) -> (Option<Cell>, Option<Cell>) {
        let sides: Vec<Ordering> = self.corners().iter().map(|c| plane.side(c)).collect();
        if !sides.contains(&Ordering::Greater) {
            return (Some(self.clone()), None);
        }
        if !sides.contains(&Ordering::Less) {
            return (None, Some(self.clone()));
        }
        let (mut below, mut above) = (Vec::new(), Vec::new());
        let mut cut: Vec<Point> = Vec::new();
        for face in &self.faces {
            let (low, high, on) = clip(face, plane);
            cut.extend(on);
            below.extend(low);
            above.extend(high);
        }
        let n = plane.normal();
        let lid = outline(&cut, n);
        let top = Polygon { corners: lid };
        below.push(top.clone());
        above.push(top.flipped());
        (Some(Cell { faces: below }), Some(Cell { faces: above }))
    }
}

/// Sutherland and Hodgman's clip of `face` by `plane`: the part on each side, and the points of the
/// face on the plane.
fn clip(face: &Polygon, plane: &Plane) -> (Option<Polygon>, Option<Polygon>, Vec<Point>) {
    let (mut low, mut high, mut on) = (Vec::new(), Vec::new(), Vec::new());
    for (a, b) in edges(face.corners()) {
        let (sa, sb) = (plane.side(a), plane.side(b));
        match sa {
            Ordering::Less => low.push(a.clone()),
            Ordering::Greater => high.push(a.clone()),
            Ordering::Equal => {
                low.push(a.clone());
                high.push(a.clone());
                on.push(a.clone());
            }
        }
        if sa != Ordering::Equal && sb != Ordering::Equal && sa != sb {
            let (da, db) = (
                dot(plane.normal(), a) - &plane.offset,
                dot(plane.normal(), b) - &plane.offset,
            );
            let t = &da / (&da - db);
            let x = plus(a, &times(&minus(b, a), &t));
            low.push(x.clone());
            high.push(x.clone());
            on.push(x);
        }
    }
    let piece = |corners: Vec<Point>| {
        let has_area = corners.len() >= 3 && !is_zero(&area_vector(&corners));
        has_area.then_some(Polygon { corners })
    };
    (piece(low), piece(high), on)
}

/// The arrangement of `planes` inside the box from `low` to `high`: every plane cuts every cell.
///
/// # Errors
///
/// As [`Solid::cuboid`], for a box with no extent.
pub fn arrangement(
    planes: &[Plane],
    low: &Point,
    high: &Point,
) -> Result<Vec<Cell>, ReferenceError> {
    let distinct: BTreeSet<Plane> = planes.iter().map(Plane::canonical).collect();
    let mut cells = vec![Cell {
        faces: cuboid_faces(low, high)?,
    }];
    for plane in &distinct {
        let mut next = Vec::with_capacity(cells.len().saturating_mul(2));
        for cell in &cells {
            let (below, above) = cell.split(plane);
            next.extend(below);
            next.extend(above);
        }
        cells = next;
    }
    Ok(cells)
}

/// A box a whole unit larger, on every side, than everything in `solids`.
fn surround(solids: &[&Solid]) -> Option<(Point, Point)> {
    let mut corners = solids.iter().flat_map(|s| s.corners());
    let first = corners.next()?;
    let (mut low, mut high) = (first.clone(), first);
    for c in corners {
        for i in 0..3 {
            let (l, h) = (low.get_mut(i)?, high.get_mut(i)?);
            let x = at(&c, i);
            if x < &*l {
                l.clone_from(x);
            }
            if x > &*h {
                h.clone_from(x);
            }
        }
    }
    let one = [Q::ONE, Q::ONE, Q::ONE];
    Some((minus(&low, &one), plus(&high, &one)))
}

/// The arrangement of every face plane of `solids` and `extra`, each cell with which solids it
/// lies in, in `solids`' order.
///
/// # Errors
///
/// [`ReferenceError::NoArea`] for no solids at all.
pub fn labelled_cells(
    solids: &[&Solid],
    extra: &[Plane],
) -> Result<Vec<(Cell, Vec<bool>)>, ReferenceError> {
    let (low, high) = surround(solids).ok_or(ReferenceError::NoArea)?;
    let mut planes: Vec<Plane> = solids.iter().flat_map(|s| s.planes()).collect();
    planes.extend_from_slice(extra);
    let cells = arrangement(&planes, &low, &high)?;
    Ok(cells
        .into_iter()
        .map(|cell| {
            let inner = cell.inner();
            let inside = solids
                .iter()
                .map(|s| s.locate(&inner) == Where::In)
                .collect();
            (cell, inside)
        })
        .collect())
}

/// A face as a ray meets it: its polygon, plane and canonical plane, the two axes it is located
/// on, and its box in every coordinate.
struct Target {
    polygon: Polygon,
    plane: Plane,
    key: Plane,
    axes: (usize, usize),
    low: [Q; 3],
    high: [Q; 3],
}

/// The least and greatest of `values`, or zeros for none.
fn span<'q>(values: impl Iterator<Item = &'q Q>) -> (Q, Q) {
    let mut out: Option<(Q, Q)> = None;
    for v in values {
        out = Some(match out {
            None => (v.clone(), v.clone()),
            Some((lo, hi)) => (lo.min(v.clone()), hi.max(v.clone())),
        });
    }
    out.unwrap_or((Q::ZERO, Q::ZERO))
}

impl Target {
    fn of(face: &Polygon) -> Target {
        let [(x0, x1), (y0, y1), (z0, z1)] =
            [0, 1, 2].map(|k| span(face.corners.iter().map(|c| at(c, k))));
        let plane = face.plane();
        Target {
            polygon: face.clone(),
            key: plane.canonical(),
            axes: axes(plane.normal()),
            plane,
            low: [x0, y0, z0],
            high: [x1, y1, z1],
        }
    }

    /// Whether the two boxes meet, touching included.
    fn meets(&self, other: &Target) -> bool {
        (0..3).all(|k| {
            at(&self.low, k) <= at(&other.high, k) && at(&other.low, k) <= at(&self.high, k)
        })
    }
}

/// The two coordinates other than `axis`.
fn across(axis: usize) -> (usize, usize) {
    match axis {
        0 => (1, 2),
        1 => (2, 0),
        _ => (0, 1),
    }
}

/// A solid's faces filed by where they lie across one axis, so that a ray along it from a point
/// meets only the faces of the point's bucket: a grid of `m` by `m` buckets over the faces' extent
/// in the other two coordinates, `m²` about the number of faces, a face filed in every bucket its
/// box touches.
struct Rays {
    origin: [Q; 2],
    /// `m` over the extent, in each of the other two coordinates.
    scale: [Q; 2],
    buckets: BTreeMap<(IBig, IBig), Vec<usize>>,
}

impl Rays {
    fn new(targets: &[Target], axis: usize) -> Rays {
        let (u, v) = across(axis);
        let (lu, hu) = span(targets.iter().flat_map(|t| [at(&t.low, u), at(&t.high, u)]));
        let (lv, hv) = span(targets.iter().flat_map(|t| [at(&t.low, v), at(&t.high, v)]));
        let mut m = 1_u64;
        let faces = u128::try_from(targets.len()).unwrap_or(u128::MAX);
        while u128::from(m) * u128::from(m) < faces {
            m += 1;
        }
        let per = |low: &Q, high: &Q| {
            let extent = high - low;
            if extent == Q::ZERO {
                Q::ONE
            } else {
                Q::from(IBig::from(m)) / extent
            }
        };
        let mut rays = Rays {
            scale: [per(&lu, &hu), per(&lv, &hv)],
            origin: [lu, lv],
            buckets: BTreeMap::new(),
        };
        for (i, t) in targets.iter().enumerate() {
            let (u0, v0) = rays.bucket(at(&t.low, u), at(&t.low, v));
            let (u1, v1) = rays.bucket(at(&t.high, u), at(&t.high, v));
            let mut a = u0;
            while a <= u1 {
                let mut b = v0.clone();
                while b <= v1 {
                    rays.buckets
                        .entry((a.clone(), b.clone()))
                        .or_default()
                        .push(i);
                    b += IBig::ONE;
                }
                a += IBig::ONE;
            }
        }
        rays
    }

    /// The bucket of the point whose other two coordinates are `(a, b)`.
    fn bucket(&self, a: &Q, b: &Q) -> (IBig, IBig) {
        let ([oa, ob], [sa, sb]) = (&self.origin, &self.scale);
        (((a - oa) * sa).floor(), ((b - ob) * sb).floor())
    }
}

/// A solid read for rays: its faces, its box, and its faces filed for a ray along each axis.
struct Shot {
    targets: Vec<Target>,
    low: [Q; 3],
    high: [Q; 3],
    rays: [Rays; 3],
}

impl Shot {
    fn new(solid: &Solid) -> Shot {
        let targets: Vec<Target> = solid.faces.iter().map(Target::of).collect();
        let [(x0, x1), (y0, y1), (z0, z1)] =
            [0, 1, 2].map(|k| span(targets.iter().flat_map(|t| [at(&t.low, k), at(&t.high, k)])));
        let rays = [0, 1, 2].map(|axis| Rays::new(&targets, axis));
        Shot {
            targets,
            low: [x0, y0, z0],
            high: [x1, y1, z1],
            rays,
        }
    }

    /// The winding number of the solid's surface just off `p` along the open ray from `p` along
    /// `axis`, forward when `forward`: each face the ray crosses counts one, plus where its normal
    /// points along the ray and minus where against it. Faces of the plane `own` (canonical), the
    /// one `p` lies on, are where the ray starts and do not count. `None` when the ray meets an
    /// outline, lies in a face's plane or passes through `p` on another face.
    fn along(&self, p: &Point, axis: usize, forward: bool, own: &Plane) -> Option<i64> {
        let (u, v) = across(axis);
        let (pu, pv, pa) = (at(p, u), at(p, v), at(p, axis));
        // A ray that misses the solid's box crosses none of it.
        let beyond = if forward {
            pa > at(&self.high, axis)
        } else {
            pa < at(&self.low, axis)
        };
        if beyond
            || pu < at(&self.low, u)
            || pu > at(&self.high, u)
            || pv < at(&self.low, v)
            || pv > at(&self.high, v)
        {
            return Some(0);
        }
        let rays = self.rays.get(axis)?;
        let Some(filed) = rays.buckets.get(&rays.bucket(pu, pv)) else {
            return Some(0);
        };
        let mut winding = 0_i64;
        for &i in filed {
            let t = self.targets.get(i)?;
            let behind = if forward {
                at(&t.high, axis) < pa
            } else {
                at(&t.low, axis) > pa
            };
            if behind
                || pu < at(&t.low, u)
                || pu > at(&t.high, u)
                || pv < at(&t.low, v)
                || pv > at(&t.high, v)
            {
                continue;
            }
            let n = t.plane.normal();
            let na = at(n, axis);
            if *na == Q::ZERO {
                if t.plane.side(p) == Ordering::Equal {
                    return None;
                }
                continue;
            }
            let hit = (&t.plane.offset - at(n, u) * pu - at(n, v) * pv) / na;
            let ahead = if forward { &hit - pa } else { pa - &hit };
            if ahead < Q::ZERO {
                continue;
            }
            let coordinate = |k: usize| if k == axis { &hit } else { at(p, k) };
            let (tu, tv) = t.axes;
            let found = t
                .polygon
                .locate_across(coordinate(tu), coordinate(tv), t.axes);
            if ahead == Q::ZERO {
                if t.key == *own {
                    continue;
                }
                if found != Where::Out {
                    return None;
                }
                continue;
            }
            match found {
                Where::On => return None,
                Where::In => winding += if (*na > Q::ZERO) == forward { 1 } else { -1 },
                Where::Out => {}
            }
        }
        Some(winding)
    }

    /// As [`Shot::along`] for a slanted ray in `direction`, against every face.
    fn slanted(&self, p: &Point, direction: &Point, own: &Plane) -> Option<i64> {
        let mut winding = 0_i64;
        for t in &self.targets {
            let slope = dot(t.plane.normal(), direction);
            if slope == Q::ZERO {
                if t.plane.side(p) == Ordering::Equal {
                    return None;
                }
                continue;
            }
            let s = (&t.plane.offset - dot(t.plane.normal(), p)) / &slope;
            if s < Q::ZERO {
                continue;
            }
            let found = t.polygon.locate_in_plane(&plus(p, &times(direction, &s)));
            if s == Q::ZERO {
                if t.key == *own {
                    continue;
                }
                if found != Where::Out {
                    return None;
                }
                continue;
            }
            match found {
                Where::On => return None,
                Where::In => winding += if slope > Q::ZERO { 1 } else { -1 },
                Where::Out => {}
            }
        }
        Some(winding)
    }
}

/// Whether the faces `f` and `g`, of different planes, meet along a stretch of the line where their
/// planes cross: `g` cuts `f` there, so the winding numbers beside `f` may change across it. Faces
/// that touch at a point, or whose planes cross away from one of them, do not.
fn crosses(f: &Target, g: &Target) -> bool {
    let reaches = |t: &Target, plane: &Plane| {
        let s: BTreeSet<Ordering> = t.polygon.corners().iter().map(|c| plane.side(c)).collect();
        s.len() > 1 || s.contains(&Ordering::Equal)
    };
    if !reaches(f, &g.plane) || !reaches(g, &f.plane) {
        return false;
    }
    let (p, r) = (&f.plane, &g.plane);
    let along = cross(p.normal(), r.normal());
    if is_zero(&along) {
        return false;
    }
    let n1n2 = cross(r.normal(), &along);
    let n3n1 = cross(&along, p.normal());
    let denominator = dot(&along, &along);
    let from = times(
        &plus(&times(&n1n2, &p.offset), &times(&n3n1, &r.offset)),
        &(Q::ONE / denominator),
    );
    let on_g = line_in_polygon(&g.polygon, &from, &along);
    line_in_polygon(&f.polygon, &from, &along)
        .iter()
        .any(|(s0, s1)| {
            on_g.iter()
                .any(|(t0, t1)| s0.clone().max(t0.clone()) < s1.clone().min(t1.clone()))
        })
}

/// The winding numbers of every solid just in front of a face piece, on the side its normal
/// points to, read at a point of the open piece: along an axis its plane is not parallel to, from
/// the piece's mean or a point moved from it toward a corner when that ray meets an outline, and
/// along slanted rays when every one does.
fn in_front(shots: &[Shot], piece: &Polygon, plane: &Plane, own: &Plane) -> Option<Vec<i64>> {
    let n = plane.normal();
    let axis = (0..3).find(|k| *at(n, *k) != Q::ZERO)?;
    let ahead = *at(n, axis) > Q::ZERO;
    let corners = piece.corners();
    let count = u64::try_from(corners.len()).ok()?;
    let mut mean = [Q::ZERO, Q::ZERO, Q::ZERO];
    for c in corners {
        mean = plus(&mean, c);
    }
    let mean = times(&mean, &ratio(1, count));
    let three = int(3);
    let points = core::iter::once(mean.clone()).chain(
        corners
            .iter()
            .map(|c| times(&plus(&times(&mean, &three), c), &ratio(1, 4))),
    );
    for p in points {
        let front: Option<Vec<i64>> = shots
            .iter()
            .map(|s| s.along(&p, axis, ahead, own))
            .collect();
        if front.is_some() {
            return front;
        }
    }
    for k in 1..u64::MAX {
        let d = [Q::ONE, ratio(1, 1009 + k), ratio(1, 2003 + 7 * k)];
        let along = dot(&d, n);
        if along == Q::ZERO {
            continue;
        }
        let forth = if along > Q::ZERO {
            d
        } else {
            times(&d, &int(-1))
        };
        let front: Option<Vec<i64>> = shots
            .iter()
            .map(|s| s.slanted(&mean, &forth, own))
            .collect();
        if front.is_some() {
            return front;
        }
    }
    None
}

/// Six times the volume where `keep` holds of the winding numbers of `solids`' surfaces, in
/// `solids`' order: the regularised result of any boolean of them, by evaluating its boundary.
///
/// What it does: every face is cut, as a polygon, by the plane of every other face (of any solid,
/// its own included) that meets it along a stretch of a line, and by the planes
/// along the edges of every face in its own plane whose box it meets; the pieces are convex and
/// no face crosses one's inside. Just in front of a piece each solid has one winding number, read
/// by a ray; just behind it each is more by one for each face of its plane over the piece that
/// faces the piece's way, and less by one for each that faces the other way. The piece is on the
/// result's boundary when `keep` differs on its two sides, facing the way that leaves the kept
/// side, and where faces of one plane overlap, only the first face of them, in `solids`' and their
/// faces' order, counts the piece. The volume is the divergence theorem's sum over the boundary
/// pieces.
///
/// The winding number counts a surface that crosses itself (a mesh read from a file may) as the
/// regularised booleans of 11.1 and 19.5 do: a point is inside a solid where its winding number is
/// above zero. Where [`labelled_cells`] cuts space by every plane, this cuts only the faces, so it
/// serves solids of thousands of faces.
///
/// # Errors
///
/// [`ReferenceError::NoArea`] if `keep` holds outside every solid (the result would be unbounded)
/// or a piece has no point a ray can read, which a closed solid cannot give.
pub fn six_volume_where(
    solids: &[&Solid],
    keep: impl Fn(&[i64]) -> bool,
) -> Result<Q, ReferenceError> {
    let one: [Test<'_>; 1] = [&keep];
    let mut out = six_volumes_where(solids, &one)?;
    out.pop().ok_or(ReferenceError::NoArea)
}

/// [`six_volume_where`] for several tests at once, one volume each, in order: the faces are cut and
/// each piece's winding numbers read once for all of them.
///
/// # Errors
///
/// As [`six_volume_where`], for any of the tests.
pub fn six_volumes_where(solids: &[&Solid], keeps: &[Test<'_>]) -> Result<Vec<Q>, ReferenceError> {
    if keeps.iter().any(|keep| keep(&vec![0; solids.len()])) {
        return Err(ReferenceError::NoArea);
    }
    let shots: Vec<Shot> = solids.iter().map(|s| Shot::new(s)).collect();
    let mut all: Vec<Face<'_>> = Vec::new();
    for (s, shot) in shots.iter().enumerate() {
        for (f, t) in shot.targets.iter().enumerate() {
            all.push((s, f, t));
        }
    }
    let near = near_pairs(&all);
    let mut sixes = vec![Q::ZERO; keeps.len()];
    for (i, (s, f, t)) in all.iter().enumerate() {
        let (pieces, level) =
            pieces_of(t, near.get(i).map(Vec::as_slice).unwrap_or_default(), &all);
        for piece in &pieces {
            let mut middle = [Q::ZERO, Q::ZERO, Q::ZERO];
            for c in piece.corners() {
                middle = plus(&middle, c);
            }
            let count = u64::try_from(piece.corners().len()).unwrap_or(1);
            let middle = times(&middle, &ratio(1, count));
            let over: Vec<&Face<'_>> = level
                .iter()
                .filter_map(|&j| all.get(j))
                .filter(|(_, _, g)| g.polygon.holds(&middle))
                .collect();
            // Where faces of one plane overlap, the first of them counts the piece.
            if over.iter().any(|(gs, gf, _)| (*gs, *gf) < (*s, *f)) {
                continue;
            }
            let front = in_front(&shots, piece, &t.plane, &t.key).ok_or(ReferenceError::NoArea)?;
            let mut back = front.clone();
            if let Some(w) = back.get_mut(*s) {
                *w += 1;
            }
            for (gs, _, g) in &over {
                if let Some(w) = back.get_mut(*gs) {
                    *w += if dot(g.plane.normal(), t.plane.normal()) > Q::ZERO {
                        1
                    } else {
                        -1
                    };
                }
            }
            let mut fan: Option<Q> = None;
            for (keep, six) in keeps.iter().zip(&mut sixes) {
                let (inside, outside) = (keep(&back), keep(&front));
                if inside == outside {
                    continue;
                }
                let area = fan.get_or_insert_with(|| fan_of(piece));
                if inside {
                    *six += &*area;
                } else {
                    *six -= &*area;
                }
            }
        }
    }
    Ok(sixes)
}

/// A face, by its solid and its row there.
type Face<'t> = (usize, usize, &'t Target);

/// A test on the winding numbers of solids, in their order.
pub type Test<'k> = &'k dyn Fn(&[i64]) -> bool;

/// Six times the volume of the fan of tetrahedra from the origin over a polygon.
fn fan_of(piece: &Polygon) -> Q {
    let mut sum = Q::ZERO;
    if let Some((first, rest)) = piece.corners().split_first() {
        for pair in rest.windows(2) {
            if let [b, c] = pair {
                sum += dot(first, &cross(b, c));
            }
        }
    }
    sum
}

/// For each face, the faces whose boxes meet its box, by a sweep along x.
fn near_pairs(all: &[Face<'_>]) -> Vec<Vec<usize>> {
    let mut order: Vec<usize> = (0..all.len()).collect();
    order.sort_by(|a, b| {
        let key = |i: &usize| all.get(*i).map(|(_, _, t)| at(&t.low, 0));
        key(a).cmp(&key(b))
    });
    let mut near: Vec<Vec<usize>> = vec![Vec::new(); all.len()];
    for (k, &i) in order.iter().enumerate() {
        let Some((_, _, ti)) = all.get(i) else {
            continue;
        };
        for &j in order.iter().skip(k + 1) {
            let Some((_, _, tj)) = all.get(j) else {
                continue;
            };
            if at(&tj.low, 0) > at(&ti.high, 0) {
                break;
            }
            if ti.meets(tj) {
                if let Some(list) = near.get_mut(i) {
                    list.push(j);
                }
                if let Some(list) = near.get_mut(j) {
                    list.push(i);
                }
            }
        }
    }
    near
}

/// Face `t` cut into convex pieces no other face crosses, by the planes of the faces `near` it
/// that cross it and the edges of those in its own plane; with those of its own plane.
fn pieces_of(t: &Target, near: &[usize], all: &[Face<'_>]) -> (Vec<Polygon>, Vec<usize>) {
    let mut cuts: BTreeSet<Plane> = BTreeSet::new();
    let mut level: Vec<usize> = Vec::new();
    for &j in near {
        let Some((_, _, g)) = all.get(j) else {
            continue;
        };
        if g.key == t.key {
            level.push(j);
            for (a, b) in edges(g.polygon.corners()) {
                let normal = cross(&minus(b, a), t.plane.normal());
                if !is_zero(&normal) {
                    let offset = dot(&normal, a);
                    cuts.insert(Plane { normal, offset }.canonical());
                }
            }
            continue;
        }
        if crosses(t, g) {
            cuts.insert(g.key.clone());
        }
    }
    let mut pieces = vec![t.polygon.clone()];
    for cut in &cuts {
        let mut next = Vec::with_capacity(pieces.len());
        for piece in pieces {
            let across_cut: Vec<Ordering> = piece.corners().iter().map(|c| cut.side(c)).collect();
            if across_cut.contains(&Ordering::Less) && across_cut.contains(&Ordering::Greater) {
                let (low, high, _) = clip(&piece, cut);
                next.extend(low);
                next.extend(high);
            } else {
                next.push(piece);
            }
        }
        pieces = next;
    }
    (pieces, level)
}

impl Solid {
    /// The winding number of the solid's surface about `p`, a point on none of its faces, by a ray
    /// in a slanted direction as [`Solid::locate`] casts it: each face crossed counts one, plus
    /// where its normal points along the ray and minus where against it. `None` for a point on a
    /// face. For a solid whose faces do not cross, it is 1 inside and 0 outside.
    #[must_use]
    pub fn winding(&self, p: &Point) -> Option<i64> {
        if self.faces.iter().any(|face| face.holds(p)) {
            return None;
        }
        for k in 1..u64::MAX {
            let direction = [Q::ONE, ratio(1, 1009 + k), ratio(1, 2003 + 7 * k)];
            if let Some(w) = self.ray_winding(p, &direction) {
                return Some(w);
            }
        }
        None
    }

    /// The signed count of faces the ray from `p` along `direction` crosses; `None` if it meets an
    /// outline or lies in a face's plane.
    fn ray_winding(&self, p: &Point, direction: &Point) -> Option<i64> {
        let mut winding = 0_i64;
        for face in &self.faces {
            let plane = face.plane();
            let slope = dot(plane.normal(), direction);
            if slope == Q::ZERO {
                if plane.side(p) == Ordering::Equal {
                    return None;
                }
                continue;
            }
            let t = (&plane.offset - dot(plane.normal(), p)) / &slope;
            if t <= Q::ZERO {
                continue;
            }
            match face.locate_in_plane(&plus(p, &times(direction, &t))) {
                Where::On => return None,
                Where::In => winding += if slope > Q::ZERO { 1 } else { -1 },
                Where::Out => {}
            }
        }
        Some(winding)
    }
}

/// A boolean operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// In either.
    Union,
    /// In both.
    Intersection,
    /// In the first and not the second.
    Difference,
    /// In exactly one.
    Exclusive,
}

impl Op {
    fn keeps(self, a: bool, b: bool) -> bool {
        match self {
            Op::Union => a || b,
            Op::Intersection => a && b,
            Op::Difference => a && !b,
            Op::Exclusive => a != b,
        }
    }
}

/// A set of arrangement cells: what a boolean leaves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    cells: Vec<Cell>,
}

impl Region {
    /// The cells.
    #[must_use]
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// The volume.
    #[must_use]
    pub fn volume(&self) -> Q {
        self.cells
            .iter()
            .map(Cell::volume)
            .fold(Q::ZERO, |a, b| a + b)
    }

    /// The centroid; `None` for an empty region.
    #[must_use]
    pub fn centroid(&self) -> Option<Point> {
        let mut volume = Q::ZERO;
        let mut moment = [Q::ZERO, Q::ZERO, Q::ZERO];
        for cell in &self.cells {
            let (v, m) = cell.mass();
            volume += v;
            moment = plus(&moment, &m);
        }
        (volume != Q::ZERO).then(|| times(&moment, &(Q::ONE / volume)))
    }

    /// The boundary: the cell faces with a kept cell on one side only. An arrangement's cells meet
    /// face to face, so a face seen twice has kept cells on both sides.
    #[must_use]
    pub fn boundary(&self) -> Vec<Polygon> {
        let mut seen: BTreeMap<Vec<Point>, (u32, &Polygon)> = BTreeMap::new();
        for face in self.cells.iter().flat_map(Cell::faces) {
            let entry = seen.entry(face.key()).or_insert((0, face));
            entry.0 += 1;
        }
        seen.into_values()
            .filter(|(count, _)| *count == 1)
            .map(|(_, face)| face.clone())
            .collect()
    }

    /// The boundary's twice-areas by plane: for each canonical plane, the total of `s` where a
    /// boundary piece's area vector is `s` times the canonical normal, taken positive whichever way
    /// it faces. A plane's true area is that total times half the canonical normal's length, which
    /// is why the exact part is kept apart from the square root.
    #[must_use]
    pub fn areas(&self) -> BTreeMap<Plane, Q> {
        let mut out: BTreeMap<Plane, Q> = BTreeMap::new();
        for face in self.boundary() {
            let plane = face.plane().canonical();
            let vector = face.area_vector();
            let along = dot(&vector, plane.normal()) / dot(plane.normal(), plane.normal());
            let size = if along < Q::ZERO { -along } else { along };
            *out.entry(plane).or_insert(Q::ZERO) += size;
        }
        out
    }
}

/// `a op b`, regularised.
///
/// # Errors
///
/// As [`labelled_cells`].
pub fn boolean(a: &Solid, b: &Solid, op: Op) -> Result<Region, ReferenceError> {
    let cells = labelled_cells(&[a, b], &[])?
        .into_iter()
        .filter(|(_, inside)| matches!(inside.as_slice(), [x, y] if op.keeps(*x, *y)))
        .map(|(cell, _)| cell)
        .collect();
    Ok(Region { cells })
}

/// The solid itself as a region of its arrangement.
///
/// # Errors
///
/// As [`labelled_cells`].
pub fn region(solid: &Solid) -> Result<Region, ReferenceError> {
    boolean(solid, solid, Op::Intersection)
}

/// The closed pieces of `solid` in `plane`: the arrangement's cell faces on the plane with the solid
/// on a side of them, each once, facing the plane's way.
///
/// # Errors
///
/// As [`labelled_cells`].
pub fn section(solid: &Solid, plane: &Plane) -> Result<Vec<Polygon>, ReferenceError> {
    let cells = labelled_cells(&[solid], std::slice::from_ref(plane))?;
    let canonical = plane.canonical();
    let mut pieces: BTreeMap<Vec<Point>, Polygon> = BTreeMap::new();
    for (cell, inside) in &cells {
        if inside.first() != Some(&true) {
            continue;
        }
        for face in cell.faces() {
            if face.plane().canonical() == canonical {
                let facing = dot(&face.area_vector(), plane.normal()) > Q::ZERO;
                let piece = if facing { face.clone() } else { face.flipped() };
                pieces.insert(piece.key(), piece);
            }
        }
    }
    Ok(pieces.into_values().collect())
}

/// Twice the total area of `pieces`, all in `plane`, as the total of `s` where each area vector is
/// `s` times the plane's canonical normal.
#[must_use]
pub fn twice_area(pieces: &[Polygon], plane: &Plane) -> Q {
    let canonical = plane.canonical();
    let n = canonical.normal();
    pieces.iter().fold(Q::ZERO, |sum, piece| {
        let along = dot(&piece.area_vector(), n) / dot(n, n);
        sum + if along < Q::ZERO { -along } else { along }
    })
}

/// A straight piece, its ends in sorted order.
pub type Segment = (Point, Point);

/// The parts of the line `from + s · along` in the closed polygon, as sorted intervals of `s`, by
/// the line's crossings of its outline.
fn line_in_polygon(polygon: &Polygon, from: &Point, along: &Point) -> Vec<(Q, Q)> {
    let n = polygon.area_vector();
    let (u, v) = axes(&n);
    let mut stops: BTreeSet<Q> = BTreeSet::new();
    for (a, b) in edges(polygon.corners()) {
        let edge = minus(b, a);
        let denominator = at(along, u) * at(&edge, v) - at(along, v) * at(&edge, u);
        // An edge along the line adds no stop: the edges either side of its run cross there.
        if denominator == Q::ZERO {
            continue;
        }
        let gap = minus(a, from);
        let s = (at(&gap, u) * at(&edge, v) - at(&gap, v) * at(&edge, u)) / &denominator;
        let w = (at(&gap, u) * at(along, v) - at(&gap, v) * at(along, u)) / &denominator;
        if w >= Q::ZERO && w <= Q::ONE {
            stops.insert(s);
        }
    }
    let stops: Vec<Q> = stops.into_iter().collect();
    stops
        .windows(2)
        .filter_map(|w| {
            let [a, b] = w else { return None };
            let middle = (a + b) * ratio(1, 2);
            let p = plus(from, &times(along, &middle));
            (polygon.locate_in_plane(&p) != Where::Out).then(|| (a.clone(), b.clone()))
        })
        .collect()
}

/// The segments where `a`'s faces cross `b`'s, testing every pair of faces whose planes differ:
/// the edges an imprint of `b` on `a` adds, each once, with touching runs joined.
#[must_use]
pub fn imprint(a: &Solid, b: &Solid) -> Vec<Segment> {
    let mut found: BTreeSet<Segment> = BTreeSet::new();
    for f in a.faces() {
        for g in b.faces() {
            let (p, r) = (f.plane(), g.plane());
            let along = cross(p.normal(), r.normal());
            if is_zero(&along) {
                continue;
            }
            // A point of both planes, on the plane through the origin normal to the line.
            let n1n2 = cross(r.normal(), &along);
            let n3n1 = cross(&along, p.normal());
            let denominator = dot(&along, &along);
            let from = times(
                &plus(&times(&n1n2, &p.offset), &times(&n3n1, &r.offset)),
                &(Q::ONE / denominator),
            );
            for (s0, s1) in line_in_polygon(f, &from, &along) {
                for (t0, t1) in line_in_polygon(g, &from, &along) {
                    let (low, high) = (s0.clone().max(t0.clone()), s1.clone().min(t1));
                    if low < high {
                        let (x, y) = (
                            plus(&from, &times(&along, &low)),
                            plus(&from, &times(&along, &high)),
                        );
                        found.insert(if x < y { (x, y) } else { (y, x) });
                    }
                }
            }
        }
    }
    found.into_iter().collect()
}

/// How two solids stand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relation {
    /// No shared point.
    Apart,
    /// Shared boundary points, no shared volume.
    Touching,
    /// Shared volume, and neither inside the other.
    Overlapping,
    /// The first holds all of the second.
    Contains,
    /// The second holds all of the first.
    Within,
}

/// Sorts `a` and `b`. Their closed intersection is empty, or has an extreme point, which is a
/// vertex of the arrangement of their face planes; so the cells' corners decide.
///
/// # Errors
///
/// As [`labelled_cells`].
pub fn clash(a: &Solid, b: &Solid) -> Result<Relation, ReferenceError> {
    let cells = labelled_cells(&[a, b], &[])?;
    let mut shared = false;
    let (mut a_alone, mut b_alone) = (false, false);
    for (_, inside) in &cells {
        match inside.as_slice() {
            [true, true] => shared = true,
            [true, false] => a_alone = true,
            [false, true] => b_alone = true,
            _ => {}
        }
    }
    if shared {
        return Ok(match (a_alone, b_alone) {
            (true, true) => Relation::Overlapping,
            (true, false) => Relation::Contains,
            (false, _) => Relation::Within,
        });
    }
    let touching = cells
        .iter()
        .flat_map(|(cell, _)| cell.corners())
        .any(|c| a.locate(&c) != Where::Out && b.locate(&c) != Where::Out);
    Ok(if touching {
        Relation::Touching
    } else {
        Relation::Apart
    })
}

/// Clash of every pair of `solids`, as the pairs that share a point, by index.
///
/// # Errors
///
/// As [`clash`].
pub fn clashes(solids: &[Solid]) -> Result<Vec<(usize, usize)>, ReferenceError> {
    let mut found = Vec::new();
    for (i, a) in solids.iter().enumerate() {
        for (j, b) in solids.iter().enumerate().skip(i.saturating_add(1)) {
            if clash(a, b)? != Relation::Apart {
                found.push((i, j));
            }
        }
    }
    Ok(found)
}

/// Whether the closed segments `a` and `b` share a point, both in one plane of normal `n`.
fn segments_meet(a: &(Point, Point), b: &(Point, Point), n: &Point) -> bool {
    let (u, v) = axes(n);
    let orient = |o: &Point, p: &Point, q: &Point| {
        ((at(p, u) - at(o, u)) * (at(q, v) - at(o, v))
            - (at(p, v) - at(o, v)) * (at(q, u) - at(o, u)))
        .cmp(&Q::ZERO)
    };
    let within = |o: &Point, p: &Point, q: &Point| {
        at(q, u) >= at(o, u).min(at(p, u))
            && at(q, u) <= at(o, u).max(at(p, u))
            && at(q, v) >= at(o, v).min(at(p, v))
            && at(q, v) <= at(o, v).max(at(p, v))
    };
    let (p, q) = (&a.0, &a.1);
    let (r, s) = (&b.0, &b.1);
    let (d1, d2, d3, d4) = (
        orient(r, s, p),
        orient(r, s, q),
        orient(p, q, r),
        orient(p, q, s),
    );
    let opposite =
        |x: Ordering, y: Ordering| x != y && x != Ordering::Equal && y != Ordering::Equal;
    (opposite(d1, d2) && opposite(d3, d4))
        || (d1 == Ordering::Equal && within(r, s, p))
        || (d2 == Ordering::Equal && within(r, s, q))
        || (d3 == Ordering::Equal && within(p, q, r))
        || (d4 == Ordering::Equal && within(p, q, s))
}

/// The first `t` in `[0, 1]` at which `moving`, moved by `t · by`, touches `still`; `None` if it
/// never does. Polyhedra first meet at a vertex on a face or an edge on an edge, so every such pair
/// gives a time, and the least that holds is the answer.
///
/// # Errors
///
/// As [`clash`].
pub fn first_contact(
    moving: &Solid,
    still: &Solid,
    by: &Point,
) -> Result<Option<Q>, ReferenceError> {
    if clash(moving, still)? != Relation::Apart {
        return Ok(Some(Q::ZERO));
    }
    let mut best: Option<Q> = None;
    let mut consider = |t: Q| {
        if t >= Q::ZERO && t <= Q::ONE && best.as_ref().is_none_or(|b| t < *b) {
            best = Some(t);
        }
    };
    // A vertex of the moving solid reaching a face of the still one, and the reverse.
    for (vertex, face, sign) in moving
        .corners()
        .iter()
        .flat_map(|v| still.faces().iter().map(move |f| (v.clone(), f, Q::ONE)))
        .chain(
            still
                .corners()
                .iter()
                .flat_map(|v| moving.faces().iter().map(move |f| (v.clone(), f, -Q::ONE))),
        )
    {
        let plane = face.plane();
        let slope = dot(plane.normal(), by) * &sign;
        if slope == Q::ZERO {
            continue;
        }
        let t = (&plane.offset - dot(plane.normal(), &vertex)) / &slope;
        if face.holds(&plus(&vertex, &times(by, &(&t * &sign)))) {
            consider(t);
        }
    }
    // An edge of each, meeting.
    for (p, q) in moving.edges() {
        let e = minus(&q, &p);
        for (r, s) in still.edges() {
            let f = minus(&s, &r);
            let n = cross(&e, &f);
            let slope = dot(&n, by);
            if slope == Q::ZERO {
                continue;
            }
            let t = dot(&n, &minus(&r, &p)) / &slope;
            let shift = times(by, &t);
            let moved = (plus(&p, &shift), plus(&q, &shift));
            if segments_meet(&moved, &(r.clone(), s.clone()), &n) {
                consider(t);
            }
        }
    }
    Ok(best)
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test cases; the rules are for kernel code"
)]
mod tests {
    use super::*;

    /// A point of the plane z = 7, off the origin so that no coordinate cancels.
    fn flat(x: i64, y: i64) -> Point {
        point(x, y, 7)
    }

    /// Whether the two segments of that plane meet.
    fn meet(a: [(i64, i64); 2], b: [(i64, i64); 2]) -> bool {
        let end = |(x, y): (i64, i64)| flat(x, y);
        segments_meet(
            &(end(a[0]), end(a[1])),
            &(end(b[0]), end(b[1])),
            &point(0, 0, 1),
        )
    }

    #[test]
    fn a_point_is_in_on_or_out_of_a_polygon() {
        let triangle = Polygon::new(vec![flat(1, 1), flat(5, 1), flat(1, 5)]).expect("a triangle");
        let cases = [
            (flat(2, 2), Where::In),
            (flat(3, 3), Where::On),
            (flat(3, 1), Where::On),
            (flat(1, 3), Where::On),
            (flat(1, 1), Where::On),
            (flat(5, 1), Where::On),
            (flat(1, 5), Where::On),
            (flat(4, 4), Where::Out),
            (flat(6, 0), Where::Out),
            (flat(0, 1), Where::Out),
            (flat(6, 1), Where::Out),
            (flat(1, 6), Where::Out),
            (flat(1, 0), Where::Out),
        ];
        for (p, want) in cases {
            assert_eq!(triangle.locate_in_plane(&p), want, "{p:?}");
        }
        // A notch: a point in it is out, whichever way the parity runs.
        let notched = Polygon::new(vec![
            flat(1, 1),
            flat(5, 1),
            flat(5, 3),
            flat(3, 3),
            flat(3, 5),
            flat(1, 5),
        ])
        .expect("an L");
        assert_eq!(notched.locate_in_plane(&flat(4, 4)), Where::Out);
        assert_eq!(notched.locate_in_plane(&flat(2, 4)), Where::In);
        assert_eq!(notched.locate_in_plane(&flat(4, 2)), Where::In);
        assert_eq!(notched.locate_in_plane(&flat(4, 3)), Where::On);
    }

    #[test]
    fn segments_meet_where_they_should() {
        // Crossing, and crossing lines whose segments stop short, each way round.
        assert!(meet([(1, 1), (5, 5)], [(1, 5), (5, 1)]));
        assert!(!meet([(1, 1), (2, 2)], [(1, 5), (5, 1)]));
        assert!(!meet([(1, 5), (5, 1)], [(1, 1), (2, 2)]));
        assert!(!meet([(1, 1), (5, 5)], [(5, 2), (7, 0)]));
        assert!(!meet([(5, 2), (7, 0)], [(1, 1), (5, 5)]));
        // An end of one on the other, each of the four ends in turn.
        assert!(meet([(3, 3), (5, 8)], [(1, 1), (5, 5)]));
        assert!(meet([(5, 8), (3, 3)], [(1, 1), (5, 5)]));
        assert!(meet([(1, 1), (5, 5)], [(3, 3), (5, 8)]));
        assert!(meet([(1, 1), (5, 5)], [(5, 8), (3, 3)]));
        // The same ends past the other's end, on its line.
        assert!(!meet([(7, 7), (9, 2)], [(1, 1), (5, 5)]));
        assert!(!meet([(9, 2), (7, 7)], [(1, 1), (5, 5)]));
        assert!(!meet([(1, 1), (5, 5)], [(7, 7), (9, 2)]));
        assert!(!meet([(1, 1), (5, 5)], [(9, 2), (7, 7)]));
        // And off the other side, along each axis.
        assert!(!meet([(0, 0), (-1, 4)], [(1, 1), (5, 5)]));
        assert!(!meet([(1, 1), (5, 5)], [(0, 0), (-1, 4)]));
        // On one line: apart, end to end, one inside the other.
        assert!(!meet([(1, 1), (2, 2)], [(3, 3), (4, 4)]));
        assert!(meet([(1, 1), (3, 3)], [(3, 3), (4, 4)]));
        assert!(meet([(1, 1), (4, 4)], [(2, 2), (3, 3)]));
        assert!(meet([(2, 2), (3, 3)], [(1, 1), (4, 4)]));
        // Side by side, and a level pair, whose slopes hide nothing.
        assert!(!meet([(1, 1), (4, 4)], [(1, 2), (4, 5)]));
        assert!(meet([(1, 3), (5, 3)], [(3, 1), (3, 5)]));
        assert!(!meet([(1, 3), (2, 3)], [(3, 1), (3, 5)]));
    }

    #[test]
    fn an_outline_keeps_the_strict_corners_in_order() {
        // A square with its edges' midpoints, its middle and a repeat, in no order.
        let points = vec![
            flat(3, 1),
            flat(1, 1),
            flat(5, 5),
            flat(3, 3),
            flat(5, 1),
            flat(1, 3),
            flat(1, 5),
            flat(5, 3),
            flat(3, 5),
            flat(1, 1),
        ];
        for n in [point(0, 0, 1), point(0, 0, -1)] {
            let mut hull = outline(&points, &n);
            assert_eq!(hull.len(), 4, "{hull:?}");
            assert!(dot(&area_vector(&hull), &n) > Q::ZERO);
            hull.sort();
            assert_eq!(hull, [flat(1, 1), flat(1, 5), flat(5, 1), flat(5, 5)]);
        }
        // A triangle of the plane x + y + z = 6, with its edges' midpoints and a point inside.
        let tilted = vec![
            point(3, 3, 0),
            point(5, 1, 0),
            point(2, 2, 2),
            point(1, 1, 4),
            point(1, 3, 2),
            point(3, 1, 2),
            point(1, 5, 0),
        ];
        for n in [point(1, 1, 1), point(-1, -1, -1), point(2, 2, 2)] {
            let mut hull = outline(&tilted, &n);
            assert_eq!(hull.len(), 3, "{hull:?}");
            assert!(dot(&area_vector(&hull), &n) > Q::ZERO);
            hull.sort();
            assert_eq!(hull, [point(1, 1, 4), point(1, 5, 0), point(5, 1, 0)]);
        }
        // An irregular pentagon with points inside: a grid's symmetry can hide a wrong turn.
        let corners = [flat(1, 1), flat(8, 2), flat(10, 7), flat(5, 10), flat(2, 6)];
        let mut scattered = vec![flat(4, 4), flat(6, 5), flat(7, 6), flat(3, 5)];
        scattered.extend(corners.iter().cloned());
        let around = |hull: &[Point], order: &[Point]| {
            assert_eq!(hull.len(), order.len(), "{hull:?}");
            let start = hull.iter().position(|p| *p == order[0]).expect("a corner");
            for (i, want) in order.iter().enumerate() {
                assert_eq!(&hull[(start + i) % hull.len()], want);
            }
        };
        around(&outline(&scattered, &point(0, 0, 1)), &corners);
        let mut backwards = corners.to_vec();
        backwards.reverse();
        around(&outline(&scattered, &point(0, 0, -1)), &backwards);
    }

    #[test]
    fn a_clip_drops_a_side_with_no_area() {
        // Three corners in a line, on the plane, and the last above it.
        let face = Polygon {
            corners: vec![flat(1, 1), flat(3, 1), flat(5, 1), flat(1, 5)],
        };
        let plane = Plane::new(point(0, 1, 0), int(1)).expect("a plane");
        let (low, high, on) = clip(&face, &plane);
        assert_eq!(low, None);
        assert_eq!(high, Some(face));
        assert_eq!(on, [flat(1, 1), flat(3, 1), flat(5, 1)]);
    }

    #[test]
    fn a_solid_lists_each_edge_once() {
        let cube = Solid::cuboid(&point(1, 2, 3), &point(4, 6, 8)).expect("a box");
        let edges = cube.edges();
        assert_eq!(edges.len(), 12);
        assert!(edges.iter().all(|(a, b)| a < b));
        let along_one_axis =
            |(a, b): &(Point, Point)| a.iter().zip(b).filter(|(x, y)| x != y).count() == 1;
        assert!(edges.iter().all(along_one_axis));
        let tet = Solid::tetrahedron([
            point(1, 1, 1),
            point(3, 1, 1),
            point(1, 3, 1),
            point(1, 1, 3),
        ])
        .expect("a tetrahedron");
        assert_eq!(tet.edges().len(), 6);
    }
}
