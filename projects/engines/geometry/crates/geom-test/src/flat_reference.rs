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
//! <https://doi.org/10.1109/TPAMI.1986.4767788>.
//!
//! Cost: `p` planes cut the box into up to O(p³) cells, each cut costing every face of every cell,
//! and rationals grow with each cut. This reference accepts that, because it runs on small cases
//! only. No recursion.

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
        let (pu, pv) = (at(p, u), at(p, v));
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
        if denominator == Q::ZERO {
            // An edge along the line: its ends are stops, if they are on it.
            let from_a = minus(a, from);
            if at(&from_a, u) * at(along, v) == at(&from_a, v) * at(along, u) {
                for end in [a, b] {
                    let gap = minus(end, from);
                    stops.insert(dot(&gap, along) / dot(along, along));
                }
            }
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
