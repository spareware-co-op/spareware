//! Region finding on an arrangement (chapter 14.4): the regions linework closes, with their holes,
//! the dangles and cut edges left out of them, the region a pick point lies in, and which islands
//! a hatch's island rule keeps.
//!
//! What it does: [`polygonize`] turns an [`Arrangement`] into [`Regions`]: one [`Region`] for each
//! bounded face, its shell (the face's outer boundary) and its holes (the outer boundaries of what
//! lies inside it, and any loop a cut edge tied to its shell), each a ring of half-edges with the
//! region on its left; the dangles (edges with a free end, pruned until none is left) and the cut
//! edges (edges with the same face on both sides once the dangles are gone) are left out of every
//! ring and listed. [`Regions::at`] names the region a point lies in, or says the point is open
//! (in the unbounded face) or on the linework. [`Regions::filled`] applies an [`IslandRule`] to a
//! region: the region alone with its holes ([`IslandRule::Outermost`]), its shell with no holes
//! ([`IslandRule::Ignore`]), or the region and every region nested inside it at an even depth
//! ([`IslandRule::Parity`]). [`Regions::twice_area`] is a ring's area, doubled, exactly.
//!
//! Guarantees:
//!
//! - **The arrangement's own faces.** Every region is a bounded face of the arrangement and every
//!   bounded face is a region; no ring is guessed from coordinates. Which ring of a face is its
//!   shell is decided by the face's lowest-leftmost vertex, which lies on its outer boundary.
//! - **Dangles and cut edges never bound a region.** An edge with a free end, after every such
//!   edge is removed in turn, is a dangle; an edge left with the same face on both sides is a cut
//!   edge. Both are listed, in edge order, and no ring holds either.
//! - **Exact.** Locating a point decides by exact orientation and comparison signs of rational
//!   points ([`Rational2::orient`], [`Rational2::cmp_y`]); a doubled area is an exact integer when
//!   every vertex of the ring is a grid point.
//! - **Deterministic.** Regions are in face order, rings in half-edge order from their first
//!   half-edge, holes by their first half-edge.
//!
//! Method: the polygonizer of JTS (`Polygonizer`, `PolygonizeGraph`, `EdgeRing`): dangles deleted
//! by pruning nodes of degree one, cut edges found as edges whose two directed edges lie on one
//! ring, rings walked by taking at each node the next edge clockwise from the one arrived by, and
//! holes assigned to the shell that holds them; here the arrangement's faces already say which
//! shell holds which hole, so no hole is assigned by a point-in-ring test. Locating a point counts
//! the crossings of a ray to the right, as Shimrat's even-odd test (Comm. ACM 5(8), 1962,
//! doi:10.1145/368637.368653), on exact signs. The island rules are DXF HATCH's
//! (<https://help.autodesk.com/cloudhelp/2018/ENU/AutoCAD-DXF/files/GUID-C6C71CED-CE0F-4184-82A5-07AD6241F15B.htm>).
//!
//! Consulted: JTS, BSD-3-Clause,
//!   <https://github.com/locationtech/jts/blob/5a0a73632216e3e9d277150336078ec423fd215f/modules/core/src/main/java/org/locationtech/jts/operation/polygonize/Polygonizer.java>,
//!   under its Eclipse Distribution License 1.0, for the order of its steps (dangles, cut edges,
//!   rings, holes). No code copied.
//!
//! Cost: linear in the half-edges for the pruning, the faces and the rings; a point location is
//! linear in the edges of the rings it tests. No recursion: nesting for the parity rule is walked
//! with an explicit stack.

#![allow(
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "p, a, b, h, v and e are the geometry's own letters, and the polygonizer is one procedure of JTS's that reads best whole"
)]

use alloc::vec::Vec;
use core::fmt;

use geom_num::carrier::Rational2;
use geom_num::{NumError, Sign};

use crate::arrange::{Along, Arrangement, At};

/// Why a region could not be found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionError {
    /// A step of the exact arithmetic did not fit its budget.
    Range,
    /// An index past the arrangement's half-edges, vertices or faces: the arrangement is not one
    /// [`crate::arrange`] built.
    Malformed,
    /// A point was located against a ring with an arc or a one-root vertex, which this module does
    /// not yet decide (chapter 14.4's arcs come with their own sides).
    Curved,
}

impl From<NumError> for RegionError {
    fn from(_: NumError) -> RegionError {
        RegionError::Range
    }
}

impl fmt::Display for RegionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            RegionError::Range => "a step of the exact arithmetic did not fit its budget",
            RegionError::Malformed => "the arrangement is not one the sweep built",
            RegionError::Curved => "locating a point among arcs is not decided here yet",
        })
    }
}

impl core::error::Error for RegionError {}

/// Which islands a hatch keeps (DXF HATCH's style).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IslandRule {
    /// Odd parity: the region, its islands left out, the islands' islands filled again, and so on.
    Parity,
    /// The region with its holes, nothing nested inside them.
    Outermost,
    /// The region's shell, its holes ignored.
    Ignore,
}

/// One region: a bounded face of the arrangement, its shell and its holes, each a ring of
/// half-edges with the region on its left.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    /// The face, as [`Arrangement::faces`] numbers it.
    pub face: u32,
    /// The outer boundary, counter-clockwise.
    pub shell: Vec<u32>,
    /// The holes, each clockwise, by their first half-edge.
    pub holes: Vec<Vec<u32>>,
}

/// A filled area: a shell and the holes kept in it, each a ring of half-edges.
pub type Filled = (Vec<u32>, Vec<Vec<u32>>);

/// Where a point lies among the regions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Located {
    /// Inside the region at this place of [`Regions::regions`].
    In(usize),
    /// In no region: in the unbounded face, or in a face no closed linework bounds.
    Open,
    /// On a vertex or an edge of the linework.
    On,
}

/// What [`polygonize`] found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Regions {
    found: Vec<Region>,
    dangles: Vec<u32>,
    cut_edges: Vec<u32>,
    /// For each half-edge, the face on its left.
    face_of: Vec<u32>,
    /// For each face, its region's place, if it is bounded.
    region_of: Vec<Option<usize>>,
}

/// The half-edge that runs the other way along the same edge.
const fn twin(h: u32) -> u32 {
    h ^ 1
}

fn at<T: Copy>(list: &[T], i: u32) -> Result<T, RegionError> {
    usize::try_from(i)
        .ok()
        .and_then(|i| list.get(i))
        .copied()
        .ok_or(RegionError::Malformed)
}

fn index(n: usize) -> Result<u32, RegionError> {
    u32::try_from(n).map_err(|_| RegionError::Malformed)
}

/// The vertices a half-edge runs from and to.
fn ends(arr: &Arrangement, h: u32) -> Result<(u32, u32), RegionError> {
    let e = arr
        .edges()
        .get(usize::try_from(h / 2).map_err(|_| RegionError::Malformed)?)
        .ok_or(RegionError::Malformed)?;
    Ok(if h.is_multiple_of(2) {
        (e.from, e.to)
    } else {
        (e.to, e.from)
    })
}

/// The regions of `arr`: see the module documentation.
///
/// # Errors
///
/// [`RegionError::Malformed`] for an arrangement whose indices do not agree.
pub fn polygonize(arr: &Arrangement) -> Result<Regions, RegionError> {
    let edges = arr.edges().len();
    let halves = edges.checked_mul(2).ok_or(RegionError::Malformed)?;
    // Dangles: prune vertices of degree one until none is left.
    let mut gone = alloc::vec![false; edges];
    let mut degree: Vec<usize> = (0..arr.vertices().len())
        .map(|v| index(v).map(|v| arr.around(v).len()))
        .collect::<Result<_, _>>()?;
    let mut stack: Vec<u32> = Vec::new();
    for (v, &d) in degree.iter().enumerate() {
        if d == 1 {
            stack.push(index(v)?);
        }
    }
    while let Some(v) = stack.pop() {
        for &h in arr.around(v) {
            let e = usize::try_from(h / 2).map_err(|_| RegionError::Malformed)?;
            let Some(flag) = gone.get_mut(e) else {
                return Err(RegionError::Malformed);
            };
            if *flag {
                continue;
            }
            *flag = true;
            for end in [ends(arr, h)?.0, ends(arr, h)?.1] {
                let slot = degree
                    .get_mut(usize::try_from(end).map_err(|_| RegionError::Malformed)?)
                    .ok_or(RegionError::Malformed)?;
                *slot = slot.saturating_sub(1);
                if *slot == 1 {
                    stack.push(end);
                }
            }
        }
    }
    let dangles: Vec<u32> = (0..edges)
        .filter(|&e| gone.get(e) == Some(&true))
        .map(index)
        .collect::<Result<_, _>>()?;
    // The face on the left of each half-edge, from the arrangement's faces.
    let mut face_of = alloc::vec![u32::MAX; halves];
    for (f, face) in arr.faces().iter().enumerate() {
        let f = index(f)?;
        for c in face.outer.iter().chain(&face.holes) {
            let cycle = arr
                .cycles()
                .get(usize::try_from(*c).map_err(|_| RegionError::Malformed)?)
                .ok_or(RegionError::Malformed)?;
            for &h in cycle {
                let slot = face_of
                    .get_mut(usize::try_from(h).map_err(|_| RegionError::Malformed)?)
                    .ok_or(RegionError::Malformed)?;
                *slot = f;
            }
        }
    }
    // Cut edges: what is left with one face on both sides.
    let mut cut_edges = Vec::new();
    for e in 0..edges {
        if gone.get(e) == Some(&true) {
            continue;
        }
        let h = index(e)?.checked_mul(2).ok_or(RegionError::Malformed)?;
        if at(&face_of, h)? == at(&face_of, twin(h))? {
            cut_edges.push(index(e)?);
            if let Some(flag) = gone.get_mut(e) {
                *flag = true;
            }
        }
    }
    let kept = |h: u32| -> bool {
        usize::try_from(h / 2)
            .ok()
            .and_then(|e| gone.get(e))
            .is_some_and(|g| !*g)
    };
    // Rings: from each kept half-edge, the next kept half-edge clockwise from its twin at its end.
    let mut seen = alloc::vec![false; halves];
    let mut rings: Vec<Vec<u32>> = Vec::new();
    for start in 0..halves {
        let start = index(start)?;
        if !kept(start) || at(&seen, start)? {
            continue;
        }
        let mut ring = Vec::new();
        let mut h = start;
        loop {
            let slot = seen
                .get_mut(usize::try_from(h).map_err(|_| RegionError::Malformed)?)
                .ok_or(RegionError::Malformed)?;
            if *slot {
                break;
            }
            *slot = true;
            ring.push(h);
            let (_, v) = ends(arr, h)?;
            let out: Vec<u32> = arr.around(v).iter().copied().filter(|&o| kept(o)).collect();
            let back = out
                .iter()
                .position(|&o| o == twin(h))
                .ok_or(RegionError::Malformed)?;
            let before = back.checked_sub(1).unwrap_or(out.len().saturating_sub(1));
            h = *out.get(before).ok_or(RegionError::Malformed)?;
        }
        if h != start {
            return Err(RegionError::Malformed);
        }
        rings.push(ring);
    }
    // Shells and holes, face by face: a bounded face's shell is the ring through its outer
    // cycle's lowest-leftmost vertex, which the sweep numbered lowest.
    let faces = arr.faces();
    let mut region_of = alloc::vec![None; faces.len()];
    let mut regions = Vec::new();
    for (f, face) in faces.iter().enumerate() {
        let Some(outer) = face.outer else {
            continue;
        };
        let cycle = arr
            .cycles()
            .get(usize::try_from(outer).map_err(|_| RegionError::Malformed)?)
            .ok_or(RegionError::Malformed)?;
        let mut lowest: Option<(u32, u32)> = None;
        for &h in cycle.iter().filter(|&&h| kept(h)) {
            let (v, _) = ends(arr, h)?;
            if lowest.is_none_or(|(lv, _)| v < lv) {
                lowest = Some((v, h));
            }
        }
        let Some((_, first)) = lowest else {
            continue;
        };
        let f32 = index(f)?;
        let mut shell = Vec::new();
        let mut holes = Vec::new();
        for ring in &rings {
            let Some(&h) = ring.first() else { continue };
            if at(&face_of, h)? != f32 {
                continue;
            }
            if ring.contains(&first) {
                shell.clone_from(ring);
            } else {
                holes.push(ring.clone());
            }
        }
        if let Some(slot) = region_of.get_mut(f) {
            *slot = Some(regions.len());
        }
        regions.push(Region {
            face: f32,
            shell,
            holes,
        });
    }
    Ok(Regions {
        found: regions,
        dangles,
        cut_edges,
        face_of,
        region_of,
    })
}

impl Regions {
    /// The regions, in face order.
    #[must_use]
    pub fn regions(&self) -> &[Region] {
        &self.found
    }

    /// The dangles, in edge order.
    #[must_use]
    pub fn dangles(&self) -> &[u32] {
        &self.dangles
    }

    /// The cut edges, in edge order.
    #[must_use]
    pub fn cut_edges(&self) -> &[u32] {
        &self.cut_edges
    }

    /// The regions a hatch of the region at `place` fills under `rule`, as (shell, holes kept)
    /// pairs: see [`IslandRule`].
    ///
    /// # Errors
    ///
    /// [`RegionError::Malformed`] for a place past the regions.
    pub fn filled(&self, place: usize, rule: IslandRule) -> Result<Vec<Filled>, RegionError> {
        let r = self.found.get(place).ok_or(RegionError::Malformed)?;
        match rule {
            IslandRule::Ignore => Ok(alloc::vec![(r.shell.clone(), Vec::new())]),
            IslandRule::Outermost => Ok(alloc::vec![(r.shell.clone(), r.holes.clone())]),
            IslandRule::Parity => {
                // Depth 0 is filled, an island (depth 1) is not, its islands (depth 2) are.
                let mut out = Vec::new();
                let mut stack = alloc::vec![(place, 0_u32)];
                let mut visited = alloc::vec![false; self.found.len()];
                while let Some((p, depth)) = stack.pop() {
                    let Some(v) = visited.get_mut(p) else {
                        return Err(RegionError::Malformed);
                    };
                    if *v {
                        continue;
                    }
                    *v = true;
                    let region = self.found.get(p).ok_or(RegionError::Malformed)?;
                    if depth % 2 == 0 {
                        out.push((region.shell.clone(), region.holes.clone()));
                    }
                    for hole in &region.holes {
                        for &h in hole {
                            let inside = at(&self.face_of, twin(h))?;
                            let Some(&Some(q)) = usize::try_from(inside)
                                .ok()
                                .and_then(|i| self.region_of.get(i))
                            else {
                                continue;
                            };
                            stack.push((q, depth.saturating_add(1)));
                        }
                    }
                }
                Ok(out)
            }
        }
    }

    /// Where `p` lies: in a region, open, or on the linework.
    ///
    /// # Errors
    ///
    /// [`RegionError::Curved`] if a ring tested has an arc or a one-root vertex;
    /// [`RegionError::Range`] past the exact budget.
    pub fn at(&self, arr: &Arrangement, p: &Rational2) -> Result<Located, RegionError> {
        // The innermost region whose shell holds p and none of whose holes do.
        for (place, region) in self.found.iter().enumerate() {
            match ring_side(arr, &region.shell, p)? {
                Sign::Zero => return Ok(Located::On),
                Sign::Negative => continue,
                Sign::Positive => {}
            }
            let mut in_hole = false;
            for hole in &region.holes {
                match ring_side(arr, hole, p)? {
                    Sign::Zero => return Ok(Located::On),
                    Sign::Positive => {
                        in_hole = true;
                        break;
                    }
                    Sign::Negative => {}
                }
            }
            if !in_hole {
                return Ok(Located::In(place));
            }
        }
        Ok(Located::Open)
    }

    /// Twice the area of a ring, exactly, when every vertex is a grid point; `None` for a ring
    /// with an arc or a vertex off the grid. Positive for a counter-clockwise ring.
    ///
    /// # Errors
    ///
    /// [`RegionError::Range`] past 128 bits; [`RegionError::Malformed`] for a half-edge past the
    /// arrangement.
    pub fn twice_area(arr: &Arrangement, ring: &[u32]) -> Result<Option<i128>, RegionError> {
        let mut sum = 0_i128;
        for &h in ring {
            if !matches!(arr.along(h / 2), Some(Along::Line(_))) {
                return Ok(None);
            }
            let (a, b) = ends(arr, h)?;
            let (Some((ax, ay)), Some((bx, by))) = (grid(arr, a)?, grid(arr, b)?) else {
                return Ok(None);
            };
            // Each coordinate is below 2^41, so each product is below 2^82.
            let term = ax
                .checked_mul(by)
                .and_then(|l| ay.checked_mul(bx).and_then(|r| l.checked_sub(r)))
                .ok_or(RegionError::Range)?;
            sum = sum.checked_add(term).ok_or(RegionError::Range)?;
        }
        Ok(Some(sum))
    }
}

/// The vertex as a grid point, if it is one.
fn grid(arr: &Arrangement, v: u32) -> Result<Option<(i128, i128)>, RegionError> {
    let vertex = arr
        .vertices()
        .get(usize::try_from(v).map_err(|_| RegionError::Malformed)?)
        .ok_or(RegionError::Malformed)?;
    let Some(r) = vertex.rational() else {
        return Ok(None);
    };
    let ([x, y], w) = r.coordinates();
    Ok((w == 1).then_some((x, y)))
}

/// The rational point of a vertex.
fn rational(arr: &Arrangement, v: u32) -> Result<Rational2, RegionError> {
    match arr
        .vertices()
        .get(usize::try_from(v).map_err(|_| RegionError::Malformed)?)
        .ok_or(RegionError::Malformed)?
    {
        At::Rational(r) => Ok(*r),
        At::Root(_) => Err(RegionError::Curved),
    }
}

/// Whether `p` is inside the closed ring (`Positive`), outside it (`Negative`) or on it (`Zero`),
/// by the parity of the ring's edges a ray from `p` to the right crosses, each crossing decided by
/// exact signs; an edge counts when one end is above `p` and the other is not.
fn ring_side(arr: &Arrangement, ring: &[u32], p: &Rational2) -> Result<Sign, RegionError> {
    let mut inside = false;
    for &h in ring {
        if !matches!(arr.along(h / 2), Some(Along::Line(_))) {
            return Err(RegionError::Curved);
        }
        let (a, b) = ends(arr, h)?;
        let (a, b) = (rational(arr, a)?, rational(arr, b)?);
        let turn = Rational2::orient(&a, &b, p)?;
        // On the edge: on its line and between its ends.
        if turn == Sign::Zero {
            let along_x = a.cmp_x(p)?.mul(p.cmp_x(&b)?);
            let along_y = a.cmp_y(p)?.mul(p.cmp_y(&b)?);
            if along_x != Sign::Negative && along_y != Sign::Negative {
                return Ok(Sign::Zero);
            }
        }
        let a_above = a.cmp_y(p)? == Sign::Positive;
        let b_above = b.cmp_y(p)? == Sign::Positive;
        if a_above != b_above {
            // The edge crosses the ray's level; it is to the right of p when p is on the side of
            // the edge that faces down its upward direction's left.
            let upward_turn = if b_above { turn } else { turn.negate() };
            if upward_turn == Sign::Positive {
                inside = !inside;
            }
        }
    }
    Ok(if inside {
        Sign::Positive
    } else {
        Sign::Negative
    })
}
