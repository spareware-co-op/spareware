//! Sections and elevations: what a vertical plane cuts, and the outlines of what is seen beyond it,
//! to a depth (job 8.1; CON-A8-F2-P1, the rest; note A12's build order, steps 3 and 5).
//!
//! What it does:
//!
//! - [`SectionView`]: a section's or an elevation's line in plan, the side it looks to and how far
//!   it sees, as a view of this crate: the line must run along a grid axis, so the view looks
//!   north, south, east or west ([`View`]); a line at any other angle is refused in words
//!   ([`DrawingError::View`]: the seam cuts only planes square to a grid axis). A section cuts
//!   what its plane crosses; an elevation does not, and draws what lies beyond its plane. It maps
//!   the view's own frame (a distance along the line from its first end, and a height) to the
//!   drawing's points ([`SectionView::point`]), which is how a crop held in that frame is applied.
//! - [`part`]: what one element gives the view. Where the view cuts, the element's cut: closed
//!   regions with their materials and cut lines named by face, exactly as a plan's cut
//!   ([`crate::plan::plan`] with the view's vertical plane). And the outline of what of it lies
//!   between the plane and the view's depth, seen along the view, as projected lines: the
//!   boundary of its shadow (the crate's private sweep, from exact cuts between its corners'
//!   depths), each side named by the face or the edge it lies on. An element with layers is one
//!   shadow, its layers together.
//! - [`section`]: the drawing of the parts: every cut line and region, and each outline less what
//!   hides it: a piece of an outline inside a cut region, or inside the shadow of an element wholly
//!   nearer the viewer (its farthest depth at or before this one's nearest), or along the outline
//!   of either, is left out. [`draw`] is both steps for a list of built elements.
//!
//! A face is named through its plane, as the plan's cut names it. A line seen along an edge, the
//! path a corner of the cut takes, is named by its two faces: an [`Entity::Edge`] whose name is
//! the two faces' names, the lesser first, joined by `+`, and whose role is their roles in the same
//! order. An outline is one line for each straight run of it; where pieces of the shadow lie on
//! one another or run on in one line, the least of their sources names it (faces before edges,
//! then by name), so a face seen edge on names the line it makes.
//!
//! Interim, and which job replaces it: only the outline of each element is drawn, not the edges
//! inside it (a layer's joint on a face seen square on, a door's panel): job 8.2 (hidden-lines-a,
//! the visible and hidden edges of one element). An outline is hidden only by the cut regions and
//! by elements wholly nearer than it; two elements whose depths overlap both draw their whole
//! outlines, so a line one of them hides can show: job 9.1 (hidden-lines-b, hidden edges between
//! elements). Lines of one material that meet are not joined across elements: job 9.2. The shadow
//! is made by the private `sweep` module, which the kernel's own projection and hidden lines
//! (geom-draw, pieces G388, G389 and G394) replace with the same answers; its row is in the
//! interim register.
//!
//! Promises: exact, as the rest of the crate; the same elements in any order give the same drawing;
//! every refusal is a [`DrawingError`] value.

use std::collections::BTreeMap;

use construct_categories::CategoryId;
use construct_params::Length;
use construct_regen::kernel::{Kernel, NamedFace};
use engine_geometry::value::engine::{Engine, GridPlane};
use kit_core::Id;

use crate::DrawingError;
use crate::drawing::{Drawing, Entity, Line, LineKind, Region, Shape, Source, ViewPoint};
use crate::overlay::{Cover, Overlay, Q};
use crate::plan::{PlanElement, plan};
use crate::sweep::{Mark, sweep};
use crate::view::{Looking, View};

/// Which side of its line a section or an elevation looks to, walking from the line's first end
/// to its second.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Look {
    /// To the left.
    Left,
    /// To the right.
    Right,
}

/// A section or an elevation, as this crate draws it: a vertical plane along a grid axis, the way
/// it looks, how far it sees, and whether it cuts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SectionView {
    view: View,
    depth: Length,
    cuts: bool,
    /// The drawing's x of the line's first end.
    origin: i128,
    /// `1` when the distance along the line grows to the right on the sheet, `-1` to the left.
    sign: i128,
    /// The line's length, in grid steps.
    length: i128,
}

fn steps(length: Length) -> i128 {
    i128::from(length.steps())
}

impl SectionView {
    /// The view of the line from `from` to `to` (each the project's x and y), looking to `look`
    /// as far as `depth` beyond the line; a section when `cuts`, else an elevation.
    ///
    /// # Errors
    ///
    /// [`DrawingError::View`] for a line of no length, a line not along a grid axis, or a depth
    /// not above zero.
    pub fn on_line(
        from: [Length; 2],
        to: [Length; 2],
        look: Look,
        depth: Length,
        cuts: bool,
    ) -> Result<Self, DrawingError> {
        if depth <= Length::from_steps(0) {
            return Err(DrawingError::View(
                "a section or an elevation must see some depth beyond its line".to_owned(),
            ));
        }
        let [x0, y0] = from.map(steps);
        let [x1, y1] = to.map(steps);
        let (dx, dy) = (
            x1.checked_sub(x0).ok_or(DrawingError::TooBig)?,
            y1.checked_sub(y0).ok_or(DrawingError::TooBig)?,
        );
        let (ux, uy, length) = match (dx.signum(), dy.signum()) {
            (0, 0) => {
                return Err(DrawingError::View(
                    "a section's or an elevation's line has no length".to_owned(),
                ));
            }
            (s, 0) => (s, 0, dx.abs()),
            (0, s) => (0, s, dy.abs()),
            _ => {
                return Err(DrawingError::View(
                    "only a section or an elevation whose line runs along a grid axis is drawn \
                     yet: the engine seam cuts planes square to a grid axis only (the kernel's \
                     exact view predicates and sections, pieces G388 and G389, lift this)"
                        .to_owned(),
                ));
            }
        };
        // The direction looked in: the line's direction turned a quarter to the left or right.
        let less = |c: i128| c.checked_neg().ok_or(DrawingError::TooBig);
        let (vx, vy) = match look {
            Look::Left => (less(uy)?, ux),
            Look::Right => (uy, less(ux)?),
        };
        let (looking, cut) = match (vx, vy) {
            (0, 1) => (Looking::North, from[1]),
            (0, -1) => (Looking::South, from[1]),
            (1, 0) => (Looking::East, from[0]),
            _ => (Looking::West, from[0]),
        };
        let [rx, ry, _] = looking.right();
        let along = |a: i128, b: i128| {
            rx.checked_mul(a)
                .zip(ry.checked_mul(b))
                .and_then(|(p, q)| p.checked_add(q))
                .ok_or(DrawingError::TooBig)
        };
        Ok(Self {
            view: View { looking, cut },
            depth,
            cuts,
            origin: along(x0, y0)?,
            sign: along(ux, uy)?,
            length,
        })
    }

    /// The view: the way it looks and where its plane lies along that axis.
    #[must_use]
    pub const fn view(&self) -> View {
        self.view
    }

    /// How far beyond the plane it sees.
    #[must_use]
    pub const fn depth(&self) -> Length {
        self.depth
    }

    /// Whether it cuts what its plane crosses: a section does, an elevation does not.
    #[must_use]
    pub const fn cuts(&self) -> bool {
        self.cuts
    }

    /// The drawing's point of a point of the view's own frame: `along` the line from its first
    /// end, and `height` above the project's zero.
    ///
    /// # Errors
    ///
    /// [`DrawingError::TooBig`] never for lengths of the grid; it is checked all the same.
    pub fn point(&self, along: Length, height: Length) -> Result<ViewPoint, DrawingError> {
        let x = self
            .sign
            .checked_mul(steps(along))
            .and_then(|a| self.origin.checked_add(a))
            .ok_or(DrawingError::TooBig)?;
        Ok(ViewPoint::grid(x, steps(height)))
    }

    /// The drawing's x of the line's two ends, the lesser first: the view's width.
    ///
    /// # Errors
    ///
    /// [`DrawingError::TooBig`] never for lengths of the grid; it is checked all the same.
    pub fn ends(&self) -> Result<[i128; 2], DrawingError> {
        let far = self
            .sign
            .checked_mul(self.length)
            .and_then(|a| self.origin.checked_add(a))
            .ok_or(DrawingError::TooBig)?;
        Ok([self.origin.min(far), self.origin.max(far)])
    }

    /// The plane's depth along the view, `v · p` of its points.
    fn near(&self) -> Result<Q, DrawingError> {
        let v = self.view.looking.direction();
        let along = v.get(self.view.looking.axis()).copied().unwrap_or(1);
        Ok(Q::whole(
            along
                .checked_mul(steps(self.view.cut))
                .ok_or(DrawingError::TooBig)?,
        ))
    }

    /// The depth it sees to.
    fn far(&self) -> Result<Q, DrawingError> {
        Ok(self.near()?.plus(&Q::whole(steps(self.depth))))
    }

    /// The plan's points the view's line runs between, and its plane: for the views looking north
    /// or south, the x the line spans and the y of its plane; east or west, the y and the x.
    pub(crate) fn span(&self) -> Result<[i128; 2], DrawingError> {
        let [a, b] = self.ends()?;
        // The drawing's x is `right · p`, and `right` is a whole unit vector along the line.
        let [rx, ry, _] = self.view.looking.right();
        let sign = rx.checked_add(ry).ok_or(DrawingError::TooBig)?;
        let (a, b) = (
            a.checked_mul(sign).ok_or(DrawingError::TooBig)?,
            b.checked_mul(sign).ok_or(DrawingError::TooBig)?,
        );
        Ok([a.min(b), a.max(b)])
    }

    /// The depths it sees between, as the plan's coordinate of the axis it looks along: the
    /// plane's place, and the place `depth` beyond it, the lesser first.
    pub(crate) fn band(&self) -> Result<[i128; 2], DrawingError> {
        let v = self.view.looking.direction();
        let along = v.get(self.view.looking.axis()).copied().unwrap_or(1);
        let cut = steps(self.view.cut);
        let far = along
            .checked_mul(steps(self.depth))
            .and_then(|d| cut.checked_add(d))
            .ok_or(DrawingError::TooBig)?;
        Ok([cut.min(far), cut.max(far)])
    }
}

/// What one element gives a section or an elevation, before what hides it is taken out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Part {
    element: Id,
    lines: Vec<Line>,
    regions: Vec<Region>,
    outline: Vec<Line>,
    shadow: Vec<Vec<ViewPoint>>,
    reach: Option<(Q, Q)>,
}

impl Part {
    /// The element.
    #[must_use]
    pub fn element(&self) -> &Id {
        &self.element
    }

    /// Its cut lines (none in an elevation).
    #[must_use]
    pub fn cut_lines(&self) -> &[Line] {
        &self.lines
    }

    /// Its cut regions (none in an elevation).
    #[must_use]
    pub fn regions(&self) -> &[Region] {
        &self.regions
    }

    /// The outline of what of it is seen beyond the plane, before anything hides it.
    #[must_use]
    pub fn outline(&self) -> &[Line] {
        &self.outline
    }
}

fn unnamed(plane: &GridPlane) -> DrawingError {
    DrawingError::UnnamedFace(format!("{plane:?}"))
}

/// The source of a side the sweep marked, through `faces`' names.
fn source(element: &Id, faces: &[NamedFace], mark: Mark) -> Result<Source, DrawingError> {
    let named = |plane: GridPlane| {
        faces
            .iter()
            .find(|f| f.plane == plane)
            .ok_or_else(|| unnamed(&plane))
    };
    Ok(match mark {
        Mark::Face(plane) => {
            let face = named(plane)?;
            Source {
                element: element.clone(),
                entity: Entity::Face,
                name: face.name.name.clone(),
                role: face.name.role.clone(),
            }
        }
        Mark::Edge(a, b) => {
            let (a, b) = (named(a)?, named(b)?);
            let (first, second) = if a.name.name <= b.name.name {
                (a, b)
            } else {
                (b, a)
            };
            Source {
                element: element.clone(),
                entity: Entity::Edge,
                name: format!("{}+{}", first.name.name, second.name.name),
                role: format!("{}+{}", first.name.role, second.name.role),
            }
        }
    })
}

/// `ring` (each vertex with the label of the side leaving it) with every vertex that lies between
/// two sides running on in one line taken out, the joined side keeping the lesser label: an
/// outline is one line for each straight run, whichever pieces of the shadow made it.
fn straightened(ring: &[(ViewPoint, usize)]) -> Result<Vec<(ViewPoint, usize)>, DrawingError> {
    let at = |p: ViewPoint| -> Result<(Q, Q), DrawingError> {
        Ok((Q::fraction(p.x, p.den)?, Q::fraction(p.y, p.den)?))
    };
    let mut out: Vec<(ViewPoint, usize)> = ring.to_vec();
    loop {
        let n = out.len();
        if n <= 3 {
            return Ok(out);
        }
        let mut dropped = None;
        for i in 0..n {
            let before = i.checked_sub(1).unwrap_or(n.saturating_sub(1));
            let after = i.checked_add(1).and_then(|j| j.checked_rem(n)).unwrap_or(0);
            let (Some(a), Some(b), Some(c)) = (out.get(before), out.get(i), out.get(after)) else {
                continue;
            };
            let ((ax, ay), (bx, by), (cx, cy)) = (at(a.0)?, at(b.0)?, at(c.0)?);
            let (ux, uy) = (bx.minus(&ax), by.minus(&ay));
            let (vx, vy) = (cx.minus(&bx), cy.minus(&by));
            let cross = ux.times(&vy).minus(&uy.times(&vx));
            let dot = ux.times(&vx).plus(&uy.times(&vy));
            if cross.is_zero() && dot.sign() == std::cmp::Ordering::Greater {
                dropped = Some((before, i, a.1.min(b.1)));
                break;
            }
        }
        let Some((before, i, label)) = dropped else {
            return Ok(out);
        };
        if let Some(kept) = out.get_mut(before) {
            kept.1 = label;
        }
        out.remove(i);
    }
}

/// The outline of the union of `named` (loops each vertex with the source of the side leaving
/// it), as projected lines of `category` in order, and the union's loops, its shadow.
fn outline_of(
    named: &[Vec<(ViewPoint, Source)>],
    category: CategoryId,
) -> Result<(Vec<Line>, Vec<Vec<ViewPoint>>), DrawingError> {
    // Labels in the order of the sources, faces before edges: where pieces of an outline lie on
    // one another, or run on in one line, the least source names the line.
    let sources: Vec<Source> = named
        .iter()
        .flatten()
        .map(|(_, s)| s.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let label_of: BTreeMap<&Source, usize> =
        sources.iter().enumerate().map(|(i, s)| (s, i)).collect();
    let mut rings = Vec::with_capacity(named.len());
    for ring in named {
        let mut labelled = Vec::with_capacity(ring.len());
        for (point, src) in ring {
            let label = label_of
                .get(src)
                .copied()
                .ok_or_else(|| DrawingError::Overlay("a source of no label".to_owned()))?;
            labelled.push((*point, label));
        }
        rings.push(labelled);
    }
    let mut outline = Vec::new();
    let mut shadow = Vec::new();
    for area in Overlay::union(&rings)? {
        for ring in &area.loops {
            shadow.push(ring.iter().map(|(p, _)| *p).collect());
            let ring = straightened(ring)?;
            let n = ring.len();
            for (i, (start, label)) in ring.iter().enumerate() {
                let end = ring
                    .get(i.checked_add(1).and_then(|j| j.checked_rem(n)).unwrap_or(0))
                    .map(|(p, _)| *p)
                    .ok_or_else(|| DrawingError::Overlay("a loop of no points".to_owned()))?;
                let src = sources
                    .get(*label)
                    .ok_or_else(|| DrawingError::Overlay("a label of no source".to_owned()))?;
                outline.push(Line {
                    kind: LineKind::Projected,
                    source: src.clone(),
                    category,
                    shape: Shape::Segment { start: *start, end },
                });
            }
        }
    }
    outline.sort();
    Ok((outline, shadow))
}

/// What `view` gives of one element, `layers` (its solids, one per layer, each with the element's
/// ID, category and placement), built by `kernel`.
///
/// # Errors
///
/// [`DrawingError::BadShape`] for no layers or layers of more than one element or category; the
/// plan's cut's refusals ([`crate::plan::plan`]); [`DrawingError::Geometry`] for the engine's
/// refusal of a cut beyond the plane; [`DrawingError::UnnamedFace`] for a face of no name;
/// [`DrawingError::Overlay`] and [`DrawingError::TooBig`] from the exact work.
pub fn part<E: Engine + Send + 'static>(
    kernel: &Kernel<E>,
    view: &SectionView,
    layers: &[PlanElement<'_, E::Solid>],
) -> Result<Part, DrawingError> {
    let Some(first) = layers.first() else {
        return Err(DrawingError::BadShape(
            "an element with no solid".to_owned(),
        ));
    };
    let (element, category) = (first.id.clone(), first.category);
    if layers
        .iter()
        .any(|l| l.id != element || l.category != category)
    {
        return Err(DrawingError::BadShape(
            "the solids of one part are of more than one element or category".to_owned(),
        ));
    }
    let (lines, regions) = if view.cuts {
        let cut = plan(kernel, view.view, layers)?;
        (cut.lines().to_vec(), cut.regions().to_vec())
    } else {
        (Vec::new(), Vec::new())
    };
    let units = kernel.units();
    let in_steps = |mm: i64| {
        units.steps(mm).map_err(|e| DrawingError::Geometry {
            code: e.code,
            message: e.message,
        })
    };
    let (near, far) = (view.near()?, view.far()?);
    let mut named = Vec::new();
    let mut reach: Option<(Q, Q)> = None;
    for layer in layers {
        let p = layer.placement;
        let by = [in_steps(p.x)?, in_steps(p.y)?, in_steps(p.z)?];
        let swept = sweep(kernel, view.view.looking, by, layer.shape, (&near, &far))?;
        for ring in swept.rings {
            let mut sourced = Vec::with_capacity(ring.len());
            for (point, mark) in ring {
                sourced.push((point, source(&element, &layer.shape.faces, mark)?));
            }
            named.push(sourced);
        }
        if let Some((a, b)) = swept.reach {
            reach = Some(match reach {
                None => (a, b),
                Some((lo, hi)) => (lo.min(a), hi.max(b)),
            });
        }
    }
    let (outline, shadow) = outline_of(&named, category)?;
    Ok(Part {
        element,
        lines,
        regions,
        outline,
        shadow,
        reach,
    })
}

/// The drawing of `parts` in `view`: every cut line and region, and each part's outline less what
/// hides it (a cut region, or the shadow of a part wholly nearer the viewer).
///
/// # Errors
///
/// [`DrawingError::Overlay`] and [`DrawingError::TooBig`] from the exact work, and the checks of
/// [`Drawing::new`].
pub fn section(view: &SectionView, parts: &[Part]) -> Result<Drawing, DrawingError> {
    let mut lines = Vec::new();
    let mut regions = Vec::new();
    for part in parts {
        lines.extend(part.lines.iter().cloned());
        regions.extend(part.regions.iter().cloned());
    }
    for (i, part) in parts.iter().enumerate() {
        let Some((near, _)) = &part.reach else {
            continue;
        };
        let mut covering: Vec<&[ViewPoint]> = Vec::new();
        for region in &regions {
            covering.extend(region.loops.iter().map(Vec::as_slice));
        }
        for (j, other) in parts.iter().enumerate() {
            let nearer = other
                .reach
                .as_ref()
                .is_some_and(|(_, other_far)| other_far <= near);
            if j != i && nearer {
                covering.extend(other.shadow.iter().map(Vec::as_slice));
            }
        }
        let cover = Cover::new(covering)?;
        for line in &part.outline {
            let Shape::Segment { start, end } = line.shape else {
                continue;
            };
            for [a, b] in cover.outside(start, end)? {
                lines.push(Line {
                    shape: Shape::Segment { start: a, end: b },
                    ..line.clone()
                });
            }
        }
    }
    Drawing::new(view.view, lines, regions)
}

/// The drawing of `elements` (each layer of each element its own entry; an element's layers share
/// its ID) in `view`, built by `kernel`: [`part`] for each element, then [`section`].
///
/// # Errors
///
/// What [`part`] and [`section`] refuse, for the first element refused.
pub fn draw<E: Engine + Send + 'static>(
    kernel: &Kernel<E>,
    view: &SectionView,
    elements: &[PlanElement<'_, E::Solid>],
) -> Result<Drawing, DrawingError> {
    let mut by_element: BTreeMap<(Id, CategoryId), Vec<PlanElement<'_, E::Solid>>> =
        BTreeMap::new();
    for e in elements {
        by_element
            .entry((e.id.clone(), e.category))
            .or_default()
            .push(PlanElement {
                id: e.id.clone(),
                category: e.category,
                material: e.material.clone(),
                placement: e.placement,
                shape: e.shape,
            });
    }
    let mut parts = Vec::with_capacity(by_element.len());
    for layers in by_element.values() {
        parts.push(part(kernel, view, layers)?);
    }
    section(view, &parts)
}
