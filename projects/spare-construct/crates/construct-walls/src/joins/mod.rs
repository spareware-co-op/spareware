//! Wall joins in plan (CON-A4-F2-P2): which layered walls meet, and the plan outline of each layer
//! of a wall once its joints are made, so that the layers of walls that meet tile the joined
//! shape with no overlap, no gap and no sliver. L and T joins by layer priority are job 5.1's;
//! crosses, joins at any angle, a person's choices (butt, mitre or square-off, which wall runs
//! through, joins allowed or not at each end) and wrapping at free ends are job 6.1's. The rule
//! and every case it leaves unjoined are in note CON-A4-F2-P2-wall-joins; wrapping is note
//! CON-A4-F2-P4-wall-wrapping.
//!
//! - [`PlanWall`] and [`PlanLayer`]: a wall as the rule reads it, its location line in the model's
//!   plan, its heights, its layers' faces and core, how it wraps at a free end and its own
//!   choices ([`crate::Joining`]), all construct-params `Length`s.
//! - [`join`]: a wall's [`Joined`] outlines among the walls given, and why each wall that meets it
//!   is not joined with it, if it is not.
//! - [`lines_meet`]: whether two location lines meet, the test the walls given to [`join`] are
//!   found by (the walls a wall meets, and the walls they meet).
//! - [`meeting`]: how two walls meet, from their lines alone: what a command checks a choice
//!   against.
//!
//! Promises: pure and exact (integers on the kernel's grid steps, every step checked; a corner of
//! walls along the plan's axes, or on whole ratios, is where two faces cross with no rounding;
//! at any other angle each corner is the true crossing rounded once to the nearest step, within
//! half a step on each axis, [`exact`]); deterministic (walls are read in the order given, a
//! joint's choice is the one the wall of the lower ID keeps, and the answer does not depend on
//! which of two meeting walls is asked); the walls of a joint reach it from the same walls, so
//! each is shaped to fit the other; nothing panics. A joint the rule does not cover is left
//! unjoined, never a failure. Depends on construct-params (`Length`), this crate's [`Function`]
//! (each layer's join priority), [`crate::joining`] (the choices) and [`crate::wrap`] (wrapping),
//! and num-bigint for the exact crossings. Nothing here is interim.

mod corner;
mod exact;
mod plan;
mod shape;

use crate::joining::{Choice, JoinKind, Joining, WallEnd, keeper};
use crate::layer::Function;
use crate::wrap::{AtEnds, WrapLayer, turned};
use construct_params::Length;
use corner::Tie;
use plan::{
    LIMIT, P, add, along, cross, crosses, dot, farther, length_up, less, near, negated, plus,
    right, side, signed,
};
use shape::{Band, Cut, Edge, Gap, Layout, Notch};

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
    /// Whether it wraps where its wall wraps its side ([`crate::Layer::wraps`]).
    pub wraps: bool,
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
    /// Its core: the layers `core[0]..core[1]`, its type's two boundaries.
    pub core: [usize; 2],
    /// How it wraps at a free end: its type's choice, or its own over it.
    pub wrap_ends: AtEnds,
    /// Its own choices: where joins are allowed, and the choices of the joints it keeps.
    pub joining: Joining,
}

impl PlanWall {
    /// A wall with no wrapping and no choices of its own, its core the layers `core[0]..core[1]`.
    #[must_use]
    pub fn new(
        id: String,
        (start, end): ([Length; 2], [Length; 2]),
        (base, top): (Length, Length),
        layers: Vec<PlanLayer>,
        core: [usize; 2],
    ) -> Self {
        Self {
            id,
            start,
            end,
            base,
            top,
            layers,
            core,
            wrap_ends: AtEnds::None,
            joining: Joining::default(),
        }
    }
}

/// A polygon in the model's plan, counter-clockwise seen from above.
pub type Outline = Vec<[Length; 2]>;

/// A wall's shape once its joints are made.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Joined {
    /// Each layer's pieces, from the exterior: none for a layer with no thickness, one for a
    /// layer only its ends change, more where abutting or crossing walls cut it back, a cross
    /// cuts it in two, or a wrapped end turns it. Empty when no joint or wrap changed the wall.
    pub layers: Vec<Vec<Outline>>,
    /// The whole wall's outline. Empty when no joint or wrap changed it.
    pub outline: Outline,
    /// The walls it is joined with, by ID, in the order given.
    pub with: Vec<String>,
    /// For each wall that meets it and is not joined with it, why, in words.
    pub unjoined: Vec<String>,
}

/// How two walls meet, from their lines alone ([`meeting`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Meeting {
    /// An end of each at one point: an L.
    Corner,
    /// An end of one on the other's line away from its ends: a T, the wall named running through.
    Tee {
        /// The wall the other abuts, by ID.
        through: String,
    },
    /// The two lines cross, away from both walls' ends.
    Cross,
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

    fn stored(self) -> WallEnd {
        match self {
            End::Start => WallEnd::Start,
            End::Finish => WallEnd::End,
        }
    }
}

/// A wall as the rule works on it, in steps.
struct Wall<'a> {
    id: &'a str,
    start: P,
    end: P,
    /// Its direction: its end less its start.
    d: P,
    /// Its faces' offsets, face 0 the exterior, face `k + 1` layer `k`'s interior.
    faces: Vec<i64>,
    /// Each face's corner at its start, `start + R(offset)`: face `k` is the line through it
    /// along `d`, as the builders lay it out.
    at_start: Vec<P>,
    /// Each face's corner at its end, `end + R(offset)`.
    at_end: Vec<P>,
    /// Each layer's rank: its priority, a membrane weakest.
    ranks: Vec<u8>,
    /// Each layer's flag: whether it wraps.
    wraps: Vec<bool>,
    /// Its core's layers.
    core: (usize, usize),
    /// How it wraps at a free end.
    wrap_ends: AtEnds,
    /// Its own choices.
    joining: &'a Joining,
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

/// `p` if it is within the grid.
fn in_grid(p: P) -> Option<P> {
    let [x, y] = p;
    (x.unsigned_abs() <= LIMIT.unsigned_abs() && y.unsigned_abs() <= LIMIT.unsigned_abs())
        .then_some(p)
}

impl<'a> Wall<'a> {
    /// The wall `w` describes, when it can take part: a line with a length, on the grid, with
    /// layers face to face and a thickness, and a core among them.
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
        let [c0, c1] = w.core;
        if c0 >= c1 || c1 > w.layers.len() {
            return None;
        }
        let d = less(end, start)?;
        let mut at_start = Vec::with_capacity(faces.len());
        let mut at_end = Vec::with_capacity(faces.len());
        for &face in &faces {
            let r = exact::across(face, d)?;
            at_start.push(in_grid(plus(start, r)?)?);
            at_end.push(in_grid(plus(end, r)?)?);
        }
        let reach = faces.iter().map(|f| f.unsigned_abs()).max()?;
        Some(Wall {
            id: &w.id,
            start,
            end,
            d,
            faces,
            at_start,
            at_end,
            ranks,
            wraps: w.layers.iter().map(|l| l.wraps).collect(),
            core: (c0, c1),
            wrap_ends: w.wrap_ends,
            joining: &w.joining,
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
        match end {
            End::Start => Some(self.d),
            End::Finish => negated(self.d),
        }
    }

    /// Where `p` lies along it, as [`along`] gives it.
    fn along(&self, p: P) -> Option<i128> {
        along(p, self.start, self.d)
    }

    /// Face `k`'s corner at its start: the face is the line through it along `d`.
    fn face(&self, k: usize) -> Option<P> {
        self.at_start.get(k).copied()
    }

    /// The index of its last face.
    fn last(&self) -> usize {
        self.faces.len().saturating_sub(1)
    }

    /// Whether joins are allowed at `end`.
    fn allows(&self, end: End) -> bool {
        self.joining.allows(end.stored())
    }

    /// Its strongest rank.
    fn best(&self) -> u8 {
        self.ranks.iter().copied().min().unwrap_or(u8::MAX)
    }

    /// Where its face `k` meets `other`'s face `j`.
    fn meet(&self, k: usize, other: &Wall<'_>, j: usize) -> Option<P> {
        exact::meet(self.face(k)?, self.d, other.face(j)?, other.d)
    }

    /// `other`'s face `j` as a cut across this wall.
    fn cut(&self, other: &Wall<'_>, j: usize) -> Option<Cut> {
        let point = other.face(j)?;
        let on_line = exact::meet(self.start, self.d, point, other.d)?;
        Some(Cut {
            point,
            dir: other.d,
            key: self.along(on_line)?,
        })
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

/// How walls `a` and `b` of `walls` meet, from their lines alone; `None` when they do not meet,
/// or one does not read as a wall.
#[must_use]
pub fn meeting(walls: &[PlanWall], a: usize, b: usize) -> Option<Meeting> {
    let (wa, wb) = (Wall::read(walls.get(a)?)?, Wall::read(walls.get(b)?)?);
    if a == b || !segments_meet(wa.start, wa.end, wb.start, wb.end) {
        return None;
    }
    let tol = TOLERANCE.steps();
    let at_end = |w: &Wall<'_>, p: P| near(p, w.start, w.start, tol) || near(p, w.end, w.end, tol);
    for (one, other) in [(&wa, &wb), (&wb, &wa)] {
        for p in [one.start, one.end] {
            if other.on(p) {
                return Some(if at_end(other, p) {
                    Meeting::Corner
                } else {
                    Meeting::Tee {
                        through: other.id.to_owned(),
                    }
                });
            }
        }
    }
    Some(Meeting::Cross)
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
    /// Wall `through` crosses it and runs through: it is cut in two.
    Cut { through: usize },
    /// Wall `other` crosses it and is cut in two by it.
    Crossed { other: usize },
}

fn beyond() -> Why {
    "a length is beyond the kernel's grid".to_owned()
}

/// How wall `d`'s layers come into wall `c` from one side.
struct Tee {
    /// Whether `c`'s layer 0 is on `d`'s side.
    natural: bool,
    /// For each of `d`'s layers, how many of `c`'s layers, from `d`'s side, it cuts back.
    depths: Vec<usize>,
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

    /// The choice the walls `i` and `j` keep for their joint.
    fn choice(&self, i: usize, j: usize) -> Option<&Choice> {
        let (a, b) = (self.wall(i)?, self.wall(j)?);
        Joining::choice_of((a.id, a.joining), (b.id, b.joining))
    }

    /// The wall a butt runs through first: the one the choice names, else the lower ID.
    fn first_of(&self, i: usize, j: usize) -> usize {
        let named = self.choice(i, j).and_then(|c| c.first.as_deref());
        match named {
            Some(id) if id == self.id(i) => i,
            Some(id) if id == self.id(j) => j,
            _ => {
                if keeper(self.id(i), self.id(j)) == self.id(i) {
                    i
                } else {
                    j
                }
            }
        }
    }

    /// How two layers of the same priority meet at the corner of walls `a` and `b`, seen from `a`.
    fn tie(&self, a: usize, b: usize) -> Tie {
        match self.choice(a, b).map(|c| c.kind) {
            None | Some(JoinKind::Mitre) => Tie::Mitre,
            Some(JoinKind::Butt | JoinKind::SquareOff) => {
                if self.first_of(a, b) == a {
                    Tie::A
                } else {
                    Tie::B
                }
            }
        }
    }

    /// Wall `j`'s band along wall `i`'s line, as [`along`] gives it: where `j`'s outer faces
    /// cross `i`'s, or for a wall in line with it, its line's reach along `i` widened by its
    /// thickness and the tolerance.
    fn band_along(&self, j: usize, i: usize) -> Option<(i128, i128)> {
        let (host, other) = (self.wall(i)?, self.wall(j)?);
        if cross(host.d, other.d) != 0 {
            let mut values = Vec::with_capacity(4);
            for hf in [0, host.last()] {
                for of in [0, other.last()] {
                    values.push(host.along(host.meet(hf, other, of)?)?);
                }
            }
            return Some((*values.iter().min()?, *values.iter().max()?));
        }
        let (from, to) = (host.along(other.start)?, host.along(other.end)?);
        let widen =
            i128::from(add(other.reach, TOLERANCE.steps())?).checked_mul(length_up(host.d)?)?;
        Some((
            from.min(to).checked_sub(widen)?,
            from.max(to).checked_add(widen)?,
        ))
    }

    /// The reach along wall `i` of its end `end` and every wall meeting it there.
    fn region(&self, i: usize, end: End) -> Option<(i128, i128)> {
        let wall = self.wall(i)?;
        let at_end = wall.at(end);
        let here = wall.along(at_end)?;
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
        let wall = self.wall(i)?;
        let (_, start) = self.region(i, End::Start)?;
        let (finish, _) = self.region(i, End::Finish)?;
        Some(farther(
            finish.checked_sub(start)?,
            TOLERANCE.steps(),
            wall.d,
        ))
    }

    /// Why walls `i` and `j` cannot be joined at any angle, if they cannot: they run in line.
    fn crossing(&self, i: usize, j: usize) -> Result<(), Why> {
        let (Some(first), Some(second)) = (self.wall(i), self.wall(j)) else {
            return Err(beyond());
        };
        if cross(first.d, second.d) == 0 {
            return Err("they run in line: walls in line are not joined".to_owned());
        }
        Ok(())
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
            return Err(
                "more than two walls meet there, and such points are not joined".to_owned(),
            );
        }
        if !b.allows(ej) {
            return Err(format!("{} does not allow joins at {}", b.id, ej.words()));
        }
        for (w, p) in [(a, pb), (b, pa)] {
            if near(p, w.start, w.start, tol) && near(p, w.end, w.end, tol) {
                return Err(format!("{} is too short to join", w.id));
            }
        }
        self.crossing(i, j)?;
        match (self.room(i), self.room(j)) {
            (Some(true), Some(true)) => Ok(()),
            (Some(false), _) => Err(format!("{} is too short for joints at both its ends", a.id)),
            (_, Some(false)) => Err(format!("{} is too short for joints at both its ends", b.id)),
            _ => Err(beyond()),
        }
    }

    /// How wall `d`'s layers come into wall `c` at `d`'s end `ed`, or why they do not.
    fn tee(&self, d: usize, ed: End, c: usize) -> Result<Tee, Why> {
        let (wd, wc) = (
            self.wall(d).ok_or_else(beyond)?,
            self.wall(c).ok_or_else(beyond)?,
        );
        if self.at(d, wd.at(ed)) != [c] {
            return Err(
                "more than two walls meet there, and such points are not joined".to_owned(),
            );
        }
        if !wd.allows(ed) {
            return Err(format!("{} does not allow joins at {}", wd.id, ed.words()));
        }
        self.crossing(d, c)?;
        let tee = self
            .depths(d, wd.into(ed).ok_or_else(beyond)?, c)
            .ok_or_else(beyond)?;
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
        self.clear_on(d, c)?;
        Ok(tee)
    }

    /// Why wall `d`'s band along wall `c` is not clear of `c`'s ends and of every other wall
    /// meeting `c` there, if it is not.
    fn clear_on(&self, d: usize, c: usize) -> Result<(), Why> {
        let wc = self.wall(c).ok_or_else(beyond)?;
        let tol = TOLERANCE.steps();
        let (lo, hi) = self.band_along(d, c).ok_or_else(beyond)?;
        let (_, start) = self.region(c, End::Start).ok_or_else(beyond)?;
        let (finish, _) = self.region(c, End::Finish).ok_or_else(beyond)?;
        let clear = |a: i128, b: i128| b.checked_sub(a).is_some_and(|gap| farther(gap, tol, wc.d));
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
        Ok(())
    }

    /// For each of wall `d`'s layers, coming into wall `c` along `into`, the first of `c`'s
    /// layers from `d`'s side as strong or stronger: how many of `c`'s layers it cuts back. The
    /// inner error is a layer of `d` with no such layer in `c`.
    fn depths(&self, d: usize, into: P, c: usize) -> Option<Result<Tee, usize>> {
        let (wd, wc) = (self.wall(d)?, self.wall(c)?);
        let rho = side(into, right(wc.d)?);
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

    /// At a cross of walls `i` and `j`, the one that runs through: the stronger, or where both are
    /// as strong, the first the choice names (else the lower ID).
    fn through_of(&self, i: usize, j: usize) -> Option<usize> {
        let (a, b) = (self.wall(i)?, self.wall(j)?);
        Some(match a.best().cmp(&b.best()) {
            std::cmp::Ordering::Less => i,
            std::cmp::Ordering::Greater => j,
            std::cmp::Ordering::Equal => self.first_of(i, j),
        })
    }

    /// How wall `x`, crossed by wall `t` running through, comes into `t` from its start's side and
    /// from its end's side, or why they do not cross cleanly.
    fn cross_cut(&self, x: usize, t: usize) -> Result<(Tee, Tee), Why> {
        let wx = self.wall(x).ok_or_else(beyond)?;
        self.crossing(x, t)?;
        let into_start = negated(wx.d).ok_or_else(beyond)?;
        let side_one = self.depths(x, into_start, t).ok_or_else(beyond)?;
        let side_two = self.depths(x, wx.d, t).ok_or_else(beyond)?;
        let (Ok(one), Ok(two)) = (side_one, side_two) else {
            return Err(format!(
                "a layer of {} is stronger than every layer of {}",
                wx.id,
                self.id(t)
            ));
        };
        self.clear_on(x, t)?;
        self.clear_on(t, x)?;
        Ok((one, two))
    }
}

/// Every layer's end on the corner of wall `a`'s end `ea` with wall `b`'s end `eb`: its corners
/// on its exterior and interior faces.
fn corner_edges(m: &Model<'_>, a: usize, ea: End, b: usize, eb: End) -> Option<Vec<Edge>> {
    let (wa, wb) = (m.wall(a)?, m.wall(b)?);
    let out_a = negated(wa.into(ea)?)?;
    let out_b = negated(wb.into(eb)?)?;
    // Each wall's outside of the corner: the side of its line away from the other's body.
    let (sa, sb) = (side(out_b, right(wa.d)?), side(out_a, right(wb.d)?));
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
    let tie = m.tie(a, b);
    let steps = corner::path(&ra, &rb, tie)?;
    let ends = corner::ends_of_a(&steps, ra.len())?;
    let last_b = wb.last();
    let b_face = |j: usize| {
        if natural_b {
            Some(j)
        } else {
            last_b.checked_sub(j)
        }
    };
    let n = wa.ranks.len();
    let mut edges = Vec::with_capacity(n);
    let mut through = Vec::with_capacity(n);
    for k in 0..n {
        let row = if natural_a {
            k
        } else {
            n.checked_sub(1)?.checked_sub(k)?
        };
        let (top, bottom) = *ends.get(row)?;
        through.push((top, bottom) == (0, 0));
        let (outer, inner) = if natural_a {
            (k, k.checked_add(1)?)
        } else {
            (k.checked_add(1)?, k)
        };
        let p_outer = wa.meet(outer, wb, b_face(top)?)?;
        let p_inner = wa.meet(inner, wb, b_face(bottom)?)?;
        edges.push(if natural_a {
            (p_outer, p_inner)
        } else {
            (p_inner, p_outer)
        });
    }
    if m.choice(a, b).map(|c| c.kind) == Some(JoinKind::SquareOff) && tie == Tie::A {
        square_off(wa, out_a, &mut edges, &through)?;
    }
    Some(edges)
}

/// A square-off's through wall's end: every layer of it that runs through the other wall ends on
/// the line square to its own through the farthest out of their corners, `out` the way out of it
/// there; its other layers end where the butt ends them.
fn square_off(wall: &Wall<'_>, out: P, edges: &mut [Edge], through: &[bool]) -> Option<()> {
    let Some(far) = edges
        .iter()
        .zip(through)
        .filter(|(_, through)| **through)
        .flat_map(|(&(e, i), _)| [e, i])
        .max_by_key(|&p| dot(p, out))
    else {
        return Some(());
    };
    let across = right(wall.d)?;
    for (k, (edge, through)) in edges.iter_mut().zip(through).enumerate() {
        if *through {
            *edge = (
                exact::meet(wall.face(k)?, wall.d, far, across)?,
                exact::meet(wall.face(k.checked_add(1)?)?, wall.d, far, across)?,
            );
        }
    }
    Some(())
}

/// Where each of wall `d`'s layers ends in wall `c`: on the near face of the layer of `c` it
/// stops at.
fn abut_edges(model: &Model<'_>, d: usize, c: usize, tee: &Tee) -> Option<Vec<Edge>> {
    let (wd, wc) = (model.wall(d)?, model.wall(c)?);
    let count = wc.ranks.len();
    let mut edges = Vec::with_capacity(tee.depths.len());
    for (k, &depth) in tee.depths.iter().enumerate() {
        let face = if tee.natural {
            depth
        } else {
            count.checked_sub(depth)?
        };
        edges.push((wd.meet(k, wc, face)?, wd.meet(k.checked_add(1)?, wc, face)?));
    }
    Some(edges)
}

/// The notch wall `d`'s layers cut in wall `c`, coming in as `tee` says.
fn notch(model: &Model<'_>, d: usize, c: usize, tee: &Tee) -> Option<Notch> {
    let (wd, wc) = (model.wall(d)?, model.wall(c)?);
    let mut bands = Vec::with_capacity(tee.depths.len());
    for (layer, &depth) in tee.depths.iter().enumerate() {
        let one = wc.cut(wd, layer)?;
        let two = wc.cut(wd, layer.checked_add(1)?)?;
        let (lo, hi) = if one.key <= two.key {
            (one, two)
        } else {
            (two, one)
        };
        bands.push(Band { lo, hi, depth });
    }
    bands.sort_unstable_by_key(|b| (b.lo.key, b.hi.key, b.depth));
    Some(Notch {
        from_first: tee.natural,
        bands,
    })
}

/// Where a cross by wall `t` cuts each layer of wall `x` in two: from `x`'s start side the layer
/// stops on the near face of the layer of `t` it stops at, and from its end side the same.
fn gaps(model: &Model<'_>, x: usize, t: usize, (one, two): &(Tee, Tee)) -> Option<Vec<Gap>> {
    let (wx, wt) = (model.wall(x)?, model.wall(t)?);
    let count = wt.ranks.len();
    let face_of = |tee: &Tee, depth: usize| {
        if tee.natural {
            Some(depth)
        } else {
            count.checked_sub(depth)
        }
    };
    let mut out = Vec::with_capacity(one.depths.len());
    for (&d1, &d2) in one.depths.iter().zip(&two.depths) {
        let lo = wx.cut(wt, face_of(one, d1)?)?;
        let hi = wx.cut(wt, face_of(two, d2)?)?;
        out.push(Gap { lo, hi });
    }
    Some(out)
}

/// The joints at wall `i`'s two ends, and why each wall meeting an end is not joined with it.
fn end_joints(model: &Model<'_>, i: usize, found: &mut Vec<Joint>, why: &mut Vec<Why>) {
    let Some(wall) = model.wall(i) else { return };
    let tol = TOLERANCE.steps();
    for end in [End::Start, End::Finish] {
        let at_end = wall.at(end);
        let here = model.at(i, at_end);
        if !here.is_empty() && !wall.allows(end) {
            why.push(format!("{}: joins are not allowed there", end.words()));
            continue;
        }
        let [j] = here.as_slice() else {
            if here.len() > 1 {
                let names: Vec<&str> = here.iter().map(|&j| model.id(j)).collect();
                why.push(format!(
                    "{}: {} meet there: more than two walls at a point are not joined",
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

/// The walls abutting or crossing wall `i` along its line, and why each that abuts or crosses it
/// is not joined with it.
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
        if abuts || touches || !crosses(wall.start, wall.end, other.start, other.end) {
            continue;
        }
        let Some(t) = model.through_of(i, j) else {
            why.push(format!("{} crosses it: {}", other.id, beyond()));
            continue;
        };
        let x = if t == i { j } else { i };
        match model.cross_cut(x, t) {
            Ok(_) if t == i => found.push(Joint::Crossed { other: j }),
            Ok(_) => found.push(Joint::Cut { through: j }),
            Err(reason) => why.push(format!("{} crosses it: {reason}", other.id)),
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

/// The square edges of a wall's end: each layer's corners there.
fn square(corners: &[P]) -> Option<Vec<Edge>> {
    (0..corners.len().saturating_sub(1))
        .map(|k| Some((*corners.get(k)?, *corners.get(k.checked_add(1)?)?)))
        .collect()
}

/// A wall's end wrapped as its free-end wrapping says: each layer's edge set back from it, and
/// each turned part. `None` when nothing turns; an inner `None` when it cannot be laid out.
#[allow(
    clippy::type_complexity,
    reason = "the edges and the turned parts of one end, read once by the caller"
)]
fn wrapped(wall: &Wall<'_>, end: End) -> Option<Option<(Vec<Edge>, Vec<(usize, Vec<P>)>)>> {
    let (exterior, interior) = wall.wrap_ends.sides();
    if !exterior && !interior {
        return None;
    }
    let mut layers = Vec::with_capacity(wall.ranks.len());
    let mut depth_of = vec![0_i64];
    for pair in wall.faces.windows(2) {
        let [a, b] = pair else { return Some(None) };
        let thickness = i64::try_from(a.abs_diff(*b)).ok();
        let Some(thickness) = thickness else {
            return Some(None);
        };
        let k = layers.len();
        layers.push(WrapLayer {
            thickness,
            wraps: wall.wraps.get(k).copied().unwrap_or(false),
        });
        let Some(next) = depth_of.last().and_then(|d| d.checked_add(thickness)) else {
            return Some(None);
        };
        depth_of.push(next);
    }
    let turns = turned(&layers, wall.core, exterior, interior)?;
    if !turns.turns_any() {
        return None;
    }
    Some(wrap_end(wall, end, &turns, &depth_of))
}

/// [`wrapped`]'s layout of one end.
#[allow(clippy::type_complexity, reason = "as wrapped")]
fn wrap_end(
    wall: &Wall<'_>,
    end: End,
    turns: &crate::wrap::Turned,
    depth_of: &[i64],
) -> Option<(Vec<Edge>, Vec<(usize, Vec<P>)>)> {
    let corners = match end {
        End::Start => &wall.at_start,
        End::Finish => &wall.at_end,
    };
    // Into the wall from this end.
    let inward = wall.into(end)?;
    let shift = |p: P, setback: i64| plus(p, exact::back(setback, inward)?);
    let face_at = |depth: i64| depth_of.iter().position(|&d| d == depth);
    let mut edges = Vec::with_capacity(wall.ranks.len());
    for k in 0..wall.ranks.len() {
        let setback = turns
            .setbacks
            .get(k)
            .and_then(|parts| parts.first())
            .map_or(0, |part| part.setback);
        edges.push((
            shift(*corners.get(k)?, setback)?,
            shift(*corners.get(k.checked_add(1)?)?, setback)?,
        ));
    }
    let mut parts = Vec::with_capacity(turns.turns.len());
    for turn in &turns.turns {
        let (a, b) = (face_at(turn.from)?, face_at(turn.to)?);
        let (ca, cb) = (*corners.get(a)?, *corners.get(b)?);
        parts.push((
            turn.layer,
            vec![
                shift(ca, turn.far)?,
                shift(ca, turn.near)?,
                shift(cb, turn.near)?,
                shift(cb, turn.far)?,
            ],
        ));
    }
    Some((edges, parts))
}

/// Wall `i`'s pieces and outline with its joints made and its free ends wrapped; `None` when they
/// cannot be laid out on the grid.
fn shaped(model: &Model<'_>, i: usize, found: &[Joint]) -> Option<(Vec<Vec<Outline>>, Outline)> {
    let wall = model.wall(i)?;
    let (mut start, mut finish) = (square(&wall.at_start)?, square(&wall.at_end)?);
    let mut joined_ends = [false, false];
    let mut notches = Vec::new();
    let mut gaps_of: Vec<Vec<Gap>> = vec![Vec::new(); wall.ranks.len()];
    for joint in found {
        match *joint {
            Joint::Corner {
                end,
                other,
                other_end,
            } => {
                let edges = corner_edges(model, i, end, other, other_end)?;
                set_end(&mut start, &mut finish, &mut joined_ends, end, edges);
            }
            Joint::Abuts { end, through } => {
                let tee = model.tee(i, end, through).ok()?;
                let edges = abut_edges(model, i, through, &tee)?;
                set_end(&mut start, &mut finish, &mut joined_ends, end, edges);
            }
            Joint::Through { other, other_end } => {
                let tee = model.tee(other, other_end, i).ok()?;
                notches.push(notch(model, other, i, &tee)?);
            }
            Joint::Crossed { other } => {
                let (one, two) = model.cross_cut(other, i).ok()?;
                notches.push(notch(model, other, i, &one)?);
                notches.push(notch(model, other, i, &two)?);
            }
            Joint::Cut { through } => {
                let sides = model.cross_cut(i, through).ok()?;
                for (layer, gap) in gaps(model, i, through, &sides)?.into_iter().enumerate() {
                    gaps_of.get_mut(layer)?.push(gap);
                }
            }
        }
    }
    let (outline_start, outline_finish) = (start.clone(), finish.clone());
    let mut turns = Vec::new();
    for (end, joined) in [End::Start, End::Finish].into_iter().zip(joined_ends) {
        if joined {
            continue;
        }
        if let Some(made) = wrapped(wall, end) {
            let (edges, parts) = made?;
            match end {
                End::Start => start = edges,
                End::Finish => finish = edges,
            }
            turns.extend(parts);
        }
    }
    let layout = Layout {
        d: wall.d,
        origin: wall.start,
        faces: &wall.at_start,
        start,
        finish,
        outline_start,
        outline_finish,
        notches,
        gaps: gaps_of,
        turns,
    };
    let to_lengths = |points: Vec<P>| points.into_iter().map(length2).collect::<Outline>();
    let layers = layout
        .all_pieces()?
        .into_iter()
        .map(|pieces| pieces.into_iter().map(to_lengths).collect())
        .collect();
    Some((layers, to_lengths(layout.outline()?)))
}

fn set_end(
    start: &mut Vec<Edge>,
    finish: &mut Vec<Edge>,
    joined: &mut [bool; 2],
    end: End,
    edges: Vec<Edge>,
) {
    match end {
        End::Start => {
            *start = edges;
            joined[0] = true;
        }
        End::Finish => {
            *finish = edges;
            joined[1] = true;
        }
    }
}

/// Whether wall `i` wraps at a free end: some end of it would turn a layer.
fn wraps_at_ends(model: &Model<'_>, i: usize) -> bool {
    model
        .wall(i)
        .is_some_and(|w| matches!(wrapped(w, End::Start), Some(Some(_))))
}

/// Wall `me` of `walls` with its joints made and its free ends wrapped: each layer's pieces and
/// its outline, the walls it is joined with, and why each other wall that meets it is not.
///
/// `walls` must hold every wall that meets `me` and every wall that meets one of those (more do
/// no harm), in an order every caller keeps (by ID); a wall whose layers do not run face to face
/// or whose line has no length takes no part. A wall with no joint made and no wrapped end has no
/// layers or outline here: it is built as it was.
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
            Joint::Corner { other, .. }
            | Joint::Through { other, .. }
            | Joint::Crossed { other } => other,
            Joint::Abuts { through, .. } | Joint::Cut { through } => through,
        })
        .collect();
    with.sort_unstable();
    with.dedup();
    if found.is_empty() && !wraps_at_ends(&model, me) {
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
        let named: Vec<&str> = with.iter().map(|&j| model.id(j)).collect();
        unjoined.push(if named.is_empty() {
            "its wrapped ends could not be laid out on the kernel's grid".to_owned()
        } else {
            format!(
                "its joints with {} could not be laid out on the kernel's grid",
                named.join(", ")
            )
        });
        Joined {
            unjoined,
            ..Joined::default()
        }
    }
}
