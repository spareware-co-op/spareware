//! Wall joins in plan, L and T, by layer priority (job 5.1, the first half of CON-A4-F2-P2): which
//! layered walls meet, and the plan outline of each layer of a wall once its joints are made, so
//! that the layers of two walls that meet tile the joined shape with no overlap, no gap and no
//! sliver. The rule and every case it leaves unjoined are in note CON-A4-F2-P2-wall-joins.
//!
//! - [`PlanWall`] and [`PlanLayer`]: a wall as the rule reads it, its location line in the model's
//!   plan, its heights and its layers' faces, all construct-params `Length`s.
//! - [`join`]: a wall's [`Joined`] outlines among the walls given, and why each wall that meets it
//!   is not joined with it, if it is not.
//! - [`lines_meet`]: whether two location lines meet, the test the walls given to [`join`] are
//!   found by (the walls a wall meets, and the walls they meet).
//!
//! Promises: pure and exact (integers on the kernel's grid steps, every step checked; nothing is
//! rounded: every corner is the crossing of two layer faces of walls along the plan's axes);
//! deterministic (walls are read in the order given, and the answer does not depend on which of
//! two meeting walls is asked); the two walls of a joint reach it from the same walls, so each
//! is shaped to fit the other; nothing panics. A joint the rule does not cover is left unjoined,
//! never a failure. Depends on construct-params (`Length`) and this crate's [`Function`] (each
//! layer's join priority). Nothing here is interim.

mod corner;
mod plan;
mod shape;

use crate::layer::Function;
use construct_params::Length;
use plan::{Frame, LIMIT, P, add, along, crosses, near, signed, sub};
use shape::{Edge, Layout, Notch};

/// How near two walls' lines must come to meet: the model's tolerance, 0.01 mm.
pub const TOLERANCE: Length = Length::from_steps(64);

/// One layer of a wall, as the join rule reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanLayer {
    /// What it does: its join priority.
    pub function: Function,
    /// Its exterior face's offset from the wall's location line, positive to the right of the
    /// line walking from its start to its end.
    pub exterior: Length,
    /// Its interior face's offset, the same way.
    pub interior: Length,
}

/// A wall as the join rule reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanWall {
    /// Its element's ID: the walls a sentence names.
    pub id: String,
    /// Its location line's start, in the model's plan.
    pub start: [Length; 2],
    /// Its location line's end.
    pub end: [Length; 2],
    /// The height of its base, in the model.
    pub base: Length,
    /// The height of its top.
    pub top: Length,
    /// Its layers, from the exterior face to the interior face, one's interior the next's
    /// exterior.
    pub layers: Vec<PlanLayer>,
}

/// A polygon in the model's plan, counter-clockwise seen from above.
pub type Outline = Vec<[Length; 2]>;

/// A wall's shape once its joints are made.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Joined {
    /// Each layer's pieces, from the exterior: none for a layer with no thickness, one for a
    /// layer only its ends change, more where abutting walls cut it back. Empty when no joint
    /// changed the wall.
    pub layers: Vec<Vec<Outline>>,
    /// The whole wall's outline. Empty when no joint changed it.
    pub outline: Outline,
    /// The walls it is joined with, by ID, in the order given.
    pub with: Vec<String>,
    /// For each wall that meets it and is not joined with it, why, in words.
    pub unjoined: Vec<String>,
}

/// Which end of a wall.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
    Start,
    Finish,
}

impl End {
    fn words(self) -> &'static str {
        match self {
            End::Start => "its start",
            End::Finish => "its end",
        }
    }
}

/// A wall as the rule works on it, in steps.
struct Wall<'a> {
    id: &'a str,
    start: P,
    end: P,
    frame: Option<Frame>,
    /// Its faces' offsets, face 0 the exterior, face `k + 1` layer `k`'s interior.
    faces: Vec<i64>,
    /// Each layer's rank: its priority, a membrane weakest.
    ranks: Vec<u8>,
    /// The farthest any face is from its line.
    reach: i64,
    z: [i64; 2],
}

fn within(length: Length) -> Option<i64> {
    let steps = length.steps();
    (steps.unsigned_abs() <= LIMIT.unsigned_abs()).then_some(steps)
}

fn steps2([x, y]: [Length; 2]) -> Option<P> {
    Some([within(x)?, within(y)?])
}

fn length2([x, y]: P) -> [Length; 2] {
    [Length::from_steps(x), Length::from_steps(y)]
}

impl<'a> Wall<'a> {
    /// The wall `w` describes, when it can take part: a line with a length, on the grid, with
    /// layers face to face and a thickness.
    fn read(w: &'a PlanWall) -> Option<Wall<'a>> {
        let (start, end) = (steps2(w.start)?, steps2(w.end)?);
        if start == end {
            return None;
        }
        let first = w.layers.first()?;
        let mut faces = vec![within(first.exterior)?];
        let mut ranks = Vec::with_capacity(w.layers.len());
        for layer in &w.layers {
            if Some(&within(layer.exterior)?) != faces.last() {
                return None;
            }
            faces.push(within(layer.interior)?);
            ranks.push(layer.function.priority().unwrap_or(u8::MAX));
        }
        let (outer, inner) = (*faces.first()?, *faces.last()?);
        let one_way = faces.windows(2).all(|pair| match pair {
            [a, b] => (outer > inner && a >= b) || (outer < inner && a <= b),
            _ => false,
        });
        if outer == inner || !one_way {
            return None;
        }
        let reach = faces.iter().map(|f| f.unsigned_abs()).max()?;
        Some(Wall {
            id: &w.id,
            start,
            end,
            frame: Frame::of(start, end),
            faces,
            ranks,
            reach: i64::try_from(reach).ok()?,
            z: [within(w.base)?, within(w.top)?],
        })
    }

    fn at(&self, end: End) -> P {
        match end {
            End::Start => self.start,
            End::Finish => self.end,
        }
    }

    /// Whether `p` is within the tolerance of its line.
    fn on(&self, p: P) -> bool {
        near(p, self.start, self.end, TOLERANCE.steps())
    }

    /// The direction into it from `end`.
    fn into(&self, end: End) -> Option<P> {
        let [ux, uy] = self.frame?.u;
        Some(match end {
            End::Start => [ux, uy],
            End::Finish => [ux.checked_neg()?, uy.checked_neg()?],
        })
    }

    /// Its line's across value in its own frame.
    fn line(&self) -> Option<i64> {
        along(self.start, self.frame?.r)
    }
}

/// Whether two location lines meet: an end of one within the tolerance of the other, or the two
/// crossing.
#[must_use]
pub fn lines_meet(a: [[Length; 2]; 2], b: [[Length; 2]; 2]) -> bool {
    let ([a0, a1], [b0, b1]) = (a, b);
    match (steps2(a0), steps2(a1), steps2(b0), steps2(b1)) {
        (Some(a0), Some(a1), Some(b0), Some(b1)) => segments_meet(a0, a1, b0, b1),
        _ => false,
    }
}

fn segments_meet(a0: P, a1: P, b0: P, b1: P) -> bool {
    let tol = TOLERANCE.steps();
    near(a0, b0, b1, tol)
        || near(a1, b0, b1, tol)
        || near(b0, a0, a1, tol)
        || near(b1, a0, a1, tol)
        || crosses(a0, a1, b0, b1)
}

/// The walls given, as the rule works on them.
struct Model<'a> {
    walls: Vec<Option<Wall<'a>>>,
}

/// Why a joint is not made, in words.
type Why = String;

/// A joint of the wall being shaped.
enum Joint {
    /// At its end `end`, a corner with wall `other`'s end `other_end`.
    Corner {
        end: End,
        other: usize,
        other_end: End,
    },
    /// At its end `end`, it abuts wall `through`.
    Abuts { end: End, through: usize },
    /// Wall `other`'s end `other_end` abuts it.
    Through { other: usize, other_end: End },
}

fn beyond() -> Why {
    "a length is beyond the kernel's grid".to_owned()
}

impl<'a> Model<'a> {
    fn wall(&self, i: usize) -> Option<&Wall<'a>> {
        self.walls.get(i)?.as_ref()
    }

    fn id(&self, i: usize) -> &str {
        self.wall(i).map_or("", |w| w.id)
    }

    /// Whether walls `i` and `j` meet: their lines meet and their heights overlap.
    fn meets(&self, i: usize, j: usize) -> bool {
        let (Some(a), Some(b)) = (self.wall(i), self.wall(j)) else {
            return false;
        };
        i != j
            && a.z[0] < b.z[1]
            && b.z[0] < a.z[1]
            && segments_meet(a.start, a.end, b.start, b.end)
    }

    /// Every wall that meets wall `i`, in the order given.
    fn met(&self, i: usize) -> Vec<usize> {
        (0..self.walls.len())
            .filter(|&j| self.meets(i, j))
            .collect()
    }

    /// Every wall that meets wall `i` and whose line is within the tolerance of `p`.
    fn at(&self, i: usize, p: P) -> Vec<usize> {
        self.met(i)
            .into_iter()
            .filter(|&j| self.wall(j).is_some_and(|w| w.on(p)))
            .collect()
    }

    /// Wall `j`'s band along wall `i`'s line: exact for a wall square to it along an axis, else
    /// its line's reach along `i`'s widened by its thickness and the tolerance.
    fn band_along(&self, j: usize, i: usize) -> Option<(i64, i64)> {
        let (host, other) = (self.wall(i)?, self.wall(j)?);
        let dir = host.frame?.u;
        if let Some(frame) = other.frame
            && plan::dot(frame.u, dir) == 0
        {
            let sign = along(dir, frame.r)?;
            let line = other.line()?;
            let mut values = Vec::with_capacity(other.faces.len());
            for &face in &other.faces {
                values.push(signed(add(line, face)?, sign)?);
            }
            return Some((*values.iter().min()?, *values.iter().max()?));
        }
        let (from, to) = (along(other.start, dir)?, along(other.end, dir)?);
        let widen = add(other.reach, TOLERANCE.steps())?;
        Some((sub(from.min(to), widen)?, add(from.max(to), widen)?))
    }

    /// The reach along wall `i` of its end `end` and every wall meeting it there.
    fn region(&self, i: usize, end: End) -> Option<(i64, i64)> {
        let wall = self.wall(i)?;
        let at_end = wall.at(end);
        let here = along(at_end, wall.frame?.u)?;
        let (mut lo, mut hi) = (here, here);
        for j in self.at(i, at_end) {
            let (low, high) = self.band_along(j, i)?;
            lo = lo.min(low);
            hi = hi.max(high);
        }
        Some((lo, hi))
    }

    /// Whether wall `i`'s two ends, with what meets them, stay apart by more than the tolerance.
    fn room(&self, i: usize) -> Option<bool> {
        let (_, start) = self.region(i, End::Start)?;
        let (finish, _) = self.region(i, End::Finish)?;
        Some(add(start, TOLERANCE.steps())? < finish)
    }

    /// Why walls `i` and `j`, both along the plan's axes, are not square to each other, if not.
    fn square(&self, i: usize, j: usize) -> Result<(), Why> {
        let (Some(first), Some(second)) = (self.wall(i), self.wall(j)) else {
            return Err(beyond());
        };
        match (first.frame, second.frame) {
            (Some(one), Some(two)) if plan::dot(one.u, two.u) == 0 => Ok(()),
            (Some(_), Some(_)) => Err("they run in line: walls in line are not joined".to_owned()),
            _ => Err(format!(
                "{} and {} do not both run along the plan's axes: their faces would not cross on \
                 the kernel's grid (joins at any angle are job 6.1)",
                first.id, second.id
            )),
        }
    }

    /// Whether wall `i`'s end `ei` and wall `j`'s end `ej` make a corner, and if not, why.
    fn corner(&self, i: usize, ei: End, j: usize, ej: End) -> Result<(), Why> {
        let (a, b) = (
            self.wall(i).ok_or_else(beyond)?,
            self.wall(j).ok_or_else(beyond)?,
        );
        let (pa, pb) = (a.at(ei), b.at(ej));
        let tol = TOLERANCE.steps();
        if self.at(i, pa) != [j] || self.at(j, pb) != [i] {
            return Err("more than two walls meet there (job 6.1)".to_owned());
        }
        for (w, p) in [(a, pb), (b, pa)] {
            if near(p, w.start, w.start, tol) && near(p, w.end, w.end, tol) {
                return Err(format!("{} is too short to join", w.id));
            }
        }
        self.square(i, j)?;
        match (self.room(i), self.room(j)) {
            (Some(true), Some(true)) => Ok(()),
            (Some(false), _) => Err(format!("{} is too short for joints at both its ends", a.id)),
            (_, Some(false)) => Err(format!("{} is too short for joints at both its ends", b.id)),
            _ => Err(beyond()),
        }
    }
}

/// Every layer's end on the corner of wall `a`'s end `ea` with wall `b`'s end `eb`, as `a`'s
/// along values on each layer's exterior and interior faces.
fn corner_edges(m: &Model<'_>, a: usize, ea: End, b: usize, eb: End) -> Option<Vec<Edge>> {
    let (wa, wb) = (m.wall(a)?, m.wall(b)?);
    let (fa, fb) = (wa.frame?, wb.frame?);
    let ([ax, ay], [bx, by]) = (wa.into(ea)?, wb.into(eb)?);
    let out_a = [ax.checked_neg()?, ay.checked_neg()?];
    let out_b = [bx.checked_neg()?, by.checked_neg()?];
    // The square's own axes: across A outward along `-ub`, across B outward along `-ua`.
    let (sa, sb) = (along(out_b, fa.r)?, along(out_a, fb.r)?);
    let natural_a = signed(*wa.faces.first()?, sa)? >= signed(*wa.faces.last()?, sa)?;
    let natural_b = signed(*wb.faces.first()?, sb)? >= signed(*wb.faces.last()?, sb)?;
    let outward = |v: &[u8], natural: bool| {
        let mut v = v.to_vec();
        if !natural {
            v.reverse();
        }
        v
    };
    let (ra, rb) = (outward(&wa.ranks, natural_a), outward(&wb.ranks, natural_b));
    let line_b = wb.line()?;
    let mut xs = Vec::with_capacity(wb.faces.len());
    for &face in &wb.faces {
        xs.push(signed(add(line_b, face)?, sb)?);
    }
    if !natural_b {
        xs.reverse();
    }
    let steps = corner::path(&ra, &rb)?;
    let ends = corner::ends_of_a(&steps, ra.len())?;
    let sigma = along(out_a, fa.u)?;
    let n = wa.ranks.len();
    let mut edges = Vec::with_capacity(n);
    for k in 0..n {
        let row = if natural_a {
            k
        } else {
            n.checked_sub(1)?.checked_sub(k)?
        };
        let (top, bottom) = *ends.get(row)?;
        let outer = signed(*xs.get(top)?, sigma)?;
        let inner = signed(*xs.get(bottom)?, sigma)?;
        edges.push(if natural_a {
            (outer, inner)
        } else {
            (inner, outer)
        });
    }
    Some(edges)
}

/// How wall `d`, whose end `ed` abuts wall `c`, comes into it.
struct Tee {
    /// Whether `c`'s layer 0 is on `d`'s side.
    natural: bool,
    /// For each of `d`'s layers, how many of `c`'s layers, from `d`'s side, it cuts back.
    depths: Vec<usize>,
}

impl Model<'_> {
    /// How wall `d`'s layers come into wall `c` at `d`'s end `ed`, or why they do not.
    fn tee(&self, d: usize, ed: End, c: usize) -> Result<Tee, Why> {
        let (wd, wc) = (
            self.wall(d).ok_or_else(beyond)?,
            self.wall(c).ok_or_else(beyond)?,
        );
        if self.at(d, wd.at(ed)) != [c] {
            return Err("more than two walls meet there (job 6.1)".to_owned());
        }
        self.square(d, c)?;
        let tee = self.depths(d, ed, c).ok_or_else(beyond)?;
        let tee = tee.map_err(|k| {
            format!(
                "{}'s layer {k} is stronger than every layer of {}: it would cut it in two",
                wd.id, wc.id
            )
        })?;
        if !self.room(d).ok_or_else(beyond)? {
            return Err(format!(
                "{} is too short for joints at both its ends",
                wd.id
            ));
        }
        let tol = TOLERANCE.steps();
        let (lo, hi) = self.band_along(d, c).ok_or_else(beyond)?;
        let (_, start) = self.region(c, End::Start).ok_or_else(beyond)?;
        let (finish, _) = self.region(c, End::Finish).ok_or_else(beyond)?;
        let clear = |a: i64, b: i64| add(a, tol).is_some_and(|a| a < b);
        if !clear(start, lo) || !clear(hi, finish) {
            return Err(format!("it is too near an end of {}", wc.id));
        }
        for z in self.met(c).into_iter().filter(|&z| z != d) {
            let (zl, zh) = self.band_along(z, c).ok_or_else(beyond)?;
            if !(clear(zh, lo) || clear(hi, zl)) {
                return Err(format!(
                    "it is too near {}, which meets {} there",
                    self.id(z),
                    wc.id
                ));
            }
        }
        Ok(tee)
    }

    /// For each of wall `d`'s layers, the first of wall `c`'s layers from `d`'s side as strong or
    /// stronger: how many of `c`'s layers it cuts back. The inner error is a layer of `d` with no
    /// such layer in `c`.
    fn depths(&self, d: usize, ed: End, c: usize) -> Option<Result<Tee, usize>> {
        let (wd, wc) = (self.wall(d)?, self.wall(c)?);
        let rho = along(wd.into(ed)?, wc.frame?.r)?;
        let natural = signed(*wc.faces.first()?, rho)? >= signed(*wc.faces.last()?, rho)?;
        let mut near_first = wc.ranks.clone();
        if !natural {
            near_first.reverse();
        }
        let mut depths = Vec::with_capacity(wd.ranks.len());
        for (k, &rank) in wd.ranks.iter().enumerate() {
            match near_first.iter().position(|&r| r <= rank) {
                Some(depth) => depths.push(depth),
                None => return Some(Err(k)),
            }
        }
        Some(Ok(Tee { natural, depths }))
    }
}

/// Where each of wall `d`'s layers ends in wall `c`: on the near face of the layer of `c` it
/// stops at, as `d`'s along values.
fn abut_edges(model: &Model<'_>, d: usize, c: usize, tee: &Tee) -> Option<Vec<Edge>> {
    let (wd, wc) = (model.wall(d)?, model.wall(c)?);
    let sign = along(wd.frame?.u, wc.frame?.r)?;
    let line = wc.line()?;
    let count = wc.ranks.len();
    let mut edges = Vec::with_capacity(tee.depths.len());
    for &depth in &tee.depths {
        let face = if tee.natural {
            depth
        } else {
            count.checked_sub(depth)?
        };
        let value = signed(add(line, *wc.faces.get(face)?)?, sign)?;
        edges.push((value, value));
    }
    Some(edges)
}

/// The notch wall `d`'s layers cut in wall `c`.
fn notch(model: &Model<'_>, d: usize, c: usize, tee: &Tee) -> Option<Notch> {
    let (wd, wc) = (model.wall(d)?, model.wall(c)?);
    let sign = along(wc.frame?.u, wd.frame?.r)?;
    let line = wd.line()?;
    let mut bands = Vec::with_capacity(tee.depths.len());
    for (layer, &depth) in tee.depths.iter().enumerate() {
        let one = signed(add(line, *wd.faces.get(layer)?)?, sign)?;
        let two = signed(add(line, *wd.faces.get(layer.checked_add(1)?)?)?, sign)?;
        bands.push((one.min(two), one.max(two), depth));
    }
    bands.sort_unstable();
    Some(Notch {
        from_first: tee.natural,
        bands,
    })
}

/// The joints at wall `i`'s two ends, and why each wall meeting an end is not joined with it.
fn end_joints(model: &Model<'_>, i: usize, found: &mut Vec<Joint>, why: &mut Vec<Why>) {
    let Some(wall) = model.wall(i) else { return };
    let tol = TOLERANCE.steps();
    for end in [End::Start, End::Finish] {
        let at_end = wall.at(end);
        let here = model.at(i, at_end);
        let [j] = here.as_slice() else {
            if here.len() > 1 {
                let names: Vec<&str> = here.iter().map(|&j| model.id(j)).collect();
                why.push(format!(
                    "{}: {} meet there: more than two walls at a point are job 6.1's",
                    end.words(),
                    names.join(", ")
                ));
            }
            continue;
        };
        let j = *j;
        let Some(other) = model.wall(j) else { continue };
        let corner_end = if near(at_end, other.start, other.start, tol) {
            Some(End::Start)
        } else if near(at_end, other.end, other.end, tol) {
            Some(End::Finish)
        } else {
            None
        };
        let made = match corner_end {
            Some(other_end) => model.corner(i, end, j, other_end).map(|()| Joint::Corner {
                end,
                other: j,
                other_end,
            }),
            None => model
                .tee(i, end, j)
                .map(|_| Joint::Abuts { end, through: j }),
        };
        match made {
            Ok(joint) => found.push(joint),
            Err(reason) => why.push(format!("{} meets {}: {reason}", end.words(), other.id)),
        }
    }
}

/// The walls abutting wall `i` along its line, and why each that abuts or crosses it is not
/// joined with it.
fn through_joints(model: &Model<'_>, i: usize, found: &mut Vec<Joint>, why: &mut Vec<Why>) {
    let Some(wall) = model.wall(i) else { return };
    let tol = TOLERANCE.steps();
    for j in model.met(i) {
        let Some(other) = model.wall(j) else { continue };
        let mut abuts = false;
        for other_end in [End::Start, End::Finish] {
            let theirs = other.at(other_end);
            if !wall.on(theirs)
                || near(theirs, wall.start, wall.start, tol)
                || near(theirs, wall.end, wall.end, tol)
            {
                continue;
            }
            abuts = true;
            match model.tee(j, other_end, i) {
                Ok(_) => found.push(Joint::Through {
                    other: j,
                    other_end,
                }),
                Err(reason) => why.push(format!("{} abuts it: {reason}", other.id)),
            }
        }
        let touches = [other.start, other.end].iter().any(|&e| wall.on(e))
            || [wall.start, wall.end].iter().any(|&e| other.on(e));
        if !abuts && !touches && crosses(wall.start, wall.end, other.start, other.end) {
            why.push(format!(
                "{} crosses it: crossing walls are joined by job 6.1",
                other.id
            ));
        }
    }
}

/// The joints of wall `i` among the walls of `model`, and why each other wall that meets it is
/// not joined with it, in order: its start, its end, then each wall that abuts or crosses it.
fn joints(model: &Model<'_>, i: usize) -> (Vec<Joint>, Vec<Why>) {
    let (mut found, mut why) = (Vec::new(), Vec::new());
    end_joints(model, i, &mut found, &mut why);
    through_joints(model, i, &mut found, &mut why);
    (found, why)
}

/// Wall `i`'s pieces and outline with its joints made.
fn shaped(model: &Model<'_>, i: usize, found: &[Joint]) -> Option<(Vec<Vec<Outline>>, Outline)> {
    let wall = model.wall(i)?;
    let frame = wall.frame?;
    let count = wall.ranks.len();
    let square = |end: End| along(wall.at(end), frame.u).map(|a| vec![(a, a); count]);
    let (mut start, mut finish) = (square(End::Start)?, square(End::Finish)?);
    let mut notches = Vec::new();
    for joint in found {
        match *joint {
            Joint::Corner {
                end,
                other,
                other_end,
            } => {
                let edges = corner_edges(model, i, end, other, other_end)?;
                *(if end == End::Start {
                    &mut start
                } else {
                    &mut finish
                }) = edges;
            }
            Joint::Abuts { end, through } => {
                let tee = model.tee(i, end, through).ok()?;
                let edges = abut_edges(model, i, through, &tee)?;
                *(if end == End::Start {
                    &mut start
                } else {
                    &mut finish
                }) = edges;
            }
            Joint::Through { other, other_end } => {
                let tee = model.tee(other, other_end, i).ok()?;
                notches.push(notch(model, other, i, &tee)?);
            }
        }
    }
    let layout = Layout {
        frame,
        line: wall.line()?,
        faces: &wall.faces,
        start,
        finish,
        notches,
    };
    let to_lengths = |points: Vec<P>| points.into_iter().map(length2).collect::<Outline>();
    let layers = layout
        .all_pieces()?
        .into_iter()
        .map(|pieces| pieces.into_iter().map(to_lengths).collect())
        .collect();
    Some((layers, to_lengths(layout.outline()?)))
}

/// Wall `me` of `walls` with its joints made: each layer's pieces and its outline, the walls it
/// is joined with, and why each other wall that meets it is not.
///
/// `walls` must hold every wall that meets `me` and every wall that meets one of those (more do
/// no harm), in an order every caller keeps (by ID); a wall whose layers do not run face to face
/// or whose line has no length takes no part. A wall with no joint made has no layers or outline
/// here: it is built as it was.
#[must_use]
pub fn join(walls: &[PlanWall], me: usize) -> Joined {
    let model = Model {
        walls: walls.iter().map(Wall::read).collect(),
    };
    if model.wall(me).is_none() {
        return Joined::default();
    }
    let (found, unjoined) = joints(&model, me);
    let mut with: Vec<usize> = found
        .iter()
        .map(|joint| match *joint {
            Joint::Corner { other, .. } | Joint::Through { other, .. } => other,
            Joint::Abuts { through, .. } => through,
        })
        .collect();
    with.sort_unstable();
    with.dedup();
    if found.is_empty() {
        return Joined {
            unjoined,
            ..Joined::default()
        };
    }
    if let Some((layers, outline)) = shaped(&model, me, &found) {
        Joined {
            layers,
            outline,
            with: with.into_iter().map(|j| model.id(j).to_owned()).collect(),
            unjoined,
        }
    } else {
        let mut unjoined = unjoined;
        unjoined.push(format!(
            "its joints with {} could not be laid out on the kernel's grid",
            with.iter()
                .map(|&j| model.id(j))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        Joined {
            unjoined,
            ..Joined::default()
        }
    }
}
