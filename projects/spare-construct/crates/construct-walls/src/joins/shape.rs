//! A joined wall's pieces and outline, from where each layer ends at each end, where its layers
//! are cut back for the walls that abut or cross it, where a cross cuts it in two, and the turned
//! parts of its wrapped ends.
//!
//! Face `k` of the wall is the line through its start's corner on that face ([`Layout::faces`])
//! along the wall's direction. Each layer `k` with a thickness is the strip between faces `k` and
//! `k + 1`, from its start edge to its end edge, less every band a notch or a gap removes from
//! it; each piece left is a quadrilateral with two sides on the layer's faces, its corners where
//! the faces meet the edges ([`Edge`], each corner rounded once by [`super::exact::meet`]). So
//! the pieces of all the layers, and of the walls the joint joins, tile the joined shape: exactly
//! along the plan's axes and on whole ratios, and elsewhere to within the half step a corner is
//! rounded by.

use super::exact::meet;
use super::plan::{P, along, convex, cross, less, twice_area};

/// Where a layer ends at one end of the wall, or where a band begins or ends: its corner on the
/// layer's exterior face (face `k`) and on its interior face (face `k + 1`).
pub(super) type Edge = (P, P);

/// A line across the wall a band is cut on: a face of another wall, through `point` along `dir`,
/// with its place along this wall (`key`, as [`along`] gives it) to order the bands by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Cut {
    /// A point on it.
    pub(super) point: P,
    /// Its direction.
    pub(super) dir: P,
    /// Where it crosses this wall's line, along it.
    pub(super) key: i128,
}

/// A band a notch cuts: between two cuts, cutting back `depth` of this wall's layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Band {
    /// The cut nearer the wall's start.
    pub(super) lo: Cut,
    /// The cut nearer its end.
    pub(super) hi: Cut,
    /// How many layers it cuts back, counted from the side it comes from.
    pub(super) depth: usize,
}

/// A notch: where a wall abuts or crosses this one, the bands along this wall each abutting
/// layer takes, with how many of this wall's layers it cuts back, counted from the side it comes
/// from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Notch {
    /// Whether the layers are counted from layer 0 (the exterior face's side) or from the last.
    pub(super) from_first: bool,
    /// Each band, in order along the wall, touching end to end.
    pub(super) bands: Vec<Band>,
}

/// Where a cross cuts one layer of this wall in two: the layer stops on `lo` coming from the
/// start and starts again on `hi`, each a line across it, `lo` the nearer the start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Gap {
    /// The line it stops on.
    pub(super) lo: Cut,
    /// The line it starts again on.
    pub(super) hi: Cut,
}

/// The wall, as its pieces are laid out.
pub(super) struct Layout<'a> {
    /// Its direction, from its start to its end.
    pub(super) d: P,
    /// Its start, where [`along`] measures from.
    pub(super) origin: P,
    /// Each face's corner at the wall's start: face `k` is the line through it along `d`.
    pub(super) faces: &'a [P],
    /// Where each layer ends at its start.
    pub(super) start: Vec<Edge>,
    /// Where each layer ends at its end.
    pub(super) finish: Vec<Edge>,
    /// The wall's outer edge at its start, for its outline: [`Layout::start`] but where its layers
    /// are set back from a wrapped end, which keeps its square end.
    pub(super) outline_start: Vec<Edge>,
    /// The same at its end.
    pub(super) outline_finish: Vec<Edge>,
    /// Its notches.
    pub(super) notches: Vec<Notch>,
    /// Each layer's gaps, where crosses cut it in two.
    pub(super) gaps: Vec<Vec<Gap>>,
    /// The turned parts of its wrapped ends: each with the layer it belongs to, its corners
    /// counter-clockwise.
    pub(super) turns: Vec<(usize, Vec<P>)>,
}

/// `points` counter-clockwise: reversed if they run the other way; `None` for no area.
fn placed(mut out: Vec<P>) -> Option<Vec<P>> {
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

    /// Where face `k` meets `cut`.
    fn on(&self, k: usize, cut: &Cut) -> Option<P> {
        meet(*self.faces.get(k)?, self.d, cut.point, cut.dir)
    }

    /// The edge of layer `k` on `cut`.
    fn edge(&self, k: usize, cut: &Cut) -> Option<Edge> {
        Some((self.on(k, cut)?, self.on(k.checked_add(1)?, cut)?))
    }

    /// Where `p` lies along the wall.
    fn at(&self, p: P) -> Option<i128> {
        along(p, self.origin, self.d)
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

    /// The bands and gaps cut out of layer `k`, merged where they touch, in order along the wall.
    fn cuts(&self, k: usize) -> Vec<(Cut, Cut)> {
        let mut cut: Vec<(Cut, Cut)> = self
            .notches
            .iter()
            .flat_map(|n| n.bands.iter().map(move |b| (n.from_first, *b)))
            .filter(|&(first, b)| b.lo.key < b.hi.key && self.reaches(first, b.depth, k))
            .map(|(_, b)| (b.lo, b.hi))
            .chain(
                self.gaps
                    .get(k)
                    .into_iter()
                    .flatten()
                    .map(|gap| (gap.lo, gap.hi)),
            )
            .collect();
        cut.sort_unstable_by_key(|(lo, hi)| (lo.key, hi.key));
        let mut merged: Vec<(Cut, Cut)> = Vec::with_capacity(cut.len());
        for (lo, hi) in cut {
            match merged.last_mut() {
                Some(last) if lo.key <= last.1.key => {
                    if hi.key > last.1.key {
                        last.1 = hi;
                    }
                }
                _ => merged.push((lo, hi)),
            }
        }
        merged
    }

    /// The piece of layer `k` from edge `l` to edge `r`: strictly convex, counter-clockwise.
    fn piece(&self, l: Edge, r: Edge) -> Option<Vec<P>> {
        if self.at(l.0)? >= self.at(r.0)? || self.at(l.1)? >= self.at(r.1)? {
            return None;
        }
        let corners = placed(vec![l.0, r.0, r.1, l.1])?;
        convex(&corners).then_some(corners)
    }

    /// Layer `k`'s pieces, each counter-clockwise in the model's plan: its strip's pieces, then
    /// its turned parts.
    fn pieces(&self, k: usize) -> Option<Vec<Vec<P>>> {
        if !self.thick(k) {
            return Some(Vec::new());
        }
        let mut out = Vec::new();
        let mut left = *self.start.get(k)?;
        for (lo, hi) in self.cuts(k) {
            out.push(self.piece(left, self.edge(k, &lo)?)?);
            left = self.edge(k, &hi)?;
        }
        out.push(self.piece(left, *self.finish.get(k)?)?);
        for (_, corners) in self.turns.iter().filter(|(layer, _)| *layer == k) {
            let corners = placed(corners.clone())?;
            if !convex(&corners) {
                return None;
            }
            out.push(corners);
        }
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
                Some(depth)
            } else {
                n.checked_sub(depth)
            }
        };
        let edge = face_at(0)?;
        let mut notches: Vec<&Notch> = self
            .notches
            .iter()
            .filter(|notch| notch.from_first == from_first)
            .collect();
        notches.sort_by_key(|notch| notch.bands.first().map(|b| b.lo.key));
        for notch in notches {
            let bands: Vec<_> = notch.bands.iter().filter(|b| b.lo.key < b.hi.key).collect();
            let (Some(first), Some(last)) = (bands.first(), bands.last()) else {
                continue;
            };
            out.push(self.on(edge, &first.lo)?);
            for band in &bands {
                let at = face_at(band.depth)?;
                out.push(self.on(at, &band.lo)?);
                out.push(self.on(at, &band.hi)?);
            }
            out.push(self.on(edge, &last.hi)?);
        }
        Some(())
    }

    /// The wall's whole outline in plan, counter-clockwise, with no point repeated and none
    /// between two others on one line. A gap a cross cuts is inside it: a wall cut in two keeps
    /// its whole outline, and the wall it is cut by covers the crossing too.
    pub(super) fn outline(&self) -> Option<Vec<P>> {
        let n = self.count();
        let mut path: Vec<P> = Vec::new();
        for k in (0..n).rev().filter(|&k| self.thick(k)) {
            let (ext, int) = *self.outline_start.get(k)?;
            path.push(int);
            path.push(ext);
        }
        self.side(true, &mut path)?;
        for k in (0..n).filter(|&k| self.thick(k)) {
            let (ext, int) = *self.outline_finish.get(k)?;
            path.push(ext);
            path.push(int);
        }
        let mut back = Vec::new();
        self.side(false, &mut back)?;
        back.reverse();
        path.extend(back);
        placed(simple(path)?)
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
