//! A joined wall's pieces and outline, from where each layer ends at each end and where its
//! layers are cut back for the walls that abut it.
//!
//! In the wall's [`Frame`] a point is `(along, across)`: along its line from the origin, and
//! across it to the right. Face `k` is the line `across = line + faces[k]`. Each layer `k` with a
//! thickness is the strip between faces `k` and `k + 1`, from its start edge to its end edge, less
//! every band a notch removes from it; each piece left is a quadrilateral with two sides on the
//! layer's faces, so the pieces of all the layers tile the wall's shape exactly.

use super::plan::{Frame, P, add, cross, less, point, twice_area};

/// Where a layer ends at one end of the wall: the along value of its corner on its exterior face
/// (face `k`) and on its interior face (face `k + 1`).
pub(super) type Edge = (i64, i64);

/// A notch: where a wall abuts this one, the bands along this wall each abutting layer takes,
/// with how many of this wall's layers it cuts back, counted from the side it comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Notch {
    /// Whether the layers are counted from layer 0 (the exterior face's side) or from the last.
    pub(super) from_first: bool,
    /// Each band, `(low, high, depth)` along the wall, in order along it, touching end to end.
    pub(super) bands: Vec<(i64, i64, usize)>,
}

/// The wall, as its pieces are laid out.
pub(super) struct Layout<'a> {
    /// Its frame.
    pub(super) frame: Frame,
    /// Its line's across value.
    pub(super) line: i64,
    /// Its faces' offsets from its line, face 0 the exterior.
    pub(super) faces: &'a [i64],
    /// Where each layer ends at its start.
    pub(super) start: Vec<Edge>,
    /// Where each layer ends at its end.
    pub(super) finish: Vec<Edge>,
    /// Its notches.
    pub(super) notches: Vec<Notch>,
}

/// A polygon of `(along, across)` points in the model's plan, counter-clockwise from above.
fn placed(frame: Frame, points: &[P]) -> Option<Vec<P>> {
    let mut out = Vec::with_capacity(points.len());
    for &[a, c] in points {
        out.push(point(a, frame.u, c, frame.r)?);
    }
    match twice_area(&out)?.signum() {
        1 => Some(out),
        -1 => {
            out.reverse();
            Some(out)
        }
        _ => None,
    }
}

impl Layout<'_> {
    /// How many layers.
    fn count(&self) -> usize {
        self.faces.len().saturating_sub(1)
    }

    /// Face `k`'s across value.
    fn face(&self, k: usize) -> Option<i64> {
        add(self.line, *self.faces.get(k)?)
    }

    /// Whether layer `k` has a thickness.
    fn thick(&self, k: usize) -> bool {
        self.faces.get(k) != self.faces.get(k.saturating_add(1))
    }

    /// Whether a band cutting back `depth` layers from its side reaches layer `k`.
    fn reaches(&self, from_first: bool, depth: usize, k: usize) -> bool {
        let counted = if from_first {
            Some(k)
        } else {
            self.count()
                .checked_sub(1)
                .and_then(|last| last.checked_sub(k))
        };
        counted.is_some_and(|c| depth > c)
    }

    /// The bands cut out of layer `k`, merged where they touch, in order along the wall.
    fn cuts(&self, k: usize) -> Vec<(i64, i64)> {
        let mut cut: Vec<(i64, i64)> = self
            .notches
            .iter()
            .flat_map(|n| n.bands.iter().map(move |b| (n.from_first, *b)))
            .filter(|&(first, (lo, hi, depth))| lo < hi && self.reaches(first, depth, k))
            .map(|(_, (lo, hi, _))| (lo, hi))
            .collect();
        cut.sort_unstable();
        let mut merged: Vec<(i64, i64)> = Vec::with_capacity(cut.len());
        for (lo, hi) in cut {
            match merged.last_mut() {
                Some(last) if lo <= last.1 => last.1 = last.1.max(hi),
                _ => merged.push((lo, hi)),
            }
        }
        merged
    }

    /// Layer `k`'s pieces, each counter-clockwise in the model's plan.
    fn pieces(&self, k: usize) -> Option<Vec<Vec<P>>> {
        if !self.thick(k) {
            return Some(Vec::new());
        }
        let (ext, int) = (self.face(k)?, self.face(k.checked_add(1)?)?);
        let mut out = Vec::new();
        let mut left = *self.start.get(k)?;
        let piece = |l: Edge, r: Edge| -> Option<Vec<P>> {
            if l.0 >= r.0 || l.1 >= r.1 {
                return None;
            }
            placed(
                self.frame,
                &[[l.0, ext], [r.0, ext], [r.1, int], [l.1, int]],
            )
        };
        for (lo, hi) in self.cuts(k) {
            out.push(piece(left, (lo, lo))?);
            left = (hi, hi);
        }
        out.push(piece(left, *self.finish.get(k)?)?);
        Some(out)
    }

    /// Every layer's pieces, from the exterior.
    pub(super) fn all_pieces(&self) -> Option<Vec<Vec<Vec<P>>>> {
        (0..self.count()).map(|k| self.pieces(k)).collect()
    }

    /// The points along one side, from the start to the end: face `edge` with a dip into each
    /// notch on that side, to the face its depth reaches.
    fn side(&self, from_first: bool, out: &mut Vec<P>) -> Option<()> {
        let n = self.count();
        let face_at = |depth: usize| {
            if from_first {
                self.face(depth)
            } else {
                self.face(n.checked_sub(depth)?)
            }
        };
        let edge = face_at(0)?;
        let mut notches: Vec<&Notch> = self
            .notches
            .iter()
            .filter(|notch| notch.from_first == from_first)
            .collect();
        notches.sort_by_key(|notch| notch.bands.first().map(|b| b.0));
        for notch in notches {
            let bands: Vec<_> = notch.bands.iter().filter(|b| b.0 < b.1).collect();
            let (Some(first), Some(last)) = (bands.first(), bands.last()) else {
                continue;
            };
            out.push([first.0, edge]);
            for &&(lo, hi, depth) in &bands {
                let at = face_at(depth)?;
                out.push([lo, at]);
                out.push([hi, at]);
            }
            out.push([last.1, edge]);
        }
        Some(())
    }

    /// The wall's whole outline in plan, counter-clockwise, with no point repeated and none
    /// between two others on one line.
    pub(super) fn outline(&self) -> Option<Vec<P>> {
        let n = self.count();
        let mut path: Vec<P> = Vec::new();
        for k in (0..n).rev().filter(|&k| self.thick(k)) {
            let (ext, int) = *self.start.get(k)?;
            path.push([int, self.face(k.checked_add(1)?)?]);
            path.push([ext, self.face(k)?]);
        }
        self.side(true, &mut path)?;
        for k in (0..n).filter(|&k| self.thick(k)) {
            let (ext, int) = *self.finish.get(k)?;
            path.push([ext, self.face(k)?]);
            path.push([int, self.face(k.checked_add(1)?)?]);
        }
        let mut back = Vec::new();
        self.side(false, &mut back)?;
        back.reverse();
        path.extend(back);
        placed(self.frame, &simple(path)?)
    }
}

/// `points` as a closed polygon with no point repeated and none on the line between its two
/// neighbours.
fn simple(points: Vec<P>) -> Option<Vec<P>> {
    let mut out: Vec<P> = Vec::with_capacity(points.len());
    for p in points {
        if out.last() != Some(&p) {
            out.push(p);
        }
    }
    while out.len() > 1 && out.first() == out.last() {
        out.pop();
    }
    loop {
        let len = out.len();
        let mut dropped = None;
        for at in 0..len {
            let before = *out.get(at.checked_add(len)?.checked_sub(1)?.checked_rem(len)?)?;
            let here = *out.get(at)?;
            let after = *out.get(at.checked_add(1)?.checked_rem(len)?)?;
            if cross(less(here, before)?, less(after, here)?) == 0 {
                dropped = Some(at);
                break;
            }
        }
        match dropped {
            Some(at) if len > 3 => {
                out.remove(at);
            }
            _ => return Some(out),
        }
    }
}
